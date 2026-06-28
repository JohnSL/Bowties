import type { OfflineChangeRow } from '$lib/api/sync';
import type { DirtyBreakdown } from '$lib/layout';
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

/**
 * Derive the SaveControls toolbar view state from a single
 * `DirtyBreakdown` snapshot (Spec 018 / S1.2, ADR-0011 extension
 * 2026-06-28). The breakdown is the single source of truth for which
 * stores are dirty and by how much — this function shapes those raw
 * counts into the display strings the toolbar renders.
 *
 * `offlineDraftRows` and `connectorWarningCount` are still passed
 * separately because they expose information not captured in
 * `DirtyBreakdown` (per-node distribution of offline drafts; aggregate
 * connector compatibility warnings).
 */
export function deriveSaveControlsViewState(args: {
  breakdown: DirtyBreakdown;
  connectorWarningCount: number;
  layoutIsOfflineMode: boolean;
  offlineDraftRows: OfflineChangeRow[];
  saveProgressState: SaveProgress['state'];
}): SaveControlsViewState {
  const {
    breakdown,
    connectorWarningCount,
    layoutIsOfflineMode,
    offlineDraftRows,
    saveProgressState,
  } = args;

  const dirtyCount = breakdown.config;
  const dirtyNodeCount = breakdown.configNodes;

  const offlineDirtyNodeCount = new Set(
    offlineDraftRows
      .filter((row) => row.status === 'pending' && row.nodeId)
      .map((row) => row.nodeId as string),
  ).size;

  const hasConfigEdits = dirtyCount > 0;
  const hasMetadataEdits = breakdown.metadata > 0;
  const metadataEditCount = breakdown.metadata;
  const hasUnsavedNewNodes = breakdown.unsavedNewNodes > 0;
  // `breakdown.layoutStruct` covers ONLY LayoutFile-struct edits (post-ADR-0011).
  // Unsaved-new-node additions are a separate signal that also counts as a
  // "layout edit" for display purposes.
  const hasLayoutOrNewNodeEdits = breakdown.layoutStruct > 0 || hasUnsavedNewNodes;
  const hasLayoutOnlyEdits = hasLayoutOrNewNodeEdits && !hasMetadataEdits;
  // Each fully-captured unsaved-in-memory addition counts as a distinct
  // layout edit. If the layout is dirty for any reason and we have no node
  // additions to attribute the dirtiness to, fall back to a count of 1 so
  // legacy non-node layout edits (e.g. element-deck reordering) still show.
  const layoutOnlyEditCount = hasLayoutOnlyEdits
    ? (hasUnsavedNewNodes ? breakdown.unsavedNewNodes : 1)
    : 0;
  const hasRevertedPersisted = breakdown.offlineRevertedPersisted > 0;
  const hasOfflineEdits = layoutIsOfflineMode && hasConfigEdits;
  const hasConnectorOrChannelEdits =
    breakdown.connectorSelections > 0 || breakdown.channels > 0;
  const hasFacilityEdits = breakdown.facilities > 0;
  const hasEdits =
    hasConfigEdits
    || hasMetadataEdits
    || hasOfflineEdits
    || hasRevertedPersisted
    || hasLayoutOrNewNodeEdits
    || hasConnectorOrChannelEdits
    || hasFacilityEdits;
  const pendingEditCount =
    dirtyCount
    + breakdown.offlineRevertedPersisted
    + metadataEditCount
    + layoutOnlyEditCount
    + breakdown.connectorSelections
    + breakdown.channels
    + breakdown.facilities;
  const pendingHintText = `${pendingEditCount} ${layoutIsOfflineMode ? 'unsaved edit' : 'unsaved change'}${pendingEditCount === 1 ? '' : 's'}`;
  const isSaving = saveProgressState === 'saving';
  const baseDiscardFieldCount = dirtyCount + breakdown.offlineRevertedPersisted;
  const baseDiscardNodeCount = dirtyNodeCount;
  const discardFieldCount =
    baseDiscardFieldCount
    + metadataEditCount
    + layoutOnlyEditCount
    + breakdown.connectorSelections
    + breakdown.channels
    + breakdown.facilities;
  const discardNodeCount = discardFieldCount > 0 ? Math.max(1, baseDiscardNodeCount) : 0;

  return {
    canSave: hasEdits && !isSaving,
    connectorWarningCount,
    dirtyCount,
    dirtyNodeCount,
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