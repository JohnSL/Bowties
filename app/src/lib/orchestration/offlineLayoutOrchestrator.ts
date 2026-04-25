import type { CloseLayoutResult, OpenLayoutResult, OfflineNodeSnapshot } from '$lib/api/layout';
import type { ActiveLayoutMode } from '$lib/stores/layout.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import {
  failLayoutOpen,
  finishLayoutHydration,
  finishOfflineReplay,
  startLayoutHydration,
  startLayoutOpen,
  startOfflineReplay,
} from '$lib/stores/layoutOpenLifecycle';

interface OpenOfflineLayoutWithReplayArgs {
  path: string;
  openLayout: (path: string) => Promise<OpenLayoutResult>;
  hydrateOfflineSnapshots: (snapshots: OfflineNodeSnapshot[]) => Promise<void>;
  applyPersistedOfflinePendingToTrees: () => void;
  onOpened?: () => void;
}

interface ClearActiveLayoutWithResetArgs {
  activeLayoutMode: ActiveLayoutMode | null | undefined;
  closeLayout: (decision: 'discard') => Promise<CloseLayoutResult>;
  clearRecentLayout: () => Promise<void>;
  resetLayoutState: () => Promise<void>;
  onRecentLayoutClearError?: (error: unknown) => void;
}

export async function clearActiveLayoutWithReset({
  activeLayoutMode,
  closeLayout,
  clearRecentLayout,
  resetLayoutState,
  onRecentLayoutClearError,
}: ClearActiveLayoutWithResetArgs): Promise<boolean> {
  if (activeLayoutMode === 'offline_file') {
    const result = await closeLayout('discard');
    if (!result.closed) return false;
  } else {
    try {
      await clearRecentLayout();
    } catch (error) {
      onRecentLayoutClearError?.(error);
    }
  }

  await resetLayoutState();
  return true;
}

export async function openOfflineLayoutWithReplay({
  path,
  openLayout,
  hydrateOfflineSnapshots,
  applyPersistedOfflinePendingToTrees,
  onOpened,
}: OpenOfflineLayoutWithReplayArgs): Promise<OpenLayoutResult> {
  startLayoutOpen();

  try {
    const result = await openLayout(path);

    startLayoutHydration();
    await hydrateOfflineSnapshots(result.nodeSnapshots);
    finishLayoutHydration();

    layoutStore.setActiveContext({
      layoutId: result.layoutId,
      rootPath: path,
      mode: 'offline_file',
      capturedAt: result.capturedAt,
      pendingOfflineChangeCount: result.pendingOfflineChangeCount,
    });

    startOfflineReplay();
    await offlineChangesStore.reloadFromBackend();
    applyPersistedOfflinePendingToTrees();
    onOpened?.();
    finishOfflineReplay();

    return result;
  } catch (error) {
    failLayoutOpen();
    throw error;
  }
}