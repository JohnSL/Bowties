import { describe, expect, it, vi } from 'vitest';
import type {
  DiscoveredNode,
  ProtocolFlags,
  QueryPipResponse,
  QuerySnipResponse,
  SNIPData,
} from '$lib/api/tauri';
import {
  handleDiscoveredNode,
  refreshReinitializedNode,
} from './discoveryOrchestrator';

function makeSnipData(overrides: Partial<SNIPData> = {}): SNIPData {
  return {
    manufacturer: 'Acme',
    model: 'Node',
    hardware_version: '1',
    software_version: '1.0',
    user_name: 'Test Node',
    user_description: 'Test Description',
    ...overrides,
  };
}

function makePipFlags(overrides: Partial<ProtocolFlags> = {}): ProtocolFlags {
  return {
    simple_protocol: true,
    datagram: true,
    stream: false,
    memory_configuration: true,
    reservation: false,
    event_exchange: true,
    identification: true,
    teach_learn: false,
    remote_button: false,
    acdi: true,
    display: false,
    snip: true,
    cdi: true,
    traction_control: false,
    function_description_information: false,
    dcc_command_station: false,
    simple_train_node: false,
    function_configuration: false,
    firmware_upgrade: false,
    firmware_upgrade_active: false,
    ...overrides,
  };
}

function makeNode(overrides: Partial<DiscoveredNode> = {}): DiscoveredNode {
  return {
    node_id: [0x05, 0x02, 0x01, 0x02, 0x03, 0x00],
    alias: 0x701,
    snip_data: makeSnipData(),
    snip_status: 'Complete',
    connection_status: 'Connected',
    last_verified: null,
    last_seen: '2026-04-21T00:00:00.000Z',
    cdi: { xml_content: '<cdi/>', retrieved_at: '2026-04-21T00:00:00.000Z' },
    pip_flags: makePipFlags(),
    pip_status: 'Complete',
    ...overrides,
  };
}

