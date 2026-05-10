import { describe, expect, it } from 'vitest';

import type { LeafConfigNode } from '$lib/types/nodeTree';
import type { ConnectorConstraintState } from '$lib/utils/connectorConstraints';

import { decideConnectorLeafValue } from './connectorLeafDecision';

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

function makeState(overrides: Partial<ConnectorConstraintState> = {}): ConnectorConstraintState {
  return {
    slotId: 'connector-a',
    hidden: false,
    disabled: false,
    readOnly: false,
    allowedValues: [1, 2],
    deniedValues: [],
    explanations: [],
    ...overrides,
  };
}

describe('decideConnectorLeafValue', () => {
  it('returns compatible when the current value is allowed', () => {
    const leaf = makeLeaf();

    const decision = decideConnectorLeafValue({
      leaf,
      currentValue: { type: 'int', value: 2 },
      constraintState: makeState(),
    });

    expect(decision).toEqual({ kind: 'compatible' });
  });

  it('returns the first compatible allowed value when auto-correction is possible', () => {
    const leaf = makeLeaf();

    const decision = decideConnectorLeafValue({
      leaf,
      currentValue: { type: 'int', value: 3 },
      constraintState: makeState({ allowedValues: [2, 1] }),
    });

    expect(decision).toEqual({
      kind: 'autoCorrect',
      nextValue: { type: 'int', value: 2 },
    });
  });

  it('returns unsupported when no compatible allowed value can be derived', () => {
    const leaf = makeLeaf();

    const decision = decideConnectorLeafValue({
      leaf,
      currentValue: { type: 'int', value: 3 },
      constraintState: makeState({ allowedValues: ['Missing Label'] }),
    });

    expect(decision).toEqual({
      kind: 'unsupported',
      reason: 'No compatible allowed value could be derived for this leaf.',
    });
  });

  it('returns compatible for the repaired value after auto-correction', () => {
    const leaf = makeLeaf();
    const state = makeState({ allowedValues: [1, 2] });

    const repair = decideConnectorLeafValue({
      leaf,
      currentValue: { type: 'int', value: 3 },
      constraintState: state,
    });

    expect(repair.kind).toBe('autoCorrect');

    const postRepair = decideConnectorLeafValue({
      leaf,
      currentValue: (repair as { nextValue: { type: 'int'; value: number } }).nextValue,
      constraintState: state,
    });

    expect(postRepair).toEqual({ kind: 'compatible' });
  });

  it('returns compatible when no slot governs the leaf', () => {
    const decision = decideConnectorLeafValue({
      leaf: makeLeaf(),
      currentValue: { type: 'int', value: 3 },
      constraintState: makeState({ slotId: null }),
    });

    expect(decision).toEqual({ kind: 'compatible' });
  });

  it('detects denied values as incompatible', () => {
    const decision = decideConnectorLeafValue({
      leaf: makeLeaf(),
      currentValue: { type: 'int', value: 3 },
      constraintState: makeState({ allowedValues: [], deniedValues: [3] }),
    });

    expect(decision.kind).not.toBe('compatible');
  });
});