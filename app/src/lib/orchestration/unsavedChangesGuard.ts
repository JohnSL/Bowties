import { configChangesStore } from '$lib/stores/configChanges.svelte';

/**
 * True when an exit action (close layout, switch layout, disconnect, app
 * window close) should prompt the user to confirm discarding unsaved work.
 *
 * ADR-0011: callers pass `effectiveNodeStore.isDirty` as the aggregate
 * in-memory-change signal; it already folds in LayoutFile-struct edits,
 * config drafts, bowtie metadata edits, offline drafts, reverted-persisted
 * offline rows, and fully-captured discovered nodes not yet saved. The
 * other parameters are accepted for explicitness so this helper stays a
 * pure predicate testable without store wiring.
 */
export function hasUnsavedPromptChanges(
  treeNodeIds: Iterable<string>,
  bowtieMetadataDirty: boolean,
  draftCount: number,
  aggregateInMemoryDirty: boolean,
  revertedPersistedCount: number = 0,
): boolean {
  for (const nodeId of treeNodeIds) {
    if (configChangesStore.hasDraftsForNode(nodeId)) return true;
  }
  return (
    bowtieMetadataDirty ||
    draftCount > 0 ||
    aggregateInMemoryDirty ||
    revertedPersistedCount > 0
  );
}