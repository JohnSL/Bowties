import { describe, expect, it } from 'vitest';

import type { ConnectorProfileView, ConnectorSelectionDocument } from '$lib/types/connectorProfile';
import type { LeafConfigNode, NodeConfigTree } from '$lib/types/nodeTree';

import { computeConnectorCompatibilityState, buildAutoCreatedChannels, buildAutoCreatedChannelsForSlot } from './connectorSelectionOrchestrator';

function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Input Function',
    description: null,
    elementType: 'int',
    address: 0,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0#1', 'elem:0'],
    value: { type: 'int', value: 3 },
    eventRole: null,
    constraints: {
      min: 0,
      max: 8,
      defaultValue: null,
      mapEntries: [
        { value: 1, label: 'Normal' },
        { value: 2, label: 'Active Lo' },
        { value: 3, label: 'Alt Action Hi' },
      ],
    },
    ...overrides,
  };
}

function makeTree(leaf: LeafConfigNode): NodeConfigTree {
  return {
    nodeId: '05.02.01.02.03.00',
    identity: null,
    segments: [
      {
        name: 'Port I/O',
        description: null,
        origin: 0,
        space: 253,
        children: [leaf],
      },
    ],
  };
}

function makeProfile(): ConnectorProfileView {
  return {
    nodeId: '05.02.01.02.03.00',
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
                targetPath: 'Port I/O/Line/Input Function',
                resolvedPath: ['seg:0', 'elem:0', 'elem:0'],
                effect: 'allowValues',
                allowedValues: [1, 2],
                allowedValueLabels: ['Normal', 'Active Lo'],
              },
            ],
          },
        ],
      },
    ],
    supportedDaughterboards: [
      {
        daughterboardId: 'BOD-8-SM',
        displayName: 'BOD-8-SM',
      },
    ],
  };
}

function makeDocument(): ConnectorSelectionDocument {
  return {
    nodeId: '05.02.01.02.03.00',
    carrierKey: 'rr-cirkits::tower-lcc',
    slotSelections: [
      {
        slotId: 'connector-a',
        selectedDaughterboardId: 'BOD-8-SM',
        status: 'selected',
      },
    ],
  };
}

describe('computeConnectorCompatibilityState', () => {
  it('stages the first allowed value when the current value is incompatible', () => {
    const state = computeConnectorCompatibilityState(makeTree(makeLeaf()), makeProfile(), makeDocument());

    expect(state.warnings).toEqual([]);
    expect(state.stagedRepairs).toMatchObject([
      {
        targetPath: 'seg:0/elem:0#1/elem:0',
        baselineValue: '3',
        plannedValue: '1',
        reason: 'Auto-staged first compatible allowed value',
      },
    ]);
  });

  it('does not stage a repair when no allowed subset is authored', () => {
    const profile = makeProfile();
    profile.slots[0].supportedDaughterboardConstraints![0].validityRules![0].allowedValues = [];

    const state = computeConnectorCompatibilityState(makeTree(makeLeaf()), profile, makeDocument());

    expect(state.stagedRepairs).toEqual([]);
    expect(state.warnings).toEqual([]);
  });

  it('uses the supplied effective current value when judging compatibility', () => {
    const leaf = makeLeaf();

    const state = computeConnectorCompatibilityState(
      makeTree(leaf),
      makeProfile(),
      makeDocument(),
      () => ({ type: 'int', value: 2 }),
    );

    expect(state.stagedRepairs).toEqual([]);
    expect(state.warnings).toEqual([]);
  });

  it('detects incompatible in-flight draft values when a custom resolver is supplied', () => {
    const leaf = makeLeaf({ value: { type: 'int', value: 2 } });

    const state = computeConnectorCompatibilityState(
      makeTree(leaf),
      makeProfile(),
      makeDocument(),
      () => ({ type: 'int', value: 3 }),
    );

    expect(state.stagedRepairs).toMatchObject([
      {
        plannedValue: '1',
        reason: 'Auto-staged first compatible allowed value',
      },
    ]);
  });

  it('stages no repairs when selection is "None installed" and no constraints apply', () => {
    const document = makeDocument();
    document.slotSelections[0].selectedDaughterboardId = undefined;
    document.slotSelections[0].status = 'none';

    const state = computeConnectorCompatibilityState(
      makeTree(makeLeaf()),
      makeProfile(),
      document,
    );

    expect(state.stagedRepairs).toEqual([]);
    expect(state.warnings).toEqual([]);
  });
});

