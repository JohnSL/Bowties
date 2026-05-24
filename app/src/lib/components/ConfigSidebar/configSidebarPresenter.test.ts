import { describe, expect, it, vi, beforeEach } from 'vitest';
import type { DiscoveredNode } from '$lib/api/tauri';
import type { OfflineChangeRow } from '$lib/api/sync';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import {
  buildSidebarNodeEntries,
  getNodePendingState,
  getSegmentPendingState,
  shouldShowConfigNotReadBadge,
} from './configSidebarPresenter';

const mockHasDraftsForNode = vi.fn().mockReturnValue(false);
const mockChangeLayers = vi.fn().mockReturnValue([]);

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    hasDraftsForNode: (...args: unknown[]) => mockHasDraftsForNode(...args),
    changeLayers: (...args: unknown[]) => mockChangeLayers(...args),
  },
}));

function makeNode(overrides: Partial<DiscoveredNode> = {}): DiscoveredNode {
  return {
    alias: 0x123,
    cdi: null,
    connection_status: 'Connected',
    last_seen: '2026-04-25T00:00:00.000Z',
    last_verified: null,
    node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x01],
    pip_flags: {
      acdi: true,
      cdi: true,
      datagram: true,
      dcc_command_station: false,
      display: false,
      event_exchange: true,
      firmware_upgrade: false,
      firmware_upgrade_active: false,
      function_configuration: false,
      function_description_information: false,
      identification: true,
      memory_configuration: true,
      remote_button: false,
      reservation: false,
      simple_protocol: true,
      simple_train_node: false,
      snip: true,
      stream: false,
      teach_learn: false,
      traction_control: false,
    },
    pip_status: 'Complete',
    snip_data: {
      hardware_version: '1.0',
      manufacturer: 'ACME',
      model: 'Node-8',
      software_version: '1.0',
      user_description: 'Panel note',
      user_name: 'East Panel',
    },
    snip_status: 'Complete',
    ...overrides,
  };
}

function makeTree(): NodeConfigTree {
  return {
    identity: null,
    nodeId: '02.01.57.00.00.01',
    segments: [
      {
        children: [
          {
            address: 16,
            constraints: null,
            description: null,
            elementType: 'int',
            eventRole: null,
            kind: 'leaf',
            modifiedValue: { type: 'int', value: 12 },
            name: 'Channel',
            path: ['seg:0', 'elem:0'],
            size: 1,
            space: 253,
            value: { type: 'int', value: 10 },
          },
        ],
        description: null,
        name: 'Port I/O',
        origin: 0,
        space: 253,
      },
    ],
  };
}

describe('buildSidebarNodeEntries', () => {
  it('disambiguates duplicate friendly names and includes tooltip details', () => {
    const entries = buildSidebarNodeEntries(new Map([
      ['02.01.57.00.00.01', makeNode()],
      ['02.01.57.00.00.02', makeNode({
        alias: 0x124,
        node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x02],
      })],
    ]));

    expect(entries.map((entry) => entry.nodeName)).toEqual([
      'East Panel (00.01)',
      'East Panel (00.02)',
    ]);
    expect(entries[0].nodeDetail).toBe('ACME Node-8');
    expect(entries[0].nodeTooltip).toContain('Alias: 0x123');
    expect(entries[0].nodeTooltip).toContain('Manufacturer: ACME');
  });

  it('flags discovered nodes that are not in the saved layout as isUnsavedNew (S8)', () => {
    const entries = buildSidebarNodeEntries(
      new Map([
        ['02.01.57.00.00.01', makeNode()],
        ['02.01.57.00.00.99', makeNode({
          alias: 0x999,
          node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x99],
        })],
      ]),
      ['020157000001'],
    );

    const byId = new Map(entries.map((e) => [e.nodeId, e]));
    expect(byId.get('02.01.57.00.00.01')?.isUnsavedNew).toBe(false);
    expect(byId.get('02.01.57.00.00.99')?.isUnsavedNew).toBe(true);
  });

  it('treats every node as saved when savedNodeIds is undefined (pre-S8 contexts)', () => {
    const entries = buildSidebarNodeEntries(
      new Map([['02.01.57.00.00.01', makeNode()]]),
    );

    expect(entries[0].isUnsavedNew).toBe(false);
  });
});

describe('shouldShowConfigNotReadBadge', () => {
  it('suppresses the badge during offline layout mode and shows it for unread live nodes', () => {
    const node = makeNode();

    expect(shouldShowConfigNotReadBadge({
      configReadNodes: new Set(),
      layoutIsOfflineMode: true,
      layoutOpenInProgress: false,
      node,
      nodeId: '02.01.57.00.00.01',
    })).toBe(false);

    expect(shouldShowConfigNotReadBadge({
      configReadNodes: new Set(),
      layoutIsOfflineMode: false,
      layoutOpenInProgress: false,
      node,
      nodeId: '02.01.57.00.00.01',
    })).toBe(true);
  });

  it('suppresses the badge until CDI support is confirmed by PIP', () => {
    expect(shouldShowConfigNotReadBadge({
      configReadNodes: new Set(),
      layoutIsOfflineMode: false,
      layoutOpenInProgress: false,
      node: makeNode({ pip_status: 'Unknown', pip_flags: null }),
      nodeId: '02.01.57.00.00.01',
    })).toBe(false);
  });
});

describe('pending state helpers', () => {
  const pendingRows: OfflineChangeRow[] = [
    {
      baselineValue: '10',
      changeId: 'cfg-1',
      kind: 'config',
      nodeId: '02.01.57.00.00.01',
      offset: '0x00000010',
      plannedValue: '12',
      space: 253,
      status: 'pending',
    },
  ];

  it('reports node-level pending edits and applies from the tree and persisted rows', () => {
    mockHasDraftsForNode.mockReturnValue(true);
    expect(getNodePendingState(
      '02.01.57.00.00.01',
      makeTree(),
      false,
      pendingRows,
    )).toEqual({
      hasPendingApply: true,
      hasPendingEdits: true,
    });
  });

  it('reports segment-level pending edits and applies for the matching segment origin', () => {
    mockChangeLayers.mockReturnValue([{ type: 'draft', value: { type: 'int', value: 12 } }]);
    expect(getSegmentPendingState(
      '02.01.57.00.00.01',
      makeTree(),
      0,
      false,
      pendingRows,
    )).toEqual({
      hasPendingApply: true,
      hasPendingEdits: true,
    });
    mockChangeLayers.mockReturnValue([]);
  });

  it('reports hasPendingEdits only for the segment containing the drafted leaf', () => {
    // The tree has one segment (origin 0) with a leaf at address 16.
    // changeLayers returns a draft only for address 16's key.
    mockChangeLayers.mockImplementation((key: string) => {
      if (key === '020157000001:253:16') {
        return [{ type: 'draft', value: { type: 'int', value: 99 } }];
      }
      return [];
    });

    // Segment origin 0 contains the leaf -> hasPendingEdits true
    expect(getSegmentPendingState(
      '02.01.57.00.00.01',
      makeTree(),
      0,
      false,
      [],
    )).toEqual({
      hasPendingApply: false,
      hasPendingEdits: true,
    });

    // Segment origin 999 doesn't exist -> hasPendingEdits false
    expect(getSegmentPendingState(
      '02.01.57.00.00.01',
      makeTree(),
      999,
      false,
      [],
    )).toEqual({
      hasPendingApply: false,
      hasPendingEdits: false,
    });

    mockChangeLayers.mockReturnValue([]);
  });
});
