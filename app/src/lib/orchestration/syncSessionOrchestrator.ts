import type { SyncSession, LayoutMatchStatus, SyncMode } from '$lib/api/sync';

interface SyncPanelLifecycleStore {
  isDismissed: boolean;
  matchStatus: LayoutMatchStatus | null;
  syncMode: SyncMode | null;
  session: SyncSession | null;
  reset(): void;
  computeMatch(discoveredNodeIds: string[]): Promise<void>;
  loadSession(): Promise<void>;
}

interface MaybeTriggerSyncArgs {
  hasLayoutFile: boolean;
  pendingCount: number;
  discoveredNodeIds: string[];
  syncPanelStore: SyncPanelLifecycleStore;
  showSyncPanel: () => void;
}

interface ScheduleAutoSyncArgs {
  hasLayoutFile: boolean;
  pendingCount: number;
  triggerSync: () => Promise<void> | void;
}

interface DisconnectWithOfflineFallbackArgs {
  disconnect: () => Promise<void>;
  afterDisconnect: () => void;
  hasLayoutFile: boolean;
  hasSnapshots: boolean;
  rehydrateOffline: () => Promise<void>;
  preserveLiveState?: () => void;
  clearLiveState: () => void;
  showConnectionDialog?: () => void;
  onError: (message: string) => void;
}

export type DisconnectTransition =
  | 'rehydrated_offline'
  | 'preserved_layout'
  | 'cleared_to_connection';

export function resolveDisconnectTransition(
  hasLayoutFile: boolean,
  hasSnapshots: boolean,
): DisconnectTransition {
  if (hasLayoutFile && hasSnapshots) {
    return 'rehydrated_offline';
  }

  if (hasLayoutFile) {
    return 'preserved_layout';
  }

  return 'cleared_to_connection';
}

export function hasSyncSessionContent(session: SyncSession | null): boolean {
  return !!session && (
    session.conflictRows.length > 0 ||
    session.cleanRows.length > 0 ||
    session.nodeMissingRows.length > 0
  );
}

export class SyncSessionOrchestrator {
  private discoverySettleTimer: ReturnType<typeof setTimeout> | null = null;
  private syncTriggered = false;

  constructor(private readonly settleDelayMs = 1000) {}

  resetAutoTrigger(): void {
    this.cancelPendingTrigger();
    this.syncTriggered = false;
  }

  cancelPendingTrigger(): void {
    if (this.discoverySettleTimer) {
      clearTimeout(this.discoverySettleTimer);
      this.discoverySettleTimer = null;
    }
  }

  scheduleAutoSync({ hasLayoutFile, pendingCount, triggerSync }: ScheduleAutoSyncArgs): void {
    if (this.syncTriggered || !hasLayoutFile || pendingCount === 0) return;

    this.cancelPendingTrigger();
    this.discoverySettleTimer = setTimeout(() => {
      this.discoverySettleTimer = null;
      if (this.syncTriggered) return;
      this.syncTriggered = true;
      void triggerSync();
    }, this.settleDelayMs);
  }

  async maybeTriggerSync({
    hasLayoutFile,
    pendingCount,
    discoveredNodeIds,
    syncPanelStore,
    showSyncPanel,
  }: MaybeTriggerSyncArgs): Promise<boolean> {
    if (!hasLayoutFile || pendingCount === 0) return false;
    if (syncPanelStore.isDismissed && this.syncTriggered) return false;

    await syncPanelStore.computeMatch(discoveredNodeIds);

    if (
      syncPanelStore.matchStatus &&
      syncPanelStore.matchStatus.classification !== 'likely_same' &&
      syncPanelStore.syncMode === null
    ) {
      showSyncPanel();
      return true;
    }

    await syncPanelStore.loadSession();
    if (hasSyncSessionContent(syncPanelStore.session)) {
      showSyncPanel();
      return true;
    }

    return false;
  }

  async forceSyncPanel(args: MaybeTriggerSyncArgs): Promise<boolean> {
    args.syncPanelStore.reset();
    this.resetAutoTrigger();
    return this.maybeTriggerSync(args);
  }
}

export async function disconnectWithOfflineFallback({
  disconnect,
  afterDisconnect,
  hasLayoutFile,
  hasSnapshots,
  rehydrateOffline,
  preserveLiveState,
  clearLiveState,
  showConnectionDialog,
  onError,
}: DisconnectWithOfflineFallbackArgs): Promise<DisconnectTransition | null> {
  try {
    await disconnect();
    afterDisconnect();

    const transition = resolveDisconnectTransition(hasLayoutFile, hasSnapshots);

    if (transition === 'rehydrated_offline') {
      await rehydrateOffline();
    } else if (transition === 'preserved_layout') {
      preserveLiveState?.();
    } else {
      clearLiveState();
      showConnectionDialog?.();
    }

    return transition;
  } catch (error) {
    onError(`Disconnect failed: ${error}`);
    return null;
  }
}