describe('buildAutoCreatedChannels', () => {
  function makeProfileWithChannelInputs(): ConnectorProfileView {
    return {
      nodeId: '05.02.01.02.03.00',
      carrierKey: 'rr-cirkits::tower-lcc',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD-8-SM'],
          affectedPaths: [],
          supportedDaughterboardConstraints: [],
        },
        {
          slotId: 'connector-b',
          label: 'Connector B',
          order: 1,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD4'],
          affectedPaths: [],
          supportedDaughterboardConstraints: [],
        },
      ],
      supportedDaughterboards: [
        {
          daughterboardId: 'BOD-8-SM',
          displayName: 'BOD-8-SM',
          kind: 'detection',
          channelInputs: [
            { channelType: 'block-occupancy', style: 'bod-block-detector-input', inputs: [1, 2, 3, 4, 5, 6, 7, 8] },
          ],
        },
        {
          daughterboardId: 'BOD4',
          displayName: 'BOD4',
          kind: 'mixed-io',
          channelInputs: [
            { channelType: 'block-occupancy', style: 'bod-block-detector-input', inputs: [1, 2, 3, 4] },
          ],
        },
        {
          daughterboardId: 'FOB-A',
          displayName: 'FOB-A',
          kind: 'breakout',
          // no channelInputs — breakout board
        },
      ],
    };
  }

  function makeDocumentWithSelections(
    selections: { slotId: string; selectedDaughterboardId?: string }[],
  ): ConnectorSelectionDocument {
    return {
      nodeId: '05.02.01.02.03.00',
      carrierKey: 'rr-cirkits::tower-lcc',
      slotSelections: selections.map((s) => ({
        ...s,
        status: s.selectedDaughterboardId ? 'selected' as const : 'none' as const,
      })),
    };
  }

  it('creates 8 block-occupancy channels for BOD-8-SM on connector-a', () => {
    const profile = makeProfileWithChannelInputs();
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM' },
    ]);

    const channels = buildAutoCreatedChannels(profile, document, 'West Yard');

    expect(channels).toHaveLength(8);
    for (let i = 0; i < 8; i++) {
      expect(channels[i]).toMatchObject({
        name: `West Yard — Connector A — Input ${i + 1}`,
        role: 'block-occupancy',
        style: 'bod-block-detector-input',
        ownership: 'hardware-owned',
        binding: {
          kind: 'connectorInput',
          nodeKey: '050201020300',
          connector: 'connector-a',
          input: i + 1,
        },
      });
      // Each channel gets a unique UUID
      expect(channels[i].id).toBeTruthy();
    }
    // All IDs are unique
    const ids = channels.map((c) => c.id);
    expect(new Set(ids).size).toBe(8);
  });

  it('creates 4 channels for BOD4 (detection-half only)', () => {
    const profile = makeProfileWithChannelInputs();
    const document = makeDocumentWithSelections([
      { slotId: 'connector-b', selectedDaughterboardId: 'BOD4' },
    ]);

    const channels = buildAutoCreatedChannels(profile, document, 'East Staging');

    expect(channels).toHaveLength(4);
    expect(channels[0]).toMatchObject({
      name: 'East Staging — Connector B — Input 1',
      role: 'block-occupancy',
      style: 'bod-block-detector-input',
      ownership: 'hardware-owned',
      binding: {
        kind: 'connectorInput',
        nodeKey: '050201020300',
        connector: 'connector-b',
        input: 1,
      },
    });
    expect(channels[3]).toMatchObject({
      name: 'East Staging — Connector B — Input 4',
      binding: { kind: 'connectorInput', input: 4 },
    });
  });

  it('returns empty array when selected board has no channelInputs', () => {
    const profile = makeProfileWithChannelInputs();
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'FOB-A' },
    ]);

    const channels = buildAutoCreatedChannels(profile, document, 'Node 1');

    expect(channels).toEqual([]);
  });

  it('returns empty array when no board is selected', () => {
    const profile = makeProfileWithChannelInputs();
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a' },
    ]);

    const channels = buildAutoCreatedChannels(profile, document, 'Node 1');

    expect(channels).toEqual([]);
  });

  it('returns empty array when profile has no supportedDaughterboards', () => {
    const profile = makeProfileWithChannelInputs();
    profile.supportedDaughterboards = undefined;
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM' },
    ]);

    const channels = buildAutoCreatedChannels(profile, document, 'Node 1');

    expect(channels).toEqual([]);
  });

  it('creates channels for multiple slots with boards selected', () => {
    const profile = makeProfileWithChannelInputs();
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM' },
      { slotId: 'connector-b', selectedDaughterboardId: 'BOD4' },
    ]);

    const channels = buildAutoCreatedChannels(profile, document, 'West Yard');

    expect(channels).toHaveLength(12); // 8 + 4
    // First 8 are connector-a
    expect(channels[0].binding.kind === 'connectorInput' && channels[0].binding.connector).toBe('connector-a');
    expect(channels[7].binding.kind === 'connectorInput' && channels[7].binding.connector).toBe('connector-a');
    // Next 4 are connector-b
    expect(channels[8].binding.kind === 'connectorInput' && channels[8].binding.connector).toBe('connector-b');
    expect(channels[11].binding.kind === 'connectorInput' && channels[11].binding.connector).toBe('connector-b');
  });
});

