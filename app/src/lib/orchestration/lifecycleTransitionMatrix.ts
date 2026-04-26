export type StartupTransition =
  | 'startup_disconnected_idle'
  | 'startup_fresh_live'
  | 'startup_preserved_layout';

export type ConnectTransition =
  | 'connect_fresh_live'
  | 'connect_preserved_layout';

export type DisconnectTransition =
  | 'rehydrated_offline'
  | 'preserved_layout'
  | 'cleared_to_connection';

export type LifecycleTransition =
  | StartupTransition
  | ConnectTransition
  | DisconnectTransition;

type LayoutPresence = 'with_layout' | 'without_layout';
type Connectivity = 'connected' | 'disconnected';
type SnapshotPresence = 'with_snapshots' | 'without_snapshots';

const startupTransitionMatrix: Record<Connectivity, Record<LayoutPresence, StartupTransition>> = {
  connected: {
    with_layout: 'startup_preserved_layout',
    without_layout: 'startup_fresh_live',
  },
  disconnected: {
    with_layout: 'startup_disconnected_idle',
    without_layout: 'startup_disconnected_idle',
  },
};

const connectTransitionMatrix: Record<LayoutPresence, ConnectTransition> = {
  with_layout: 'connect_preserved_layout',
  without_layout: 'connect_fresh_live',
};

const disconnectTransitionMatrix: Record<LayoutPresence, Record<SnapshotPresence, DisconnectTransition>> = {
  with_layout: {
    with_snapshots: 'rehydrated_offline',
    without_snapshots: 'preserved_layout',
  },
  without_layout: {
    with_snapshots: 'cleared_to_connection',
    without_snapshots: 'cleared_to_connection',
  },
};

function layoutPresence(hasLayoutFile: boolean): LayoutPresence {
  return hasLayoutFile ? 'with_layout' : 'without_layout';
}

function connectivity(connected: boolean): Connectivity {
  return connected ? 'connected' : 'disconnected';
}

function snapshotPresence(hasSnapshots: boolean): SnapshotPresence {
  return hasSnapshots ? 'with_snapshots' : 'without_snapshots';
}

export function resolveStartupTransition(
  connected: boolean,
  hasLayoutFile: boolean,
): StartupTransition {
  return startupTransitionMatrix[connectivity(connected)][layoutPresence(hasLayoutFile)];
}

export function resolveConnectTransition(hasLayoutFile: boolean): ConnectTransition {
  return connectTransitionMatrix[layoutPresence(hasLayoutFile)];
}

export function resolveDisconnectTransition(
  hasLayoutFile: boolean,
  hasSnapshots: boolean,
): DisconnectTransition {
  return disconnectTransitionMatrix[layoutPresence(hasLayoutFile)][snapshotPresence(hasSnapshots)];
}

export function shouldResetFreshLiveSession(transition: LifecycleTransition): boolean {
  return transition === 'startup_fresh_live' || transition === 'connect_fresh_live';
}

export function shouldProbeAfterTransition(transition: LifecycleTransition): boolean {
  return transition === 'startup_fresh_live' ||
    transition === 'startup_preserved_layout' ||
    transition === 'connect_fresh_live' ||
    transition === 'connect_preserved_layout';
}