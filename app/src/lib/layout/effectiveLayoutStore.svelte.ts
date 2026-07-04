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
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
import {
  isLeaf,
  collectEventIdLeaves,
  replicationInstances,
  getInstanceDisplayName,
} from '$lib/types/nodeTree';
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
import type { ChannelRole, InformationChannel } from '$lib/api/channels';
import { editKeyForLeaf } from '$lib/utils/editKey';
import { isPlaceholderEventId } from '$lib/utils/eventIds';
import { makeValueResolver } from '$lib/utils/displayResolution';

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

  // ── Channel ↔ Facility-slot usage (Spec 018 / S4 — D1, D2) ─────────────

  /**
   * Map of channelId → facility-slot entries currently consuming the
   * channel (flattened from every facility's `slotBindings`, regardless
   * of slot cardinality). Per ADR-0004 this is the single owner of
   * `usedBy` derivation; `ChannelsPanel` / `ChannelRow` consume it via
   * a resolver prop wired by the route. Empty channels are absent from
   * the map (the row renders em-dash).
   */
  get channelUsageMap(): Map<string, ChannelUsageEntry[]> {
    const map = new Map<string, ChannelUsageEntry[]>();
    for (const facility of facilitiesStore.facilities) {
      for (const [slotLabel, channelIds] of Object.entries(facility.slotBindings)) {
        for (const channelId of channelIds) {
          const entry: ChannelUsageEntry = {
            facilityId: facility.facilityId,
            facilityName: facility.name,
            slotLabel,
          };
          const list = map.get(channelId);
          if (list) list.push(entry);
          else map.set(channelId, [entry]);
        }
      }
    }
    return map;
  }

  /**
   * Unbound channels filtered by `role` (Spec 018 / S4 — D2: one-slot-
   * per-channel invariant). A channel is "unbound" when it does not
   * appear in any facility's slot bindings. The optional `excludeIds`
   * set lets Rebind include the currently-bound channel as the pre-
   * selected option even though it is technically bound.
   */
  unboundChannelsForRole(
    role: ChannelRole,
    opts?: { excludeIds?: ReadonlySet<string> },
  ): InformationChannel[] {
    const usage = this.channelUsageMap;
    const exclude = opts?.excludeIds;
    return channelsStore.channels.filter((ch) => {
      if (ch.role !== role) return false;
      if (usage.has(ch.id)) return exclude?.has(ch.id) ?? false;
      return true;
    });
  }

  /**
   * Spec 018 / S5 (D1) — eligible Direct Lamp Control rows for the
   * given style id, grouped by node, across every node in the roster
   * whose CDI tree declares a `Direct Lamp Control` segment.
   *
   * Today only `single-led-direct-lamp` resolves to non-empty groups.
   * Rows already claimed by a `lampRow`-binding channel are excluded;
   * `excludeChannelId` puts a row back into the picker when Rebind
   * needs it as the pre-selected option (Rebind on output is itself
   * deferred to S6; the opts shape is forward-compatible).
   *
   * Row labels reuse [`getInstanceDisplayName`] (the Config-tab helper):
   * when the row's `Lamp Description` field has a non-empty value the
   * label reads `"My Block 5 (7)"`, otherwise it falls back to
   * `instanceLabel` (`"Lamp 7"`). Draft and offline-pending edits to the
   * description flow through `makeValueResolver`, so a renamed-but-not-yet-
   * saved lamp shows the new name immediately.
   *
   * D5 deferral: this slice does NOT apply the constraint-based
   * filter (`Lamp Selection != "Used by Mast"`). When the future
   * `compute_active_styles` driver lands, that filter slots in here.
   */
  eligibleLampRowsForStyle(
    styleId: string,
    opts?: { excludeChannelId?: string },
  ): EligibleLampRowGroup[] {
    if (styleId !== 'single-led-direct-lamp') return [];

    const claims = new Set<string>();
    for (const ch of channelsStore.channels) {
      if (ch.binding.kind !== 'lampRow') continue;
      if (opts?.excludeChannelId && ch.id === opts.excludeChannelId) continue;
      claims.add(`${ch.binding.nodeKey}|${ch.binding.rowOrdinal}`);
    }

    const groups: EligibleLampRowGroup[] = [];
    for (const entry of nodeRoster.allEntries) {
      const tree = entry.tree;
      if (!tree) continue;
      const segment = tree.segments.find((s) => s.name === 'Direct Lamp Control');
      if (!segment) continue;
      const nodeName = resolveNodeNameForKey(entry.nodeKey);
      const resolveValue = makeValueResolver(tree.nodeId);
      const rows: EligibleLampRow[] = [];
      for (const instance of replicationInstances(segment.children, 'Lamp')) {
        const ordinal = instance.instance;
        if (claims.has(`${entry.nodeKey}|${ordinal}`)) continue;
        const rowLabel =
          instance.displayName ?? getInstanceDisplayName(instance, resolveValue);
        rows.push({
          nodeKey: entry.nodeKey,
          nodeName,
          rowOrdinal: ordinal,
          rowLabel,
        });
      }
      if (rows.length === 0) continue;
      groups.push({ nodeKey: entry.nodeKey, nodeName, rows });
    }
    return groups;
  }

  private _roleMatches(nodeId: string, leaf: LeafConfigNode, filter: EventRole): boolean {
    const effective = this.effectiveRole(nodeId, leaf);
    if (effective === null || effective === 'Ambiguous') return true;
    return effective === filter;
  }

  /**
   * Spec 018 / S6 (D5) — derived facility status from slot fullness.
   *
   * `'Wired'` iff, for every slot declared by the facility's template,
   * `facility.slotBindings[slot.label].length >= slot.minChannels`. Unknown
   * facility ids return `'Incomplete'` (defensive default). Slots with
   * `minChannels === 0` are always considered satisfied (forward-compat
   * with future optional slots — Block Indicator does not exercise this).
   *
   * This is the sole reader for the FacilityCard status pill per ADR-0004.
   */
  facilityStatus(facilityId: string): 'Wired' | 'Incomplete' {
    const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
    if (!facility) return 'Incomplete';
    const template = behaviorTemplatesStore.findByTemplateId(facility.templateId);
    // If the template is missing we cannot verify fullness — default to Incomplete.
    if (!template) return 'Incomplete';
    for (const slot of template.slots) {
      const bound = facility.slotBindings[slot.label]?.length ?? 0;
      if (bound < slot.minChannels) return 'Incomplete';
    }
    return 'Wired';
  }
}

