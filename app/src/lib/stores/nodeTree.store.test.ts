/**
 * Store-level tests for nodeTreeStore (nodeTree.svelte.ts).
 *
 * Expected behavior: after discovery completes, the backend emits
 * `node-tree-updated` for every node whose CDI was read.  The frontend
 * nodeTreeStore should load trees for all such nodes automatically, so that
 * `nodeTreeStore.trees` is populated without requiring the user to manually
 * expand each node in the config sidebar.
 *
 * This is required for the bowtie preview store's `collectEntriesForEventId()`
 * to have data — layout-named bowties should show their producer/consumer
 * entries as soon as discovery completes.
 *
 * Also covers Step 1 of plan-config-nav-refactor: CDI-index vs array-index
 * correctness in findLeafByPathInChildren (tested indirectly via updateLeafValue).
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NodeConfigTree } from '$lib/types/nodeTree';

// ─── Capture the node-tree-updated listener so tests can simulate the event ──

let capturedNodeTreeListener: ((event: { payload: { nodeId: string; leafCount: number } }) => void) | null = null;

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (eventName: string, callback: Function) => {
    if (eventName === 'node-tree-updated') {
      capturedNodeTreeListener = callback as any;
    }
    return () => {};
  }),
}));

// ─── Mock invoke so get_node_tree returns a valid tree ────────────────────────

const NODE_ID = '05.02.01.00.00.00';

const MOCK_TREE: NodeConfigTree = {
  nodeId: NODE_ID,
  identity: null,
  segments: [
    { name: 'Config', description: null, origin: 0, space: 253, children: [] },
  ],
};

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(MOCK_TREE),
}));

// ─── Import store AFTER mocks are in place ────────────────────────────────────

const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');
const { invoke } = await import('@tauri-apps/api/core');

// ─── Tests ────────────────────────────────────────────────────────────────────

beforeEach(async () => {
  nodeTreeStore.reset();
  nodeTreeStore.stopListening();
  capturedNodeTreeListener = null;
  vi.clearAllMocks();
  (invoke as any).mockResolvedValue(MOCK_TREE);
});

describe('nodeTreeStore — node-tree-updated', () => {
  it('loads tree for a newly discovered node when backend emits node-tree-updated after CDI scan', async () => {
    // Precondition: store is empty, node has never been expanded
    expect(nodeTreeStore.trees.has(NODE_ID)).toBe(false);

    // Register the listener (simulates app startup)
    await nodeTreeStore.startListening();
    expect(capturedNodeTreeListener).not.toBeNull();

    // Simulate backend emitting node-tree-updated after CDI read completes
    capturedNodeTreeListener!({ payload: { nodeId: NODE_ID, leafCount: 3 } });

    // Currently the listener exits early because `!_trees.has(NODE_ID)`,
    // so the tree is never fetched — the store should fetch it regardless.
    await vi.waitFor(() => {
      expect(nodeTreeStore.trees.has(NODE_ID)).toBe(true);
    }, { timeout: 1000 });

    expect(nodeTreeStore.getTree(NODE_ID)).toMatchObject({ nodeId: NODE_ID });
  });

  it('refreshes tree for a node that is already loaded when backend emits node-tree-updated', async () => {
    // Pre-populate the store as if the user had already expanded this node
    nodeTreeStore.setTree(NODE_ID, MOCK_TREE);
    expect(nodeTreeStore.trees.has(NODE_ID)).toBe(true);

    await nodeTreeStore.startListening();
    expect(capturedNodeTreeListener).not.toBeNull();

    // Simulate backend emitting node-tree-updated (e.g. after config save)
    capturedNodeTreeListener!({ payload: { nodeId: NODE_ID, leafCount: 3 } });

    // This already works today — the existing guard `if (_trees.has(nodeId))` allows it
    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('get_node_tree', { nodeId: NODE_ID });
    }, { timeout: 1000 });
  });
});

// ─── Step 1: CDI-index vs array-index (findLeafByPathInChildren) ─────────────
//
// Tested indirectly via updateLeafValue, which calls findLeafByPath →
// findLeafByPathInChildren internally.  The key scenario: the Rust backend
// encodes CDI element index i in path strings, but spacer groups are skipped
// before push(), so children[0] may have path "elem:1" (not "elem:0").
// The fix uses path-component matching instead of array-index lookup.

import type { ConfigNode, LeafConfigNode, GroupConfigNode } from '$lib/types/nodeTree';

function makeTestLeaf(path: string[], address = 0): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Test',
    description: null,
    elementType: 'int',
    address,
    size: 1,
    space: 253,
    path,
    value: { type: 'int', value: 42 },
    eventRole: null,
    constraints: null,
  };
}

function makeTestGroup(path: string[], children: ConfigNode[]): GroupConfigNode {
  return {
    kind: 'group',
    name: 'G',
    description: null,
    instance: 1,
    instanceLabel: 'G 1',
    replicationOf: 'G',
    replicationCount: 1,
    path,
    children,
    displayName: null,
  };
}

describe('updateLeafValue — CDI-index vs array-index (Step 1)', () => {
  it('no spacers — finds leaf by path at matching array index', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [leaf] }],
    });
    nodeTreeStore.updateLeafValue(NODE_ID, ['seg:0', 'elem:0'], { type: 'int', value: 99 });
    expect(nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0]).toMatchObject({
      kind: 'leaf', value: { type: 'int', value: 99 },
    });
  });

  it('spacer before target — finds elem:1 at children[0] by path matching, not array index', () => {
    // Simulates: CDI elem:0 was a spacer (not pushed), CDI elem:1 was pushed → children[0]
    // has path ending in "elem:1".
    const leaf = makeTestLeaf(['seg:0', 'elem:1'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [leaf] }],
    });
    // Before fix: children[1] → undefined → null. After fix: path-match finds children[0].
    nodeTreeStore.updateLeafValue(NODE_ID, ['seg:0', 'elem:1'], { type: 'int', value: 77 });
    expect(nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0]).toMatchObject({
      kind: 'leaf', value: { type: 'int', value: 77 },
    });
  });

  it('spacer before replicated instance — finds elem:1#2 at correct child by path matching', () => {
    // Wrapper group at CDI index 1 (spacer skipped at 0), containing two instances.
    const inst1 = makeTestGroup(['seg:0', 'elem:1#1'], [makeTestLeaf(['seg:0', 'elem:1#1', 'elem:0'], 10)]);
    const inst2 = makeTestGroup(['seg:0', 'elem:1#2'], [makeTestLeaf(['seg:0', 'elem:1#2', 'elem:0'], 11)]);
    // Wrapper has path ending in "elem:1" (the base without instance suffix)
    const wrapper = makeTestGroup(['seg:0', 'elem:1'], [inst1, inst2]);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [wrapper] }],
    });
    // Path targets leaf inside instance 2 of the replicated group
    nodeTreeStore.updateLeafValue(NODE_ID, ['seg:0', 'elem:1#2', 'elem:0'], { type: 'int', value: 55 });
    const tree = nodeTreeStore.getTree(NODE_ID)!;
    const wrapperNode = tree.segments[0].children[0] as GroupConfigNode;
    const inst2Node = wrapperNode.children[1] as GroupConfigNode;
    expect(inst2Node.children[0]).toMatchObject({ kind: 'leaf', value: { type: 'int', value: 55 } });
  });
});

// ─── Offline mode: setLeafModifiedValue and clearAllModifiedValues ─────────
//
// These helpers support offline mode dirty tracking without IPC.
// setLeafModifiedValue() marks a leaf as having in-memory edits (for parent indicators).
// clearAllModifiedValues() resets all trees after save completes.

describe('setLeafModifiedValue — offline dirty marker', () => {
  it('sets modifiedValue on a leaf without IPC', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [leaf] }],
    });

    // Initially no modifiedValue (should be null or undefined)
    const leafBefore = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0];
    expect(leafBefore.modifiedValue).toBeFalsy();

    // Set a modifiedValue (e.g. user edited this field in offline mode)
    const modVal: TreeConfigValue = { type: 'int', value: 99 };
    nodeTreeStore.setLeafModifiedValue(NODE_ID, ['seg:0', 'elem:0'], modVal);

    // Verify modifiedValue is set and tree is still accessible
    const updatedLeaf = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0];
    expect(updatedLeaf).toMatchObject({
      kind: 'leaf',
      modifiedValue: modVal,
    });
  });

  it('clears modifiedValue when passed null', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [leaf] }],
    });

    // Set modifiedValue
    nodeTreeStore.setLeafModifiedValue(NODE_ID, ['seg:0', 'elem:0'], { type: 'int', value: 99 });
    expect(nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0].modifiedValue).toBeDefined();

    // Clear it
    nodeTreeStore.setLeafModifiedValue(NODE_ID, ['seg:0', 'elem:0'], null);

    // Verify it's cleared and writeState/writeError are also cleared
    const updatedLeaf = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0];
    expect(updatedLeaf.modifiedValue).toBeNull();
    expect(updatedLeaf.writeState).toBeNull();
    expect(updatedLeaf.writeError).toBeNull();
  });

  it('handles nested paths with groups', () => {
    const innerLeaf = makeTestLeaf(['seg:0', 'elem:1#1', 'elem:0'], 10);
    const inst = makeTestGroup(['seg:0', 'elem:1#1'], [innerLeaf]);
    const wrapper = makeTestGroup(['seg:0', 'elem:1'], [inst]);

    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [wrapper] }],
    });

    // Set modifiedValue on nested leaf
    nodeTreeStore.setLeafModifiedValue(NODE_ID, ['seg:0', 'elem:1#1', 'elem:0'], { type: 'int', value: 50 });

    const tree = nodeTreeStore.getTree(NODE_ID)!;
    const wrapperNode = tree.segments[0].children[0] as GroupConfigNode;
    const instNode = wrapperNode.children[0] as GroupConfigNode;
    const targetLeaf = instNode.children[0];

    expect(targetLeaf).toMatchObject({
      kind: 'leaf',
      modifiedValue: { type: 'int', value: 50 },
    });
  });
});

// ─── Offline pending: applyOfflinePendingValues ────────────────────────────

import type { OfflineChangeRow } from '$lib/api/sync';

function makePendingRow(overrides: Partial<OfflineChangeRow> = {}): OfflineChangeRow {
  return {
    changeId: 'row-1',
    kind: 'config',
    nodeId: NODE_ID,
    space: 253,
    offset: '0x00000000',
    baselineValue: '3',
    plannedValue: '5',
    status: 'pending',
    ...overrides,
  };
}

describe('applyOfflinePendingValues', () => {
  it('sets modifiedValue and isOfflinePending on a matching leaf (int)', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'Config', description: null, origin: 0, space: 253, children: [leaf] }],
    });

    nodeTreeStore.applyOfflinePendingValues([makePendingRow()]);

    const updated = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0] as LeafConfigNode;
    expect(updated.modifiedValue).toEqual({ type: 'int', value: 5 });
    expect(updated.isOfflinePending).toBe(true);
  });

  it('does not modify leaves when no row matches address', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 4); // address 4, row targets 0x00000000
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'Config', description: null, origin: 0, space: 253, children: [leaf] }],
    });

    nodeTreeStore.applyOfflinePendingValues([makePendingRow()]);

    const updated = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0] as LeafConfigNode;
    expect(updated.modifiedValue).toBeFalsy();
    expect(updated.isOfflinePending).toBeFalsy();
  });

  it('skips rows with status !== pending', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'Config', description: null, origin: 0, space: 253, children: [leaf] }],
    });

    nodeTreeStore.applyOfflinePendingValues([makePendingRow({ status: 'applied' as any })]);

    const updated = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0] as LeafConfigNode;
    expect(updated.modifiedValue).toBeFalsy();
    expect(updated.isOfflinePending).toBeFalsy();
  });

  it('does not modify trees for nodes not in the store', () => {
    // Store is empty (reset in beforeEach) — should not throw
    expect(() => {
      nodeTreeStore.applyOfflinePendingValues([makePendingRow()]);
    }).not.toThrow();
  });

  it('matches canonical row nodeId to dotted tree nodeId', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'Config', description: null, origin: 0, space: 253, children: [leaf] }],
    });

    nodeTreeStore.applyOfflinePendingValues([
      makePendingRow({ nodeId: '050201000000' }),
    ]);

    const updated = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0] as LeafConfigNode;
    expect(updated.modifiedValue).toEqual({ type: 'int', value: 5 });
    expect(updated.isOfflinePending).toBe(true);
  });

  it('sets modifiedValue as string for non-numeric plannedValue', () => {
    const leaf = makeTestLeaf(['seg:0', 'elem:0'], 0);
    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'Config', description: null, origin: 0, space: 253, children: [leaf] }],
    });

    nodeTreeStore.applyOfflinePendingValues([makePendingRow({ plannedValue: 'Hello' })]);

    const updated = nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0] as LeafConfigNode;
    expect(updated.modifiedValue).toEqual({ type: 'string', value: 'Hello' });
    expect(updated.isOfflinePending).toBe(true);
  });
});

describe('clearAllModifiedValues — offline save cleanup', () => {
  it('clears modifiedValue from all trees across all nodes', () => {
    const NODE_ID_2 = '06.02.01.00.00.00';

    // Set up two trees with modified leaves
    const leaf1 = makeTestLeaf(['seg:0', 'elem:0'], 0);
    const leaf2 = makeTestLeaf(['seg:0', 'elem:0'], 0);

    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [leaf1] }],
    });
    nodeTreeStore.setTree(NODE_ID_2, {
      nodeId: NODE_ID_2, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [leaf2] }],
    });

    // Set modifiedValue on both
    nodeTreeStore.setLeafModifiedValue(NODE_ID, ['seg:0', 'elem:0'], { type: 'int', value: 99 });
    nodeTreeStore.setLeafModifiedValue(NODE_ID_2, ['seg:0', 'elem:0'], { type: 'string', value: 'test' });

    expect(nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0].modifiedValue).toBeDefined();
    expect(nodeTreeStore.getTree(NODE_ID_2)!.segments[0].children[0].modifiedValue).toBeDefined();

    // Clear all
    nodeTreeStore.clearAllModifiedValues();

    // Verify both are cleared
    expect(nodeTreeStore.getTree(NODE_ID)!.segments[0].children[0].modifiedValue).toBeNull();
    expect(nodeTreeStore.getTree(NODE_ID_2)!.segments[0].children[0].modifiedValue).toBeNull();
  });

  it('handles deeply nested structures when clearing', () => {
    const innerLeaf = makeTestLeaf(['seg:0', 'elem:1#1', 'elem:0'], 10);
    const inst = makeTestGroup(['seg:0', 'elem:1#1'], [innerLeaf]);
    const wrapper = makeTestGroup(['seg:0', 'elem:1'], [inst]);

    nodeTreeStore.setTree(NODE_ID, {
      nodeId: NODE_ID, identity: null,
      segments: [{ name: 'S', description: null, origin: 0, space: 253, children: [wrapper] }],
    });

    // Set modifiedValue on nested leaf
    nodeTreeStore.setLeafModifiedValue(NODE_ID, ['seg:0', 'elem:1#1', 'elem:0'], { type: 'int', value: 50 });

    const tree1Before = nodeTreeStore.getTree(NODE_ID)!;
    const wrapperBefore = tree1Before.segments[0].children[0] as GroupConfigNode;
    const instBefore = wrapperBefore.children[0] as GroupConfigNode;
    expect(instBefore.children[0].modifiedValue).toBeDefined();

    // Clear all
    nodeTreeStore.clearAllModifiedValues();

    // Verify nested leaf is cleared
    const tree1After = nodeTreeStore.getTree(NODE_ID)!;
    const wrapperAfter = tree1After.segments[0].children[0] as GroupConfigNode;
    const instAfter = wrapperAfter.children[0] as GroupConfigNode;
    expect(instAfter.children[0].modifiedValue).toBeNull();
  });
});
