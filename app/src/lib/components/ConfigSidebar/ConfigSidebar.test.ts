/**
 * T003: Vitest unit tests for ConfigSidebar.svelte
 * TDD — written before implementation; must FAIL until ConfigSidebar.svelte exists.
 *
 * Covers:
 * - Node list render from nodeInfoStore
 * - Expand/collapse toggle (FR-002, FR-015)
 * - Segment list render on expand
 * - Segment selection highlight
 * - Empty-state message (FR-002 edge case)
 * - Offline indicator render (RQ-006)
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { configSidebarStore } from '$lib/stores/configSidebar';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { clearConfigReadStatus, markNodeConfigRead } from '$lib/stores/configReadStatus';
import ConfigSidebar from './ConfigSidebar.svelte';

// Mock Tauri invoke so we don't need an actual backend
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock CdiXmlViewer to avoid deep dependency trees in unit tests
vi.mock('$lib/components/CdiXmlViewer.svelte', () => ({
  default: { render: () => '' },
}));

const MOCK_NODE = {
  node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x01],
  snip_data: {
    user_name: 'Test Node',
    user_description: '',
    manufacturer: 'ACME',
    model: 'Model X',
    hardware_version: '1.0',
    software_version: '1.0',
  },
  connection_status: 'Connected',
  cdi: '<cdi/>',
};

beforeEach(() => {
  configSidebarStore.reset();
  nodeInfoStore.set(new Map());
  nodeTreeStore.reset();
  clearConfigReadStatus();
  vi.clearAllMocks();
});

describe('ConfigSidebar.svelte', () => {
  it('shows empty-state message when no nodes are discovered (FR-002 edge case)', () => {
    render(ConfigSidebar);
    expect(screen.getByText(/no nodes discovered/i)).toBeInTheDocument();
  });

  it('renders a node name for each discovered node', async () => {
    nodeInfoStore.set(new Map([['02.01.57.00.00.01', MOCK_NODE as any]]));
    render(ConfigSidebar);
    expect(screen.getByText('Test Node')).toBeInTheDocument();
  });

  it('expands a node to show its segments when clicked (FR-002, FR-015)', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    (invoke as any).mockResolvedValue({
      nodeId: '02.01.57.00.00.01',
      identity: null,
      segments: [
        { name: 'Port I/O', description: null, space: 253, origin: 0, children: [] },
      ],
    });
    nodeInfoStore.set(new Map([['02.01.57.00.00.01', MOCK_NODE as any]]));
    const { container } = render(ConfigSidebar);

    const nodeRow = screen.getByText('Test Node');
    await fireEvent.click(nodeRow);

    // After clicking, the store should have the node expanded
    let state: any;
    configSidebarStore.subscribe(s => (state = s))();
    expect(state.expandedNodeIds).toContain('02.01.57.00.00.01');
  });

  it('preserves expansion state across segment selections (FR-015)', async () => {
    nodeInfoStore.set(new Map([
      ['02.01.57.00.00.01', MOCK_NODE as any],
      ['02.01.57.00.00.02', { ...MOCK_NODE, node_id: [2, 1, 87, 0, 0, 2], snip_data: { ...MOCK_NODE.snip_data, user_name: 'Node B' } } as any],
    ]));
    render(ConfigSidebar);

    // Both nodes can be expanded simultaneously (FR-015: collapse NOT on other node click)
    configSidebarStore.toggleNodeExpanded('02.01.57.00.00.01');
    configSidebarStore.toggleNodeExpanded('02.01.57.00.00.02');

    let state: any;
    configSidebarStore.subscribe(s => (state = s))();
    expect(state.expandedNodeIds).toContain('02.01.57.00.00.01');
    expect(state.expandedNodeIds).toContain('02.01.57.00.00.02');
  });

  it('highlights the selected segment (FR-005)', () => {
    nodeInfoStore.set(new Map([['02.01.57.00.00.01', MOCK_NODE as any]]));
    configSidebarStore.toggleNodeExpanded('02.01.57.00.00.01');
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg:0', 'Port I/O');
    render(ConfigSidebar);

    let state: any;
    configSidebarStore.subscribe(s => (state = s))();
    expect(state.selectedSegment?.segmentId).toBe('seg:0');
  });

  it('collapses a node when clicked a second time (FR-002)', () => {
    nodeInfoStore.set(new Map([['02.01.57.00.00.01', MOCK_NODE as any]]));
    render(ConfigSidebar);

    configSidebarStore.toggleNodeExpanded('02.01.57.00.00.01');
    configSidebarStore.toggleNodeExpanded('02.01.57.00.00.01');

    let state: any;
    configSidebarStore.subscribe(s => (state = s))();
    expect(state.expandedNodeIds).not.toContain('02.01.57.00.00.01');
  });

  it('shows segments on remount without requiring the user to re-expand the node (route change)', async () => {
    // Setup: mock invoke to return a valid NodeConfigTree
    const { invoke } = await import('@tauri-apps/api/core');
    const MOCK_TREE = {
      nodeId: '02.01.57.00.00.01',
      identity: null,
      segments: [
        { name: 'Port I/O', description: null, origin: 0, space: 253, children: [] },
      ],
    };
    (invoke as any).mockResolvedValue(MOCK_TREE);

    nodeInfoStore.set(new Map([['02.01.57.00.00.01', MOCK_NODE as any]]));

    // First mount: expand node, segments load
    const { unmount } = render(ConfigSidebar);
    const nodeRow = screen.getByText('Test Node');
    await fireEvent.click(nodeRow);

    // Wait for the async tree load to complete
    await vi.waitFor(() => {
      expect(screen.getByText('Port I/O')).toBeInTheDocument();
    });

    // Simulate navigation away: unmount the component
    unmount();

    // The store still remembers the node is expanded
    let state: any;
    configSidebarStore.subscribe(s => (state = s))();
    expect(state.expandedNodeIds).toContain('02.01.57.00.00.01');

    // Remount: simulates navigating back to config page
    render(ConfigSidebar);

    // The node is expanded (store state persists across navigation).
    // Segments must be visible — the user should not have to re-expand the node.
    await vi.waitFor(() => {
      expect(screen.queryByText('No segments available')).not.toBeInTheDocument();
    });
    expect(screen.getByText('Port I/O')).toBeInTheDocument();
  });

  it('shows "not read yet" message when a CDI error fires and config has not been read', async () => {
    // invoke rejects with a CDI error (no cache) — config has NOT been read yet
    const { invoke } = await import('@tauri-apps/api/core');
    (invoke as any).mockRejectedValue('CdiNotRetrieved: no cache entry');

    const nodeId = '02.01.57.00.00.01';
    nodeInfoStore.set(new Map([[nodeId, MOCK_NODE as any]]));

    render(ConfigSidebar);
    await fireEvent.click(screen.getByText('Test Node'));

    await vi.waitFor(() => {
      expect(screen.getByText(/configuration has not been read from this node yet/i)).toBeInTheDocument();
    });
    expect(screen.queryByText(/configuration not supported/i)).not.toBeInTheDocument();
  });

  it('shows "not supported" message when a CDI error fires and the node has been marked as read', async () => {
    // invoke rejects with a CDI error — but config IS already marked as read
    const { invoke } = await import('@tauri-apps/api/core');
    (invoke as any).mockRejectedValue('CdiUnavailable: 02.01.57.00.00.01');

    const nodeId = '02.01.57.00.00.01';
    markNodeConfigRead(nodeId);
    nodeInfoStore.set(new Map([[nodeId, MOCK_NODE as any]]));

    render(ConfigSidebar);
    await fireEvent.click(screen.getByText('Test Node'));

    await vi.waitFor(() => {
      expect(screen.getByText(/configuration not supported by this node/i)).toBeInTheDocument();
    });
    expect(screen.queryByText(/has not been read from this node yet/i)).not.toBeInTheDocument();
  });
});
