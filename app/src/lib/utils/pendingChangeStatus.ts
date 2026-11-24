import type { PendingChangeRow } from '$lib/api/sync';

/**
 * True while the board has not confirmed a Pending Change: every status except
 * applied, already applied and clean. Mirrors `PendingChangeStatus::is_waiting`
 * in bowties-core; the one shared "still waiting" predicate (E3b D3).
 */
export function isWaitingPendingChange(row: Pick<PendingChangeRow, 'status'>): boolean {
  return row.status !== 'applied' && row.status !== 'alreadyApplied' && row.status !== 'clean';
}
