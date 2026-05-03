import type { OfflineChangeRow } from '$lib/api/sync';
import { countModifiedLeaves, type NodeConfigTree, type SaveProgress } from '$lib/types/nodeTree';

export interface SaveControlsViewState {
  autoRepairCount: number;
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
  repairSummaryText: string | null;
}

export function deriveSaveControlsViewState(args: {
  autoRepairCount: number;
  bowtieMetadataEditCount: number;
  bowtieMetadataIsDirty: boolean;
  connectorWarningCount: number;
  layoutIsDirty: boolean;
  layoutIsOfflineMode: boolean;
  offlineDraftCount: number;
  offlineDraftRows: OfflineChangeRow[];
  saveProgressState: SaveProgress['state'];
  trees: Map<string, NodeConfigTree>;
}): SaveControlsViewState {
  const {
    autoRepairCount,
    bowtieMetadataEditCount,
    bowtieMetadataIsDirty,
    connectorWarningCount,
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
  const metadataEditCount = hasMetadataEdits ? Math.max(1, bowtieMetadataEditCount) : 0;
  const hasLayoutOnlyEdits = layoutIsDirty && !hasMetadataEdits && offlineDraftCount === 0;
  const layoutOnlyEditCount = hasLayoutOnlyEdits ? 1 : 0;
  const hasOfflineEdits = layoutIsOfflineMode && offlineDraftCount > 0;
  const hasEdits = hasConfigEdits || hasMetadataEdits || hasOfflineEdits || layoutIsDirty;
  const pendingEditCount = layoutIsOfflineMode
    ? offlineDraftCount + metadataEditCount + layoutOnlyEditCount
    : dirtyCount + metadataEditCount + layoutOnlyEditCount;
  const pendingHintText = `${pendingEditCount} ${layoutIsOfflineMode ? 'unsaved edit' : 'unsaved change'}${pendingEditCount === 1 ? '' : 's'}`;
  const repairSummaryText = autoRepairCount > 0
    ? `${autoRepairCount} auto-staged compatibility repair${autoRepairCount === 1 ? '' : 's'}`
    : null;
  const isSaving = saveProgressState === 'saving';
  const baseDiscardFieldCount = layoutIsOfflineMode ? offlineDraftCount : dirtyCount;
  const baseDiscardNodeCount = layoutIsOfflineMode ? offlineDirtyNodeCount : dirtyNodeIds.size;
  const discardFieldCount = baseDiscardFieldCount + metadataEditCount + layoutOnlyEditCount;
  const discardNodeCount = discardFieldCount > 0 ? Math.max(1, baseDiscardNodeCount) : 0;

  return {
    autoRepairCount,
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
    repairSummaryText,
  };
}