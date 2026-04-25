/**
 * Svelte 5 reactive stores for the Bowties tab — Feature 006 + 009.
 *
 * bowtieCatalogStore  — holds the latest BowtieCatalog or null (before first CDI read)
 * cdiReadCompleteStore — true once the first cdi-read-complete event has been received
 * editableBowtiePreview — derived view merging catalog + pending edits + metadata
 *
 * The stores are populated by registering a persistent Tauri event listener for
 * the `cdi-read-complete` event emitted by the backend after CDI reads + the
 * Identify Events exchange both complete.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { get } from 'svelte/store';
import { type BowtieCatalog, type BowtieCard, type CdiReadCompletePayload, type EventSlotEntry } from '../api/tauri';
import type { PreviewBowtieCard, EditableBowtiePreview } from '$lib/types/bowtie';
import { buildElementLabel, collectEventIdLeaves, effectiveValue, findLeafByPath, hasModifiedLeaves } from '$lib/types/nodeTree';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { resolveNodeDisplayName as resolveSharedNodeDisplayName } from '$lib/utils/nodeDisplayName';

// ─── Store class ─────────────────────────────────────────────────────────────

class BowtieCatalogStore {
  /** The latest built catalog, or null if CDI reads have not yet completed. */
  private _catalog = $state<BowtieCatalog | null>(null);

  /** True once the first `cdi-read-complete` event has been received. */
  private _readComplete = $state<boolean>(false);

  /** Active listener handle (stored so it can be cleaned up). */
  private _unlisten: UnlistenFn | null = null;

  // ── Reactive getters ──────────────────────────────────────────────────────

  get catalog(): BowtieCatalog | null {
    return this._catalog;
  }

  get readComplete(): boolean {
    return this._readComplete;
  }

  /**
   * Derived map: event_id_hex → BowtieCard.
   * O(1) lookup used by cross-reference navigation (FR-008, research.md RQ-10).
   * Because `_catalog` is `$state`, this getter is re-evaluated reactively
   * whenever the catalog changes.
   */
  get usedInMap(): Map<string, BowtieCard> {
    const map = new Map<string, BowtieCard>();
    if (!this._catalog) return map;
    for (const card of this._catalog.bowties) {
      map.set(card.event_id_hex, card);
    }
    return map;
  }

  /**
   * Derived map: `${nodeId}:${elementPath.join('/')}` → BowtieCard.
   * Used by ElementCard to look up cross-reference by structural identity
   * (node + CDI path) rather than current byte value, which may not yet be
   * fetched at render time (FR-008, SC-005).
   */
  get nodeSlotMap(): Map<string, BowtieCard> {
    const map = new Map<string, BowtieCard>();
    if (!this._catalog) return map;
    for (const card of this._catalog.bowties) {
      for (const entry of [...card.producers, ...card.consumers, ...card.ambiguous_entries]) {
        const key = `${entry.node_id}:${entry.element_path.join('/')}`;
        if (!map.has(key)) {
          map.set(key, card);
        }
      }
    }
    return map;
  }

  /**
   * Like `nodeSlotMap`, but also covers eventId leaves whose value has been
   * modified but not yet saved (pending / unsaved consumers or producers).
   *
   * For committed entries the structural identity lookup is used (same as
   * nodeSlotMap).  For leaves that only have a `modifiedValue`, the effective
   * hex value is matched against catalog event IDs so the "Used in" link
   * appears immediately after an assignment is made.
   */
  get effectiveNodeSlotMap(): Map<string, BowtieCard> {
    // Start from the committed structural entries.
    const map = new Map(this.nodeSlotMap);
    if (!this._catalog) return map;

    // Build event_id_hex → BowtieCard reverse index for the pending-value scan.
    const cardByEventId = new Map<string, BowtieCard>();
    for (const card of this._catalog.bowties) {
      cardByEventId.set(card.event_id_hex, card);
    }

    // Scan every eventId leaf in every loaded tree.  When the effective value is
    // an eventId that matches a catalog card, and the structural key isn't already
    // present (committed entry wins), add an entry for the leaf's path.
    for (const [nodeId, tree] of nodeTreeStore.trees) {
      for (const leaf of collectEventIdLeaves(tree)) {
        const val = effectiveValue(leaf);
        if (val?.type !== 'eventId') continue;
        const key = `${nodeId}:${leaf.path.join('/')}`;
        if (map.has(key)) continue;
        const card = cardByEventId.get(val.hex);
        if (card) map.set(key, card);
      }
    }
    return map;
  }

  /**
   * Display name for a bowtie, preferring the user-defined name from metadata
   * over the backend catalog name, falling back to the raw event ID hex.
   *
   * Used by config-page "Used in" links to show a meaningful label (FR-008).
   */
  getDisplayName(eventIdHex: string): string {
    return (
      bowtieMetadataStore.getMetadata(eventIdHex)?.name ??
      this.usedInMap.get(eventIdHex)?.name ??
      eventIdHex
    );
  }

  // ── Mutations ─────────────────────────────────────────────────────────────

  /** Set the catalog directly (used by the event listener and in tests). */
  setCatalog(catalog: BowtieCatalog): void {
    this._catalog = catalog;
    this._readComplete = true;
  }

  /** Reset store to initial empty state (useful on disconnect). */
  reset(): void {
    this._catalog = null;
    this._readComplete = false;
  }

  // ── Listener lifecycle ────────────────────────────────────────────────────

  /**
   * Register a persistent Tauri event listener for `cdi-read-complete`.
   * Safe to call multiple times — subsequent calls are no-ops until destroyed.
   */
  async startListening(): Promise<void> {
    if (this._unlisten) return; // already registered

    this._unlisten = await listen<CdiReadCompletePayload>(
      'cdi-read-complete',
      (event) => {
        this.setCatalog(event.payload.catalog);
      }
    );
  }

  /**
   * Remove the Tauri event listener.
   * Called on component teardown (onDestroy) or page navigation away.
   */
  stopListening(): void {
    if (this._unlisten) {
      this._unlisten();
      this._unlisten = null;
    }
  }
}

