/**
 * Layered change state for config field edits.
 *
 * Owns draft/offlinePending/baseline layer resolution for every config field.
 * Has no knowledge of CDI structure, constraints, profiles, or cascading.
 *
 * Layer resolution order (highest to lowest):
 *   1. draft         — user edit not yet saved (owned by this store)
 *   2. offlinePending — persisted offline change from offlineChangesStore
 *   3. baseline      — authoritative device value from the tree (leaf.value)
 *
 * Design constraints (from refactor plan):
 * - Never mutates the tree store.
 * - No knowledge of online/offline mode.
 * - Reads nodeTreeStore and offlineChangesStore on demand (lazy resolution).
 * - Write interface restricted to: ConfigEditor, save/discard orchestration.
 */

import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import { findLeafByAddress } from '$lib/types/nodeTree';
import type { TreeConfigValue } from '$lib/types/nodeTree';
import {
  parseEditKey,
  addressToOffsetHex,
  parseOfflineValueString,
} from '$lib/utils/editKey';
import { normalizeNodeId } from '$lib/utils/nodeId';

// ─── Types ────────────────────────────────────────────────────────────────────

export type ChangeLayerType = 'draft' | 'offlinePending' | 'baseline';

export interface ChangeLayer {
  type: ChangeLayerType;
  value: TreeConfigValue;
}

export interface ConfigDraftEntry {
  key: string;
  value: TreeConfigValue;
}

// ─── Store class ──────────────────────────────────────────────────────────────

class ConfigChangesStore {
  /** Draft edits keyed by canonical edit key. */
  private _drafts = $state<Map<string, TreeConfigValue>>(new Map());

  // ── Read interface ────────────────────────────────────────────────────────

  /**
   * The value a control should display for this field.
   *
   * Resolution order: draft → offlinePending → baseline.
   * Returns null when no layer exists (leaf has no value and no edits).
   */
  visibleValue(key: string): TreeConfigValue | null {
    const draft = this._drafts.get(key);
    if (draft !== undefined) return draft;

    const offline = this._resolveOfflinePending(key);
    if (offline !== null) return offline;

    return this._resolveBaseline(key);
  }

  /**
   * Ordered list of change layers for this field, from top (most specific) to
   * bottom (baseline). Only layers with a value are included.
   *
   * Used by annotation components to render "from → to" history.
   * - layers[0] is the "to" value
   * - layers[1] is the "from" value (next layer down)
   */
  changeLayers(key: string): ChangeLayer[] {
    const layers: ChangeLayer[] = [];

    const draft = this._drafts.get(key);
    if (draft !== undefined) {
      layers.push({ type: 'draft', value: draft });
    }

    const offline = this._resolveOfflinePending(key);
    if (offline !== null) {
      layers.push({ type: 'offlinePending', value: offline });
    }

    const baseline = this._resolveBaseline(key);
    if (baseline !== null) {
      layers.push({ type: 'baseline', value: baseline });
    }

    return layers;
  }

  /** Count of draft entries for the given node. */
  countDraftsForNode(nodeId: string): number {
    const prefix = `${normalizeNodeId(nodeId)}:`;
    let count = 0;
    for (const key of this._drafts.keys()) {
      if (key.startsWith(prefix)) count++;
    }
    return count;
  }

  /** True when at least one draft exists for the given node. */
  hasDraftsForNode(nodeId: string): boolean {
    return this.countDraftsForNode(nodeId) > 0;
  }

  /**
   * True when any draft exists for a field whose CDI path starts with the
   * given prefix within the node.
   *
   * The edit key encodes (nodeId, space, address) but not the CDI path, so
   * this performs an O(n) scan of drafts and looks up each address in the
   * tree to check the path prefix.
   */
  hasDraftsUnderPath(nodeId: string, pathPrefix: string): boolean {
    const normalizedId = normalizeNodeId(nodeId);
    const prefix = `${normalizedId}:`;
    for (const key of this._drafts.keys()) {
      if (!key.startsWith(prefix)) continue;
      const { space, address } = parseEditKey(key);
      const leaf = this._findLeaf(normalizedId, space, address);
      if (leaf && leaf.path.join('/').startsWith(pathPrefix)) return true;
    }
    return false;
  }