describe('handleDiscoveredNode', () => {
  it('rebases async discovery enrichment onto the latest published node list', async () => {
    let liveNodes: DiscoveredNode[] = [];
    let resolveFirstSnip: ((value: QuerySnipResponse) => void) | null = null;
    let resolveSecondSnip: ((value: QuerySnipResponse) => void) | null = null;

    const firstSnip = new Promise<QuerySnipResponse>((resolve) => {
      resolveFirstSnip = resolve;
    });
    const secondSnip = new Promise<QuerySnipResponse>((resolve) => {
      resolveSecondSnip = resolve;
    });

    const querySnip = vi.fn((alias: number): Promise<QuerySnipResponse> => {
      if (alias === 0x101) return firstSnip;
      if (alias === 0x202) return secondSnip;
      throw new Error(`Unexpected alias ${alias}`);
    });

    const queryPip = vi.fn(async (alias: number): Promise<QueryPipResponse> => ({
      alias,
      pip_flags: makePipFlags(),
      status: 'Complete',
    }));

    const publishNodes = vi.fn((nextNodes: DiscoveredNode[]) => {
      liveNodes = nextNodes;
    });

    const firstDiscovery = handleDiscoveredNode({
      currentNodes: liveNodes,
      getCurrentNodes: () => liveNodes,
      nodeId: '05.02.01.02.03.01',
      alias: 0x101,
      registerNode: vi.fn(async () => {}),
      querySnip,
      queryPip,
      publishNodes,
      now: () => '2026-04-25T12:00:00.000Z',
    });

    const secondDiscovery = handleDiscoveredNode({
      currentNodes: liveNodes,
      getCurrentNodes: () => liveNodes,
      nodeId: '05.02.01.02.03.02',
      alias: 0x202,
      registerNode: vi.fn(async () => {}),
      querySnip,
      queryPip,
      publishNodes,
      now: () => '2026-04-25T12:00:01.000Z',
    });

    resolveSecondSnip?.({
      alias: 0x202,
      snip_data: makeSnipData({ user_name: 'Node Two' }),
      status: 'Complete',
    });
    await secondDiscovery;

    resolveFirstSnip?.({
      alias: 0x101,
      snip_data: makeSnipData({ user_name: 'Node One' }),
      status: 'Complete',
    });
    await firstDiscovery;

    expect(liveNodes).toHaveLength(2);
    expect(liveNodes.map((node) => node.alias)).toEqual([0x101, 0x202]);
    expect(liveNodes.map((node) => node.snip_data?.user_name)).toEqual(['Node One', 'Node Two']);
  });

  it('skips true duplicates without registering or querying again', async () => {
    const registerNode = vi.fn(async () => {});
    const querySnip = vi.fn(async (): Promise<QuerySnipResponse> => ({ alias: 0x123, snip_data: null, status: 'Unknown' }));
    const queryPip = vi.fn(async (): Promise<QueryPipResponse> => ({ alias: 0x123, pip_flags: null, status: 'Unknown' }));
    const publishNodes = vi.fn();
    const existing = makeNode({ alias: 0x123 });

    const result = await handleDiscoveredNode({
      currentNodes: [existing],
      nodeId: '05.02.01.02.03.00',
      alias: 0x123,
      registerNode,
      querySnip,
      queryPip,
      publishNodes,
    });

    expect(result.skipped).toBe(true);
    expect(result.nodes).toEqual([existing]);
    expect(registerNode).not.toHaveBeenCalled();
    expect(querySnip).not.toHaveBeenCalled();
    expect(queryPip).not.toHaveBeenCalled();
    expect(publishNodes).not.toHaveBeenCalled();
  });

  it('retries SNIP and PIP enrichment when a rediscovered node still has incomplete live data', async () => {
    const registerNode = vi.fn(async () => {});
    const querySnip = vi.fn(async (): Promise<QuerySnipResponse> => ({
      alias: 0x123,
      snip_data: makeSnipData({ user_name: 'Recovered Node' }),
      status: 'Complete',
    }));
    const queryPip = vi.fn(async (): Promise<QueryPipResponse> => ({
      alias: 0x123,
      pip_flags: makePipFlags(),
      status: 'Complete',
    }));
    const publishNodes = vi.fn();
    const existing = makeNode({
      alias: 0x123,
      snip_data: null,
      snip_status: 'Unknown',
      pip_flags: null,
      pip_status: 'Unknown',
    });

    const result = await handleDiscoveredNode({
      currentNodes: [existing],
      nodeId: '05.02.01.02.03.00',
      alias: 0x123,
      registerNode,
      querySnip,
      queryPip,
      publishNodes,
      now: () => '2026-04-25T10:00:00.000Z',
    });

    expect(result.skipped).toBe(false);
    expect(registerNode).toHaveBeenCalledWith('05.02.01.02.03.00', 0x123);
    expect(querySnip).toHaveBeenCalledWith(0x123);
    expect(queryPip).toHaveBeenCalledWith(0x123);
    expect(result.nodes[0]).toMatchObject({
      alias: 0x123,
      snip_status: 'Complete',
      pip_status: 'Complete',
    });
  });

  it('upgrades an offline skeleton with the live alias and preserves offline fallback data', async () => {
    const registerNode = vi.fn(async () => {});
    const querySnip = vi.fn(async (): Promise<QuerySnipResponse> => ({
      alias: 0x222,
      snip_data: null,
      status: 'Timeout',
    }));
    const queryPip = vi.fn(async (): Promise<QueryPipResponse> => ({
      alias: 0x222,
      pip_flags: null,
      status: 'Timeout',
    }));
    const publishNodes = vi.fn();
    const offlineFallback = makeNode({
      alias: 0x701,
      connection_status: 'Unknown',
      snip_status: 'Unknown',
      pip_status: 'Unknown',
      last_seen: '2026-04-20T00:00:00.000Z',
    });

    const result = await handleDiscoveredNode({
      currentNodes: [offlineFallback],
      nodeId: '05.02.01.02.03.00',
      alias: 0x222,
      registerNode,
      querySnip,
      queryPip,
      publishNodes,
      now: () => '2026-04-21T12:00:00.000Z',
    });

    expect(registerNode).toHaveBeenCalledWith('05.02.01.02.03.00', 0x222);
    expect(querySnip).toHaveBeenCalledWith(0x222);
    expect(queryPip).toHaveBeenCalledWith(0x222);
    expect(publishNodes).toHaveBeenCalledTimes(2);
    expect(result.skipped).toBe(false);
    expect(result.nodes[0]).toMatchObject({
      alias: 0x222,
      connection_status: 'Connected',
      last_seen: '2026-04-21T12:00:00.000Z',
      snip_data: offlineFallback.snip_data,
      snip_status: 'Timeout',
      pip_flags: offlineFallback.pip_flags,
      pip_status: 'Timeout',
    });
  });

  it('adds a new skeleton immediately and merges live SNIP and PIP results', async () => {
    const registerNode = vi.fn(async () => {});
    const publishNodes = vi.fn();
    const snipData = makeSnipData({ user_name: 'Live Node' });
    const pipFlags = makePipFlags({ firmware_upgrade: true });

    const result = await handleDiscoveredNode({
      currentNodes: [],
      nodeId: '05.02.01.02.03.00',
      alias: 0x321,
      registerNode,
      querySnip: vi.fn(async (): Promise<QuerySnipResponse> => ({ alias: 0x321, snip_data: snipData, status: 'Complete' })),
      queryPip: vi.fn(async (): Promise<QueryPipResponse> => ({ alias: 0x321, pip_flags: pipFlags, status: 'Complete' })),
      publishNodes,
      now: () => '2026-04-21T13:00:00.000Z',
    });

    expect(publishNodes).toHaveBeenCalledTimes(2);
    expect(result.nodes).toHaveLength(1);
    expect(result.nodes[0]).toMatchObject({
      node_id: [0x05, 0x02, 0x01, 0x02, 0x03, 0x00],
      alias: 0x321,
      snip_data: snipData,
      snip_status: 'Complete',
      pip_flags: pipFlags,
      pip_status: 'Complete',
      last_seen: '2026-04-21T13:00:00.000Z',
    });
  });

  it('keeps the upgraded node visible when registration or query work fails', async () => {
    const error = new Error('network failed');
    const warn = vi.fn();
    const publishNodes = vi.fn();

    const result = await handleDiscoveredNode({
      currentNodes: [],
      nodeId: '05.02.01.02.03.00',
      alias: 0x444,
      registerNode: vi.fn(async () => {
        throw error;
      }),
      querySnip: vi.fn(async (): Promise<QuerySnipResponse> => ({ alias: 0x444, snip_data: null, status: 'Unknown' })),
      queryPip: vi.fn(async (): Promise<QueryPipResponse> => ({ alias: 0x444, pip_flags: null, status: 'Unknown' })),
      publishNodes,
      now: () => '2026-04-21T14:00:00.000Z',
      warn,
    });

    expect(result.skipped).toBe(false);
    expect(result.nodes).toHaveLength(1);
    expect(result.nodes[0]).toMatchObject({
      alias: 0x444,
      snip_data: null,
      pip_flags: null,
      last_seen: '2026-04-21T14:00:00.000Z',
    });
    expect(publishNodes).toHaveBeenCalledTimes(1);
    expect(warn).toHaveBeenCalledWith('Failed to query node 05.02.01.02.03.00:', error);
  });
});