/** A single facility-slot consumer of a channel. */
export interface ChannelUsageEntry {
  facilityId: string;
  facilityName: string;
  slotLabel: string;
}

/** Spec 018 / S5 (D1) — one eligible Direct Lamp Control row, ready for the AddChannelPicker. */
export interface EligibleLampRow {
  nodeKey: string;
  nodeName: string;
  rowOrdinal: number;
  rowLabel: string;
}

/**
 * Spec 018 / S5 — eligible lamp rows grouped by their owning node. The picker
 * renders one section header per group so the user can tell which Signal-LCC a
 * row lives on; the row labels themselves drop the redundant node prefix.
 */
export interface EligibleLampRowGroup {
  nodeKey: string;
  nodeName: string;
  rows: EligibleLampRow[];
}

/**
 * Lightweight node-name resolver used inside `effectiveLayoutStore`.
 *
 * Falls back to a SNIP-based display because we can't import the canonical
 * `resolveNodeName` from `$lib/layout` (that module re-exports us, so the
 * import would be circular). The caller-supplied `nodeName` prop on the
 * route is still preferred when the result reaches the UI.
 */
function resolveNodeNameForKey(nodeKey: string): string {
  const entry = nodeRoster.allEntries.find((e) => e.nodeKey === nodeKey);
  if (!entry) return nodeKey;
  const snip = entry.info?.snip_data ?? null;
  const userName = snip?.user_name?.trim();
  if (userName) return userName;
  const model = snip?.model?.trim();
  if (model) return model;
  return nodeKey;
}

/** Singleton effective-layout read model. */
export const effectiveLayoutStore = new EffectiveLayoutStore();
