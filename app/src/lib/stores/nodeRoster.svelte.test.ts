/**
 * Tests for the unified node-roster facade (Spec 014 / S8.7).
 *
 * The roster is the single source of truth for "the set of nodes the user
 * sees". Live discoveries and placeholders flow through the same surface,
 * so any visibility gate reading from `roster.allEntries` /
 * `roster.liveNodes` lights up uniformly — closing the S8.5-era bug where
 * adding a placeholder on an empty layout left `+page.svelte`'s page-local
 * `nodes` array empty and the main content showed "No nodes found."
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

const addPlaceholderBoardIpcMock = vi.fn();
const getNodeTreeMock = vi.fn();
const listBundledProfilesIpc = vi.fn();

vi.mock('$lib/api/layout', () => ({
  addPlaceholderBoardIpc: addPlaceholderBoardIpcMock,
  getNodeTree: getNodeTreeMock,
  listBundledProfiles: listBundledProfilesIpc,
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn(), open: vi.fn() }));

const { nodeInfoStore } = await import('$lib/stores/nodeInfo');
const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');
const { configReadNodesStore } = await import('$lib/stores/configReadStatus');
const { nodeRoster } = await import('./nodeRoster.svelte');
const { addPlaceholderBoard, deletePlaceholderBoard } = await import(
  '$lib/orchestration/placeholderBoardOrchestrator'
);

const STUB_TREE = { segments: [] } as unknown as Parameters<typeof nodeTreeStore.setTree>[1];
const STUB_PROFILES = [
  { stem: 'RR-CirKits_Tower-LCC', manufacturer: 'RR-CirKits', model: 'Tower-LCC' },
];

function makeLiveNode(idHex: string, name = 'Live Node'): import('$lib/api/tauri').DiscoveredNode {
  // Parse "02.01.57.00.00.01" → [2,1,87,0,0,1]
  const node_id = idHex.split('.').map((h) => parseInt(h, 16));
  return {
    node_id,
    alias: 0,
    snip_data: {
      manufacturer: 'TestMfg',
      model: 'TestModel',
      hardware_version: '',
      software_version: '',
      user_name: name,
      user_description: '',
    },
    snip_status: 'Complete',
    connection_status: 'Connected',
    last_verified: '',
    last_seen: '',
    cdi: null,
    pip_flags: null,
    pip_status: 'NotSupported',
  };
}

/** Create a node as offline hydration would — status Unknown, synthetic alias. */
function makeHydratedNode(idHex: string, name = 'Hydrated Node'): import('$lib/api/tauri').DiscoveredNode {
  return { ...makeLiveNode(idHex, name), connection_status: 'Unknown', alias: 0x700 };
}

let addCallCounter = 0;

beforeEach(() => {
  addCallCounter = 0;
  addPlaceholderBoardIpcMock.mockReset();
  getNodeTreeMock.mockReset();
  listBundledProfilesIpc.mockReset();
  listBundledProfilesIpc.mockResolvedValue(STUB_PROFILES);
  addPlaceholderBoardIpcMock.mockImplementation(async () => {
    addCallCounter++;
    const hex = addCallCounter.toString(16).padStart(12, '0');
    return { nodeKey: `placeholder:${hex.slice(0,8)}-${hex.slice(8,12)}-4000-8000-000000000000` };
  });
  getNodeTreeMock.mockResolvedValue(STUB_TREE);
  nodeRoster.clearLayoutScope();
});

describe('nodeRoster — bug-2 regression contract', () => {
  it('reports a non-empty roster after addPlaceholderBoard on an empty layout', async () => {
    expect(nodeRoster.allEntries).toHaveLength(0);
    expect(nodeRoster.liveNodes).toHaveLength(0);

    const { nodeKey } = await addPlaceholderBoard({
      profileStem: 'RR-CirKits_Tower-LCC',
    });

    // The bug: `+page.svelte` gated main content on `nodes.length === 0`,
    // a page-local array fed only by live discovery. The fix: that gate
    // now reads from the unified roster, which counts placeholders too.
    expect(nodeRoster.allEntries.length).toBe(1);
    expect(nodeRoster.allEntries[0].nodeKey).toBe(nodeKey);
    expect(nodeRoster.allEntries[0].kind).toBe('placeholder');
    expect(nodeRoster.allEntries[0].profileStem).toBe('RR-CirKits_Tower-LCC');
  });
});

