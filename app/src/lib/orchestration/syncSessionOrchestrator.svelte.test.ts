import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  bootstrapStartupLifecycle,
  connectLiveSession,
  disconnectWithOfflineFallback,
  hasSyncSessionContent,
  resolveConnectionLabel,
  resolveDisconnectTransition,
  SyncSessionOrchestrator,
  type SyncSessionConnectionDeps,
} from './syncSessionOrchestrator.svelte';

function createSyncPanelStore() {
  return {
    isDismissed: false,
    matchStatus: null as any,
    syncMode: null as any,
    session: null as any,
    reset: vi.fn(),
    computeMatch: vi.fn(async (_ids: string[]) => {}),
    loadSession: vi.fn(async () => {}),
  };
}

function createConnectionDeps(
  overrides: Partial<SyncSessionConnectionDeps> = {},
): SyncSessionConnectionDeps {
  return {
    disconnectLcc: vi.fn(async () => {}),
    probeForNodes: vi.fn(async () => {}),
    hasLayoutFile: vi.fn(() => false),
    hasSnapshots: vi.fn(() => false),
    setLayoutConnected: vi.fn(),
    resetFreshLiveSessionState: vi.fn(),
    rehydrateOffline: vi.fn(async () => {}),
    clearLiveState: vi.fn(),
    resetSyncPanel: vi.fn(),
    setShowConnectionDialog: vi.fn(),
    setErrorMessage: vi.fn(),
    warn: vi.fn(),
    ...overrides,
  };
}

describe('hasSyncSessionContent', () => {
  it('returns true when any sync bucket has rows', () => {
    expect(hasSyncSessionContent({ conflictRows: [{} as any], cleanRows: [], alreadyAppliedCount: 0, nodeMissingRows: [] })).toBe(true);
    expect(hasSyncSessionContent({ conflictRows: [], cleanRows: [{} as any], alreadyAppliedCount: 0, nodeMissingRows: [] })).toBe(true);
    expect(hasSyncSessionContent({ conflictRows: [], cleanRows: [], alreadyAppliedCount: 0, nodeMissingRows: [{} as any] })).toBe(true);
    expect(hasSyncSessionContent({ conflictRows: [], cleanRows: [], alreadyAppliedCount: 0, nodeMissingRows: [] })).toBe(false);
  });
});

describe('connectLiveSession', () => {
  it('formats the visible label from name, host, serial port, or the default fallback', () => {
    expect(resolveConnectionLabel({ name: 'Bench Bus', host: 'ignored', port: 12021 })).toBe('Bench Bus');
    expect(resolveConnectionLabel({ host: '127.0.0.1', port: 12021 })).toBe('127.0.0.1:12021');
    expect(resolveConnectionLabel({ serialPort: 'COM4' })).toBe('COM4');
    expect(resolveConnectionLabel({})).toBe('LCC');
  });

  it('resets the fresh live session before probing when no layout file is active', () => {
    const setConnectionLabel = vi.fn();
    const setLayoutConnected = vi.fn();
    const hideConnectionDialog = vi.fn();
    const resetSyncSessionAutoTrigger = vi.fn();
    const resetFreshLiveSessionState = vi.fn();
    const probeForNodes = vi.fn(async () => {});

    connectLiveSession({
      config: { name: 'Bench Bus' },
      hasLayoutFile: false,
      setConnectionLabel,
      setLayoutConnected,
      hideConnectionDialog,
      resetSyncSessionAutoTrigger,
      resetFreshLiveSessionState,
      probeForNodes,
    });

    expect(setConnectionLabel).toHaveBeenCalledWith('Bench Bus');
    expect(setLayoutConnected).toHaveBeenCalledWith(true);
    expect(hideConnectionDialog).toHaveBeenCalledTimes(1);
    expect(resetSyncSessionAutoTrigger).toHaveBeenCalledTimes(1);
    expect(resetFreshLiveSessionState).toHaveBeenCalledTimes(1);
    expect(probeForNodes).toHaveBeenCalledTimes(1);
    expect(resetFreshLiveSessionState.mock.invocationCallOrder[0]).toBeLessThan(
      probeForNodes.mock.invocationCallOrder[0],
    );
  });

  it('preserves existing layout state on connect when a layout file is already active', () => {
    const resetFreshLiveSessionState = vi.fn();
    const probeForNodes = vi.fn(async () => {});

    connectLiveSession({
      config: { host: 'localhost', port: 12021 },
      hasLayoutFile: true,
      setConnectionLabel: vi.fn(),
      setLayoutConnected: vi.fn(),
      hideConnectionDialog: vi.fn(),
      resetSyncSessionAutoTrigger: vi.fn(),
      resetFreshLiveSessionState,
      probeForNodes,
    });

    expect(resetFreshLiveSessionState).not.toHaveBeenCalled();
    expect(probeForNodes).toHaveBeenCalledTimes(1);
  });
});

