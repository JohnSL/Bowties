import { beforeEach, describe, expect, it, vi } from 'vitest';

const {
  layoutStoreRef,
  offlineChangesStoreRef,
  lifecycleRef,
} = vi.hoisted(() => ({
  layoutStoreRef: {
    setActiveContext: vi.fn(),
  },
  offlineChangesStoreRef: {
    reloadFromBackend: vi.fn(async () => {}),
  },
  lifecycleRef: {
    startLayoutOpen: vi.fn(),
    startLayoutHydration: vi.fn(),
    finishLayoutHydration: vi.fn(),
    startOfflineReplay: vi.fn(),
    finishOfflineReplay: vi.fn(),
    failLayoutOpen: vi.fn(),
  },
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: layoutStoreRef,
}));

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: offlineChangesStoreRef,
}));

vi.mock('$lib/stores/layoutOpenLifecycle', () => lifecycleRef);

import {
  clearActiveLayoutWithReset,
  openOfflineLayoutWithReplay,
} from './offlineLayoutOrchestrator';

beforeEach(() => {
  vi.clearAllMocks();
});

describe('openOfflineLayoutWithReplay', () => {
  it('calls onOpened after a successful offline layout replay', async () => {
    const onOpened = vi.fn();
    const hydrateOfflineSnapshots = vi.fn(async () => {});
    const applyPersistedOfflinePendingToTrees = vi.fn();
    const result = {
      layoutId: 'yard-layout',
      capturedAt: '2026-04-25T00:00:00.000Z',
      pendingOfflineChangeCount: 2,
      partialNodes: [],
      nodeSnapshots: [],
    };

    const opened = await openOfflineLayoutWithReplay({
      path: 'D:/Layouts/yard.layout.yaml',
      openLayout: vi.fn(async () => result),
      hydrateOfflineSnapshots,
      applyPersistedOfflinePendingToTrees,
      onOpened,
    });

    expect(opened).toEqual(result);
    expect(hydrateOfflineSnapshots).toHaveBeenCalledWith([]);
    expect(offlineChangesStoreRef.reloadFromBackend).toHaveBeenCalledTimes(1);
    expect(applyPersistedOfflinePendingToTrees).toHaveBeenCalledTimes(1);
    expect(onOpened).toHaveBeenCalledTimes(1);
    expect(layoutStoreRef.setActiveContext).toHaveBeenCalledWith({
      layoutId: 'yard-layout',
      rootPath: 'D:/Layouts/yard.layout.yaml',
      mode: 'offline_file',
      capturedAt: '2026-04-25T00:00:00.000Z',
      pendingOfflineChangeCount: 2,
    });
  });

  it('does not call onOpened when the layout open fails', async () => {
    const onOpened = vi.fn();
    const error = new Error('open failed');

    await expect(openOfflineLayoutWithReplay({
      path: 'D:/Layouts/yard.layout.yaml',
      openLayout: vi.fn(async () => {
        throw error;
      }),
      hydrateOfflineSnapshots: vi.fn(async () => {}),
      applyPersistedOfflinePendingToTrees: vi.fn(),
      onOpened,
    })).rejects.toThrow('open failed');

    expect(onOpened).not.toHaveBeenCalled();
    expect(lifecycleRef.failLayoutOpen).toHaveBeenCalledTimes(1);
  });
});

describe('clearActiveLayoutWithReset', () => {
  it('closes offline layouts before resetting local state', async () => {
    const closeLayout = vi.fn(async () => ({ closed: true }));
    const clearRecentLayout = vi.fn(async () => {});
    const resetLayoutState = vi.fn(async () => {});

    const cleared = await clearActiveLayoutWithReset({
      activeLayoutMode: 'offline_file',
      closeLayout,
      clearRecentLayout,
      resetLayoutState,
    });

    expect(cleared).toBe(true);
    expect(closeLayout).toHaveBeenCalledWith('discard');
    expect(clearRecentLayout).not.toHaveBeenCalled();
    expect(resetLayoutState).toHaveBeenCalledTimes(1);
  });

  it('does not reset local state when the backend refuses to close an offline layout', async () => {
    const closeLayout = vi.fn(async () => ({ closed: false, reason: 'cancelled' }));
    const resetLayoutState = vi.fn(async () => {});

    const cleared = await clearActiveLayoutWithReset({
      activeLayoutMode: 'offline_file',
      closeLayout,
      clearRecentLayout: vi.fn(async () => {}),
      resetLayoutState,
    });

    expect(cleared).toBe(false);
    expect(resetLayoutState).not.toHaveBeenCalled();
  });

  it('warns but still resets when clearing the recent legacy layout fails', async () => {
    const warning = vi.fn();
    const resetLayoutState = vi.fn(async () => {});

    const cleared = await clearActiveLayoutWithReset({
      activeLayoutMode: 'legacy_file',
      closeLayout: vi.fn(async () => ({ closed: true })),
      clearRecentLayout: vi.fn(async () => {
        throw new Error('disk busy');
      }),
      resetLayoutState,
      onRecentLayoutClearError: warning,
    });

    expect(cleared).toBe(true);
    expect(warning).toHaveBeenCalledTimes(1);
    expect(resetLayoutState).toHaveBeenCalledTimes(1);
  });
});