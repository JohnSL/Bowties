import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import ChannelRow from './ChannelRow.svelte';

const { focusConfigFieldMock } = vi.hoisted(() => ({ focusConfigFieldMock: vi.fn() }));
vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: { focusConfigField: focusConfigFieldMock },
}));

beforeEach(() => focusConfigFieldMock.mockClear());

const channel: InformationChannel = {
  id: 'ch-1',
  name: 'Block 3 Occupancy',
  role: 'block-occupancy',
  style: 'bod-block-detector-input',
  ownership: 'hardware-owned',
  binding: { kind: 'connectorInput', nodeKey: '020157000001', connector: 'connector-a', input: 2 },
};

const tree: NodeConfigTree = {
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
      resolvedAffectedPaths: [['seg:0', 'elem:0#1'], ['seg:0', 'elem:0#2']],
    }],
  },
  segments: [],
};

describe('ChannelRow configuration navigation', () => {
  it('navigates a resolved channel location through configFocusStore', async () => {
    render(ChannelRow, {
      channel,
      nodeTree: (nodeKey: string) => nodeKey === tree.nodeId ? tree : undefined,
    });

    await fireEvent.click(screen.getByTestId('location-nav'));

    expect(focusConfigFieldMock).toHaveBeenCalledWith('020157000001', ['seg:0', 'elem:0#2']);
  });

  it('degrades to plain location text when tree data is unavailable', () => {
    render(ChannelRow, { channel, nodeTree: () => undefined });

    expect(screen.queryByTestId('location-nav')).not.toBeInTheDocument();
    expect(screen.getByText('Connector A · Input 2')).toBeInTheDocument();
  });
});

describe('ChannelRow signal channel (Spec 020 / E1a)', () => {
  const signal: InformationChannel = {
    id: 'sig-1',
    name: 'Mast A',
    role: 'signal-indication',
    style: 'two-driver-rg-signal',
    ownership: 'user-owned',
    binding: { kind: 'signalLccRgOutputs', nodeKey: '020157000001', greenOutput: 1, redOutput: 3 },
  };

  it('labels the role "Signal" and names the red and green lamps from the node configuration', () => {
    const lampTree = {
      nodeId: '020157000001',
      identity: null,
      segments: [{
        name: 'Rule to Aspect', description: null, origin: 0, space: 253,
        children: [{
          kind: 'leaf', name: 'Lamp Selection', description: null, elementType: 'int',
          address: 0, size: 1, space: 253, path: ['seg:0', 'elem:0'], value: null, eventRole: null,
          constraints: {
            min: null, max: null, defaultValue: null,
            mapEntries: [{ value: 1, label: '#1 H1-G' }, { value: 3, label: '#3 H1-R' }],
          },
        }],
      }],
    } as unknown as NodeConfigTree;

    render(ChannelRow, { channel: signal, usedBy: [{ facilityName: 'ABS East', slotLabel: 'signals' }], nodeTree: () => lampTree });

    expect(screen.getByText('Signal')).toBeInTheDocument();
    expect(screen.getByText('#3 H1-R / #1 H1-G')).toBeInTheDocument();
    expect(screen.getByText('ABS East / signals')).toBeInTheDocument();
  });

  it('falls back to output numbers when the node configuration is unavailable', () => {
    render(ChannelRow, { channel: signal, nodeTree: () => undefined });

    expect(screen.getByText('#3 / #1')).toBeInTheDocument();
  });
});