describe('bootstrapStartupLifecycle', () => {
  it('boots a connected fresh live session by resetting transient state and probing after listeners are ready', async () => {
    const calls: string[] = [];
    const resetFreshLiveSessionState = vi.fn(() => {
      calls.push('resetFresh');
    });
    const probeForNodes = vi.fn(async () => {
      calls.push('probe');
    });

    await bootstrapStartupLifecycle({
      getConnectionStatus: vi.fn(async () => ({ connected: true, config: { name: 'Bench Bus' } })),
      setLayoutConnected: vi.fn((value: boolean) => {
        calls.push(`layoutConnected:${value}`);
      }),
      setConnectionLabel: vi.fn((label: string) => {
        calls.push(`label:${label}`);
      }),
      startBowtieListening: vi.fn(async () => {
        calls.push('bowties');
      }),
      restoreRecentOfflineLayout: vi.fn(async () => {
        calls.push('restore');
        return false;
      }),
      startNodeTreeListening: vi.fn(() => {
        calls.push('trees');
      }),
      hasLayoutFile: vi.fn(() => false),
      resetFreshLiveSessionState,
      probeForNodes,
    });

    expect(calls).toEqual([
      'layoutConnected:true',
      'label:Bench Bus',
      'bowties',
      'restore',
      'trees',
      'resetFresh',
      'probe',
    ]);
  });

  it('preserves startup layout state when the restore activates a layout file', async () => {
    const resetFreshLiveSessionState = vi.fn();
    const probeForNodes = vi.fn(async () => {});

    await bootstrapStartupLifecycle({
      getConnectionStatus: vi.fn(async () => ({ connected: true, config: { host: '127.0.0.1', port: 12021 } })),
      setLayoutConnected: vi.fn(),
      setConnectionLabel: vi.fn(),
      startBowtieListening: vi.fn(async () => {}),
      restoreRecentOfflineLayout: vi.fn(async () => true),
      startNodeTreeListening: vi.fn(),
      hasLayoutFile: vi.fn(() => true),
      resetFreshLiveSessionState,
      probeForNodes,
    });

    expect(resetFreshLiveSessionState).not.toHaveBeenCalled();
    expect(probeForNodes).toHaveBeenCalledTimes(1);
  });

  it('continues startup when connection status lookup fails', async () => {
    const onConnectionStatusError = vi.fn();
    const startBowtieListening = vi.fn(async () => {});
    const restoreRecentOfflineLayout = vi.fn(async () => false);
    const startNodeTreeListening = vi.fn();
    const probeForNodes = vi.fn(async () => {});

    await bootstrapStartupLifecycle({
      getConnectionStatus: vi.fn(async () => {
        throw new Error('status unavailable');
      }),
      setLayoutConnected: vi.fn(),
      setConnectionLabel: vi.fn(),
      onConnectionStatusError,
      startBowtieListening,
      restoreRecentOfflineLayout,
      startNodeTreeListening,
      hasLayoutFile: vi.fn(() => false),
      resetFreshLiveSessionState: vi.fn(),
      probeForNodes,
    });

    expect(onConnectionStatusError).toHaveBeenCalledTimes(1);
    expect(startBowtieListening).toHaveBeenCalledTimes(1);
    expect(restoreRecentOfflineLayout).toHaveBeenCalledTimes(1);
    expect(startNodeTreeListening).toHaveBeenCalledTimes(1);
    expect(probeForNodes).not.toHaveBeenCalled();
  });
});

