/**
 * Tests for ElementPicker.svelte
 *
 * Key behaviour under test:
 *   - On mount, loadTree() is called for every node in nodeInfoStore
 *   - loadTree() is called even when a tree is ALREADY loaded for a node
 *     (regression: previous code skipped already-loaded trees, so pre-profile
 *     stale copies were never refreshed after CDI reads completed)
 *   - Nodes returned in the picker list come from the trees that are loaded
 *   - Search filters by element name / node name
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import ElementPicker from './ElementPicker.svelte';
import type { NodeConfigTree, LeafConfigNode, SegmentNode } from '$lib/types/nodeTree';

// ── Hoisted refs (must be available inside vi.mock factories) ────────────────

const { loadTreeMock, nodesRef, treesRef } = vi.hoisted(() => ({
  loadTreeMock: vi.fn().mockResolvedValue(null),
  nodesRef: { map: new Map<string, any>() },
  treesRef: { map: new Map<string, NodeConfigTree>() },
}));

// ── Module mocks ─────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() { return treesRef.map; },
    getTree(nodeId: string) { return treesRef.map.get(nodeId) ?? undefined; },
    hasTree(nodeId: string) { return treesRef.map.has(nodeId); },
    loadTree: loadTreeMock,
  },
}));

vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    get catalog() { return null; },
  },
}));

vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: {
    classifyRole: vi.fn(),
  },
}));

vi.mock('$lib/stores/nodeInfo', () => ({
  nodeInfoStore: {
    subscribe: (fn: (val: any) => void) => { fn(nodesRef.map); return () => {}; },
  },
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeNodeInfo(nodeId: string, userName = '') {
  return { snip_data: { user_name: userName, manufacturer: 'Acme', model: 'Switch' } };
}

function makeEventIdLeaf(name = 'Event ID', role: string | null = 'Producer'): LeafConfigNode {
  return {
    kind: 'leaf',
    name,
    description: null,
    elementType: 'eventId',
    address: 100,
    size: 8,
    space: 253,
    path: ['seg:0', 'elem:0'],
    value: { type: 'eventId', bytes: [5, 1, 1, 1, 0, 0, 0, 1], hex: '05.01.01.01.00.00.00.01' },
    eventRole: role as any,
    constraints: null,
  };
}

function makeTree(nodeId: string, leafName = 'Event ID'): NodeConfigTree {
  const leaf = makeEventIdLeaf(leafName);
  const seg: SegmentNode = {
    name: 'Configuration', description: null, origin: 0, space: 253, children: [leaf],
  };
  return { nodeId, identity: null, segments: [seg] };
}

const NODE_A = '05.01.01.01.00.00.00.01';
const NODE_B = '05.01.01.01.00.00.00.02';

beforeEach(() => {
  nodesRef.map = new Map();
  treesRef.map = new Map();
  vi.clearAllMocks();
  loadTreeMock.mockResolvedValue(null);
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ElementPicker — onMount tree loading', () => {
  it('calls loadTree for each node in nodeInfoStore', async () => {
    nodesRef.map.set(NODE_A, makeNodeInfo(NODE_A));
    nodesRef.map.set(NODE_B, makeNodeInfo(NODE_B));

    render(ElementPicker);

    // Wait for effects to flush
    await vi.waitFor(() => {
      expect(loadTreeMock).toHaveBeenCalledWith(NODE_A);
      expect(loadTreeMock).toHaveBeenCalledWith(NODE_B);
    });
  });

  it('calls loadTree even when a tree is already loaded (stale-profile refresh)', async () => {
    // Node A already has a tree in the store (simulates post-CDI-but-pre-profile state)
    nodesRef.map.set(NODE_A, makeNodeInfo(NODE_A));
    treesRef.map.set(NODE_A, makeTree(NODE_A));

    render(ElementPicker);

    await vi.waitFor(() => {
      // loadTree must still be called to pick up the profile-annotated version
      expect(loadTreeMock).toHaveBeenCalledWith(NODE_A);
    });
  });

  it('does not call loadTree when no nodes are discovered', async () => {
    // nodeInfoStore is empty
    render(ElementPicker);

    await vi.waitFor(() => {
      expect(loadTreeMock).not.toHaveBeenCalled();
    });
  });
});

describe('ElementPicker — picker content', () => {
  it('shows "No nodes with event slots available" when no trees are loaded', () => {
    nodesRef.map.set(NODE_A, makeNodeInfo(NODE_A, 'Button Node'));
    // treesRef.map is empty — trees not yet loaded
    render(ElementPicker);
    expect(screen.getByText(/no nodes with event slots available/i)).toBeInTheDocument();
  });

  it('shows "No matching event slots found" when search has no results', async () => {
    nodesRef.map.set(NODE_A, makeNodeInfo(NODE_A, 'Button Node'));
    treesRef.map.set(NODE_A, makeTree(NODE_A, 'Event ID'));

    const { getByPlaceholderText } = render(ElementPicker);
    const search = getByPlaceholderText(/search elements/i);

    // Type something that matches nothing
    search.focus();
    await import('@testing-library/svelte').then(async ({ fireEvent }) => {
      await fireEvent.input(search, { target: { value: 'zzz_no_match_zzz' } });
    });

    expect(screen.getByText(/no matching event slots found/i)).toBeInTheDocument();
  });

  it('shows placeholder slot as disabled when event ID has leading-zero byte', async () => {
    nodesRef.map.set(NODE_A, makeNodeInfo(NODE_A, 'Button Node'));

    // Build a leaf whose value is a placeholder (leading-zero event ID)
    const placeholderLeaf: LeafConfigNode = {
      kind: 'leaf',
      name: 'Push Button',
      description: null,
      elementType: 'eventId',
      address: 200,
      size: 8,
      space: 253,
      path: ['seg:0', 'elem:0'],
      value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0xFF], hex: '00.00.00.00.00.00.00.FF' },
      eventRole: 'Producer',
      constraints: null,
    };
    const seg: SegmentNode = {
      name: 'Configuration', description: null, origin: 0, space: 253, children: [placeholderLeaf],
    };
    treesRef.map.set(NODE_A, { nodeId: NODE_A, identity: null, segments: [seg] });

    render(ElementPicker);

    // Expand the node
    const nodeToggle = await screen.findByText('Button Node');
    const { fireEvent: fe } = await import('@testing-library/svelte');
    await fe.click(nodeToggle.closest('button')!);

    // Expand the segment
    const segToggle = screen.getByText('Configuration');
    await fe.click(segToggle.closest('button')!);

    // The slot button should be present but disabled
    const slotBtn = screen.getByRole('button', { name: /push button/i });
    expect(slotBtn).toBeDisabled();
    expect(screen.getByText('(placeholder)')).toBeInTheDocument();
  });
});
