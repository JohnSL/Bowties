import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  disconnectWithOfflineFallback,
  hasSyncSessionContent,
  resolveDisconnectTransition,
  SyncSessionOrchestrator,
} from './syncSessionOrchestrator';

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

describe('hasSyncSessionContent', () => {
  it('returns true when any sync bucket has rows', () => {
    expect(hasSyncSessionContent({ conflictRows: [{} as any], cleanRows: [], alreadyAppliedCount: 0, nodeMissingRows: [] })).toBe(true);
    expect(hasSyncSessionContent({ conflictRows: [], cleanRows: [{} as any], alreadyAppliedCount: 0, nodeMissingRows: [] })).toBe(true);
    expect(hasSyncSessionContent({ conflictRows: [], cleanRows: [], alreadyAppliedCount: 0, nodeMissingRows: [{} as any] })).toBe(true);
    expect(hasSyncSessionContent({ conflictRows: [], cleanRows: [], alreadyAppliedCount: 0, nodeMissingRows: [] })).toBe(false);
  });
});

describe('SyncSessionOrchestrator', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('shows the sync panel for an uncertain match without loading a session', async () => {
    const orchestrator = new SyncSessionOrchestrator(50);
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
    const orchestrator = new SyncSessionOrchestrator(50);
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
    const orchestrator = new SyncSessionOrchestrator(50);
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
    const orchestrator = new SyncSessionOrchestrator(50);
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
    const orchestrator = new SyncSessionOrchestrator(50);
    const triggerSync = vi.fn();

    orchestrator.scheduleAutoSync({ hasLayoutFile: false, pendingCount: 1, triggerSync });
    orchestrator.scheduleAutoSync({ hasLayoutFile: true, pendingCount: 0, triggerSync });

    await vi.advanceTimersByTimeAsync(100);
    expect(triggerSync).not.toHaveBeenCalled();
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