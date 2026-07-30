/**
 * Tests for ChannelRow.svelte — channel-to-config navigation (location cell).
 *
 * Spec: channel-to-config navigation from the Railroad tab.
 * 1:1 bindings (connectorInput) navigate directly via configFocusStore.
 * 1:N bindings (lampRow spanning multiple rows) open a popover listing each
 * config target as a separate jump link.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import ChannelRow from './ChannelRow.svelte';
import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';

const { focusConfigFieldMock } = vi.hoisted(() => ({
  focusConfigFieldMock: vi.fn(),
}));
vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: {
    focusConfigField: focusConfigFieldMock,
  },
}));

beforeEach(() => {
  focusConfigFieldMock.mockClear();
});

function makeConnectorInputChannel(): InformationChannel {
  return {
    id: 'ch-1',
    name: 'Block 3 Occupancy',
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: '020157000001', connector: 'connector-a', input: 2 },
  };
}

function makeConnectorInputTree(): NodeConfigTree {
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
          resolvedAffectedPaths: [['seg:0', 'elem:0#1'], ['seg:0', 'elem:0#2']],
        },
      ],
    },
    segments: [],
  };
}

function makeLampRowChannel(): InformationChannel {
  return {
    id: 'ch-2',
    name: 'Signal 3 Aspect',
    role: 'signal-aspect',
    style: '2-led-bicolor-aspect',
    ownership: 'user-owned',
    binding: { kind: 'lampRow', nodeKey: '020158000001', rowOrdinal: 1 },
  };
}

function makeLampRowTree(): NodeConfigTree {
  return {
    nodeId: '020158000001',
    identity: null,
    segments: [
      {
        name: 'Direct Lamp Control',
        description: null,
        origin: 0,
        space: 253,
        children: [1, 2].map((ordinal) => ({
          kind: 'group' as const,
          name: `Lamp #${ordinal}`,
          description: null,
          instance: ordinal,
          instanceLabel: `Lamp #${ordinal}`,
          replicationOf: 'Lamp',
          replicationCount: 2,
          path: ['seg:0', `elem:0#${ordinal}`],
          displayName: null,
          children: [],
        })),
      },
    ],
  };
}

describe('ChannelRow — channel-to-config navigation', () => {
  it('1:1 channel: clicking the location cell navigates directly via configFocusStore', async () => {
    const tree = makeConnectorInputTree();
    render(ChannelRow, {
      channel: makeConnectorInputChannel(),
      nodeTree: (nodeKey: string) => (nodeKey === tree.nodeId ? tree : undefined),
    });

    await fireEvent.click(screen.getByTestId('location-nav'));

    expect(focusConfigFieldMock).toHaveBeenCalledWith('020157000001', ['seg:0', 'elem:0#2']);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('1:N channel: clicking the location cell opens a popover; clicking a target navigates', async () => {
    const tree = makeLampRowTree();
    render(ChannelRow, {
      channel: makeLampRowChannel(),
      nodeTree: (nodeKey: string) => (nodeKey === tree.nodeId ? tree : undefined),
    });

    await fireEvent.click(screen.getByTestId('location-nav'));

    const menu = screen.getByRole('menu');
    const items = screen.getAllByRole('menuitem');
    expect(items).toHaveLength(2);
    expect(focusConfigFieldMock).not.toHaveBeenCalled();

    await fireEvent.click(items[1]);

    expect(focusConfigFieldMock).toHaveBeenCalledWith('020158000001', ['seg:0', 'elem:0#2']);
    expect(menu).not.toBeInTheDocument();
  });
});
