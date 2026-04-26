import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { OfflineNodeSnapshot } from '$lib/api/layout';
import type { LayoutFile } from '$lib/types/bowtie';

const {
  layoutStoreRef,
  offlineChangesStoreRef,
  lifecycleRef,
} = vi.hoisted(() => ({
  layoutStoreRef: {
    hydrateOfflineLayout: vi.fn(),
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
  buildOfflineDiscoveryNodes,
  buildOfflineTreesFromSnapshots,
  clearActiveLayoutWithReset,
  openOfflineLayoutWithReplay,
  rehydrateOfflineStateFromSnapshots,
  resetFreshLiveSessionState,
  resetLayoutStateForNoLayout,
  restoreRecentOfflineLayout,
  treeFromSnapshot,
} from './offlineLayoutOrchestrator';

beforeEach(() => {
  vi.clearAllMocks();
});

function makeLayout(overrides: Partial<LayoutFile> = {}): LayoutFile {
  return {
    schemaVersion: '1.0',
    bowties: {
      '02.01.57.00.02.D9.00.06': {
        name: 'Offline Bowtie',
        tags: [],
      },
    },
    roleClassifications: {},
    ...overrides,
  };
}

describe('openOfflineLayoutWithReplay', () => {
  it('calls onOpened after a successful offline layout replay', async () => {
    const onOpened = vi.fn();
    const hydrateOfflineSnapshots = vi.fn(async () => {});
    const applyPersistedOfflinePendingToTrees = vi.fn();
    const result = {
      layoutId: 'yard-layout',
      capturedAt: '2026-04-25T00:00:00.000Z',
      layout: makeLayout(),
      offlineMode: true,
      nodeCount: 0,
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
    expect(layoutStoreRef.hydrateOfflineLayout).toHaveBeenCalledWith(makeLayout(), {
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

function makeSnapshot(overrides: Partial<OfflineNodeSnapshot> = {}): OfflineNodeSnapshot {
  return {
    nodeId: '050201020300',
    capturedAt: '2026-04-25T00:00:00.000Z',
    captureStatus: 'complete',
    missing: [],
    snip: {
      userName: 'East Panel',
      userDescription: '',
      manufacturerName: 'ACME',
      modelName: 'Node-8',
    },
    cdiRef: {
      cacheKey: 'cache-key',
      version: '1.0',
      fingerprint: 'fp',
    },
    config: {
      Main: {
        value: '10',
        space: 253,
        offset: '0x00000010',
      },
    },
    producerIdentifiedEvents: [],
    ...overrides,
  };
}

describe('offline snapshot helpers', () => {
  it('builds offline discovery skeleton nodes from snapshots', () => {
    const nodes = buildOfflineDiscoveryNodes(
      [makeSnapshot()],
      () => [0x05, 0x02, 0x01, 0x02, 0x03, 0x00],
    );

    expect(nodes).toEqual([
      expect.objectContaining({
        alias: 0x700,
        snip_status: 'Complete',
        last_seen: '2026-04-25T00:00:00.000Z',
        snip_data: expect.objectContaining({
          user_name: 'East Panel',
          manufacturer: 'ACME',
        }),
      }),
    ]);
  });

  it('falls back to raw snapshot trees when CDI tree build fails', async () => {
    const warning = vi.fn();
    const snapshot = makeSnapshot();
    const [tree] = await buildOfflineTreesFromSnapshots({
      snapshots: [snapshot],
      buildOfflineNodeTree: vi.fn(async () => {
        throw new Error('CDI not in cache');
      }),
      onTreeBuildWarning: warning,
    });

    expect(tree).toEqual(treeFromSnapshot(snapshot));
    expect(warning).toHaveBeenCalledWith(
      '[offline] CDI not cached for node 050201020300 — falling back to raw address tree',
    );
  });

  it('uses the CDI-backed tree when it is available', async () => {
    const builtTree = {
      nodeId: '05.02.01.02.03.00',
      identity: {
        manufacturer: 'ACME',
        model: 'Node-8',
        hardwareVersion: null,
        softwareVersion: null,
      },
      segments: [],
    };

    const [tree] = await buildOfflineTreesFromSnapshots({
      snapshots: [makeSnapshot()],
      buildOfflineNodeTree: vi.fn(async () => builtTree),
    });

    expect(tree).toEqual(builtTree);
  });

  it('rehydrates offline nodes and trees through the supplied state hooks', async () => {
    const publishNodes = vi.fn();
    const clearConfigReadStatus = vi.fn();
    const resetNodeTrees = vi.fn();
    const setTree = vi.fn();
    const markNodeConfigRead = vi.fn();

    await rehydrateOfflineStateFromSnapshots({
      snapshots: [makeSnapshot()],
      nodeIdStringToBytes: () => [0x05, 0x02, 0x01, 0x02, 0x03, 0x00],
      buildOfflineNodeTree: vi.fn(async () => ({
        nodeId: '05.02.01.02.03.00',
        identity: {
          manufacturer: 'ACME',
          model: 'Node-8',
          hardwareVersion: null,
          softwareVersion: null,
        },
        segments: [],
      })),
      publishNodes,
      clearConfigReadStatus,
      resetNodeTrees,
      setTree,
      markNodeConfigRead,
    });

    expect(publishNodes).toHaveBeenCalledTimes(1);
    expect(clearConfigReadStatus).toHaveBeenCalledTimes(1);
    expect(resetNodeTrees).toHaveBeenCalledTimes(1);
    expect(setTree).toHaveBeenCalledWith('05.02.01.02.03.00', {
      nodeId: '05.02.01.02.03.00',
      identity: {
        manufacturer: 'ACME',
        model: 'Node-8',
        hardwareVersion: null,
        softwareVersion: null,
      },
      segments: [],
    });
    expect(markNodeConfigRead).toHaveBeenCalledWith('05.02.01.02.03.00');
  });

  it('resets the no-layout state and reprobes only when connected', async () => {
    const probeForNodes = vi.fn(async () => {});

    await resetLayoutStateForNoLayout({
      connected: true,
      clearPartialCaptureNodes: vi.fn(),
      clearCurrentLayoutSnapshots: vi.fn(),
      clearNodes: vi.fn(),
      clearConfigReadStatus: vi.fn(),
      resetNodeTrees: vi.fn(),
      clearMetadata: vi.fn(),
      clearOfflineChanges: vi.fn(),
      resetLayoutStore: vi.fn(),
      resetSyncSessionAutoTrigger: vi.fn(),
      probeForNodes,
    });

    expect(probeForNodes).toHaveBeenCalledTimes(1);
  });

  it('skips the fresh live reset when a layout file is active', () => {
    const clearNodes = vi.fn();

    resetFreshLiveSessionState({
      hasLayoutFile: true,
      clearNodes,
      clearConfigReadStatus: vi.fn(),
      resetSidebar: vi.fn(),
      resetNodeTrees: vi.fn(),
      clearNodesWithCdi: vi.fn(),
    });

    expect(clearNodes).not.toHaveBeenCalled();
  });

  it('restores the recent offline layout and reports the loaded snapshots', async () => {
    const onRestored = vi.fn();

    const restored = await restoreRecentOfflineLayout({
      getRecentLayout: vi.fn(async () => ({ path: 'D:/Layouts/yard.layout.yaml' })),
      restoreLayout: vi.fn(async () => ({
        layoutId: 'yard-layout',
        capturedAt: '2026-04-25T00:00:00.000Z',
        offlineMode: true,
        nodeCount: 1,
        partialNodes: [],
        pendingOfflineChangeCount: 0,
        nodeSnapshots: [makeSnapshot()],
      })),
      clearRecentLayout: vi.fn(async () => {}),
      resetLayoutStateForNoLayout: vi.fn(async () => {}),
      resetLayoutOpenPhase: vi.fn(),
      onRestored,
      onWarning: vi.fn(),
    });

    expect(restored).toBe(true);
    expect(onRestored).toHaveBeenCalledTimes(1);
  });

  it('clears invalid startup layouts and resets the open phase after restore failure', async () => {
    const clearRecentLayout = vi.fn(async () => {});
    const resetLayoutState = vi.fn(async () => {});
    const resetLayoutOpenPhase = vi.fn();
    const onWarning = vi.fn();

    const restored = await restoreRecentOfflineLayout({
      getRecentLayout: vi.fn(async () => ({ path: 'D:/Layouts/yard.layout.yaml' })),
      restoreLayout: vi.fn(async () => {
        throw new Error('bad layout');
      }),
      clearRecentLayout,
      resetLayoutStateForNoLayout: resetLayoutState,
      resetLayoutOpenPhase,
      onRestored: vi.fn(),
      onWarning,
    });

    expect(restored).toBe(false);
    expect(clearRecentLayout).toHaveBeenCalledTimes(1);
    expect(resetLayoutState).toHaveBeenCalledWith(false);
    expect(resetLayoutOpenPhase).toHaveBeenCalledTimes(1);
    expect(onWarning).toHaveBeenCalled();
  });
});