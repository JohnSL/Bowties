/**
 * Unified display resolution — ADR-0003.
 *
 * **Internal helper (ADR-0004).** As of S2c, this module is consumed by the
 * `$lib/layout` facade and a small number of structural helpers
 * (`buildElementLabel`, `getInstanceDisplayName`). Components must import
 * `effectiveLayoutStore`, `makeValueResolver`, etc. from `$lib/layout`
 * instead of from this module directly. New direct imports from components
 * are a layering violation; route them through the facade.
 *
 * Single resolution function for config values, single resolution function for
 * role classifications. All frontend display paths use these so that bowtie
 * cards and the config tree agree on what to render in both online and
 * offline modes.
 *
 * Value resolution order (highest priority first):
 *   1. draft           — unsaved user edit (configChangesStore)
 *   2. offlinePending  — persisted offline change (offlineChangesStore via
 *                        configChangesStore.overrideValue)
 *   3. baseline        — leaf.value from the tree (caller already holds it)
 *
 * Role resolution order (highest priority first):
 *   1. pending edit    — unsaved role classification (bowtieMetadataStore)
 *   2. saved layout    — role classification persisted in the layout file
 *                        (also exposed by bowtieMetadataStore)
 *   3. catalog         — authoritative role across catalog cards
 *                        (bowtieCatalogStore)
 *   4. CDI baseline    — leaf.eventRole from the CDI tree
 *
 * Callers should pass the leaf they already hold so the baseline layer is
 * served without re-walking the tree. The slot-key convention
 * `"${nodeId}:${path.join('/')}"` matches `bowtieMetadataStore` /
 * `bowtieCatalogStore`; the edit-key convention `editKeyForLeaf` is used
 * internally for the value path.
 */

import type { LeafConfigNode, TreeConfigValue, EventRole } from '$lib/types/nodeTree';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
import { editKeyForLeaf } from '$lib/utils/editKey';

/**
 * Resolve the effective display value for a leaf: draft → offline pending →
 * baseline. Returns the leaf's own value when no override layer applies.
 *
 * Use this everywhere a display path needs the "current" value of a config
 * field. Never read `leaf.value` directly when an override layer might exist.
 */
export function resolveValue(
  nodeId: string,
  leaf: LeafConfigNode,
): TreeConfigValue | null {
  const key = editKeyForLeaf(nodeId, leaf.space, leaf.address);
  return configChangesStore.overrideValue(key) ?? leaf.value;
}

/**
 * Build a `(leaf) → value` resolver bound to a node, suitable for passing to
 * helpers such as `getInstanceDisplayName` and `buildElementLabel`.
 */
export function makeValueResolver(
  nodeId: string,
): (leaf: LeafConfigNode) => TreeConfigValue | null {
  return (leaf) => resolveValue(nodeId, leaf);
}

/**
 * Resolve the effective role classification for a leaf:
 * pending edit → saved layout → catalog → CDI baseline.
 *
 * `bowtieMetadataStore.getRoleClassification(slotKey)` covers both the
 * pending-edit and saved-layout layers (it checks pending edits first, then
 * falls back to `layoutStore.layout.roleClassifications`).
 *
 * Returns `null` only when no layer has a role for this slot.
 */
export function resolveRole(
  nodeId: string,
  leaf: LeafConfigNode,
): EventRole | null {
  const slotKey = `${nodeId}:${leaf.path.join('/')}`;

  const classified = bowtieMetadataStore.getRoleClassification(slotKey);
  if (classified) return classified.role;

  const catalogRole = bowtieCatalogStore.getRoleForSlot(nodeId, leaf.path);
  if (catalogRole) return catalogRole;

  return leaf.eventRole ?? null;
}
