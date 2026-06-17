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
import { type BowtieCatalog, type BowtieCard, type CdiReadCompletePayload, type EventSlotEntry } from '../api/tauri';
import type { PreviewBowtieCard, EditableBowtiePreview } from '$lib/types/bowtie';
import { buildElementLabel, collectEventIdLeaves, findLeafByPath } from '$lib/types/nodeTree';
import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { editKeyForLeaf } from '$lib/utils/editKey';
import { layoutStore } from '$lib/stores/layout.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { resolveNodeName } from '$lib/layout';
import { isPlaceholderEventId } from '$lib/utils/eventIds';
import { isWellKnownEvent } from '$lib/utils/formatters';
import { toCanonicalNodeKey } from '$lib/utils/nodeKey';

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
   * Whether a catalog card should be surfaced to UX components.
   * Single-slot unnamed events exist in the catalog for role classification
   * persistence but aren't useful as bowtie diagrams — unless the user
   * explicitly created the bowtie (present in the layout file).
   */
  private _isDisplayable(card: BowtieCard): boolean {
    const totalEntries = card.producers.length + card.consumers.length + card.ambiguous_entries.length;
    const name = bowtieMetadataStore.getMetadata(card.event_id_hex)?.name ?? card.name;
    if (totalEntries >= 2 || !!name) return true;
    // User-created bowties in the layout file are always displayable
    const layout = layoutStore.layout;
    if (layout && card.event_id_hex in layout.bowties) return true;
    return false;
  }

  /** Catalog cards that meet the display threshold (≥2 entries or named). */
  get displayableBowties(): BowtieCard[] {
    if (!this._catalog) return [];
    return this._catalog.bowties.filter(card => this._isDisplayable(card));
  }

  /**
   * Derived map: event_id_hex → BowtieCard.
   * O(1) lookup used by cross-reference navigation (FR-008, research.md RQ-10).
   * Only includes displayable cards.
   */
  get usedInMap(): Map<string, BowtieCard> {
    const map = new Map<string, BowtieCard>();
    if (!this._catalog) return map;
    for (const card of this._catalog.bowties) {
      if (this._isDisplayable(card)) {
        map.set(card.event_id_hex, card);
      }
    }
    return map;
  }

  /**
   * Derived map: `${nodeId}:${elementPath.join('/')}` → BowtieCard.
   * Used by ElementCard to look up cross-reference by structural identity.
   * Only includes entries from displayable cards.
   */
  get nodeSlotMap(): Map<string, BowtieCard> {
    const map = new Map<string, BowtieCard>();
    if (!this._catalog) return map;
    for (const card of this._catalog.bowties) {
      if (!this._isDisplayable(card)) continue;
      for (const entry of [...card.producers, ...card.consumers, ...card.ambiguous_entries]) {
        const key = `${entry.node_key}:${entry.element_path.join('/')}`;
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
    // Start from the committed structural entries (already filtered).
    const map = new Map(this.nodeSlotMap);
    if (!this._catalog) return map;

    // Build event_id_hex → BowtieCard reverse index for the pending-value scan.
    // Only include displayable cards.
    const cardByEventId = new Map<string, BowtieCard>();
    for (const card of this._catalog.bowties) {
      if (this._isDisplayable(card)) {
        cardByEventId.set(card.event_id_hex, card);
      }
    }

    // Scan every eventId leaf in every loaded tree.  When the effective value is
    // an eventId that matches a catalog card, and the structural key isn't already
    // present (committed entry wins), add an entry for the leaf's path.
    for (const [nodeId, tree] of nodeTreeStore.trees) {
      for (const leaf of collectEventIdLeaves(tree)) {
        const editKey = editKeyForLeaf(nodeId, leaf.space, leaf.address);
        const val = configChangesStore.visibleValue(editKey) ?? leaf.value;
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

  /**
   * Look up the authoritative role for a slot across ALL catalog cards,
   * including sub-threshold cards that exist only for classification persistence.
   *
   * Returns 'Producer', 'Consumer', or null if no catalog card contains this slot.
   */
  getRoleForSlot(nodeId: string, elementPath: string[]): 'Producer' | 'Consumer' | null {
    if (!this._catalog) return null;
    const canonicalId = toCanonicalNodeKey(nodeId);
    const key = `${canonicalId}:${elementPath.join('/')}`;
    for (const card of this._catalog.bowties) {
      for (const entry of card.producers) {
        if (`${entry.node_key}:${entry.element_path.join('/')}` === key) return 'Producer';
      }
      for (const entry of card.consumers) {
        if (`${entry.node_key}:${entry.element_path.join('/')}` === key) return 'Consumer';
      }
    }
    return null;
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

// ─── Editable Bowtie Preview (T018, T023; ADR-0004 / S2c) ────────────────────

/**
 * Compute the editable preview by merging catalog + tree + metadata + layout.
 *
 * Single derivation (ADR-0004 / S2c). The tree scan is the source of truth
 * for *current* slot membership; the catalog provides the canonical card
 * shape (state, ambiguous entries) for known event IDs; metadata supplies
 * user-authored names/tags; layout backfills bowties that exist in the
 * file but have no live slots yet.
 *
 * Reactively recomputes when any of its inputs change: `nodeTreeStore.trees`,
 * `bowtieCatalogStore.catalog`, `layoutStore.layout`,
 * `bowtieMetadataStore`, and `configChangesStore` are all reactive sources.
 *
 * INTERNAL — consumed only by `$lib/layout/effectiveLayoutStore`. Components
 * and routes read the preview through `effectiveLayoutStore.preview`.
 */
export function buildEffectiveBowtiePreview(): EditableBowtiePreview {
  const catalog = bowtieCatalogStore.catalog;
  const layout = layoutStore.layout;
  const metadataIsDirty = bowtieMetadataStore.isDirty;

  let configIsDirty = false;
  for (const nodeId of nodeTreeStore.trees.keys()) {
    if (configChangesStore.hasDraftsForNode(nodeId)) { configIsDirty = true; break; }
  }

  const previews: PreviewBowtieCard[] = [];
  const seenEventIds = new Set<string>();

  // Pre-compute tree entries index: eventIdHex → { producers, consumers }
  // Single-pass scan that builds an O(1) lookup map for all event entries.
  const treeEntriesIndex = buildTreeEntriesIndex();

  if (catalog) {
    for (const card of catalog.bowties) {
      const meta = bowtieMetadataStore.getMetadata(card.event_id_hex);
      const dirtyFields = new Set<string>();

      for (const f of bowtieMetadataStore.getDirtyFields(card.event_id_hex)) {
        dirtyFields.add(f);
      }

      // Use pre-computed index instead of per-card tree scan
      const treeEntries = treeEntriesIndex.get(card.event_id_hex);
      const treeProducers = treeEntries?.producers ?? [];
      const treeConsumers = treeEntries?.consumers ?? [];

      // Filter out entries already present in the catalog card using Set-based lookup
      const existingKeys = new Set<string>();
      for (const e of [...card.producers, ...card.consumers, ...card.ambiguous_entries]) {
        existingKeys.add(`${e.node_key}:${e.element_path.join('/')}`);
      }
      const newProducers = treeProducers.filter(p =>
        !existingKeys.has(`${p.node_key}:${p.element_path.join('/')}`)
      );
      const newConsumers = treeConsumers.filter(c =>
        !existingKeys.has(`${c.node_key}:${c.element_path.join('/')}`)
      );

      // Display filter: only show bowties with ≥2 entries, a name, or
      // an explicit entry in the layout file (user-created bowties).
      const totalEntries = card.producers.length + card.consumers.length + card.ambiguous_entries.length
        + newProducers.length + newConsumers.length;
      const effectiveName = meta?.name ?? card.name;
      const inLayout = layout && card.event_id_hex in layout.bowties;
      if (totalEntries < 2 && !effectiveName && !inLayout) continue;

      seenEventIds.add(card.event_id_hex);

      if (newProducers.length > 0 || newConsumers.length > 0) {
        dirtyFields.add('elements');
      }

      const newEntryKeys = new Set<string>();
      for (const e of [...newProducers, ...newConsumers]) {
        newEntryKeys.add(`${e.node_key}:${e.element_path.join('/')}`);
      }

      previews.push({
        eventIdHex: card.event_id_hex,
        eventIdBytes: card.event_id_bytes,
        producers: [...card.producers.filter(e => isEntryStillActive(e, card.event_id_hex)).map(enrichEntryLabel), ...newProducers],
        consumers: [...card.consumers.filter(e => isEntryStillActive(e, card.event_id_hex)).map(enrichEntryLabel), ...newConsumers],
        ambiguousEntries: card.ambiguous_entries.map(enrichEntryLabel),
        name: meta?.name ?? card.name ?? undefined,
        tags: meta?.tags ?? card.tags ?? [],
        state: card.state === 'Active' ? 'active' : card.state === 'Incomplete' ? 'incomplete' : 'planning',
        isDirty: dirtyFields.size > 0 || newProducers.length > 0 || newConsumers.length > 0,
        dirtyFields,
        newEntryKeys,
      });
    }
  }

  // Layout-only bowties
  if (layout) {
    for (const [eventIdHex, meta] of Object.entries(layout.bowties)) {
      if (seenEventIds.has(eventIdHex)) continue;
      seenEventIds.add(eventIdHex);
      const metaOverride = bowtieMetadataStore.getMetadata(eventIdHex);
      const treeEntries = treeEntriesIndex.get(eventIdHex);
      const dirtyFields = bowtieMetadataStore.getDirtyFields(eventIdHex);

      previews.push({
        eventIdHex,
        eventIdBytes: eventIdHexToBytes(eventIdHex),
        producers: treeEntries?.producers ?? [],
        consumers: treeEntries?.consumers ?? [],
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

  // Metadata-only bowties
  for (const eventIdHex of bowtieMetadataStore.allEventIds) {
    if (seenEventIds.has(eventIdHex)) continue;
    seenEventIds.add(eventIdHex);
    const meta = bowtieMetadataStore.getMetadata(eventIdHex);
    const treeEntries = treeEntriesIndex.get(eventIdHex);

    previews.push({
      eventIdHex,
      eventIdBytes: eventIdHexToBytes(eventIdHex),
      producers: treeEntries?.producers ?? [],
      consumers: treeEntries?.consumers ?? [],
      ambiguousEntries: [],
      name: meta?.name,
      tags: meta?.tags ?? [],
      state: 'planning',
      isDirty: true,
      dirtyFields: new Set(['name']),
      newEntryKeys: new Set<string>(),
    });
  }

  // Tree-discovered events not in catalog, layout, or metadata
  for (const [eventIdHex, entries] of treeEntriesIndex) {
    if (seenEventIds.has(eventIdHex)) continue;
    const totalEntries = entries.producers.length + entries.consumers.length;
    if (totalEntries < 2 && !isWellKnownEvent(eventIdHex)) continue;

    seenEventIds.add(eventIdHex);
    const meta = bowtieMetadataStore.getMetadata(eventIdHex);
    const dirtyFields = bowtieMetadataStore.getDirtyFields(eventIdHex);

    previews.push({
      eventIdHex,
      eventIdBytes: eventIdHexToBytes(eventIdHex),
      producers: entries.producers,
      consumers: entries.consumers,
      ambiguousEntries: [],
      name: meta?.name,
      tags: meta?.tags ?? [],
      state: deriveBowtieState(entries.producers.length, entries.consumers.length),
      isDirty: dirtyFields.size > 0,
      dirtyFields,
      newEntryKeys: new Set<string>(),
    });
  }

  return {
    bowties: previews,
    hasUnsavedChanges: metadataIsDirty || configIsDirty,
  };
}

/**
 * Refresh a catalog entry's derived display fields from live frontend state:
 *
 *   - `node_name` is re-resolved via the shared `resolveNodeName` facade
 *     (ADR-0003 point 4). The Rust catalog computes `node_name` once at
 *     build time; if SNIP had not arrived yet it is the raw node ID hex. We
 *     always re-resolve so the card shows the Display Name once SNIP lands
 *     and also reflects any pending offline User Name edit — mirroring how
 *     the config sidebar derives names.
 *   - `element_label` is re-derived from the live tree so it reflects
 *     getInstanceDisplayName (e.g. "GPIO13 (1)") and pending name edits.
 *     Falls back to element_path.join('.') when the tree or leaf cannot be found.
 */
function enrichEntryLabel(entry: EventSlotEntry): EventSlotEntry {
  // node_name resolution does not depend on the tree (SNIP lives in
  // nodeInfoStore), so resolve it before the tree/leaf guards.
  const node_name = resolveNodeName(entry.node_key);

  const tree = nodeTreeStore.getTree(entry.node_key);
  if (!tree) return { ...entry, node_name, element_label: entry.element_label ?? entry.element_path.join('.') };
  const leaf = findLeafByPath(tree, entry.element_path);
  if (!leaf) return { ...entry, node_name, element_label: entry.element_label ?? entry.element_path.join('.') };

  /** Resolve leaf value through draft → offlinePending → baseline layers. */
  const resolveValue = (l: LeafConfigNode): TreeConfigValue | null => {
    const key = editKeyForLeaf(entry.node_key, l.space, l.address);
    return configChangesStore.overrideValue(key) ?? l.value;
  };

  return { ...entry, node_name, element_label: buildElementLabel(tree, leaf, resolveValue) };
}

/**
 * Enrich all three entry arrays of a catalog card in one call.
 * Prevents the bug class where a new array is added or an existing one is
 * forgotten — every array goes through the same enrichment.
 */
function enrichCardEntries(card: import('../api/tauri').BowtieCard): {
  producers: EventSlotEntry[];
  consumers: EventSlotEntry[];
  ambiguousEntries: EventSlotEntry[];
} {
  return {
    producers: card.producers.map(enrichEntryLabel),
    consumers: card.consumers.map(enrichEntryLabel),
    ambiguousEntries: card.ambiguous_entries.map(enrichEntryLabel),
  };
}

/**
 * Check whether a catalog entry is still active — i.e., its tree leaf's
 * effective value still matches the given event ID hex.  Returns true when
 * the leaf cannot be found (conservative: keep the entry).
 */
function isEntryStillActive(entry: import('../api/tauri').EventSlotEntry, eventIdHex: string): boolean {
  const tree = nodeTreeStore.getTree(entry.node_key);
  if (!tree) return true;
  const leaf = findLeafByPath(tree, entry.element_path);
  if (!leaf) return true;
  const key = editKeyForLeaf(entry.node_key, leaf.space, leaf.address);
  const val = configChangesStore.visibleValue(key) ?? leaf.value;
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

function deriveBowtieState(producerCount: number, consumerCount: number): 'active' | 'incomplete' | 'planning' {
  if (producerCount > 0 && consumerCount > 0) return 'active';
  if (producerCount > 0 || consumerCount > 0) return 'incomplete';
  return 'planning';
}

function collectTreeEventIds(): string[] {
  const eventIds = new Set<string>();

  for (const tree of nodeTreeStore.trees.values()) {
    for (const leaf of collectEventIdLeaves(tree)) {
      const key = editKeyForLeaf(tree.nodeId, leaf.space, leaf.address);
      const value = configChangesStore.visibleValue(key) ?? leaf.value;
      if (value?.type !== 'eventId' || isPlaceholderEventId(value.hex)) continue;
      eventIds.add(value.hex);
    }
  }

  return Array.from(eventIds).sort();
}

/**
 * Build an index of all event ID entries from all trees in a single pass.
 * Returns a Map<eventIdHex, { producers, consumers }>.
 *
 * One O(trees × leaves) scan upfront gives O(1) lookups per card thereafter.
 */
function buildTreeEntriesIndex(): Map<string, { producers: EventSlotEntry[]; consumers: EventSlotEntry[] }> {
  const index = new Map<string, { producers: EventSlotEntry[]; consumers: EventSlotEntry[] }>();

  for (const [nodeId, tree] of nodeTreeStore.trees) {
    const leaves = collectEventIdLeaves(tree);
    const nodeName = resolveNodeName(nodeId);

    /** Resolve leaf value through draft → offlinePending → baseline layers. */
    const resolveValue = (l: LeafConfigNode): TreeConfigValue | null => {
      const key = editKeyForLeaf(nodeId, l.space, l.address);
      return configChangesStore.overrideValue(key) ?? l.value;
    };

    for (const leaf of leaves) {
      const key = editKeyForLeaf(nodeId, leaf.space, leaf.address);
      const val = configChangesStore.visibleValue(key) ?? leaf.value;
      if (val?.type !== 'eventId' || isPlaceholderEventId(val.hex)) continue;

      const slotKey = `${nodeId}:${leaf.path.join('/')}`;
      const classifiedRole = bowtieMetadataStore.getRoleClassification(slotKey)?.role;
      const entry: EventSlotEntry = {
        node_key: nodeId,
        node_name: nodeName,
        element_path: leaf.path,
        element_label: buildElementLabel(tree, leaf, resolveValue),
        element_description: leaf.description,
        event_id: val.bytes,
        role: classifiedRole ?? leaf.eventRole ?? 'Ambiguous',
      };

      let bucket = index.get(val.hex);
      if (!bucket) {
        bucket = { producers: [], consumers: [] };
        index.set(val.hex, bucket);
      }

      if (entry.role === 'Producer') {
        bucket.producers.push(entry);
      } else if (entry.role === 'Consumer') {
        bucket.consumers.push(entry);
      } else {
        bucket.consumers.push(entry);
      }
    }
  }

  return index;
}

/** Singleton catalog store (the editable preview is now a pure function;
 *  consumers read it via `effectiveLayoutStore.preview` in `$lib/layout`). */
