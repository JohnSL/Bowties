/**
 * layoutLifecycleOrchestrator tests — see ADR-0011.
 *
 * Contracts:
 *   1. `resetForNewLayout()` clears in-memory placeholders (R7 fix).
 *   2. `resetForFreshLiveSession()` preserves placeholders even with no layout.
 *   3. Both methods clear the in-memory inputs the facade reads
 *      (`partialCaptureNodesStore`, `nodeTreeStore`, `configReadNodesStore`).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn(), open: vi.fn() }));
vi.mock('$lib/api/layout', () => ({
  addPlaceholderBoardIpc: vi.fn(),
  getNodeTree: vi.fn(),
  listBundledProfiles: vi.fn().mockResolvedValue([]),
}));

const { nodeInfoStore } = await import('$lib/stores/nodeInfo');
const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');
const { configReadNodesStore, markNodeConfigRead } = await import('$lib/stores/configReadStatus');
const { nodeRoster } = await import('$lib/stores/nodeRoster.svelte');
const { partialCaptureNodesStore } = await import('$lib/stores/partialCaptureNodes.svelte');
const { layoutStore } = await import('$lib/stores/layout.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { layoutLifecycleOrchestrator } = await import('./layoutLifecycleOrchestrator');

const LIVE_KEY = '020157000001';
const PLACEHOLDER_KEY = 'placeholder:11111111-2222-4333-8444-555555555555';

function seedLive(): void {
  const map = new Map(get(nodeInfoStore));
  map.set(LIVE_KEY, {
    node_id: [2, 1, 87, 0, 0, 1],
    alias: 0,
    snip_data: {
      manufacturer: 'Mfg',
      model: 'Mod',
      hardware_version: '',
      software_version: '',
      user_name: '',
      user_description: '',
    },
    snip_status: 'Complete',
    connection_status: 'Connected',
    last_verified: '',
    last_seen: '',
    cdi: null,
    pip_flags: null,
    pip_status: 'NotSupported',
  } as never);
  nodeInfoStore.set(map);
  nodeTreeStore.setTree(LIVE_KEY, { segments: [] } as never);
  markNodeConfigRead(LIVE_KEY);
}

function seedPlaceholder(): void {
  const map = new Map(get(nodeInfoStore));
  map.set(PLACEHOLDER_KEY, {
    node_id: [0, 0, 0, 0, 0, 0],
    alias: 0,
    snip_data: {
      manufacturer: 'Mfg',
      model: 'Mod',
      hardware_version: '',
      software_version: '',
      user_name: 'Placeholder',
      user_description: '',
    },
    snip_status: 'Complete',
    connection_status: 'NotApplicable',
    last_verified: '',
    last_seen: '',
    cdi: null,
    pip_flags: null,
    pip_status: 'NotSupported',
  } as never);
  nodeInfoStore.set(map);
  nodeRoster.addPlaceholder({
    nodeKey: PLACEHOLDER_KEY,
    profileStem: 'stem',
    info: get(nodeInfoStore).get(PLACEHOLDER_KEY)!,
    tree: { segments: [] } as never,
  });
}

beforeEach(() => {
  nodeInfoStore.set(new Map());
  nodeTreeStore.reset();
  nodeRoster.clearLayoutScope();
  partialCaptureNodesStore.clear();
  layoutStore.reset();
});

describe('layoutLifecycleOrchestrator.resetForNewLayout', () => {
  it('clears placeholders so they do not leak into the new layout (R7)', async () => {
    seedPlaceholder();
    expect(nodeRoster.placeholderEntries.length).toBe(1);

    await layoutLifecycleOrchestrator.resetForNewLayout({ connected: false });

    expect(nodeRoster.placeholderEntries.length).toBe(0);
    expect(get(nodeInfoStore).has(PLACEHOLDER_KEY)).toBe(false);
  });

  it('clears every in-memory input the facade reads', async () => {
    seedLive();
    partialCaptureNodesStore.replace([LIVE_KEY]);

    await layoutLifecycleOrchestrator.resetForNewLayout({ connected: false });

    expect(get(nodeInfoStore).size).toBe(0);
    expect(nodeTreeStore.trees.size).toBe(0);
    expect(get(configReadNodesStore).size).toBe(0);
    expect(partialCaptureNodesStore.nodes.size).toBe(0);
  });

  it('reprobes for live nodes when connected and reprobeLiveNodes is true', async () => {
    const probe = vi.fn().mockResolvedValue(undefined);
    await layoutLifecycleOrchestrator.resetForNewLayout({
      connected: true,
      reprobeLiveNodes: true,
      probeForNodes: probe,
    });
    expect(probe).toHaveBeenCalledTimes(1);
  });

  it('does not reprobe when disconnected', async () => {
    const probe = vi.fn().mockResolvedValue(undefined);
    await layoutLifecycleOrchestrator.resetForNewLayout({
      connected: false,
      reprobeLiveNodes: true,
      probeForNodes: probe,
    });
    expect(probe).not.toHaveBeenCalled();
  });

  it('resets channelsStore so stale channels do not leak into next layout (S6)', async () => {
    channelsStore.addPendingChannels([{
      id: 'stale-ch',
      name: 'Stale Channel',
      channelType: 'block-occupancy',
      hardwareRef: { nodeKey: LIVE_KEY, connector: 'connector-a', input: 1 },
    }]);
    expect(channelsStore.channels.length).toBe(1);

    await layoutLifecycleOrchestrator.resetForNewLayout({ connected: false });

    expect(channelsStore.channels.length).toBe(0);
    expect(channelsStore.isEmpty).toBe(true);
  });
});

describe('layoutLifecycleOrchestrator.resetForFreshLiveSession', () => {
  it('preserves placeholders (they are layout-scoped, not bus-scoped)', () => {
    seedPlaceholder();
    seedLive();

    layoutLifecycleOrchestrator.resetForFreshLiveSession();

    expect(get(nodeInfoStore).has(PLACEHOLDER_KEY)).toBe(true);
    expect(get(nodeInfoStore).has(LIVE_KEY)).toBe(false);
  });

  it('clears live config-read status and trees', () => {
    seedLive();
    expect(nodeTreeStore.trees.size).toBe(1);

    layoutLifecycleOrchestrator.resetForFreshLiveSession();

    expect(nodeTreeStore.trees.size).toBe(0);
    expect(get(configReadNodesStore).size).toBe(0);
  });
});

describe('layoutLifecycleOrchestrator.closeLayout', () => {
  it('runs the backend close IPC before wiping frontend stores (offline_file)', async () => {
    seedPlaceholder();
    const calls: string[] = [];
    const closeLayoutIpc = vi.fn(async () => {
      calls.push('ipc');
      // Snapshot at this moment: frontend wipe must not have happened yet.
      expect(get(nodeInfoStore).has(PLACEHOLDER_KEY)).toBe(true);
      return { closed: true };
    });
    const clearRecentLayout = vi.fn(async () => {
      calls.push('clearRecent');
    });
    const afterReset = vi.fn(() => {
      calls.push('afterReset');
      expect(nodeRoster.placeholderEntries.length).toBe(0);
    });

    const closed = await layoutLifecycleOrchestrator.closeLayout({
      activeMode: 'offline_file',
      closeLayoutIpc,
      clearRecentLayout,
      connected: false,
      afterReset,
    });

    expect(closed).toBe(true);
    expect(closeLayoutIpc).toHaveBeenCalledWith('discard');
    expect(clearRecentLayout).not.toHaveBeenCalled();
    expect(calls).toEqual(['ipc', 'afterReset']);
    expect(nodeRoster.placeholderEntries.length).toBe(0);
  });

  it('leaves frontend state untouched when backend refuses to close', async () => {
    seedPlaceholder();
    const closeLayoutIpc = vi.fn(async () => ({ closed: false, reason: 'cancelled' }));
    const clearRecentLayout = vi.fn(async () => {});

    const closed = await layoutLifecycleOrchestrator.closeLayout({
      activeMode: 'offline_file',
      closeLayoutIpc,
      clearRecentLayout,
      connected: false,
    });

    expect(closed).toBe(false);
    expect(nodeRoster.placeholderEntries.length).toBe(1);
  });

  it('clears the recent legacy layout path for legacy_file mode and still resets', async () => {
    seedPlaceholder();
    const closeLayoutIpc = vi.fn(async () => ({ closed: true }));
    const clearRecentLayout = vi.fn(async () => {});

    const closed = await layoutLifecycleOrchestrator.closeLayout({
      activeMode: 'legacy_file',
      closeLayoutIpc,
      clearRecentLayout,
      connected: false,
    });

    expect(closed).toBe(true);
    expect(closeLayoutIpc).not.toHaveBeenCalled();
    expect(clearRecentLayout).toHaveBeenCalledTimes(1);
    expect(nodeRoster.placeholderEntries.length).toBe(0);
  });

  it('reports recent-layout errors via the callback but still resets', async () => {
    seedPlaceholder();
    const warning = vi.fn();

    const closed = await layoutLifecycleOrchestrator.closeLayout({
      activeMode: 'legacy_file',
      closeLayoutIpc: vi.fn(async () => ({ closed: true })),
      clearRecentLayout: vi.fn(async () => {
        throw new Error('disk busy');
      }),
      onRecentLayoutClearError: warning,
      connected: false,
    });

    expect(closed).toBe(true);
    expect(warning).toHaveBeenCalledTimes(1);
    expect(nodeRoster.placeholderEntries.length).toBe(0);
  });

  it('calls disconnectBeforeClose before resetting stores when connected (regression: connection indicator stays Online after close)', async () => {
    seedPlaceholder();
    const calls: string[] = [];
    const disconnectBeforeClose = vi.fn(async () => {
      calls.push('disconnect');
      // Frontend stores must not have been wiped yet when disconnect runs.
      expect(get(nodeInfoStore).has(PLACEHOLDER_KEY)).toBe(true);
    });
    const afterReset = vi.fn(() => {
      calls.push('afterReset');
    });

    const closed = await layoutLifecycleOrchestrator.closeLayout({
      activeMode: 'offline_file',
      closeLayoutIpc: vi.fn(async () => { calls.push('ipc'); return { closed: true }; }),
      clearRecentLayout: vi.fn(async () => {}),
      connected: true,
      disconnectBeforeClose,
      afterReset,
    });

    expect(closed).toBe(true);
    expect(disconnectBeforeClose).toHaveBeenCalledTimes(1);
    expect(calls).toEqual(['ipc', 'disconnect', 'afterReset']);
  });

  it('does not call disconnectBeforeClose when not connected', async () => {
    const disconnectBeforeClose = vi.fn(async () => {});

    await layoutLifecycleOrchestrator.closeLayout({
      activeMode: 'offline_file',
      closeLayoutIpc: vi.fn(async () => ({ closed: true })),
      clearRecentLayout: vi.fn(async () => {}),
      connected: false,
      disconnectBeforeClose,
    });

    expect(disconnectBeforeClose).not.toHaveBeenCalled();
  });

  it('does not call disconnectBeforeClose when backend refuses to close', async () => {
    const disconnectBeforeClose = vi.fn(async () => {});

    const closed = await layoutLifecycleOrchestrator.closeLayout({
      activeMode: 'offline_file',
      closeLayoutIpc: vi.fn(async () => ({ closed: false, reason: 'cancelled' })),
      clearRecentLayout: vi.fn(async () => {}),
      connected: true,
      disconnectBeforeClose,
    });

    expect(closed).toBe(false);
    expect(disconnectBeforeClose).not.toHaveBeenCalled();
  });
});