describe('nodeRoster — typed views', () => {
  it('partitions live vs placeholder entries by `kind`', async () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });

    expect(nodeRoster.allEntries).toHaveLength(2);
    expect(nodeRoster.liveEntries).toHaveLength(1);
    expect(nodeRoster.placeholderEntries).toHaveLength(1);
    expect(nodeRoster.liveEntries[0].kind).toBe('live');
    expect(nodeRoster.placeholderEntries[0].kind).toBe('placeholder');
  });

  it('liveNodes returns DiscoveredNode[] excluding placeholders (back-compat for +page.svelte)', async () => {
    const live = makeLiveNode('02.01.57.00.00.02', 'Alpha');
    nodeRoster.upsertLive(live);
    await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });

    expect(nodeRoster.liveNodes).toHaveLength(1);
    expect(nodeRoster.liveNodes[0].snip_data?.user_name).toBe('Alpha');
  });
});

describe('nodeRoster — mutators', () => {
  it('replaceLiveRoster preserves placeholders while swapping the live set', async () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });

    const next = [makeLiveNode('02.01.57.00.00.99', 'Replacement')];
    nodeRoster.replaceLiveRoster(next);

    expect(nodeRoster.liveNodes).toHaveLength(1);
    expect(nodeRoster.liveNodes[0].snip_data?.user_name).toBe('Replacement');
    // Placeholder must survive the live-roster swap.
    expect(nodeRoster.placeholderEntries).toHaveLength(1);
  });

  it('replaceLiveRoster skips entries with empty node_id without crashing (Bug 1 regression)', async () => {
    // When the discovery handler feeds `allEntries` (live + placeholder) back
    // through replaceLiveRoster, placeholder entries have node_id: []. The
    // method must skip them rather than crashing on liveKeyFromBytes([]).
    await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });

    const mixedArray = [
      { node_id: [], alias: 0, snip_data: null, snip_status: 'Complete' as const,
        connection_status: 'Unknown' as const, last_verified: null,
        last_seen: '', cdi: null, pip_flags: null, pip_status: 'NotSupported' as const },
      makeLiveNode('02.01.57.00.00.42', 'Live Via Mixed'),
    ];

    // Must not throw.
    nodeRoster.replaceLiveRoster(mixedArray);

    expect(nodeRoster.liveNodes).toHaveLength(1);
    expect(nodeRoster.liveNodes[0].snip_data?.user_name).toBe('Live Via Mixed');
    expect(nodeRoster.placeholderEntries).toHaveLength(1);
  });

  it('clearLayoutScope wipes every backing store in one call', async () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });
    expect(nodeRoster.allEntries).toHaveLength(2);

    nodeRoster.clearLayoutScope();

    expect(nodeRoster.allEntries).toHaveLength(0);
    expect(get(nodeInfoStore).size).toBe(0);
    expect(nodeTreeStore.trees.size).toBe(0);
    expect(get(configReadNodesStore).size).toBe(0);
    expect(nodeRoster.placeholderEntries).toHaveLength(0);
  });

  it('removePlaceholder is a no-op for a live nodeId', async () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    nodeRoster.removePlaceholder('02.01.57.00.00.01');
    expect(nodeRoster.liveNodes).toHaveLength(1);
  });

  it('end-to-end: add → delete via orchestrator round-trips through the roster', async () => {
    const { nodeKey } = await addPlaceholderBoard({
      profileStem: 'RR-CirKits_Tower-LCC',
    });
    expect(nodeRoster.has(nodeKey)).toBe(true);

    const removed = await deletePlaceholderBoard({
      nodeKey,
      confirm: async () => true,
    });
    expect(removed).toBe(true);
    expect(nodeRoster.has(nodeKey)).toBe(false);
    expect(nodeRoster.allEntries).toHaveLength(0);
  });
});

