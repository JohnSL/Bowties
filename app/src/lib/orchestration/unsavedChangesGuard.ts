import { hasModifiedLeaves, type NodeConfigTree } from '$lib/types/nodeTree';

export function hasUnsavedPromptChanges(
  trees: Iterable<NodeConfigTree>,
  bowtieMetadataDirty: boolean,
  draftCount: number,
): boolean {
  return [...trees].some((tree) => hasModifiedLeaves(tree)) || bowtieMetadataDirty || draftCount > 0;
}