// ─── Singleton exports ────────────────────────────────────────────────────────

/**
 * Singleton store holding the BowtieCatalog and read-complete flag.
 *
 * Usage in a Svelte component:
 * ```svelte
 * <script>
 *   import { bowtieCatalogStore } from '$lib/stores/bowties';
 *   import { onMount, onDestroy } from 'svelte';
 *
 *   onMount(() => bowtieCatalogStore.startListening());
 *   onDestroy(() => bowtieCatalogStore.stopListening());
 *
 *   let catalog = $derived(bowtieCatalogStore.catalog);
 *   let enabled = $derived(bowtieCatalogStore.readComplete);
 * </script>
 * ```
 */
export const bowtieCatalogStore = new BowtieCatalogStore();

// ─── Editable Bowtie Preview (T018, T023) ─────────────────────────────────────

/**
 * Derived class that merges the live BowtieCatalog + tree modifications
 * + metadata from BowtieMetadataStore to produce the current user-visible
 * bowtie state with per-card dirty flags.
 *
 * Reactively recomputes when any of its inputs change (T023).
 * Tree modifications are reflected because `nodeTreeStore.trees` is reactive
 * and `collectEntriesForEventId` reads effective values from tree leaves.
 */
class EditableBowtiePreviewStore {
  /**
   * Compute the editable preview by merging catalog, metadata, and tree modifications.
   */
  get preview(): EditableBowtiePreview {
    const catalog = bowtieCatalogStore.catalog;
    const metadataIsDirty = bowtieMetadataStore.isDirty;
    // Check whether any tree has modified leaves (replaces pendingEditsStore.hasPendingEdits)
    let configIsDirty = false;
    for (const tree of nodeTreeStore.trees.values()) {
      if (hasModifiedLeaves(tree)) { configIsDirty = true; break; }
    }
    const layout = layoutStore.layout;

    const previews: PreviewBowtieCard[] = [];
    const seenEventIds = new Set<string>();

    // 1. Process catalog cards (if available)
    if (catalog) {
    // Process each card from the catalog
    for (const card of catalog.bowties) {
      seenEventIds.add(card.event_id_hex);
      const meta = bowtieMetadataStore.getMetadata(card.event_id_hex);

      // Build dirty fields set
      const dirtyFields = new Set<string>();

      // Check if metadata has been edited for this card (compare against _edits map,
      // not layout, since _applyToLayout already merges edits into the layout in-memory)
      for (const f of bowtieMetadataStore.getDirtyFields(card.event_id_hex)) {
        dirtyFields.add(f);
      }

      // Collect entries from tree leaves (including modified values)
      const { producers: treeProducers, consumers: treeConsumers } =
        collectEntriesForEventId(card.event_id_hex);

      // Filter out entries already present in the catalog card
      const allExisting = [...card.producers, ...card.consumers, ...card.ambiguous_entries];
      const newProducers = treeProducers.filter(p =>
        !allExisting.some(e => e.node_id === p.node_id && e.element_path.join('/') === p.element_path.join('/'))
      );
      const newConsumers = treeConsumers.filter(c =>
        !allExisting.some(e => e.node_id === c.node_id && e.element_path.join('/') === c.element_path.join('/'))
      );

      if (newProducers.length > 0 || newConsumers.length > 0) {
        dirtyFields.add('elements');
      }

      const newEntryKeys = new Set<string>();
      for (const e of [...newProducers, ...newConsumers]) {
        newEntryKeys.add(`${e.node_id}:${e.element_path.join('/')}`);
      }

      previews.push({
        eventIdHex: card.event_id_hex,
        eventIdBytes: card.event_id_bytes,
        producers: [...card.producers.filter(e => isEntryStillActive(e, card.event_id_hex)).map(enrichEntryLabel), ...newProducers],
        consumers: [...card.consumers.filter(e => isEntryStillActive(e, card.event_id_hex)).map(enrichEntryLabel), ...newConsumers],
        ambiguousEntries: card.ambiguous_entries,
        name: meta?.name ?? card.name ?? undefined,
        tags: meta?.tags ?? card.tags ?? [],
        state: card.state === 'Active' ? 'active' : card.state === 'Incomplete' ? 'incomplete' : 'planning',
        isDirty: dirtyFields.size > 0 || newProducers.length > 0 || newConsumers.length > 0,
        dirtyFields,
        newEntryKeys,
      });
    }
    } // end if (catalog)

    // 2. Always process layout bowties not already covered by catalog
    if (layout) {
      for (const [eventIdHex, meta] of Object.entries(layout.bowties)) {
        if (!seenEventIds.has(eventIdHex)) {
          seenEventIds.add(eventIdHex);
          const metaOverride = bowtieMetadataStore.getMetadata(eventIdHex);

          const { producers, consumers } = collectEntriesForEventId(eventIdHex);

          // Only flag dirty if pending metadata edits actually changed something
          const dirtyFields = bowtieMetadataStore.getDirtyFields(eventIdHex);

          previews.push({
            eventIdHex,
            eventIdBytes: eventIdHexToBytes(eventIdHex),
            producers,
            consumers,
            ambiguousEntries: [],
            name: metaOverride?.name ?? meta.name,
            tags: metaOverride?.tags ?? meta.tags ?? [],
            state: 'planning',
            isDirty: dirtyFields.size > 0,
            dirtyFields,
            newEntryKeys: new Set<string>(),
          });
        }
      }
    }

    // Add newly-created bowties from metadata that aren't in catalog or layout
    for (const eventIdHex of bowtieMetadataStore.allEventIds) {
      if (!seenEventIds.has(eventIdHex)) {
        seenEventIds.add(eventIdHex);
        const meta = bowtieMetadataStore.getMetadata(eventIdHex);

        const { producers, consumers } = collectEntriesForEventId(eventIdHex);

        previews.push({
          eventIdHex,
          eventIdBytes: eventIdHexToBytes(eventIdHex),
          producers,
          consumers,
          ambiguousEntries: [],
          name: meta?.name,
          tags: meta?.tags ?? [],
          state: 'planning',
          isDirty: true,
          dirtyFields: new Set(['name']),
          newEntryKeys: new Set<string>(),
        });
      }
    }

    return {
      bowties: previews,
      hasUnsavedChanges: metadataIsDirty || configIsDirty,
    };
  }
}