describe('buildAutoCreatedChannelsForSlot', () => {
  function makeProfileWithChannelInputs(): ConnectorProfileView {
    return {
      nodeId: '05.02.01.02.03.00',
      carrierKey: 'rr-cirkits::tower-lcc',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD-8-SM'],
          affectedPaths: [],
          supportedDaughterboardConstraints: [],
        },
        {
          slotId: 'connector-b',
          label: 'Connector B',
          order: 1,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD4'],
          affectedPaths: [],
          supportedDaughterboardConstraints: [],
        },
      ],
      supportedDaughterboards: [
        {
          daughterboardId: 'BOD-8-SM',
          displayName: 'BOD-8-SM',
          kind: 'detection',
          channelInputs: [
            { channelType: 'block-occupancy', style: 'bod-block-detector-input', inputs: [1, 2, 3, 4, 5, 6, 7, 8] },
          ],
        },
        {
          daughterboardId: 'BOD4',
          displayName: 'BOD4',
          kind: 'mixed-io',
          channelInputs: [
            { channelType: 'block-occupancy', style: 'bod-block-detector-input', inputs: [1, 2, 3, 4] },
          ],
        },
      ],
    };
  }

  function makeDocumentWithSelections(
    selections: { slotId: string; selectedDaughterboardId?: string | null }[],
  ): ConnectorSelectionDocument {
    return {
      nodeId: '05.02.01.02.03.00',
      carrierKey: 'rr-cirkits::tower-lcc',
      slotSelections: selections.map((s) => ({
        ...s,
        selectedDaughterboardId: s.selectedDaughterboardId ?? undefined,
        status: s.selectedDaughterboardId ? 'selected' as const : 'none' as const,
      })),
    };
  }

  it('creates channels only for the specified slot', () => {
    const profile = makeProfileWithChannelInputs();
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM' },
      { slotId: 'connector-b', selectedDaughterboardId: 'BOD4' },
    ]);

    const channels = buildAutoCreatedChannelsForSlot(profile, document, 'West Yard', 'connector-a');

    expect(channels).toHaveLength(8);
    expect(channels.every((ch) => ch.binding.kind === 'connectorInput' && ch.binding.connector === 'connector-a')).toBe(true);
  });

  it('returns empty array when slot has no board selected', () => {
    const profile = makeProfileWithChannelInputs();
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: null },
    ]);

    const channels = buildAutoCreatedChannelsForSlot(profile, document, 'Node 1', 'connector-a');

    expect(channels).toEqual([]);
  });

  it('returns empty array when selected board has no channelInputs', () => {
    const profile = makeProfileWithChannelInputs();
    // Override to remove channelInputs from the BOD-8-SM board
    profile.supportedDaughterboards![0].channelInputs = undefined;
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM' },
    ]);

    const channels = buildAutoCreatedChannelsForSlot(profile, document, 'Node 1', 'connector-a');

    expect(channels).toEqual([]);
  });

  it('stores nodeKey in canonical wire form even when document.nodeId is dotted (ADR-0010)', () => {
    const profile = makeProfileWithChannelInputs();
    // document.nodeId is in dotted format (as returned by backend profile)
    const document = makeDocumentWithSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM' },
    ]);
    expect(document.nodeId).toBe('05.02.01.02.03.00'); // dotted

    const channels = buildAutoCreatedChannelsForSlot(profile, document, 'Node 1', 'connector-a');

    // binding.nodeKey must be canonical (no dots, uppercase)
    expect(channels[0].binding.kind === 'connectorInput' && channels[0].binding.nodeKey).toBe('050201020300');
  });
});