describe('nodeRoster — NodeKey keying (Spec 014 Step 6b)', () => {
  it('lookup via dotted and canonical NodeKey hit the same entry', async () => {
    const { nodeKey: nk } = await import('$lib/utils/nodeKey');
    nodeRoster.upsertLive(makeLiveNode('05.02.01.00.00.00'));

    const dotted = nk('05.02.01.00.00.00');
    const canonical = nk('050201000000');

    expect(nodeRoster.has(dotted)).toBe(true);
    expect(nodeRoster.has(canonical)).toBe(true);
    expect(nodeRoster.allEntries).toHaveLength(1);
  });

  it('lookup via a key built from NodeID bytes matches dotted-form lookup', async () => {
    const { nodeKey: nk } = await import('$lib/utils/nodeKey');
    const { formatNodeId } = await import('$lib/utils/nodeId');
    const node = makeLiveNode('05.02.01.00.00.00');
    nodeRoster.upsertLive(node);

    const fromBytes = nk(formatNodeId(node.node_id));
    const fromDotted = nk('05.02.01.00.00.00');

    expect(nodeRoster.has(fromBytes)).toBe(true);
    expect(nodeRoster.has(fromDotted)).toBe(true);
  });
});

describe('nodeRoster — isOffline()', () => {
  it('returns true for a node with NotResponding status', () => {
    const node = makeLiveNode('02.01.57.00.00.01');
    node.connection_status = 'NotResponding';
    nodeRoster.upsertLive(node);

    expect(nodeRoster.isOffline('020157000001')).toBe(true);
  });

  it('returns false for a Connected node', () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));

    expect(nodeRoster.isOffline('020157000001')).toBe(false);
  });

  it('returns false for a node not in the roster', () => {
    expect(nodeRoster.isOffline('020157000099')).toBe(false);
  });

  it('accepts dotted-form keys', () => {
    const node = makeLiveNode('02.01.57.00.00.01');
    node.connection_status = 'NotResponding';
    nodeRoster.upsertLive(node);

    expect(nodeRoster.isOffline('02.01.57.00.00.01')).toBe(true);
  });
});