/**
 * Compute the element_label for a catalog entry from the live tree so it
 * reflects getInstanceDisplayName (e.g. "GPIO13 (1)") and pending name edits.
 * Falls back to element_path.join('.') when the tree or leaf cannot be found.
 */
function enrichEntryLabel(entry: EventSlotEntry): EventSlotEntry {
  const tree = nodeTreeStore.getTree(entry.node_id);
  if (!tree) return { ...entry, element_label: entry.element_label ?? entry.element_path.join('.') };
  const leaf = findLeafByPath(tree, entry.element_path);
  if (!leaf) return { ...entry, element_label: entry.element_label ?? entry.element_path.join('.') };
  return { ...entry, element_label: buildElementLabel(tree, leaf) };
}

/**
 * Check whether a catalog entry is still active — i.e., its tree leaf's
 * effective value still matches the given event ID hex.  Returns true when
 * the leaf cannot be found (conservative: keep the entry).
 */
function isEntryStillActive(entry: import('../api/tauri').EventSlotEntry, eventIdHex: string): boolean {
  const tree = nodeTreeStore.getTree(entry.node_id);
  if (!tree) return true;
  const leaf = findLeafByPath(tree, entry.element_path);
  if (!leaf) return true;
  const val = effectiveValue(leaf);
  if (val?.type !== 'eventId') return false;
  return val.hex === eventIdHex;
}

