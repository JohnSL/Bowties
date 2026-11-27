/**
 * `$lib/layout` — the single import surface for layout-state reads and writes
 * from routes and components (ADR-0004).
 *
 * Components must not reach past this module into the edit-layer stores.
 * The facade provides read projections and named commands instead. Those stores
 * remain internal implementation details of the facade and orchestrators.
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

import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { editKeyForLeaf } from '$lib/utils/editKey';
import { makeValueResolver } from '$lib/utils/displayResolution';
import { resolveNodeDisplayName, resolveEffectiveUserName, resolveNodePartsFromSnip, type NodeDisplayParts } from '$lib/utils/nodeDisplayName';
import { toCanonicalNodeKey, isPlaceholderInput, type NodeKeyInput } from '$lib/utils/nodeKey';
import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import { buildElementLabel } from '$lib/types/nodeTree';
import type { ElementSelection } from '$lib/types/bowtie';
import { get } from 'svelte/store';

// ── Read model ───────────────────────────────────────────────────────────────

export { effectiveLayoutStore } from './effectiveLayoutStore.svelte';
export { effectiveNodeStore, type NodeOrigin, type DirtyBreakdown } from './effectiveNodeStore.svelte';

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
 * For live nodes:
 *   1. Effective ACDI User Name leaf (space 251, draft → offline → baseline)
 *   2. SNIP user_name → manufacturer+model → model → Node ID hex
 *
 * For placeholder nodes:
 *   1. Effective ACDI User Name leaf (if placeholder tree has space 251)
 *   2. CDI identification manufacturer+model → model → placeholder key literal
 *
 * This is the canonical single entry point for any surface that displays a
 * human-readable node name. Accepts `NodeKeyInput` (ADR-0010): both branded
 * `NodeKey` and raw strings (canonical 12-hex or `placeholder:<uuid>`).
 *
 * Do NOT read `snip_data.user_name` directly or call `resolveNodeDisplayName`
 * alone — both miss the edit layer and placeholder dispatch.
 */
export function resolveNodeName(nodeId: NodeKeyInput): string {
  const canonical = toCanonicalNodeKey(nodeId);
  if (!canonical) return '';

  const tree = nodeTreeStore.getTree(canonical);

  // Edit-layer User Name — works for both live and placeholder nodes
  // (if their tree has an ACDI user-info segment in space 251).
  const editedName = resolveEffectiveUserName(
    tree,
    (leaf) => configChangesStore.overrideValue(editKeyForLeaf(canonical, leaf.space, leaf.address)) ?? leaf.value,
  );
  if (editedName) return editedName;

  // Placeholder fallback: CDI identification from the bundled tree
  if (isPlaceholderInput(canonical)) {
    const identity = tree?.identity;
    const manufacturer = identity?.manufacturer?.trim() ?? '';
    const model = identity?.model?.trim() ?? '';
    if (manufacturer && model) return `${manufacturer} — ${model}`;
    if (model) return model;
    return canonical;
  }

  // Live-node fallback: SNIP data from nodeInfoStore
  const nodes = get(nodeInfoStore);
  const key = toCanonicalNodeKey(nodeId);
  return resolveNodeDisplayName(canonical, nodes.get(key));
}

// Re-export the parts type for consumers.
export type { NodeDisplayParts };

/**
 * Resolve structured display parts for a node, edit-layer-aware.
 *
 * Returns `{ name, model, manufacturer, isUserNamed }` following the same
 * priority waterfall as `resolveNodeName`:
 *   edit-layer ACDI User Name → SNIP user_name → model → Node ID hex
 *
 * Unlike `resolveNodeName`, the manufacturer is never baked into `name`.
 * Callers compose the display from structured parts (e.g. via `<NodeLabel>`).
 */
