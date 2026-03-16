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