describe('SyncSessionOrchestrator', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('shows the sync panel for an uncertain match without loading a session', async () => {
    const orchestrator = new SyncSessionOrchestrator(createConnectionDeps(), 50);
    const syncPanelStore = createSyncPanelStore();
    const showSyncPanel = vi.fn();

    syncPanelStore.computeMatch.mockImplementation(async () => {
      syncPanelStore.matchStatus = { classification: 'uncertain' };
      syncPanelStore.syncMode = null;
    });

    const shown = await orchestrator.maybeTriggerSync({
      hasLayoutFile: true,
      pendingCount: 2,
      discoveredNodeIds: ['05.02.01.02.03.00'],
      syncPanelStore,
      showSyncPanel,
    });

    expect(shown).toBe(true);
    expect(syncPanelStore.computeMatch).toHaveBeenCalledWith(['05.02.01.02.03.00']);
    expect(syncPanelStore.loadSession).not.toHaveBeenCalled();
    expect(showSyncPanel).toHaveBeenCalledTimes(1);
  });

  it('loads a sync session and shows the panel when rows exist', async () => {
    const orchestrator = new SyncSessionOrchestrator(createConnectionDeps(), 50);
    const syncPanelStore = createSyncPanelStore();
    const showSyncPanel = vi.fn();

    syncPanelStore.computeMatch.mockImplementation(async () => {
      syncPanelStore.matchStatus = { classification: 'likely_same' };
    });
    syncPanelStore.loadSession.mockImplementation(async () => {
      syncPanelStore.session = {
        conflictRows: [],
        cleanRows: [{ changeId: 'row-1' }],
        alreadyAppliedCount: 0,
        nodeMissingRows: [],
      } as any;
    });

    const shown = await orchestrator.maybeTriggerSync({
      hasLayoutFile: true,
      pendingCount: 1,
      discoveredNodeIds: ['05.02.01.02.03.00'],
      syncPanelStore,
      showSyncPanel,
    });

    expect(shown).toBe(true);
    expect(syncPanelStore.loadSession).toHaveBeenCalledTimes(1);
    expect(showSyncPanel).toHaveBeenCalledTimes(1);
  });

  it('does not show the sync panel when the loaded session has no rows', async () => {
    const orchestrator = new SyncSessionOrchestrator(createConnectionDeps(), 50);
    const syncPanelStore = createSyncPanelStore();
    const showSyncPanel = vi.fn();

    syncPanelStore.computeMatch.mockImplementation(async () => {
      syncPanelStore.matchStatus = { classification: 'likely_same' };
    });
    syncPanelStore.loadSession.mockImplementation(async () => {
      syncPanelStore.session = {
        conflictRows: [],
        cleanRows: [],
        alreadyAppliedCount: 0,
        nodeMissingRows: [],
      } as any;
    });

    const shown = await orchestrator.maybeTriggerSync({
      hasLayoutFile: true,
      pendingCount: 1,
      discoveredNodeIds: ['05.02.01.02.03.00'],
      syncPanelStore,
      showSyncPanel,
    });

    expect(shown).toBe(false);
    expect(syncPanelStore.loadSession).toHaveBeenCalledTimes(1);
    expect(showSyncPanel).not.toHaveBeenCalled();
  });

  it('debounces discovery-settle auto sync and can be reset for manual reopen', async () => {
    const orchestrator = new SyncSessionOrchestrator(createConnectionDeps(), 50);
    const triggerSync = vi.fn();
    const syncPanelStore = createSyncPanelStore();
    const showSyncPanel = vi.fn();

    orchestrator.scheduleAutoSync({ hasLayoutFile: true, pendingCount: 1, triggerSync });
    orchestrator.scheduleAutoSync({ hasLayoutFile: true, pendingCount: 1, triggerSync });

    await vi.advanceTimersByTimeAsync(49);
    expect(triggerSync).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(triggerSync).toHaveBeenCalledTimes(1);

    syncPanelStore.isDismissed = true;
    const shown = await orchestrator.maybeTriggerSync({
      hasLayoutFile: true,
      pendingCount: 1,
      discoveredNodeIds: ['05.02.01.02.03.00'],
      syncPanelStore,
      showSyncPanel,
    });
    expect(shown).toBe(false);

    syncPanelStore.isDismissed = false;
    await orchestrator.forceSyncPanel({
      hasLayoutFile: true,
      pendingCount: 1,
      discoveredNodeIds: ['05.02.01.02.03.00'],
      syncPanelStore,
      showSyncPanel,
    });
    expect(syncPanelStore.reset).toHaveBeenCalledTimes(1);
  });

  it('does not schedule auto sync when no layout is open or no pending rows exist', async () => {
    const orchestrator = new SyncSessionOrchestrator(createConnectionDeps(), 50);
    const triggerSync = vi.fn();

    orchestrator.scheduleAutoSync({ hasLayoutFile: false, pendingCount: 1, triggerSync });
    orchestrator.scheduleAutoSync({ hasLayoutFile: true, pendingCount: 0, triggerSync });

    await vi.advanceTimersByTimeAsync(100);
    expect(triggerSync).not.toHaveBeenCalled();
  });
});

