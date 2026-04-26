import type { OfflineChangeRow } from '$lib/api/sync';
import { countModifiedLeaves, type NodeConfigTree, type SaveProgress } from '$lib/types/nodeTree';

export interface SaveControlsViewState {
  canSave: boolean;
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
  layoutIsDirty: boolean;
  layoutIsOfflineMode: boolean;
  offlineDraftCount: number;
  offlineDraftRows: OfflineChangeRow[];
  saveProgressState: SaveProgress['state'];
  trees: Map<string, NodeConfigTree>;
}): SaveControlsViewState {
  const {
    bowtieMetadataEditCount,
    bowtieMetadataIsDirty,
    layoutIsDirty,
    layoutIsOfflineMode,
    offlineDraftCount,
    offlineDraftRows,
    saveProgressState,
    trees,
  } = args;

  let dirtyCount = 0;
  const dirtyNodeIds = new Set<string>();

  for (const [nodeId, tree] of trees) {
    const modifiedCount = countModifiedLeaves(tree);
    dirtyCount += modifiedCount;
    if (modifiedCount > 0) {
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
  const hasOfflineEdits = layoutIsOfflineMode && offlineDraftCount > 0;
  const hasEdits = hasConfigEdits || hasMetadataEdits || hasOfflineEdits || layoutIsDirty;
  const pendingEditCount = layoutIsOfflineMode
    ? offlineDraftCount + (layoutIsDirty && offlineDraftCount === 0 ? 1 : 0)
    : dirtyCount + (hasMetadataEdits ? bowtieMetadataEditCount : 0);
  const pendingHintText = `${pendingEditCount} ${layoutIsOfflineMode ? 'unsaved edit' : 'unsaved change'}${pendingEditCount === 1 ? '' : 's'}`;
  const isSaving = saveProgressState === 'saving';

  return {
    canSave: hasEdits && !isSaving,
    dirtyCount,
    dirtyNodeCount: dirtyNodeIds.size,
    discardFieldCount: layoutIsOfflineMode ? offlineDraftCount : dirtyCount,
    discardNodeCount: layoutIsOfflineMode ? offlineDirtyNodeCount : dirtyNodeIds.size,
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