export function resolveNodeParts(nodeId: NodeKeyInput): NodeDisplayParts {
  const canonical = toCanonicalNodeKey(nodeId);
  if (!canonical) return { name: '', model: null, manufacturer: null, isUserNamed: false };

  const tree = nodeTreeStore.getTree(canonical);

  // Edit-layer User Name (draft → offline → baseline).
  const editedName = resolveEffectiveUserName(
    tree,
    (leaf) => configChangesStore.overrideValue(editKeyForLeaf(canonical, leaf.space, leaf.address)) ?? leaf.value,
  );

  // Gather SNIP model/manufacturer regardless of name source.
  if (isPlaceholderInput(canonical)) {
    const identity = tree?.identity;
    const manufacturer = identity?.manufacturer?.trim() || null;
    const model = identity?.model?.trim() || null;
    if (editedName) return { name: editedName, model, manufacturer, isUserNamed: true };
    if (model) return { name: model, model, manufacturer, isUserNamed: false };
    return { name: canonical, model: null, manufacturer, isUserNamed: false };
  }

  // Live node: read SNIP from nodeInfoStore.
  const nodes = get(nodeInfoStore);
  const key = toCanonicalNodeKey(nodeId);
  const node = nodes.get(key);

  if (editedName) {
    const snip = node?.snip_data;
    const manufacturer = snip?.manufacturer?.trim() || null;
    const model = snip?.model?.trim() || null;
    return { name: editedName, model, manufacturer, isUserNamed: true };
  }

  return resolveNodePartsFromSnip(canonical, node);
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
export function buildElementSelection(leaf: LeafConfigNode, nodeId: NodeKeyInput): ElementSelection {
  const canonical = toCanonicalNodeKey(nodeId);
  const tree = nodeTreeStore.getTree(canonical);
  const resolver = makeValueResolver(canonical);
  return {
    nodeId: canonical,
    nodeName: resolveNodeName(canonical),
    elementPath: leaf.path,
    elementLabel: tree ? buildElementLabel(tree, leaf, resolver) : leaf.name,
    address: leaf.address,
    space: leaf.space,
    currentEventId: leaf.value?.type === 'eventId' ? leaf.value.hex : '00.00.00.00.00.00.00.00',
  };
}

// ── Save and discard workflows ───────────────────────────────────────────────

export { discardLayoutEdits } from '$lib/orchestration/layoutDiscardOrchestrator';

export {
  saveLayoutOrchestrated,
  type SaveLayoutOrchestratedArgs,
  type SaveLayoutOrchestratedResult,
} from '$lib/orchestration/saveLayoutOrchestrator';

// ── Bowtie catalog ───────────────────────────────────────────────────────────

export const bowtieCatalogView = {
  get catalog() { return bowtieCatalogStore.catalog; },
  get readComplete() { return bowtieCatalogStore.readComplete; },
  get displayableBowties() { return bowtieCatalogStore.displayableBowties; },
  get effectiveNodeSlotMap() { return bowtieCatalogStore.effectiveNodeSlotMap; },
};

export function catalogGetDisplayName(...args: Parameters<typeof bowtieCatalogStore.getDisplayName>) {
  return bowtieCatalogStore.getDisplayName(...args);
}

export function catalogSetCatalog(...args: Parameters<typeof bowtieCatalogStore.setCatalog>) {
  return bowtieCatalogStore.setCatalog(...args);
}

export function catalogStartListening(...args: Parameters<typeof bowtieCatalogStore.startListening>) {
  return bowtieCatalogStore.startListening(...args);
}

// ── Layout session ───────────────────────────────────────────────────────────

export const layoutSession = {
  get activeContext() { return layoutStore.activeContext; },
  get hasLayoutFile() { return layoutStore.hasLayoutFile; },
  get isConnected() { return layoutStore.isConnected; },
  get isOfflineMode() { return layoutStore.isOfflineMode; },
  get isLoaded() { return layoutStore.isLoaded; },
  get layout() { return layoutStore.layout; },
};

export function layoutSetConnected(...args: Parameters<typeof layoutStore.setConnected>) {
  return layoutStore.setConnected(...args);
}

export function layoutMarkClean(...args: Parameters<typeof layoutStore.markClean>) {
  return layoutStore.markClean(...args);
}

export function layoutHydrateFromBackend(...args: Parameters<typeof layoutStore.hydrateFromBackend>) {
  return layoutStore.hydrateFromBackend(...args);
}

export function layoutSetActiveContext(...args: Parameters<typeof layoutStore.setActiveContext>) {
  return layoutStore.setActiveContext(...args);
}

// ── Bowtie metadata ──────────────────────────────────────────────────────────

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

export function metadataGetAllTags(...args: Parameters<typeof bowtieMetadataStore.getAllTags>) {
  return bowtieMetadataStore.getAllTags(...args);
}

export function metadataCreateBowtie(...args: Parameters<typeof bowtieMetadataStore.createBowtie>) {
  return bowtieMetadataStore.createBowtie(...args);
}

export function metadataAdoptEventId(...args: Parameters<typeof bowtieMetadataStore.adoptEventId>) {
  return bowtieMetadataStore.adoptEventId(...args);
}

export function metadataDemoteToPlanningBowtie(...args: Parameters<typeof bowtieMetadataStore.demoteToPlanningBowtie>) {
  return bowtieMetadataStore.demoteToPlanningBowtie(...args);
}

export function metadataRenameBowtie(...args: Parameters<typeof bowtieMetadataStore.renameBowtie>) {
  return bowtieMetadataStore.renameBowtie(...args);
}

export function metadataAddTag(...args: Parameters<typeof bowtieMetadataStore.addTag>) {
  return bowtieMetadataStore.addTag(...args);
}

export function metadataRemoveTag(...args: Parameters<typeof bowtieMetadataStore.removeTag>) {
  return bowtieMetadataStore.removeTag(...args);
}

export function metadataClearAll(...args: Parameters<typeof bowtieMetadataStore.clearAll>) {
  return bowtieMetadataStore.clearAll(...args);
}

// ── Configuration drafts ─────────────────────────────────────────────────────

/** Record a pending config value draft for a single leaf. */
export function recordConfigDraft(editKey: string, value: TreeConfigValue): void {
  configChangesStore.set(editKey, value);
}

export function configChangeLayers(...args: Parameters<typeof configChangesStore.changeLayers>) {
  return configChangesStore.changeLayers(...args);
}

export function configHasDraftsForNode(...args: Parameters<typeof configChangesStore.hasDraftsForNode>) {
  return configChangesStore.hasDraftsForNode(...args);
}

export function configOverrideValue(...args: Parameters<typeof configChangesStore.overrideValue>) {
  return configChangesStore.overrideValue(...args);
}

export function configVisibleValue(...args: Parameters<typeof configChangesStore.visibleValue>) {
  return configChangesStore.visibleValue(...args);
}

export function configRevert(...args: Parameters<typeof configChangesStore.revert>) {
  return configChangesStore.revert(...args);
}

export function configDraftEntries(...args: Parameters<typeof configChangesStore.draftEntries>) {
  return configChangesStore.draftEntries(...args);
}

export function configClearAllDrafts(...args: Parameters<typeof configChangesStore.clearAllDrafts>) {
  return configChangesStore.clearAllDrafts(...args);
}

export function configPruneResolvedDraftsForNode(...args: Parameters<typeof configChangesStore.pruneResolvedDraftsForNode>) {
  return configChangesStore.pruneResolvedDraftsForNode(...args);
}

// ── Railroad channels ──

export const channelsView = {
  get channels() { return channelsStore.channels; },
  get groupedByHardware() { return channelsStore.groupedByHardware; },
  get isEmpty() { return channelsStore.isEmpty; },
};

export function channelsLoadChannels(...args: Parameters<typeof channelsStore.loadChannels>) {
  return channelsStore.loadChannels(...args);
}

export function channelsRenameChannel(...args: Parameters<typeof channelsStore.renameChannel>) {
  return channelsStore.renameChannel(...args);
}

// ── Facilities ──

export const facilitiesView = {
  get facilities() { return facilitiesStore.facilities; },
  get isEmpty() { return facilitiesStore.isEmpty; },
  get migrationNotice() { return facilitiesStore.migrationNotice; },
};

export function facilitiesLoadFacilities(...args: Parameters<typeof facilitiesStore.loadFacilities>) {
  return facilitiesStore.loadFacilities(...args);
}

export function facilitiesAcknowledgeMigrationNotice(...args: Parameters<typeof facilitiesStore.acknowledgeMigrationNotice>) {
  return facilitiesStore.acknowledgeMigrationNotice(...args);
}

export function facilitiesAddFacility(...args: Parameters<typeof facilitiesStore.addFacility>) {
  return facilitiesStore.addFacility(...args);
}

export function facilitiesRenameFacility(...args: Parameters<typeof facilitiesStore.renameFacility>) {
  return facilitiesStore.renameFacility(...args);
}

// ── Connector selections ──

export const connectorSelectionsView = {
  get revision() { return connectorSelectionsStore.revision; },
  get totalWarningCount() { return connectorSelectionsStore.totalWarningCount; },
};

export function connectorsGetWarnings(...args: Parameters<typeof connectorSelectionsStore.getWarnings>) {
  return connectorSelectionsStore.getWarnings(...args);
}

export function connectorsGetDocument(...args: Parameters<typeof connectorSelectionsStore.getDocument>) {
  return connectorSelectionsStore.getDocument(...args);
}

export function connectorsGetError(...args: Parameters<typeof connectorSelectionsStore.getError>) {
  return connectorSelectionsStore.getError(...args);
}

export function connectorsGetProfile(...args: Parameters<typeof connectorSelectionsStore.getProfile>) {
  return connectorSelectionsStore.getProfile(...args);
}

export function connectorsHydrateFromLayout(...args: Parameters<typeof connectorSelectionsStore.hydrateFromLayout>) {
  return connectorSelectionsStore.hydrateFromLayout(...args);
}

export function connectorsHydrateBaseline(...args: Parameters<typeof connectorSelectionsStore.hydrateBaseline>) {
  return connectorSelectionsStore.hydrateBaseline(...args);
}

export function connectorsLoadNode(...args: Parameters<typeof connectorSelectionsStore.loadNode>) {
  return connectorSelectionsStore.loadNode(...args);
}

// ── Pending changes ──

export const pendingChangesView = {
  get persistedRows() { return pendingChangesStore.persistedRows; },
  get draftRows() { return pendingChangesStore.draftRows; },
  get pendingCount() { return pendingChangesStore.pendingCount; },
  get isBusy() { return pendingChangesStore.isBusy; },
};

export function pendingHasPersistedConfigChange(...args: Parameters<typeof pendingChangesStore.hasPersistedConfigChange>) {
  return pendingChangesStore.hasPersistedConfigChange(...args);
}

export function pendingFindPersistedConfigChange(...args: Parameters<typeof pendingChangesStore.findPersistedConfigChange>) {
  return pendingChangesStore.findPersistedConfigChange(...args);
}

export function pendingRevertToBaseline(...args: Parameters<typeof pendingChangesStore.revertToBaseline>) {
  return pendingChangesStore.revertToBaseline(...args);
}

export function pendingRecordDraftsToBackend(...args: Parameters<typeof pendingChangesStore.recordDraftsToBackend>) {
  return pendingChangesStore.recordDraftsToBackend(...args);
}

export function pendingReloadFromBackend(...args: Parameters<typeof pendingChangesStore.reloadFromBackend>) {
  return pendingChangesStore.reloadFromBackend(...args);
}

export type { ActiveLayoutContext } from '$lib/stores/layout.svelte';
