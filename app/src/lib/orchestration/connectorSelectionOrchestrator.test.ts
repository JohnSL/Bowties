import { describe, expect, it } from 'vitest';

import type { ConnectorProfileView, ConnectorSelectionDocument } from '$lib/types/connectorProfile';
import type { LeafConfigNode, NodeConfigTree } from '$lib/types/nodeTree';

import { computeConnectorCompatibilityState } from './connectorSelectionOrchestrator';

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