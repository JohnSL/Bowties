import { describe, expect, it } from 'vitest';
import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import { resolveConfigTargets } from './channelConfigNavigation';

function connectorInputChannel(): InformationChannel {
  return {
    id: 'ch-1',
    name: 'Block 3 Occupancy',
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: '020157000001', connector: 'connector-a', input: 2 },
  };
}

function connectorInputTree(): NodeConfigTree {
  return {
    nodeId: '020157000001',
    identity: null,
    connectorProfile: {
      nodeId: '020157000001',
      carrierKey: 'carrier-1',
      slots: [{
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
      }],
    },
    segments: [{
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
    }],
  };
}

describe('resolveConfigTargets', () => {
  it('resolves a connector input to its profile path and CDI-derived label', () => {
    const targets = resolveConfigTargets(connectorInputChannel(), connectorInputTree());

    expect(targets).toEqual([{
      label: 'Block Occupancy Detector.Line 2',
      nodeId: '020157000001',
      elementPath: ['seg:0', 'elem:0#2'],
    }]);
  });

  it('falls back to the channel name when the profile path has no tree node', () => {
    const tree = connectorInputTree();
    tree.segments = [];

    expect(resolveConfigTargets(connectorInputChannel(), tree)[0].label).toBe('Block 3 Occupancy');
  });

  it('returns no target when tree or profile data is unavailable', () => {
    expect(resolveConfigTargets(connectorInputChannel(), undefined)).toEqual([]);
    const tree = connectorInputTree();
    tree.connectorProfile = null;
    expect(resolveConfigTargets(connectorInputChannel(), tree)).toEqual([]);
  });

  it('resolves a supported lamp-row channel to its Direct Lamp Control instance', () => {
    const channel: InformationChannel = {
      id: 'ch-2',
      name: 'Block 5 Indicator',
      role: 'lamp-indicator',
      style: 'single-led-direct-lamp',
      ownership: 'user-owned',
      binding: { kind: 'lampRow', nodeKey: '020158000001', rowOrdinal: 2 },
    };
    const tree: NodeConfigTree = {
      nodeId: '020158000001',
      identity: null,
      segments: [{
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
      }],
    };

    expect(resolveConfigTargets(channel, tree)).toEqual([{
      label: 'Direct Lamp Control.Lamp #2',
      nodeId: '020158000001',
      elementPath: ['seg:0', 'elem:0#2'],
    }]);
  });
});
