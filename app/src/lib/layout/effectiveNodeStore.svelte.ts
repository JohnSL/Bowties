/**
 * effectiveNodeStore — per-node projection across the three layout layers
 * (ADR-0011, extends ADR-0004 + ADR-0007).
 *
 * Sibling to `effectiveLayoutStore`. The value-shaped facade owns
 * `effectiveValue` / `effectiveRole` / `slotsByRole`; this store owns
 * per-node persistability so Save, the orange in-memory-changes dot,
 * the unsaved-count, and the unsaved-new badge derive from a single
 * predicate.
 *
 * Public surface:
 *
 *   - `nodeOrigin(key)`             → 'live-only' | 'layout-only' | 'both' | 'placeholder'
 *   - `isFullyCaptured(key)`        → tree present AND not partial-capture (ADR-0007)
 *   - `isConfigRead(key)`           → configReadNodesStore membership
 *   - `isPersistableInLayout(key)`  → fullyCaptured ∧ (configRead ∨ placeholder)
 *   - `unsavedInMemoryNodeIds`      → live persistable additions absent from
 *                                       `layoutStore.activeContext.layoutNodeIds`
 *   - `isDirty`                     → any persistable addition OR any
 *                                       draft / metadata / offline-change /
 *                                       layout-struct edit
 *
 * Inputs are reads only; the facade never writes through to underlying
 * stores. The lifecycle owner (`layoutLifecycleOrchestrator`) is the
 * single resetter of every store the facade reads.
 */

import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { configReadNodesStore } from '$lib/stores/configReadStatus';
import { partialCaptureNodesStore } from '$lib/stores/partialCaptureNodes.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
import {
  isPlaceholderInput,
  toCanonicalNodeKey,
  type NodeKeyInput,
} from '$lib/utils/nodeKey';

export type NodeOrigin = 'live-only' | 'layout-only' | 'both' | 'placeholder';

class EffectiveNodeStore {
  // ── Vanilla-writable bridges (mirror into $state for reactive reads) ────
  // `nodeInfoStore` and `configReadNodesStore` are legacy Svelte 3 stores.
  // Mirroring them into `$state` lets the facade expose reactive getters
  // without forcing every consumer to subscribe by hand.

  private _info = $state<Map<string, unknown>>(new Map());
  private _read = $state<Set<string>>(new Set());

  constructor() {
    nodeInfoStore.subscribe((m) => {
      this._info = m;
    });
    configReadNodesStore.subscribe((s) => {
      this._read = s;
    });
  }

  // ── Origin classification ────────────────────────────────────────────────

  nodeOrigin(key: NodeKeyInput): NodeOrigin {
    const canonical = toCanonicalNodeKey(key);
    if (isPlaceholderInput(canonical)) return 'placeholder';

    const inLive = this._info.has(canonical);
    const inLayout = this._layoutNodeIdSet().has(canonical);
    if (inLive && inLayout) return 'both';
    if (inLive) return 'live-only';
    if (inLayout) return 'layout-only';
    // Neither — surface as live-only so the predicate semantics don't
    // crash on a stale lookup. Callers checking origin generally already
    // know the key exists somewhere.
    return 'live-only';
  }

  // ── Capture / read state ─────────────────────────────────────────────────

  isFullyCaptured(key: NodeKeyInput): boolean {
    const canonical = toCanonicalNodeKey(key);
    return nodeTreeStore.trees.has(canonical) && !partialCaptureNodesStore.has(canonical);
  }

  isConfigRead(key: NodeKeyInput): boolean {
    return this._read.has(toCanonicalNodeKey(key));
  }

  // ── Persistability ───────────────────────────────────────────────────────

  /**
   * True iff this node can legitimately be promoted into the layout file.
   * Placeholders are persistable as soon as their (synthetic) tree exists;
   * live nodes additionally require config-read so the saved snapshot
   * contains real values (R5).
   */
  isPersistableInLayout(key: NodeKeyInput): boolean {
    const canonical = toCanonicalNodeKey(key);
    if (!this.isFullyCaptured(canonical)) return false;
    if (isPlaceholderInput(canonical)) return true;
    return this.isConfigRead(canonical);
  }

  /**
   * Canonical NodeKeys (live + placeholder) that are persistable AND not
   * yet in the saved layout roster. Returns `[]` when no layout context
   * is active (pre-S8 semantics — nothing to compare against).
   *
   * Placeholders are included: an unsaved placeholder is the *only*
   * pending change a freshly-added board produces, so it must surface
   * here for the Save flow to dirty and for the orchestrator to emit an
   * `addNode` delta. After save, placeholder keys land in
   * `layoutNodeIds` and the `saved.has(canonical)` check removes them.
   */
  get unsavedInMemoryNodeIds(): string[] {
    const ctx = layoutStore.activeContext;
    if (!ctx) return [];
    const saved = this._layoutNodeIdSet();
    const out: string[] = [];
    for (const canonical of nodeTreeStore.trees.keys()) {
      if (saved.has(canonical)) continue;
      if (!this.isPersistableInLayout(canonical)) continue;
      out.push(canonical);
    }
    return out;
  }

  /**
   * Canonical NodeKeys that were persisted in the open layout but have
   * been removed in-memory and not yet saved. Drives the orchestrator's
   * `removeNode` delta emission and contributes to `isDirty`.
   */
  get unsavedRemovedNodeIds(): string[] {
    return Array.from(nodeRoster.persistedRemovals);
  }

  // ── Aggregate dirty signal ───────────────────────────────────────────────

  /**
   * True when there are any in-memory changes that a Save would persist:
   * a fully-captured live addition, a config draft, a bowtie metadata
   * edit, an offline change (draft or persisted-then-reverted), or a
   * LayoutFile-struct edit. Mirrors the contract `saveControlsPresenter`
   * already enforces, sourced from a single predicate instead of four.
   */
  get isDirty(): boolean {
    if (this.unsavedInMemoryNodeIds.length > 0) return true;
    if (this.unsavedRemovedNodeIds.length > 0) return true;
    if (layoutStore.isDirty) return true;
    if (bowtieMetadataStore.isDirty) return true;
    if (configChangesStore.draftEntries().length > 0) return true;
    if (offlineChangesStore.draftCount > 0) return true;
    if (offlineChangesStore.revertedPersistedCount > 0) return true;
    return false;
  }

  // ── Internals ────────────────────────────────────────────────────────────

  private _layoutNodeIdSet(): Set<string> {
    const ids = layoutStore.activeContext?.layoutNodeIds;
    if (!ids || ids.length === 0) return new Set();
    return new Set(ids.map(toCanonicalNodeKey));
  }
}

export const effectiveNodeStore = new EffectiveNodeStore();
