import type { PendingChangeRow } from '$lib/api/sync';
import type { DirtyBreakdown } from '$lib/layout';
import { isWaitingPendingChange } from '$lib/utils/pendingChangeStatus';
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
  hasWaitingChanges: boolean;
  /** Connected with rows waiting: the toolbar offers "Review in Sync Panel". */
  canReviewWaiting: boolean;
  isSaving: boolean;
  offlineDirtyNodeCount: number;
  pendingEditCount: number;
  /** Unsaved-edit wording; empty when nothing is unsaved. */
  pendingHintText: string;
  /** Saved Pending Change rows still waiting for the board; empty when none. */
  waitingHintText: string;
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
  layoutIsConnected?: boolean;
  offlineDraftRows: PendingChangeRow[];
  saveProgressState: SaveProgress['state'];
}): SaveControlsViewState {
  const {
    breakdown,
    connectorWarningCount,
    layoutIsOfflineMode,
    layoutIsConnected = false,
    offlineDraftRows,
    saveProgressState,
  } = args;

  const dirtyCount = breakdown.config;
  const dirtyNodeCount = breakdown.configNodes;

  const offlineDirtyNodeCount = new Set(
    offlineDraftRows
      .filter((row) => isWaitingPendingChange(row) && row.nodeId)
      .map((row) => row.nodeId as string),
  ).size;

  const hasConfigEdits = breakdown.configChanges > 0;
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
  const configChangeCount = breakdown.configChanges;
  const hasOfflineEdits = layoutIsOfflineMode && hasConfigEdits;
  const hasConnectorOrChannelEdits =
    breakdown.connectorSelections > 0 || breakdown.channels > 0;
  const hasFacilityEdits = breakdown.facilities > 0;
  const hasUnsavedRemovedNodes = breakdown.unsavedRemovedNodes > 0;
  // "Other edits" are every unsaved edit that is not configuration: bowtie
  // names, facilities, channels, connector selections and layout structure
  // (including new / removed nodes). Offline and connected read the same.
  const hasOtherEdits =
    hasMetadataEdits
    || hasLayoutOrNewNodeEdits
    || hasConnectorOrChannelEdits
    || hasFacilityEdits
    || hasUnsavedRemovedNodes;
  const hasEdits = hasConfigEdits || hasOtherEdits;
  const otherEditCount =
    metadataEditCount
    + layoutOnlyEditCount
    + breakdown.connectorSelections
    + breakdown.channels
    + breakdown.facilities;
  const pendingEditCount = configChangeCount + otherEditCount;
  const pendingHintText = !hasEdits
    ? ''
    : hasConfigEdits
      ? `${configChangeCount} configuration ${configChangeCount === 1 ? 'change' : 'changes'}${hasOtherEdits ? ' + other edits' : ''}`
      : 'Unsaved changes';
  const waitingCount = breakdown.waitingPendingChanges;
  const waitingHintText = waitingCount > 0 ? `${waitingCount} waiting to be written to the board` : '';
  const isSaving = saveProgressState === 'saving';
  const discardFieldCount = configChangeCount + otherEditCount;
  const discardNodeCount = discardFieldCount > 0 ? Math.max(1, dirtyNodeCount) : 0;

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
    hasWaitingChanges: waitingCount > 0,
    canReviewWaiting: layoutIsConnected && waitingCount > 0,
    isSaving,
    offlineDirtyNodeCount,
    pendingEditCount,
    pendingHintText,
    waitingHintText,
  };
}