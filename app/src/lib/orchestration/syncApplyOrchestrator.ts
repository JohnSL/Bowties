import { buildOfflineNodeTree } from '$lib/api/layout';
import type { ApplySyncResult, SyncSession } from '$lib/api/sync';
import { markNodeConfigRead } from '$lib/stores/configReadStatus';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';

export async function reconcileOfflineTreesAfterSyncApply(
  result: ApplySyncResult,
  session: SyncSession | null,
): Promise<void> {
  await offlineChangesStore.reloadFromBackend();

  const clearedIds = new Set([...result.applied, ...result.readOnlyCleared]);
  const allRows = [
    ...(session?.conflictRows ?? []),
    ...(session?.cleanRows ?? []),
    ...(session?.nodeMissingRows ?? []),
  ];
  const affectedNodeIds = new Set(
    allRows
      .filter((row) => row.nodeId && clearedIds.has(row.changeId))
      .map((row) => row.nodeId!.replace(/\./g, '').toUpperCase())
  );

  for (const nodeId of affectedNodeIds) {
    try {
      const tree = await buildOfflineNodeTree(nodeId);
      nodeTreeStore.setTree(tree.nodeId, tree);
      markNodeConfigRead(tree.nodeId);
    } catch (error) {
      console.warn(`[sync] Failed to rebuild tree for ${nodeId}:`, error);
    }
  }

  nodeTreeStore.applyOfflinePendingValues(offlineChangesStore.persistedRows);
}