describe('SyncSessionOrchestrator connection workflow', () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it('connect() sets the connection label and mirrors connected into the layout store', () => {
    const deps = createConnectionDeps({ hasLayoutFile: vi.fn(() => false) });
    const orchestrator = new SyncSessionOrchestrator(deps);

    orchestrator.connect({ name: 'Bench Bus' });

    expect(orchestrator.connectionLabel).toBe('Bench Bus');
    expect(deps.setLayoutConnected).toHaveBeenCalledWith(true);
    expect(deps.setShowConnectionDialog).toHaveBeenCalledWith(false);
    expect(deps.resetFreshLiveSessionState).toHaveBeenCalledTimes(1);
    expect(deps.probeForNodes).toHaveBeenCalledTimes(1);
  });

  it('connect() preserves layout state but still probes when a layout file is open', () => {
    const deps = createConnectionDeps({ hasLayoutFile: vi.fn(() => true) });
    const orchestrator = new SyncSessionOrchestrator(deps);

    orchestrator.connect({ host: '127.0.0.1', port: 12021 });

    expect(orchestrator.connectionLabel).toBe('127.0.0.1:12021');
    expect(deps.setLayoutConnected).toHaveBeenCalledWith(true);
    expect(deps.resetFreshLiveSessionState).not.toHaveBeenCalled();
    expect(deps.probeForNodes).toHaveBeenCalledTimes(1);
  });

  it('disconnect() clears the label, disconnects the bus, and rehydrates offline when snapshots exist', async () => {
    const deps = createConnectionDeps({
      hasLayoutFile: vi.fn(() => true),
      hasSnapshots: vi.fn(() => true),
    });
    const orchestrator = new SyncSessionOrchestrator(deps);
    orchestrator.connect({ name: 'Bench Bus' });

    await orchestrator.disconnect();

    expect(orchestrator.connectionLabel).toBe('');
    expect(deps.setErrorMessage).toHaveBeenCalledWith('');
    expect(deps.disconnectLcc).toHaveBeenCalledTimes(1);
    expect(deps.setLayoutConnected).toHaveBeenLastCalledWith(false);
    expect(deps.resetSyncPanel).toHaveBeenCalledTimes(1);
    expect(deps.rehydrateOffline).toHaveBeenCalledTimes(1);
    expect(deps.clearLiveState).not.toHaveBeenCalled();
  });

  it('disconnect() clears live state and reopens the connection dialog when no layout is open', async () => {
    const deps = createConnectionDeps({
      hasLayoutFile: vi.fn(() => false),
      hasSnapshots: vi.fn(() => false),
    });
    const orchestrator = new SyncSessionOrchestrator(deps);

    await orchestrator.disconnect();

    expect(deps.clearLiveState).toHaveBeenCalledTimes(1);
    expect(deps.setShowConnectionDialog).toHaveBeenLastCalledWith(true);
    expect(deps.rehydrateOffline).not.toHaveBeenCalled();
  });

  it('disconnect() reports a failure through setErrorMessage', async () => {
    const deps = createConnectionDeps({
      disconnectLcc: vi.fn(async () => { throw new Error('bus offline'); }),
    });
    const orchestrator = new SyncSessionOrchestrator(deps);

    await orchestrator.disconnect();

    expect(deps.setErrorMessage).toHaveBeenLastCalledWith('Disconnect failed: Error: bus offline');
  });

  it('disconnectBeforeLayoutSwitch() tears down live state and hides the connection dialog', async () => {
    const deps = createConnectionDeps({ hasLayoutFile: vi.fn(() => true) });
    const orchestrator = new SyncSessionOrchestrator(deps);
    orchestrator.connect({ name: 'Bench Bus' });

    await orchestrator.disconnectBeforeLayoutSwitch();

    expect(orchestrator.connectionLabel).toBe('');
    expect(deps.disconnectLcc).toHaveBeenCalledTimes(1);
    expect(deps.setLayoutConnected).toHaveBeenLastCalledWith(false);
    expect(deps.resetSyncPanel).toHaveBeenCalledTimes(1);
    expect(deps.clearLiveState).toHaveBeenCalledTimes(1);
    expect(deps.setShowConnectionDialog).toHaveBeenLastCalledWith(false);
    expect(deps.rehydrateOffline).not.toHaveBeenCalled();
  });

  it('disconnectBeforeLayoutSwitch() swallows a disconnect error via warn and still tears down', async () => {
    const deps = createConnectionDeps({
      disconnectLcc: vi.fn(async () => { throw new Error('bus offline'); }),
    });
    const orchestrator = new SyncSessionOrchestrator(deps);

    await orchestrator.disconnectBeforeLayoutSwitch();

    expect(deps.warn).toHaveBeenCalledTimes(1);
    expect(deps.setLayoutConnected).toHaveBeenLastCalledWith(false);
    expect(deps.clearLiveState).toHaveBeenCalledTimes(1);
    expect(deps.setShowConnectionDialog).toHaveBeenLastCalledWith(false);
  });
});

