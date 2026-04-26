import { describe, expect, it, vi } from 'vitest';
import type { DiscoveredNode, ProtocolFlags, SNIPData } from '$lib/api/tauri';
import {
  createWaitingNodeReadStates,
  executeConfigReadCandidates,
  type FailedCdiPreflightNode,
  formatCdiPreflightFailureMessage,
  getUnreadConfigEligibleNodes,
  partitionNodesByCdiAvailability,
  pipConfirmsNoCdi,
  resolveConfigReadPreflight,
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
    expect(result.failedNodes).toEqual([]);
  });

  it('treats only CdiNotRetrieved lookup errors as missing CDI for the prompt', async () => {
    const node = makeNode();

    const result = await partitionNodesByCdiAvailability(
      [node],
      async () => {
        throw 'CdiNotRetrieved: cache miss';
      },
    );

    expect(result.nodesWithCdi).toEqual(new Set());
    expect(result.missingNodes).toEqual([
      {
        nodeId: '02.01.57.00.00.01',
        nodeName: 'East Panel',
      },
    ]);
    expect(result.failedNodes).toEqual([]);
  });

  it('returns non-downloadable CDI lookup errors separately', async () => {
    const node = makeNode();

    const result = await partitionNodesByCdiAvailability(
      [node],
      async () => {
        throw 'RetrievalFailed: timed out';
      },
    );

    expect(result.nodesWithCdi).toEqual(new Set());
    expect(result.missingNodes).toEqual([]);
    expect(result.failedNodes).toEqual<FailedCdiPreflightNode[]>([
      {
        nodeId: '02.01.57.00.00.01',
        nodeName: 'East Panel',
        reason: 'CDI retrieval failed. Check node connection and try again.',
      },
    ]);
  });
});

describe('formatCdiPreflightFailureMessage', () => {
  it('formats a specific message for a single failed node', () => {
    expect(formatCdiPreflightFailureMessage([
      {
        nodeId: '02.01.57.00.00.01',
        nodeName: 'East Panel',
        reason: 'CDI retrieval failed. Check node connection and try again.',
      },
    ], 'Cannot read configuration')).toBe(
      'Cannot read configuration for East Panel: CDI retrieval failed. Check node connection and try again.',
    );
  });

  it('formats node-specific details for multiple failures', () => {
    expect(formatCdiPreflightFailureMessage([
      {
        nodeId: '02.01.57.00.00.01',
        nodeName: 'East Panel',
        reason: 'CDI retrieval failed. Check node connection and try again.',
      },
      {
        nodeId: '02.01.57.00.00.02',
        nodeName: 'West Panel',
        reason: 'Configuration not supported by this node.',
      },
    ], 'Cannot read configuration')).toBe(
      'Cannot read configuration for 2 nodes: East Panel: CDI retrieval failed. Check node connection and try again. | West Panel: Configuration not supported by this node.',
    );
  });
});

