import { configChangesStore } from '$lib/stores/configChanges.svelte';

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
  return bowtieMetadataDirty || draftCount > 0 || layoutDirty || revertedPersistedCount > 0;
}