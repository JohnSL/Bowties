import '@testing-library/jest-dom/vitest';
import { describe, expect, it, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
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

  it('hides segment-level governed groups when the selected daughterboard marks them unavailable', async () => {
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
            supportedDaughterboardIds: ['BOD4'],
            affectedPaths: ['Port I/O/Line#1'],
            resolvedAffectedPaths: [['seg:0', 'elem:0#1']],
            supportedDaughterboardConstraints: [
              {
                daughterboardId: 'BOD4',
                validityRules: [
                  {
                    targetPath: 'Port I/O/Line/Producer Events',
                    resolvedPath: ['seg:0', 'elem:0', 'elem:5'],
                    effect: 'hide',
                  },
                ],
              },
            ],
          },
        ],
        supportedDaughterboards: [{ daughterboardId: 'BOD4', displayName: 'BOD4' }],
      },
      segments: [
        {
          name: 'Port I/O',
          description: 'Port settings',
          origin: 0,
          space: 253,
          children: [
            {
              kind: 'group',
              name: 'Line',
              description: null,
              instance: 1,
              instanceLabel: 'Line 1',
              replicationOf: 'Line',
              replicationCount: 1,
              path: ['seg:0', 'elem:0#1'],
              displayName: null,
              children: [
                {
                  kind: 'group',
                  name: 'Producer Events',
                  description: null,
                  instance: 1,
                  instanceLabel: 'Producer Events',
                  replicationOf: 'Producer Events',
                  replicationCount: 1,
                  path: ['seg:0', 'elem:0#1', 'elem:5'],
                  displayName: null,
                  children: [],
                },
              ],
            },
          ],
        },
      ],
    } as any);

    await connectorSelectionsStore.saveDocument({
      nodeId: NODE_ID,
      carrierKey: 'rr-cirkits::tower-lcc',
      slotSelections: [{ slotId: 'connector-a', selectedDaughterboardId: 'BOD4', status: 'selected' }],
    });
    configSidebarStore.selectSegment(NODE_ID, 'seg:0', 'Port I/O');

    render(SegmentView);

    expect(screen.queryByText('Producer Events')).not.toBeInTheDocument();
  });

  it('updates the currently selected governed line immediately when connector selection changes', async () => {
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
            supportedDaughterboardIds: ['BOD-8-SM'],
            affectedPaths: ['Port I/O/Line#1'],
            resolvedAffectedPaths: [['seg:0', 'elem:0#1']],
            supportedDaughterboardConstraints: [
              {
                daughterboardId: 'BOD-8-SM',
                validityRules: [
                  {
                    targetPath: 'Port I/O/Line/Output Function',
                    resolvedPath: ['seg:0', 'elem:0', 'elem:0'],
                    effect: 'allowValues',
                    allowedValues: [0],
                  },
                  {
                    targetPath: 'Port I/O/Line/Input Function',
                    resolvedPath: ['seg:0', 'elem:0', 'elem:1'],
                    effect: 'allowValues',
                    allowedValues: [2],
                  },
                ],
              },
            ],
          },
        ],
        supportedDaughterboards: [{ daughterboardId: 'BOD-8-SM', displayName: 'BOD-8-SM' }],
      },
      segments: [
        {
          name: 'Port I/O',
          description: 'Port settings',
          origin: 0,
          space: 253,
          children: [
            {
              kind: 'group',
              name: 'Line',
              description: null,
              instance: 0,
              instanceLabel: 'Line',
              replicationOf: 'Line',
              replicationCount: 2,
              path: ['seg:0', 'elem:0'],
              displayName: null,
              children: [
                {
                  kind: 'group',
                  name: 'Line',
                  description: null,
                  instance: 1,
                  instanceLabel: 'Line 1',
                  replicationOf: 'Line',
                  replicationCount: 2,
                  path: ['seg:0', 'elem:0#1'],
                  displayName: null,
                  children: [
                    {
                      kind: 'leaf',
                      name: 'Output Function',
                      description: null,
                      elementType: 'int',
                      address: 0,
                      size: 1,
                      space: 253,
                      path: ['seg:0', 'elem:0#1', 'elem:0'],
                      value: { type: 'int', value: 0 },
                      modifiedValue: null,
                      writeState: null,
                      writeError: null,
                      readOnly: false,
                      constraints: {
                        min: 0,
                        max: 16,
                        defaultValue: null,
                        mapEntries: [
                          { value: 0, label: 'No Function' },
                          { value: 1, label: 'Steady Active Hi' },
                        ],
                      },
                    },
                    {
                      kind: 'leaf',
                      name: 'Input Function',
                      description: null,
                      elementType: 'int',
                      address: 1,
                      size: 1,
                      space: 253,
                      path: ['seg:0', 'elem:0#1', 'elem:1'],
                      value: { type: 'int', value: 1 },
                      modifiedValue: null,
                      writeState: null,
                      writeError: null,
                      readOnly: false,
                      constraints: {
                        min: 0,
                        max: 8,
                        defaultValue: null,
                        mapEntries: [
                          { value: 0, label: 'Disabled' },
                          { value: 2, label: 'Active Lo' },
                          { value: 5, label: 'Sample Hi' },
                        ],
                      },
                    },
                  ],
                },
                {
                  kind: 'group',
                  name: 'Line',
                  description: null,
                  instance: 2,
                  instanceLabel: 'Line 2',
                  replicationOf: 'Line',
                  replicationCount: 2,
                  path: ['seg:0', 'elem:0#2'],
                  displayName: null,
                  children: [],
                },
              ],
            },
          ],
        },
      ],
    } as any);

    await connectorSelectionsStore.loadNode(
      NODE_ID,
      nodeTreeStore.getTree(NODE_ID)?.connectorProfile ?? null,
    );
    configSidebarStore.selectSegment(NODE_ID, 'seg:0', 'Port I/O');

    render(SegmentView);

    const outputSelect = () => screen.getByLabelText('Output Function');
    const inputSelect = () => screen.getByLabelText('Input Function');

    expect(within(outputSelect()).getByRole('option', { name: 'Steady Active Hi' })).toBeInTheDocument();
    expect(within(inputSelect()).getByRole('option', { name: 'Active Lo' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: /incompatible with selected daughterboard/i })).not.toBeInTheDocument();

    await connectorSelectionsStore.updateSlotSelection(NODE_ID, 'connector-a', 'BOD-8-SM');

    await waitFor(() => {
      expect(within(outputSelect()).queryByRole('option', { name: 'Steady Active Hi' })).not.toBeInTheDocument();
      expect(within(inputSelect()).getByRole('option', { name: 'Active Lo' })).toBeInTheDocument();
      expect(within(inputSelect()).queryByRole('option', { name: 'Sample Hi' })).not.toBeInTheDocument();
      expect(screen.queryByRole('option', { name: /incompatible with selected daughterboard/i })).not.toBeInTheDocument();
    });
  });
});