describe('createWaitingNodeReadStates', () => {
  it('builds waiting progress entries from node candidates', () => {
    expect(createWaitingNodeReadStates([
      { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
      { nodeId: '02.01.57.00.00.02', nodeName: 'Yard Node' },
    ])).toEqual([
      { nodeId: '02.01.57.00.00.01', name: 'East Panel', percentage: 0, status: 'waiting' },
      { nodeId: '02.01.57.00.00.02', name: 'Yard Node', percentage: 0, status: 'waiting' },
    ]);
  });
});

describe('executeConfigReadCandidates', () => {
  it('marks successful nodes as complete, reloads trees, and records nodes as read', async () => {
    const setNodeReadStates = vi.fn();
    const reloadTree = vi.fn(async () => null);
    const markNodeConfigRead = vi.fn();

    const result = await executeConfigReadCandidates({
      nodes: [
        { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
        { nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' },
      ],
      markNodeConfigRead,
      readAllConfigValues: vi.fn(async (nodeId) => ({
        abortError: null,
        durationMs: 10,
        failedReads: 0,
        nodeId,
        successfulReads: 2,
        totalElements: 2,
        values: {},
      })),
      reloadTree,
      setNodeReadStates,
      warn: vi.fn(),
    });

    expect(result).toEqual({
      failures: [],
      nodeReadStates: [
        { nodeId: '02.01.57.00.00.01', name: 'East Panel', percentage: 100, status: 'complete' },
        { nodeId: '02.01.57.00.00.02', name: 'West Panel', percentage: 100, status: 'complete' },
      ],
    });
    expect(markNodeConfigRead).toHaveBeenCalledTimes(2);
    expect(reloadTree).toHaveBeenNthCalledWith(1, '02.01.57.00.00.01');
    expect(reloadTree).toHaveBeenNthCalledWith(2, '02.01.57.00.00.02');
    expect(setNodeReadStates).toHaveBeenCalled();
  });

  it('marks no-cdi and failed nodes without marking them read', async () => {
    const warn = vi.fn();

    const result = await executeConfigReadCandidates({
      nodes: [
        { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
        { nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' },
        { nodeId: '02.01.57.00.00.03', nodeName: 'South Panel' },
      ],
      hasCachedCdi: vi.fn(async (nodeId) => nodeId !== '02.01.57.00.00.01'),
      markNodeConfigRead: vi.fn(),
      readAllConfigValues: vi.fn(async (nodeId) => {
        if (nodeId === '02.01.57.00.00.02') {
          return {
            abortError: 'timed out',
            durationMs: 10,
            failedReads: 0,
            nodeId,
            successfulReads: 0,
            totalElements: 2,
            values: {},
          };
        }

        return {
          abortError: null,
          durationMs: 10,
          failedReads: 1,
          nodeId,
          successfulReads: 1,
          totalElements: 2,
          values: {},
        };
      }),
      reloadTree: vi.fn(async () => null),
      setNodeReadStates: vi.fn(),
      warn,
    });

    expect(result).toEqual({
      failures: [
        { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel', status: 'no-cdi' },
        { error: 'timed out', nodeId: '02.01.57.00.00.02', nodeName: 'West Panel', status: 'failed' },
        {
          error: '1/2 elements failed',
          nodeId: '02.01.57.00.00.03',
          nodeName: 'South Panel',
          status: 'failed',
        },
      ],
      nodeReadStates: [
        { nodeId: '02.01.57.00.00.01', name: 'East Panel', percentage: 0, status: 'no-cdi' },
        { nodeId: '02.01.57.00.00.02', name: 'West Panel', percentage: 0, status: 'failed' },
        { nodeId: '02.01.57.00.00.03', name: 'South Panel', percentage: 100, status: 'complete' },
      ],
    });
    expect(warn).toHaveBeenCalled();
  });
});

describe('resolveConfigReadPreflight', () => {
  it('returns pending nodes without failures when all candidates can proceed', async () => {
    const node = makeNode();

    const result = await resolveConfigReadPreflight(
      [node],
      async () => true,
      'Cannot read configuration',
    );

    expect(result.failureMessage).toBeNull();
    expect(result.failedNodeIds).toEqual(new Set());
    expect(result.nodesWithCdi).toEqual(new Set(['02.01.57.00.00.01']));
    expect(result.missingNodes).toEqual([]);
    expect(result.pendingNodes).toEqual([
      { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
    ]);
  });

  it('drops failed nodes from the pending set while preserving missing CDI prompts', async () => {
    const okNode = makeNode();
    const missingNode = makeNode({
      node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x02],
      snip_data: makeSnipData({ user_name: 'West Panel' }),
    });
    const failedNode = makeNode({
      node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x03],
      snip_data: makeSnipData({ user_name: 'South Panel' }),
    });

    const result = await resolveConfigReadPreflight(
      [okNode, missingNode, failedNode],
      async (nodeId) => {
        if (nodeId === '02.01.57.00.00.01') return true;
        if (nodeId === '02.01.57.00.00.02') return false;
        throw 'RetrievalFailed: timed out';
      },
      'Cannot read configuration',
    );

    expect(result.failureMessage).toBe(
      'Cannot read configuration for South Panel: CDI retrieval failed. Check node connection and try again.',
    );
    expect(result.failedNodeIds).toEqual(new Set(['02.01.57.00.00.03']));
    expect(result.nodesWithCdi).toEqual(new Set(['02.01.57.00.00.01']));
    expect(result.missingNodes).toEqual([
      { nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' },
    ]);
    expect(result.pendingNodes).toEqual([
      { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
      { nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' },
    ]);
  });
});