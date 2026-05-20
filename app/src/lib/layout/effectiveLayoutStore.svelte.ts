/**
 * effectiveLayoutStore — ADR-0004 (Layout facade: effective view store).
 *
 * Single derived read model projecting the four edit-layer stores
 * (`bowtieCatalogStore`, `layoutStore`, `bowtieMetadataStore`,
 *  `configChangesStore`) and the loaded `nodeTreeStore` into the values
 * the UI renders.
 *
 * Subsumes the leaf-level `resolveValue` / `resolveRole` helpers from
 * ADR-0003 and the `EditableBowtiePreviewStore` fast/slow-path branch from
 * `bowties.svelte.ts`.
 *
 * Public API (this store is the only layout read surface for components,
 * re-exported by `$lib/layout`):
 *
 *   - `preview`           — `EditableBowtiePreview` for panel rendering
 *   - `effectiveBowties`  — `PreviewBowtieCard[]` with pending deletions
 *                           removed and pending entry edits merged
 *   - `effectiveValue`    — draft → offline pending → leaf baseline
 *   - `effectiveRole`     — pending classify → saved layout → catalog
 *                           → leaf CDI baseline
 *   - `slotsByRole`       — pre-filtered event-id leaves under a node
 *   - `isSlotFree`        — leaf is not connected through any catalog card,
 *                           ignoring catalog cards with a pending deletion
 */

import { buildEffectiveBowtiePreview, bowtieCatalogStore } from '$lib/stores/bowties.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { isLeaf, isGroup, collectEventIdLeaves } from '$lib/types/nodeTree';
import type {
  ConfigNode,
  EventRole,
  LeafConfigNode,
  TreeConfigValue,
} from '$lib/types/nodeTree';
import type {
  EditableBowtiePreview,
  PreviewBowtieCard,
} from '$lib/types/bowtie';
import { editKeyForLeaf } from '$lib/utils/editKey';
import { isPlaceholderEventId } from '$lib/utils/eventIds';

// Re-export the catalog store so consumers of the facade can drive it in tests
// without reaching into the legacy bowties.svelte module directly.
export { bowtieCatalogStore };

class EffectiveLayoutStore {
  // ── Bowtie cards ─────────────────────────────────────────────────────────

  /**
   * Full preview (cards + dirty flag) consumed by the bowtie catalog panel.
   * Pending bowtie deletions are removed up front so the UI never shows a
   * card that the user has already discarded.
   */
  get preview(): EditableBowtiePreview {
    const inner = buildEffectiveBowtiePreview();
    const bowties = inner.bowties.filter(
      (card) => !bowtieMetadataStore.hasPendingDeletion(card.eventIdHex),
    );
    return { bowties, hasUnsavedChanges: inner.hasUnsavedChanges };
  }

  /** Bowtie cards visible to the user. */
  get effectiveBowties(): PreviewBowtieCard[] {
    return this.preview.bowties;
  }

  /**
   * Map of eventIdHex → preview card for the currently visible bowties.
   * Used by connection-resolution flows that need O(1) lookups by event ID.
   */
  get usedInMap(): Map<string, PreviewBowtieCard> {
    const map = new Map<string, PreviewBowtieCard>();
    for (const card of this.preview.bowties) {
      map.set(card.eventIdHex, card);
    }
    return map;
  }

  // ── Leaf-level resolution ────────────────────────────────────────────────

  /**
   * Resolve the effective display value for a leaf.
   * Priority: draft → offlinePending → leaf baseline.
   */
  effectiveValue(nodeId: string, leaf: LeafConfigNode): TreeConfigValue | null {
    const key = editKeyForLeaf(nodeId, leaf.space, leaf.address);
    return configChangesStore.overrideValue(key) ?? leaf.value;
  }

  /**
   * Resolve the effective role classification for a leaf.
   * Priority: pending classify edit → saved layout roleClassification →
   * catalog role → leaf CDI baseline.
   */
  effectiveRole(nodeId: string, leaf: LeafConfigNode): EventRole | null {
    const slotKey = `${nodeId}:${leaf.path.join('/')}`;

    const classified = bowtieMetadataStore.getRoleClassification(slotKey);
    if (classified) return classified.role;

    const catalogRole = bowtieCatalogStore.getRoleForSlot(nodeId, leaf.path);
    if (catalogRole) return catalogRole;

    return leaf.eventRole ?? null;
  }

  // ── Slot queries ─────────────────────────────────────────────────────────

  /**
   * All event-id leaves under a node whose effective role matches `role`.
   * A `null` filter returns every event-id leaf.
   *
   * "Ambiguous" and `null` effective roles match any non-null filter — they
   * are surfaced so the picker can either auto-classify (when the picker
   * itself has a definite role) or prompt the user.
   */
  slotsByRole(nodeId: string, role: EventRole | null): LeafConfigNode[] {
    const tree = nodeTreeStore.getTree(nodeId);
    if (!tree) return [];
    const leaves = collectEventIdLeaves(tree);
    if (role === null) return leaves;
    return leaves.filter((leaf) => this._roleMatches(nodeId, leaf, role));
  }

  /**
   * True when the picker may select this slot — i.e. its effective event ID
   * is not already participating in an active bowtie.
   *
   * Catalog cards with a pending `deleteBowtie` edit do not count as
   * occupying the slot, so a user can immediately re-use the slot after
   * deleting its bowtie.
   */
  isSlotFree(nodeId: string, leaf: LeafConfigNode): boolean {
    const value = this.effectiveValue(nodeId, leaf);
    if (!value || value.type !== 'eventId') return true;
    const hex = value.hex;
    if (isPlaceholderEventId(hex)) return false;

    const catalog = bowtieCatalogStore.catalog;
    if (!catalog) return true;

    for (const card of catalog.bowties) {
      if (card.event_id_hex !== hex) continue;
      if (bowtieMetadataStore.hasPendingDeletion(card.event_id_hex)) continue;
      return false;
    }
    return true;
  }

  // ── Internals ────────────────────────────────────────────────────────────

  private _roleMatches(nodeId: string, leaf: LeafConfigNode, filter: EventRole): boolean {
    const effective = this.effectiveRole(nodeId, leaf);
    if (effective === null || effective === 'Ambiguous') return true;
    return effective === filter;
  }
}

/** Singleton effective-layout read model. */
export const effectiveLayoutStore = new EffectiveLayoutStore();