/**
 * Convert a dotted-hex event ID string to a bytes array.
 * E.g., "05.01.01.01.FF.00.00.01" → [5, 1, 1, 1, 255, 0, 0, 1]
 */
function eventIdHexToBytes(hex: string): number[] {
  return hex.split('.').map(h => parseInt(h, 16));
}

/**
 * Collect all entries for a given event ID hex by scanning all loaded tree
 * leaves (using their effective value — committed or modified).
 *
 * This mirrors the Rust catalog builder's approach: walk all CDI event-ID
 * slots, reuse tree and SNIP data for full display quality.
 */
function collectEntriesForEventId(eventIdHex: string): { producers: EventSlotEntry[]; consumers: EventSlotEntry[] } {
  const producers: EventSlotEntry[] = [];
  const consumers: EventSlotEntry[] = [];

  for (const [nodeId, tree] of nodeTreeStore.trees) {
    const leaves = collectEventIdLeaves(tree);
    for (const leaf of leaves) {
      const val = effectiveValue(leaf);
      if (val?.type !== 'eventId' || val.hex !== eventIdHex) continue;

      // Prefer the JS-side role classification (set when the user picks a slot
      // in the ElementPicker) over the Rust tree's eventRole, which may still
      // be 'Ambiguous' for unclassified slots.
      const slotKey = `${nodeId}:${leaf.path.join('/')}`;
      const classifiedRole = bowtieMetadataStore.getRoleClassification(slotKey)?.role;
      const entry: EventSlotEntry = {
        node_id: nodeId,
        node_name: resolveNodeDisplayName(nodeId),
        element_path: leaf.path,
        element_label: buildElementLabel(tree, leaf),
        element_description: leaf.description,
        event_id: val.bytes,
        role: classifiedRole ?? leaf.eventRole ?? 'Ambiguous',
      };

      if (entry.role === 'Producer') {
        producers.push(entry);
      } else if (entry.role === 'Consumer') {
        consumers.push(entry);
      } else {
        consumers.push(entry);
      }
    }
  }

  return { producers, consumers };
}

/**
 * Resolve a human-readable node name, mirroring the Rust `node_display_name()` logic:
 * user_name → "manufacturer — model" → node_id_hex.
 */
function resolveNodeDisplayName(nodeId: string): string {
  const nodes = get(nodeInfoStore);
  return resolveSharedNodeDisplayName(nodeId, nodes.get(nodeId));
}

/** Singleton editable bowtie preview store. */
export const editableBowtiePreviewStore = new EditableBowtiePreviewStore();
