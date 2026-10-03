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
