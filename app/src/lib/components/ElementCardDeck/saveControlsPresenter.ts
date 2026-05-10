import type { OfflineChangeRow } from '$lib/api/sync';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import type { SaveProgress } from '$lib/types/nodeTree';

export interface SaveControlsViewState {
  canSave: boolean;
  connectorWarningCount: number;
  dirtyCount: number;
  dirtyNodeCount: number;
  discardFieldCount: number;
  discardNodeCount: number;
  hasConfigEdits: boolean;
  hasEdits: boolean;
  hasMetadataEdits: boolean;
  hasOfflineEdits: boolean;
  isSaving: boolean;
  offlineDirtyNodeCount: number;
  pendingEditCount: number;
  pendingHintText: string;
}

export function deriveSaveControlsViewState(args: {
  bowtieMetadataEditCount: number;
  bowtieMetadataIsDirty: boolean;
  configDraftCount: number;
  connectorWarningCount: number;
  layoutIsDirty: boolean;
  layoutIsOfflineMode: boolean;
  offlineDraftCount: number;
  offlineDraftRows: OfflineChangeRow[];
  revertedPersistedCount: number;
  saveProgressState: SaveProgress['state'];
  treeNodeIds: string[];
}): SaveControlsViewState {
  const {
    bowtieMetadataEditCount,
    bowtieMetadataIsDirty,
    configDraftCount,
    connectorWarningCount,
    layoutIsDirty,
    layoutIsOfflineMode,
    offlineDraftCount,
    offlineDraftRows,
    revertedPersistedCount,
    saveProgressState,
    treeNodeIds,
  } = args;

  // Count config drafts per node via configChangesStore
  let dirtyCount = configDraftCount;
  const dirtyNodeIds = new Set<string>();
  for (const nodeId of treeNodeIds) {
    if (configChangesStore.hasDraftsForNode(nodeId)) {
      dirtyNodeIds.add(nodeId);
    }
  }

  const offlineDirtyNodeCount = new Set(
    offlineDraftRows
      .filter((row) => row.status === 'pending' && row.nodeId)
      .map((row) => row.nodeId as string),
  ).size;

  const hasConfigEdits = dirtyCount > 0;
  const hasMetadataEdits = bowtieMetadataIsDirty;
  const metadataEditCount = hasMetadataEdits ? Math.max(1, bowtieMetadataEditCount) : 0;
  const hasLayoutOnlyEdits = layoutIsDirty && !hasMetadataEdits;
  const layoutOnlyEditCount = hasLayoutOnlyEdits ? 1 : 0;
  const hasRevertedPersisted = revertedPersistedCount > 0;
  const hasOfflineEdits = layoutIsOfflineMode && dirtyCount > 0;
  const hasEdits = hasConfigEdits || hasMetadataEdits || hasOfflineEdits || hasRevertedPersisted || layoutIsDirty;
  const pendingEditCount = dirtyCount + revertedPersistedCount + metadataEditCount + layoutOnlyEditCount;
  const pendingHintText = `${pendingEditCount} ${layoutIsOfflineMode ? 'unsaved edit' : 'unsaved change'}${pendingEditCount === 1 ? '' : 's'}`;
  const isSaving = saveProgressState === 'saving';
  const baseDiscardFieldCount = dirtyCount + revertedPersistedCount;
  const baseDiscardNodeCount = dirtyNodeIds.size;
  const discardFieldCount = baseDiscardFieldCount + metadataEditCount + layoutOnlyEditCount;
  const discardNodeCount = discardFieldCount > 0 ? Math.max(1, baseDiscardNodeCount) : 0;

  return {
    canSave: hasEdits && !isSaving,
    connectorWarningCount,
    dirtyCount,
    dirtyNodeCount: dirtyNodeIds.size,
    discardFieldCount,
    discardNodeCount,
    hasConfigEdits,
    hasEdits,
    hasMetadataEdits,
    hasOfflineEdits,
    isSaving,
    offlineDirtyNodeCount,
    pendingEditCount,
    pendingHintText,
  };
}