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

import { openOfflineLayoutWithReplay } from './offlineLayoutOrchestrator';

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