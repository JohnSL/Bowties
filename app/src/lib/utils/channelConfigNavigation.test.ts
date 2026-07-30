/**
 * Tests for channelConfigNavigation.ts — binding → config-target resolution.
 *
 * Spec: channel-to-config navigation from the Railroad tab.
 */

import { describe, it, expect } from 'vitest';
import { resolveConfigTargets } from './channelConfigNavigation';
import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';

function makeConnectorInputChannel(
  overrides: Partial<InformationChannel> = {},
): InformationChannel {
  return {
    id: 'ch-1',
    name: 'Block 3 Occupancy',
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: '020157000001', connector: 'connector-a', input: 2 },
    ...overrides,
  };
}

function makeTreeWithConnectorProfile(): NodeConfigTree {
  return {
    nodeId: '020157000001',
    identity: null,
    connectorProfile: {
      nodeId: '020157000001',
      carrierKey: 'carrier-1',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['bod-8'],
          affectedPaths: [],
          resolvedAffectedPaths: [
            ['seg:0', 'elem:0#1'],
            ['seg:0', 'elem:0#2'],
            ['seg:0', 'elem:0#3'],
          ],
        },
      ],
    },
    segments: [
      {
        name: 'Block Occupancy Detector',
        description: null,
        origin: 0,
        space: 253,
        children: [1, 2, 3].map((ordinal) => ({
          kind: 'group' as const,
          name: `Line ${ordinal}`,
          description: null,
          instance: ordinal,
          instanceLabel: `Line ${ordinal}`,
          replicationOf: 'Line',
          replicationCount: 3,
          path: ['seg:0', `elem:0#${ordinal}`],
          displayName: null,
          children: [],
        })),
      },
    ],
  };
}

describe('resolveConfigTargets — connectorInput binding', () => {
  it('returns exactly one target with the correct nodeId and elementPath', () => {
    const channel = makeConnectorInputChannel();
    const tree = makeTreeWithConnectorProfile();

    const targets = resolveConfigTargets(channel, tree);

    expect(targets).toHaveLength(1);
    expect(targets[0].nodeId).toBe('020157000001');
    expect(targets[0].elementPath).toEqual(['seg:0', 'elem:0#2']);
  });

  it('uses the CDI group display name as the target label', () => {
    const channel = makeConnectorInputChannel();
    const tree = makeTreeWithConnectorProfile();

    const targets = resolveConfigTargets(channel, tree);

    expect(targets[0].label).toBe('Block Occupancy Detector.Line 2');
  });

  it('falls back to channel name when the tree has no matching group', () => {
    const channel = makeConnectorInputChannel();
    const tree = makeTreeWithConnectorProfile();
    tree.segments = [];

    const targets = resolveConfigTargets(channel, tree);

    expect(targets[0].label).toBe('Block 3 Occupancy');
  });
});

function makeLampRowChannel(overrides: Partial<InformationChannel> = {}): InformationChannel {
  return {
    id: 'ch-2',
    name: 'Signal 3 Aspect',
    role: 'signal-aspect',
    style: '2-led-bicolor-aspect',
    ownership: 'user-owned',
    binding: { kind: 'lampRow', nodeKey: '020158000001', rowOrdinal: 1 },
    ...overrides,
  };
}

function makeTreeWithLampRows(): NodeConfigTree {
  return {
    nodeId: '020158000001',
    identity: null,
    segments: [
      {
        name: 'Direct Lamp Control',
        description: null,
        origin: 0,
        space: 253,
        children: [1, 2, 3].map((ordinal) => ({
          kind: 'group' as const,
          name: `Lamp #${ordinal}`,
          description: null,
          instance: ordinal,
          instanceLabel: `Lamp #${ordinal}`,
          replicationOf: 'Lamp',
          replicationCount: 3,
          path: ['seg:0', `elem:0#${ordinal}`],
          displayName: null,
          children: [],
        })),
      },
    ],
  };
}

describe('resolveConfigTargets — lampRow binding', () => {
  it('returns N targets for a style spanning multiple rows, each with a distinct elementPath', () => {
    const channel = makeLampRowChannel();
    const tree = makeTreeWithLampRows();

    const targets = resolveConfigTargets(channel, tree);

    expect(targets).toHaveLength(2);
    expect(targets[0].elementPath).toEqual(['seg:0', 'elem:0#1']);
    expect(targets[1].elementPath).toEqual(['seg:0', 'elem:0#2']);
    expect(new Set(targets.map((t) => t.elementPath.join('/'))).size).toBe(2);
  });
});