describe('refreshReinitializedNode', () => {
  it('refreshes a known node and invalidates cached CDI', async () => {
    const publishNodes = vi.fn();
    const refreshedSnip = makeSnipData({ user_name: 'Reinitialized Node' });
    const refreshedPip = makePipFlags({ simple_train_node: true });

    const result = await refreshReinitializedNode({
      currentNodes: [makeNode({ alias: 0x123 })],
      nodeId: '05.02.01.02.03.00',
      alias: 0x555,
      querySnip: vi.fn(async (): Promise<QuerySnipResponse> => ({ alias: 0x555, snip_data: refreshedSnip, status: 'Complete' })),
      queryPip: vi.fn(async (): Promise<QueryPipResponse> => ({ alias: 0x555, pip_flags: refreshedPip, status: 'Complete' })),
      publishNodes,
    });

    expect(result.skipped).toBe(false);
    expect(publishNodes).toHaveBeenCalledTimes(1);
    expect(result.nodes[0]).toMatchObject({
      alias: 0x555,
      snip_data: refreshedSnip,
      pip_flags: refreshedPip,
      cdi: null,
    });
  });

  it('ignores reinitialized events for nodes that are not currently known', async () => {
    const querySnip = vi.fn(async (): Promise<QuerySnipResponse> => ({ alias: 0x666, snip_data: null, status: 'Unknown' }));
    const queryPip = vi.fn(async (): Promise<QueryPipResponse> => ({ alias: 0x666, pip_flags: null, status: 'Unknown' }));
    const publishNodes = vi.fn();

    const result = await refreshReinitializedNode({
      currentNodes: [],
      nodeId: '05.02.01.02.03.00',
      alias: 0x666,
      querySnip,
      queryPip,
      publishNodes,
    });

    expect(result.skipped).toBe(true);
    expect(querySnip).not.toHaveBeenCalled();
    expect(queryPip).not.toHaveBeenCalled();
    expect(publishNodes).not.toHaveBeenCalled();
  });
});