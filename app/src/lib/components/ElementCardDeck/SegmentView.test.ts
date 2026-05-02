import '@testing-library/jest-dom/vitest';
import { describe, expect, it, beforeEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import SegmentView from './SegmentView.svelte';
import { configSidebarStore } from '$lib/stores/configSidebar';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { clearConfigReadStatus, markNodeConfigRead } from '$lib/stores/configReadStatus';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';

const NODE_ID = '02.01.57.00.00.01';

function setSegmentTree(): void {
  nodeTreeStore.setTree(NODE_ID, {
    nodeId: NODE_ID,
    identity: null,
    connectorProfile: {
      nodeId: NODE_ID,
      carrierKey: 'rr-cirkits::tower-lcc',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD4-CP'],
          affectedPaths: ['Port I/O/Line'],
        },
      ],
      supportedDaughterboards: [
        {
          daughterboardId: 'BOD4-CP',
          displayName: 'BOD4-CP',
          description: 'Detector board',
        },
      ],
    },
    segments: [
      {
        name: 'Port I/O',
        description: 'Port settings',
        origin: 0,
        space: 253,
        children: [],
      },
    ],
  } as any);

  void connectorSelectionsStore.loadNode(NODE_ID, nodeTreeStore.getTree(NODE_ID)?.connectorProfile ?? null);
  configSidebarStore.selectSegment(NODE_ID, 'seg:0', 'Port I/O');
}

beforeEach(() => {
  configSidebarStore.reset();
  connectorSelectionsStore.reset();
  clearConfigReadStatus();
  layoutStore.reset();
  nodeTreeStore.reset();
});

describe('SegmentView connector selectors', () => {
  it('renders segment-local connector selectors when the selected segment is affected', () => {
    setSegmentTree();

    render(SegmentView);

    expect(screen.getByRole('group', { name: 'Connector daughterboards for Port I/O' })).toBeInTheDocument();
    expect(screen.getByLabelText('Connector A')).toBeInTheDocument();
  });

  it('enables connector selectors after the node configuration has been read online', () => {
    setSegmentTree();
    markNodeConfigRead(NODE_ID);

    render(SegmentView);

    expect(screen.getByLabelText('Connector A')).toBeEnabled();
  });

  it('emits connector selection changes from the segment-local control', async () => {
    setSegmentTree();
    markNodeConfigRead(NODE_ID);

    const received: Array<{ nodeId: string; slotId: string; selectedDaughterboardId: string | null }> = [];
    render(SegmentView, {
      props: {
        onchangeConnectorSelection: (event: CustomEvent<{ nodeId: string; slotId: string; selectedDaughterboardId: string | null }>) => {
          received.push(event.detail);
        },
      },
    });

    await fireEvent.change(screen.getByLabelText('Connector A'), {
      target: { value: 'BOD4-CP' },
    });

    expect(received).toEqual([
      {
        nodeId: NODE_ID,
        slotId: 'connector-a',
        selectedDaughterboardId: 'BOD4-CP',
      },
    ]);
  });
});