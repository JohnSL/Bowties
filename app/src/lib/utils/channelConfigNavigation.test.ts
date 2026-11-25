import { describe, expect, it } from 'vitest';
import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import { resolveConfigTargets, resolveSignalOutputOwnerTarget } from './channelConfigNavigation';

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

describe('resolveSignalOutputOwnerTarget', () => {
  const instances = (replicationOf: string, count: number, segIdx: number) =>
    Array.from({ length: count }, (_, i) => ({
      kind: 'group' as const,
      name: replicationOf,
      description: null,
      instance: i + 1,
      instanceLabel: `${replicationOf} ${i + 1}`,
      replicationOf,
      replicationCount: count,
      path: [`seg:${segIdx}`, `elem:0#${i + 1}`],
      displayName: null,
      children: [],
    }));

  function signalTree(): NodeConfigTree {
    const segment = (name: string, space: number, children: NodeConfigTree['segments'][number]['children']) => ({
      name, description: null, origin: 0, space, children,
    });
    return {
      nodeId: '050101010700',
      identity: null,
      segments: [
        segment('Rule to Aspect', 1, instances('Mast', 4, 0)),
        segment('Direct Lamp Control', 2, instances('Lamp', 16, 1)),
      ],
    } as NodeConfigTree;
  }

  it('resolves "Mast N" to that mast under Rule to Aspect', () => {
    expect(resolveSignalOutputOwnerTarget(signalTree(), 'Mast 3')).toEqual({
      label: 'Rule to Aspect.Mast 3',
      nodeId: '050101010700',
      elementPath: ['seg:0', 'elem:0#3'],
    });
  });

  it('resolves "Direct lamp N" to that lamp under Direct Lamp Control', () => {
    expect(resolveSignalOutputOwnerTarget(signalTree(), 'Direct lamp 2')).toEqual({
      label: 'Direct Lamp Control.Lamp 2',
      nodeId: '050101010700',
      elementPath: ['seg:1', 'elem:0#2'],
    });
  });

  it('returns undefined without a tree, for unknown owners and for missing instances', () => {
    expect(resolveSignalOutputOwnerTarget(undefined, 'Mast 1')).toBeUndefined();
    expect(resolveSignalOutputOwnerTarget(signalTree(), 'Turnout 1')).toBeUndefined();
    expect(resolveSignalOutputOwnerTarget(signalTree(), 'Mast 9')).toBeUndefined();
  });
});
