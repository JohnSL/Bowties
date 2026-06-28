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
 *   - `dirtyBreakdown`              → per-bucket count snapshot (ADR-0011
 *                                       extension 2026-06-28) consumed by
 *                                       SaveControls + UnsavedChangesDialog
 *   - `isDirty`                     → derived from dirtyBreakdown; true when
 *                                       any bucket > 0
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
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
import { parseEditKey } from '$lib/utils/editKey';
import {
  isPlaceholderInput,
  toCanonicalNodeKey,
  type NodeKeyInput,
} from '$lib/utils/nodeKey';

export type NodeOrigin = 'live-only' | 'layout-only' | 'both' | 'placeholder';

/**
 * Per-bucket dirty snapshot (ADR-0011 extension 2026-06-28).
 *
 * `SaveControls` and `UnsavedChangesDialog` consume this to render counts
 * without re-reading each edit-bearing store. Adding a new edit-bearing
 * store in a future spec means adding one field here and one accumulator
 * in `dirtyBreakdown` — the lifecycle reset path stays unchanged.
 */
export interface DirtyBreakdown {
  /** Config-tree draft edits (overlay layer, per leaf). */
  config: number;
  /** Distinct node count contributing to `config` (for "across N nodes"). */
  configNodes: number;
  /** Bowtie metadata edits (deletes / role classifications / …). */
  metadata: number;
  /** Pending channel creations + renames + deletions. */
  channels: number;
  /** Pending facility creations + renames + deletions. */
  facilities: number;
  /** Connector slot selections diverging from baseline. */
  connectorSelections: number;
  /** Offline-changes draft rows (Spec 013). */
  offlineDrafts: number;
  /** Persisted offline rows that have been reverted in-memory. */
  offlineRevertedPersisted: number;
  /** LayoutFile struct edits (element-deck reorder, etc.). */
  layoutStruct: number;
  /** Fully-captured live nodes absent from the saved layout roster. */
  unsavedNewNodes: number;
  /** NodeKeys removed from the saved layout but not yet flushed. */
  unsavedRemovedNodes: number;
}

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
   * Per-bucket dirty count snapshot. The single read surface for the
   * UnsavedChangesDialog and SaveControls — every edit-bearing store
   * contributes exactly one accumulator here. Adding a new edit-bearing
   * store in a future spec requires extending this method, not scattering
   * `store.isDirty` reads across the UI.
   */
  get dirtyBreakdown(): DirtyBreakdown {
    const drafts = configChangesStore.draftEntries();
    let configNodes = 0;
    if (drafts.length > 0) {
      const seen = new Set<string>();
      for (const { key } of drafts) {
        const { normalizedNodeId } = parseEditKey(key);
        seen.add(normalizedNodeId);
      }
      configNodes = seen.size;
    }
    return {
      config: drafts.length,
      configNodes,
      metadata: bowtieMetadataStore.editCount,
      channels: channelsStore.editCount,
      facilities: facilitiesStore.editCount,
      connectorSelections: connectorSelectionsStore.editCount,
      offlineDrafts: offlineChangesStore.draftCount,
      offlineRevertedPersisted: offlineChangesStore.revertedPersistedCount,
      layoutStruct: layoutStore.isDirty ? 1 : 0,
      unsavedNewNodes: this.unsavedInMemoryNodeIds.length,
      unsavedRemovedNodes: this.unsavedRemovedNodeIds.length,
    };
  }

  /**
   * True when there are any in-memory changes that a Save would persist.
   * Derived from `dirtyBreakdown` so the two predicates can never drift.
   */
  get isDirty(): boolean {
    const bd = this.dirtyBreakdown;
    return (
      bd.config > 0
      || bd.metadata > 0
      || bd.channels > 0
      || bd.facilities > 0
      || bd.connectorSelections > 0
      || bd.offlineDrafts > 0
      || bd.offlineRevertedPersisted > 0
      || bd.layoutStruct > 0
      || bd.unsavedNewNodes > 0
      || bd.unsavedRemovedNodes > 0
    );
  }

  // ── Internals ────────────────────────────────────────────────────────────

  private _layoutNodeIdSet(): Set<string> {
    const ids = layoutStore.activeContext?.layoutNodeIds;
    if (!ids || ids.length === 0) return new Set();
    return new Set(ids.map(toCanonicalNodeKey));
  }
}

export const effectiveNodeStore = new EffectiveNodeStore();
