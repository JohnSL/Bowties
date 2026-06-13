import type { SyncSession, LayoutMatchStatus, SyncMode } from '$lib/api/sync';
import {
  resolveConnectTransition,
  resolveDisconnectTransition,
  resolveStartupTransition,
  shouldProbeAfterTransition,
  shouldResetFreshLiveSession,
  type DisconnectTransition,
} from './lifecycleTransitionMatrix';

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

interface ConnectionConfig {
  name?: string | null;
  host?: string | null;
  port?: string | number | null;
  serialPort?: string | null;
}

interface ConnectLiveSessionArgs {
  config: ConnectionConfig;
  hasLayoutFile: boolean;
  setConnectionLabel: (label: string) => void;
  setLayoutConnected: (connected: boolean) => void;
  hideConnectionDialog: () => void;
  resetSyncSessionAutoTrigger: () => void;
  resetFreshLiveSessionState: () => void;
  probeForNodes: () => Promise<void> | void;
}

interface StartupConnectionStatus {
  connected: boolean;
  config?: ConnectionConfig | null;
}

interface BootstrapStartupLifecycleArgs {
  getConnectionStatus: () => Promise<StartupConnectionStatus>;
  setLayoutConnected: (connected: boolean) => void;
  setConnectionLabel: (label: string) => void;
  onConnectionStatusError?: (error: unknown) => void;
  startBowtieListening: () => Promise<void>;
  restoreRecentOfflineLayout: () => Promise<boolean>;
  startNodeTreeListening: () => void;
  hasLayoutFile: () => boolean;
  resetFreshLiveSessionState: () => void;
  probeForNodes: () => Promise<void> | void;
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

export { resolveDisconnectTransition } from './lifecycleTransitionMatrix';

export function hasSyncSessionContent(session: SyncSession | null): boolean {
  return !!session && (
    session.conflictRows.length > 0 ||
    session.cleanRows.length > 0 ||
    session.nodeMissingRows.length > 0
  );
}

export function resolveConnectionLabel(config: ConnectionConfig): string {
  return config.name ?? (config.host ? `${config.host}:${config.port}` : config.serialPort ?? 'LCC');
}

export function connectLiveSession({
  config,
  hasLayoutFile,
  setConnectionLabel,
  setLayoutConnected,
  hideConnectionDialog,
  resetSyncSessionAutoTrigger,
  resetFreshLiveSessionState,
  probeForNodes,
}: ConnectLiveSessionArgs): void {
  const transition = resolveConnectTransition(hasLayoutFile);

  setConnectionLabel(resolveConnectionLabel(config));
  setLayoutConnected(true);
  hideConnectionDialog();
  resetSyncSessionAutoTrigger();

  if (shouldResetFreshLiveSession(transition)) {
    resetFreshLiveSessionState();
  }

  if (shouldProbeAfterTransition(transition)) {
    void probeForNodes();
  }
}

export async function bootstrapStartupLifecycle({
  getConnectionStatus,
  setLayoutConnected,
  setConnectionLabel,
  onConnectionStatusError,
  startBowtieListening,
  restoreRecentOfflineLayout,
  startNodeTreeListening,
  hasLayoutFile,
  resetFreshLiveSessionState,
  probeForNodes,
}: BootstrapStartupLifecycleArgs): Promise<void> {
  let connected = false;

  try {
    const status = await getConnectionStatus();
    connected = status.connected;
    setLayoutConnected(connected);
    if (connected && status.config) {
      setConnectionLabel(resolveConnectionLabel(status.config));
    }
  } catch (error) {
    onConnectionStatusError?.(error);
  }

  await startBowtieListening();
  await restoreRecentOfflineLayout();
  startNodeTreeListening();

  const transition = resolveStartupTransition(connected, hasLayoutFile());

  if (shouldResetFreshLiveSession(transition)) {
    resetFreshLiveSessionState();
  }

  if (shouldProbeAfterTransition(transition)) {
    await probeForNodes();
  }
}

/**
 * Dependencies the orchestrator needs to drive the connect/disconnect
 * workflow. Injected via the constructor so the orchestrator stays decoupled
 * from Tauri/stores and is unit-testable with plain mocks.
 *
 * `connected` itself is NOT owned here — `layout.svelte.ts` is its authoritative
 * owner via `setLayoutConnected`. The error banner is NOT owned here either —
 * it is page-wide route state written by several workflows, so the orchestrator
 * only reports failures through the narrow `setErrorMessage` dep.
 */
export interface SyncSessionConnectionDeps {
  /** Disconnect the live bus session (Tauri `disconnect_lcc`). */
  disconnectLcc: () => Promise<void>;
  /** Probe for nodes after a fresh connect. */
  probeForNodes: () => Promise<void> | void;
  /** Whether a layout file is currently open. */
  hasLayoutFile: () => boolean;
  /** Whether the active layout has persisted snapshots (offline fallback). */
  hasSnapshots: () => boolean;
  /** Mirror connection state into the layout store (authoritative owner). */
  setLayoutConnected: (connected: boolean) => void;
  /** Reset transient state for a fresh live session on connect. */
  resetFreshLiveSessionState: () => void;
  /** Rehydrate offline snapshots after disconnect (resets tree first). */
  rehydrateOffline: () => Promise<void>;
  /** Tear down live discovery/tree state (config status + roster + tree). */
  clearLiveState: () => void;
  /** Reset + hide the sync panel. */
  resetSyncPanel: () => void;
  /** Show/hide the connection dialog (page-composition state). */
  setShowConnectionDialog: (visible: boolean) => void;
  /** Report a workflow error to the page banner (route-owned). */
  setErrorMessage: (message: string) => void;
  /** Surface a non-fatal warning (defaults to console.warn at the call site). */
  warn?: (message: string, error?: unknown) => void;
}

export class SyncSessionOrchestrator {
  private discoverySettleTimer: ReturnType<typeof setTimeout> | null = null;
  private syncTriggered = false;

