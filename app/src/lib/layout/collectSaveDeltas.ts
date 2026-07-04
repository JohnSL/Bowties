/**
 * collectAllSaveDeltas — single aggregation seam for `LayoutEditDelta`s
 * heading into `saveLayoutDirectory` (ADR-0002 atomic save + ADR-0011
 * facade pattern).
 *
 * Dispatch is registry-driven: every layout-scoped store that produces
 * edit deltas implements `collectDeltas()` on the `LayoutScopedParticipant`
 * interface and is registered in `layoutScopedParticipants`. Adding a new
 * edit-bearing store means implementing the method and appending to the
 * array — the dispatch loop picks it up automatically.
 *
 * Sibling of `effectiveNodeStore.dirtyBreakdown`. Both facades enumerate
 * every edit-bearing store and are the enforced enrollment point for new
 * stores: adding a store to the app means adding a call here AND a
 * bucket to `dirtyBreakdown`. When one is added without the other,
 * either the UI dirties without the deltas persisting (the bug this
 * facade replaces) or the save persists edits the UI never surfaced.
 *
 * The route (`+page.svelte`) calls this once and hands the result to
 * `saveLayoutDirectory`. Every edit variant — bowtie, connector, facility,
 * channel — rides the same atomic save.
 */

import type { LayoutEditDelta } from '$lib/types/bowtie';
import { layoutScopedParticipants } from '$lib/orchestration/layoutLifecycleOrchestrator';

/**
 * Collect every pending `LayoutEditDelta` across all edit-bearing stores
 * in the `layoutScopedParticipants` registry.
 *
 * Callers must not depend on the ordering across stores —
 * `apply_*_deltas` on the backend sort by category. Within a store,
 * order matches the store's own `collectDeltas` contract.
 */
export function collectAllSaveDeltas(): LayoutEditDelta[] {
  const deltas: LayoutEditDelta[] = [];
  for (const p of layoutScopedParticipants) {
    if (p.collectDeltas) {
      deltas.push(...p.collectDeltas());
    }
  }
  return deltas;
}
