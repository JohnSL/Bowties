import { describe, expect, it, vi } from 'vitest';
import type { DiscoveredNode, ProtocolFlags, SNIPData } from '$lib/api/tauri';
import {
  getUnreadConfigEligibleNodes,
  partitionNodesByCdiAvailability,
  pipConfirmsNoCdi,
  toConfigReadCandidate,
} from './configReadOrchestrator';

function makeSnipData(overrides: Partial<SNIPData> = {}): SNIPData {
  return {
    manufacturer: 'RR-CirKits',
    model: 'Tower-LCC',
    hardware_version: '1',
    software_version: '1.0',
    user_name: 'East Panel',
    user_description: 'Panel note',
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
    node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x01],
    alias: 0x123,
    snip_data: makeSnipData(),
    snip_status: 'Complete',
    connection_status: 'Connected',
    last_verified: null,
    last_seen: '2026-04-25T00:00:00.000Z',
    cdi: null,
    pip_flags: makePipFlags(),
    pip_status: 'Complete',
    ...overrides,
  };
}

describe('pipConfirmsNoCdi', () => {
  it('returns true only when PIP is complete and both CDI capabilities are false', () => {
    expect(pipConfirmsNoCdi(makeNode({
      pip_flags: makePipFlags({ cdi: false, memory_configuration: false }),
    }))).toBe(true);

    expect(pipConfirmsNoCdi(makeNode({
      pip_status: 'Unknown',
      pip_flags: makePipFlags({ cdi: false, memory_configuration: false }),
    }))).toBe(false);

    expect(pipConfirmsNoCdi(makeNode({
      pip_flags: makePipFlags({ cdi: false, memory_configuration: true }),
    }))).toBe(false);
  });
});

describe('getUnreadConfigEligibleNodes', () => {
  it('excludes confirmed CDI-less nodes without blocking other unread eligible nodes', () => {
    const eligible = makeNode({ node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x01] });
    const cdiLess = makeNode({
      node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x02],
      snip_data: makeSnipData({ user_name: 'JMRI' }),
      pip_flags: makePipFlags({ cdi: false, memory_configuration: false }),
    });
    const alreadyRead = makeNode({
      node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x03],
      snip_data: makeSnipData({ user_name: 'Read Node' }),
    });

    const unread = getUnreadConfigEligibleNodes(
      [eligible, cdiLess, alreadyRead],
      new Set(['02.01.57.00.00.03']),
    );

    expect(unread).toHaveLength(1);
    expect(toConfigReadCandidate(unread[0])).toEqual({
      nodeId: '02.01.57.00.00.01',
      nodeName: 'East Panel',
    });
  });

  it('requires SNIP data before a node becomes eligible for config reading', () => {
    const unread = getUnreadConfigEligibleNodes(
      [makeNode({ snip_data: null, snip_status: 'Unknown' })],
      new Set(),
    );

    expect(unread).toHaveLength(0);
  });
});

describe('partitionNodesByCdiAvailability', () => {
  it('returns only eligible nodes in the missing-CDI prompt and preserves friendly fallback names', async () => {
    const manufacturerFallback = makeNode({
      node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x09],
      snip_data: makeSnipData({
        user_name: '   ',
        user_description: 'Ignored note',
        manufacturer: 'RR-CirKits',
        model: 'Tower-LCC',
      }),
    });
    const nodeIdFallback = makeNode({
      node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x0A],
      snip_data: makeSnipData({
        user_name: '',
        manufacturer: '',
        model: '',
      }),
    });

    const hasCachedCdi = vi.fn(async (nodeId: string) => nodeId === '02.01.57.00.00.0A');

    const result = await partitionNodesByCdiAvailability(
      [manufacturerFallback, nodeIdFallback],
      hasCachedCdi,
    );

    expect(result.nodesWithCdi).toEqual(new Set(['02.01.57.00.00.0A']));
    expect(result.missingNodes).toEqual([
      {
        nodeId: '02.01.57.00.00.09',
        nodeName: 'RR-CirKits — Tower-LCC',
      },
    ]);
  });

  it('treats CDI lookup errors as missing CDI for the prompt', async () => {
    const node = makeNode();

    const result = await partitionNodesByCdiAvailability(
      [node],
      async () => {
        throw new Error('cache miss');
      },
    );

    expect(result.nodesWithCdi).toEqual(new Set());
    expect(result.missingNodes).toEqual([
      {
        nodeId: '02.01.57.00.00.01',
        nodeName: 'East Panel',
      },
    ]);
  });
});