  #connectionLabel = $state('');

  constructor(
    private readonly deps: SyncSessionConnectionDeps,
    private readonly settleDelayMs = 1000,
  ) {}

  /** Visible label for the active connection (status pill). */
  get connectionLabel(): string {
    return this.#connectionLabel;
  }

  /**
   * Apply a connection label discovered outside the `connect()` path — namely
   * the startup lifecycle (`bootstrapStartupLifecycle`) when the app launches
   * already connected.
   */
  setConnectionLabel(label: string): void {
    this.#connectionLabel = label;
  }

  /**
   * Connect a live bus session from a ConnectionManager `connected` event.
   * Hides preflight/transition sequencing behind the existing pure
   * `connectLiveSession` helper; owns the connection label.
   */
  connect(config: ConnectionConfig): void {
    connectLiveSession({
      config,
      hasLayoutFile: this.deps.hasLayoutFile(),
      setConnectionLabel: (label) => { this.#connectionLabel = label; },
      setLayoutConnected: this.deps.setLayoutConnected,
      hideConnectionDialog: () => this.deps.setShowConnectionDialog(false),
      resetSyncSessionAutoTrigger: () => this.resetAutoTrigger(),
      resetFreshLiveSessionState: this.deps.resetFreshLiveSessionState,
      probeForNodes: this.deps.probeForNodes,
    });
  }

  /**
   * Tear down the live bus session, falling back to offline state when a
   * layout with snapshots is active. Sequencing lives in the pure
   * `disconnectWithOfflineFallback` helper.
   */
  async disconnect(): Promise<void> {
    this.deps.setErrorMessage('');
    this.#connectionLabel = '';
    await disconnectWithOfflineFallback({
      disconnect: this.deps.disconnectLcc,
      afterDisconnect: () => {
        this.deps.setLayoutConnected(false);
        this.deps.resetSyncPanel();
        this.resetAutoTrigger();
      },
      hasLayoutFile: this.deps.hasLayoutFile(),
      hasSnapshots: this.deps.hasSnapshots(),
      rehydrateOffline: this.deps.rehydrateOffline,
      preserveLiveState: () => {
        this.deps.setShowConnectionDialog(false);
        this.deps.clearLiveState();
      },
      clearLiveState: this.deps.clearLiveState,
      showConnectionDialog: () => this.deps.setShowConnectionDialog(true),
      onError: (message) => this.deps.setErrorMessage(message),
    });
  }

  /**
   * Tear down the live bus session before switching to a different layout.
   * Skips the offline-rehydration branch of the regular disconnect path
   * because the layout (and its snapshots) are about to be replaced.
   */
  async disconnectBeforeLayoutSwitch(): Promise<void> {
    this.deps.setErrorMessage('');
    this.#connectionLabel = '';
    try {
      await this.deps.disconnectLcc();
    } catch (error) {
      this.deps.warn?.('Disconnect before layout switch failed:', error);
    }
    this.deps.setLayoutConnected(false);
    this.deps.resetSyncPanel();
    this.resetAutoTrigger();
    this.deps.clearLiveState();
    this.deps.setShowConnectionDialog(false);
  }

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