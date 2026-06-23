/**
 * `$lib/layout` — the single import surface for layout-state reads and writes
 * from routes and components (ADR-0004).
 *
 * Components must not reach past this module into the four edit-layer stores
 * (`bowtieCatalogStore`, `bowtieMetadataStore`, `configChangesStore`,
 *  `layoutStore`). Those stores keep their current responsibilities but are
 * internal implementation details of the facade and the
 * `saveLayoutOrchestrator`.
 *
 * What lives here:
 *   - `effectiveLayoutStore`         — read model (effectiveBowties,
 *                                       effectiveRole, effectiveValue,
 *                                       slotsByRole, isSlotFree, preview).
 *   - `saveLayoutOrchestrated`       — multi-step save workflow.
 *   - edit-recording commands       — thin wrappers over `bowtieMetadataStore`
 *                                      and `configChangesStore` so callers
 *                                      never import those stores directly.
 *
 * What does NOT live here:
 *   - lifecycle transitions (open / close / disconnect) — `layout.svelte`
 *     still owns those for now; future slices may absorb them into a
 *     dedicated orchestrator.
 *   - rendering. The facade returns data; components render it.
 */

import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { editKeyForLeaf } from '$lib/utils/editKey';
import { makeValueResolver } from '$lib/utils/displayResolution';
import { resolveNodeDisplayName, resolveEffectiveUserName } from '$lib/utils/nodeDisplayName';
import { toCanonicalNodeKey } from '$lib/utils/nodeKey';
import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import { buildElementLabel } from '$lib/types/nodeTree';
import type { ElementSelection } from '$lib/types/bowtie';
import { get } from 'svelte/store';

// ── Read model ───────────────────────────────────────────────────────────────

export { effectiveLayoutStore } from './effectiveLayoutStore.svelte';
export { effectiveNodeStore, type NodeOrigin } from './effectiveNodeStore.svelte';

/**
 * `makeValueResolver(nodeId)` returns a `(leaf) → value` closure suitable for
 * passing into label/name helpers such as `buildElementLabel` and
 * `getInstanceDisplayName`. The closure routes through the same waterfall as
 * `effectiveLayoutStore.effectiveValue` (draft → offline pending → baseline)
 * so labels stay consistent with cards and pickers.
 *
 * Re-exported from the internal `displayResolution` helper so components have
 * a single import surface (`$lib/layout`) and never reach into `$lib/utils`
 * for resolution semantics.
 */
export { makeValueResolver };

/**
 * Resolve a node's Display Name, edit-layer-aware (ADR-0003 point 4):
 *
 *   1. Effective ACDI User Name leaf (space 251, draft → offline → baseline)
 *   2. SNIP user_name → manufacturer+model → model → Node ID hex
 *
 * This is the canonical single entry point for any surface that displays a
 * human-readable node name. It composes `resolveEffectiveUserName` (edit tier)
 * with `resolveNodeDisplayName` (SNIP fallback) against the live stores.
 *
 * Do NOT read `snip_data.user_name` directly or call `resolveNodeDisplayName`
 * alone — both miss the edit layer.
 */
export function resolveNodeName(nodeId: string): string {
  const tree = nodeTreeStore.getTree(nodeId);
  const editedName = resolveEffectiveUserName(
    tree,
    (leaf) => configChangesStore.overrideValue(editKeyForLeaf(nodeId, leaf.space, leaf.address)) ?? leaf.value,
  );
  if (editedName) return editedName;

  const nodes = get(nodeInfoStore);
  const key = toCanonicalNodeKey(nodeId);
  return resolveNodeDisplayName(nodeId, nodes.get(key));
}

/**
 * Build a display-ready `ElementSelection` for a leaf node.
 *
 * Resolves nodeName via the canonical ADR-0003 waterfall (edit-layer → SNIP)
 * and builds the element label from the live tree (including segment name).
 *
 * This is the single construction point for ElementSelection objects. Components
 * must use this instead of inlining the object literal — prevents the class of
 * bug where one site forgets to resolve the display name or skips the tree lookup.
 */
export function buildElementSelection(leaf: LeafConfigNode, nodeId: string): ElementSelection {
  const tree = nodeTreeStore.getTree(nodeId);
  const resolver = makeValueResolver(nodeId);
  return {
    nodeId,
    nodeName: resolveNodeName(nodeId),
    elementPath: leaf.path,
    elementLabel: tree ? buildElementLabel(tree, leaf, resolver) : leaf.name,
    address: leaf.address,
    space: leaf.space,
    currentEventId: leaf.value?.type === 'eventId' ? leaf.value.hex : '00.00.00.00.00.00.00.00',
  };
}

// ── Save workflow ────────────────────────────────────────────────────────────

export {
  saveLayoutOrchestrated,
  type SaveLayoutOrchestratedArgs,
  type SaveLayoutOrchestratedResult,
} from '$lib/orchestration/saveLayoutOrchestrator';

// ── Edit-recording commands ──────────────────────────────────────────────────

/** Record a pending bowtie deletion. Honoured immediately by the read model. */
export function recordBowtieDeletion(eventIdHex: string): void {
  bowtieMetadataStore.deleteBowtie(eventIdHex);
}

/** Record a pending role classification for a slot. */
export function recordRoleClassification(
  slotKey: string,
  role: 'Producer' | 'Consumer',
): void {
  bowtieMetadataStore.classifyRole(slotKey, role);
}

/** Record a pending config value draft for a single leaf. */
export function recordConfigDraft(editKey: string, value: TreeConfigValue): void {
  configChangesStore.set(editKey, value);
}