describe('disconnectWithOfflineFallback', () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it('classifies disconnect transitions for rehydrate, preserve, and clear paths', () => {
    expect(resolveDisconnectTransition(true, true)).toBe('rehydrated_offline');
    expect(resolveDisconnectTransition(true, false)).toBe('preserved_layout');
    expect(resolveDisconnectTransition(false, false)).toBe('cleared_to_connection');
  });

  it('rehydrates offline state when a layout with snapshots is active', async () => {
    const disconnect = vi.fn(async () => {});
    const afterDisconnect = vi.fn();
    const rehydrateOffline = vi.fn(async () => {});
    const preserveLiveState = vi.fn();
    const clearLiveState = vi.fn();
    const showConnectionDialog = vi.fn();
    const onError = vi.fn();

    const transition = await disconnectWithOfflineFallback({
      disconnect,
      afterDisconnect,
      hasLayoutFile: true,
      hasSnapshots: true,
      rehydrateOffline,
      preserveLiveState,
      clearLiveState,
      showConnectionDialog,
      onError,
    });

    expect(transition).toBe('rehydrated_offline');
    expect(disconnect).toHaveBeenCalledTimes(1);
    expect(afterDisconnect).toHaveBeenCalledTimes(1);
    expect(rehydrateOffline).toHaveBeenCalledTimes(1);
    expect(preserveLiveState).not.toHaveBeenCalled();
    expect(clearLiveState).not.toHaveBeenCalled();
    expect(showConnectionDialog).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });

  it('preserves the live layout tree when a layout is open without snapshots', async () => {
    const disconnect = vi.fn(async () => {});
    const afterDisconnect = vi.fn();
    const rehydrateOffline = vi.fn(async () => {});
    const preserveLiveState = vi.fn();
    const clearLiveState = vi.fn();
    const showConnectionDialog = vi.fn();
    const onError = vi.fn();

    const transition = await disconnectWithOfflineFallback({
      disconnect,
      afterDisconnect,
      hasLayoutFile: true,
      hasSnapshots: false,
      rehydrateOffline,
      preserveLiveState,
      clearLiveState,
      showConnectionDialog,
      onError,
    });

    expect(transition).toBe('preserved_layout');
    expect(preserveLiveState).toHaveBeenCalledTimes(1);
    expect(rehydrateOffline).not.toHaveBeenCalled();
    expect(clearLiveState).not.toHaveBeenCalled();
    expect(showConnectionDialog).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });

  it('clears live state and transitions to the connection dialog when no layout is open', async () => {
    const disconnect = vi.fn(async () => {});
    const afterDisconnect = vi.fn();
    const rehydrateOffline = vi.fn(async () => {});
    const preserveLiveState = vi.fn();
    const clearLiveState = vi.fn();
    const showConnectionDialog = vi.fn();
    const onError = vi.fn();

    const transition = await disconnectWithOfflineFallback({
      disconnect,
      afterDisconnect,
      hasLayoutFile: false,
      hasSnapshots: false,
      rehydrateOffline,
      preserveLiveState,
      clearLiveState,
      showConnectionDialog,
      onError,
    });

    expect(transition).toBe('cleared_to_connection');
    expect(clearLiveState).toHaveBeenCalledTimes(1);
    expect(showConnectionDialog).toHaveBeenCalledTimes(1);
    expect(preserveLiveState).not.toHaveBeenCalled();
    expect(rehydrateOffline).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });

  it('reports disconnect errors without invoking any fallback transition handlers', async () => {
    const disconnect = vi.fn(async () => {
      throw new Error('bus offline');
    });
    const afterDisconnect = vi.fn();
    const rehydrateOffline = vi.fn(async () => {});
    const preserveLiveState = vi.fn();
    const clearLiveState = vi.fn();
    const showConnectionDialog = vi.fn();
    const onError = vi.fn();

    const transition = await disconnectWithOfflineFallback({
      disconnect,
      afterDisconnect,
      hasLayoutFile: false,
      hasSnapshots: false,
      rehydrateOffline,
      preserveLiveState,
      clearLiveState,
      showConnectionDialog,
      onError,
    });

    expect(transition).toBeNull();
    expect(afterDisconnect).not.toHaveBeenCalled();
    expect(rehydrateOffline).not.toHaveBeenCalled();
    expect(preserveLiveState).not.toHaveBeenCalled();
    expect(clearLiveState).not.toHaveBeenCalled();
    expect(showConnectionDialog).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith('Disconnect failed: Error: bus offline');
  });
});