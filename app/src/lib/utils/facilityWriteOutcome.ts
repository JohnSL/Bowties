import type { PendingChangeRow } from '$lib/api/sync';
import { isWaitingPendingChange } from '$lib/utils/pendingChangeStatus';
import { normalizeNodeId } from '$lib/utils/nodeId';

/** One board's share of a facility's waiting Pending Change rows. */
export interface BoardWriteOutcome {
  nodeKey: string;
  /** Waiting rows that have no result yet: never tried, so nothing to explain. */
  notAttempted: number;
  /** Failure or conflict message of each waiting row that has one. */
  waitingReasons: string[];
  /** Waiting rows with a result: failed, conflict or skipped. */
  waiting: number;
}

export interface FacilityWriteOutcome {
  boards: BoardWriteOutcome[];
  hasWaiting: boolean;
}

/**
 * Pure derivation of a facility's per-board write outcome from Pending Change
 * rows tagged with `origin.facilityId`. "Waiting" follows the shared
 * `isWaitingPendingChange` predicate; a waiting row that is still `pending` has
 * not been attempted. A written row is removed (ADR-0001), so only boards with
 * a waiting row appear.
 */
export function facilityWriteOutcome(
  facilityId: string,
  rows: readonly PendingChangeRow[],
): FacilityWriteOutcome {
  const byBoard = new Map<string, BoardWriteOutcome>();
  for (const row of rows) {
    if (row.origin?.facilityId !== facilityId || !isWaitingPendingChange(row)) continue;
    const nodeKey = normalizeNodeId(row.nodeId);
    let board = byBoard.get(nodeKey);
    if (!board) {
      board = { nodeKey, notAttempted: 0, waitingReasons: [], waiting: 0 };
      byBoard.set(nodeKey, board);
    }
    if (row.status === 'pending') {
      board.notAttempted += 1;
      if (row.error) board.waitingReasons.push(row.error);
    } else {
      board.waiting += 1;
      if (row.error) board.waitingReasons.push(row.error);
    }
  }
  const boards = [...byBoard.values()];
  return { boards, hasWaiting: boards.length > 0 };
}
