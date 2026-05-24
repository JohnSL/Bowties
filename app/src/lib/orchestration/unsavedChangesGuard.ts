import { configChangesStore } from '$lib/stores/configChanges.svelte';

/**
 * True when an exit action (close layout, switch layout, disconnect, app
 * window close) should prompt the user to confirm discarding unsaved work.
 *
 * S8 centralised the "unsaved in-memory" signal on `layoutStore.isDirty`
 * (which now includes fully-captured discovered nodes not yet in the saved
 * roster), so callers no longer pass a separate discovered-node count.
 */
export function hasUnsavedPromptChanges(
  treeNodeIds: Iterable<string>,
  bowtieMetadataDirty: boolean,
  draftCount: number,
  layoutDirty: boolean,
  revertedPersistedCount: number = 0,
): boolean {
  for (const nodeId of treeNodeIds) {
    if (configChangesStore.hasDraftsForNode(nodeId)) return true;
  }
  return (
    bowtieMetadataDirty ||
    draftCount > 0 ||
    layoutDirty ||
    revertedPersistedCount > 0
  );
}