describe('nodeRoster — off-bus saved node injection', () => {
  it('injectOffBusSavedNodes creates NotResponding entries for saved IDs not on the bus', () => {
    // Two nodes on the bus
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.02'));

    // Layout has three saved nodes — one is missing from the bus
    const savedNodeIds = ['020157000001', '020157000002', '020157000003'];

    nodeRoster.injectOffBusSavedNodes(savedNodeIds);

    // The missing node should now appear in the roster
    expect(nodeRoster.allEntries).toHaveLength(3);
    const offBusEntry = nodeRoster.allEntries.find(
      (e) => e.nodeKey === '020157000003',
    );
    expect(offBusEntry).toBeDefined();
    expect(offBusEntry!.info.connection_status).toBe('NotResponding');
    expect(offBusEntry!.kind).toBe('live');
  });

  it('injectOffBusSavedNodes is a no-op when all saved nodes are already on the bus', () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.02'));

    const savedNodeIds = ['020157000001', '020157000002'];
    nodeRoster.injectOffBusSavedNodes(savedNodeIds);

    expect(nodeRoster.allEntries).toHaveLength(2);
    // All should remain Connected
    for (const entry of nodeRoster.allEntries) {
      expect(entry.info.connection_status).toBe('Connected');
    }
  });

  it('injectOffBusSavedNodes does not duplicate a node already in the roster', () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    const savedNodeIds = ['020157000001'];

    nodeRoster.injectOffBusSavedNodes(savedNodeIds);

    expect(nodeRoster.allEntries).toHaveLength(1);
    expect(nodeRoster.allEntries[0].info.connection_status).toBe('Connected');
  });

  it('injectOffBusSavedNodes skips placeholder node keys', () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    const savedNodeIds = ['020157000001', 'placeholder:test-uuid-0000'];

    nodeRoster.injectOffBusSavedNodes(savedNodeIds);

    // Only the live node — placeholder should not get a synthetic entry
    expect(nodeRoster.allEntries).toHaveLength(1);
  });

  it('injectOffBusSavedNodes is a no-op when savedNodeIds is undefined', () => {
    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));

    nodeRoster.injectOffBusSavedNodes(undefined);

    expect(nodeRoster.allEntries).toHaveLength(1);
  });

  it('off-bus injected nodes render as isOffline in sidebar entries', async () => {
    const { buildSidebarNodeEntries } = await import(
      '$lib/components/ConfigSidebar/configSidebarPresenter'
    );

    nodeRoster.upsertLive(makeLiveNode('02.01.57.00.00.01'));
    const savedNodeIds = ['020157000001', '020157000003'];
    nodeRoster.injectOffBusSavedNodes(savedNodeIds);

    const entries = buildSidebarNodeEntries(get(nodeInfoStore), savedNodeIds);
    const offlineEntry = entries.find((e) => e.nodeId === '020157000003');
    expect(offlineEntry).toBeDefined();
    expect(offlineEntry!.isOffline).toBe(true);

    const onlineEntry = entries.find((e) => e.nodeId === '020157000001');
    expect(onlineEntry).toBeDefined();
    expect(onlineEntry!.isOffline).toBe(false);
  });

  it('marks offline-hydrated Unknown nodes as NotResponding (connect after offline open)', () => {
    // Simulate offline hydration: 3 nodes loaded from layout snapshots
    // with connection_status: 'Unknown' (as buildOfflineDiscoveryNodes does)
    nodeRoster.replaceLiveRoster([
      makeHydratedNode('02.01.57.00.00.01', 'Node A'),
      makeHydratedNode('02.01.57.00.00.02', 'Node B'),
      makeHydratedNode('02.01.57.00.00.03', 'Node C'),
    ]);

    // Simulate connect: discovery finds nodes A and B, updates them to Connected.
    // Node C never responds.
    const nodeA = makeLiveNode('02.01.57.00.00.01', 'Node A');
    const nodeB = makeLiveNode('02.01.57.00.00.02', 'Node B');
    // After discovery, replaceLiveRoster is called with the accumulated result
    // which includes the original hydrated C (still Unknown)
    nodeRoster.replaceLiveRoster([
      nodeA,
      nodeB,
      makeHydratedNode('02.01.57.00.00.03', 'Node C'),
    ]);

    // injectOffBusSavedNodes should mark C as NotResponding
    const savedNodeIds = ['020157000001', '020157000002', '020157000003'];
    nodeRoster.injectOffBusSavedNodes(savedNodeIds);

    const entries = nodeRoster.allEntries;
    expect(entries).toHaveLength(3);

    const nodeC = entries.find((e) => e.nodeKey === '020157000003');
    expect(nodeC).toBeDefined();
    expect(nodeC!.info.connection_status).toBe('NotResponding');
    // SNIP data from hydration is preserved
    expect(nodeC!.info.snip_data?.user_name).toBe('Node C');

    // Connected nodes are not downgraded
    const foundA = entries.find((e) => e.nodeKey === '020157000001');
    expect(foundA!.info.connection_status).toBe('Connected');
    const foundB = entries.find((e) => e.nodeKey === '020157000002');
    expect(foundB!.info.connection_status).toBe('Connected');
  });

  it('does not downgrade a Verifying node to NotResponding', () => {
    const verifying = makeLiveNode('02.01.57.00.00.01');
    verifying.connection_status = 'Verifying';
    nodeRoster.upsertLive(verifying);

    nodeRoster.injectOffBusSavedNodes(['020157000001']);

    expect(nodeRoster.allEntries[0].info.connection_status).toBe('Verifying');
  });
});