  /** Snapshot of all current config drafts in insertion order. */
  draftEntries(): ConfigDraftEntry[] {
    return [...this._drafts.entries()].map(([key, value]) => ({ key, value }));
  }

  // ── Write interface ───────────────────────────────────────────────────────

  /** Create or update the draft (top layer) for this field. */
  set(key: string, value: TreeConfigValue): void {
    this._drafts = new Map(this._drafts);
    this._drafts.set(key, value);
  }

  /** Remove the draft for this field. Returns true if a draft was removed. */
  revert(key: string): boolean {
    if (!this._drafts.has(key)) return false;
    this._drafts = new Map(this._drafts);
    this._drafts.delete(key);
    return true;
  }

  /** Remove all draft entries (used by discard-all). */
  clearAllDrafts(): void {
    this._drafts = new Map();
  }

  /** Remove all drafts belonging to the given node (used after save). */
  clearDraftsForNode(nodeId: string): void {
    const prefix = `${normalizeNodeId(nodeId)}:`;
    const next = new Map(this._drafts);
    for (const key of next.keys()) {
      if (key.startsWith(prefix)) next.delete(key);
    }
    this._drafts = next;
  }

  /**
   * Remove drafts whose current tree baseline now matches the draft value.
   *
   * Used after a tree refresh so successful writes disappear without clearing
   * unrelated drafts that still differ from the refreshed baseline.
   */
  pruneResolvedDraftsForNode(nodeId: string): string[] {
    const normalizedId = normalizeNodeId(nodeId);
    const prefix = `${normalizedId}:`;
    const next = new Map(this._drafts);
    const removed: string[] = [];

    for (const [key, draft] of this._drafts.entries()) {
      if (!key.startsWith(prefix)) continue;
      const baseline = this._resolveBaseline(key);
      if (baseline !== null && this._valuesEqual(draft, baseline)) {
        next.delete(key);
        removed.push(key);
      }
    }

    if (removed.length > 0) {
      this._drafts = next;
    }

    return removed;
  }

  // ── Private layer resolution ──────────────────────────────────────────────

  /**
   * Read the offline pending value from offlineChangesStore for this key.
   *
   * Returns the persisted offline row only. Draft rows in offlineChangesStore
   * are persistence staging; connector repairs that need display visibility
   * are written separately to the config-draft layer.
   */
  private _resolveOfflinePending(key: string): TreeConfigValue | null {
    const { normalizedNodeId, space, address } = parseEditKey(key);
    const offset = addressToOffsetHex(address);
    const row = offlineChangesStore.findPersistedConfigChange(
      normalizedNodeId,
      space,
      offset,
    );
    if (!row) return null;
    return parseOfflineValueString(row.plannedValue);
  }

  /**
   * Read the baseline value from the tree store for this key.
   *
   * Iterates nodeTreeStore.trees to find the matching tree by normalized
   * nodeId, then uses findLeafByAddress. Returns null when tree is not loaded
   * or leaf has no value.
   */
  private _resolveBaseline(key: string): TreeConfigValue | null {
    const { normalizedNodeId, space, address } = parseEditKey(key);
    const leaf = this._findLeaf(normalizedNodeId, space, address);
    return leaf?.value ?? null;
  }

  /**
   * Find a leaf in the tree store by normalized nodeId, space, and address.
   * Handles trees stored under dotted and undotted node ID keys.
   */
  private _findLeaf(
    normalizedNodeId: string,
    _space: number,
    address: number,
  ): import('$lib/types/nodeTree').LeafConfigNode | null {
    for (const [treeKey, tree] of nodeTreeStore.trees.entries()) {
      if (normalizeNodeId(treeKey) !== normalizedNodeId) continue;
      const leaf = findLeafByAddress(tree, address);
      if (leaf) return leaf;
    }
    return null;
  }

  private _valuesEqual(left: TreeConfigValue, right: TreeConfigValue): boolean {
    if (left.type !== right.type) return false;

    if (left.type === 'int' && right.type === 'int') return left.value === right.value;
    if (left.type === 'float' && right.type === 'float') return left.value === right.value;
    if (left.type === 'string' && right.type === 'string') return left.value === right.value;
    if (left.type === 'eventId' && right.type === 'eventId') return left.hex === right.hex;

    return false;
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const configChangesStore = new ConfigChangesStore();
