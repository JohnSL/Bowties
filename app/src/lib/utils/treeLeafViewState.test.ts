import { describe, expect, it } from 'vitest';
import type { OfflineChangeRow } from '$lib/api/sync';
import type { LeafConfigNode } from '$lib/types/nodeTree';
import type { ConnectorConstraintState } from '$lib/utils/connectorConstraints';
import { resolveLeafOfflineValueState, resolveLeafSelectViewState } from '$lib/utils/treeLeafViewState';

function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Input Function',
    description: null,
    elementType: 'int',
    address: 100,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0'],
    value: { type: 'int', value: 1 },
    eventRole: null,
    constraints: {
      min: 0,
      max: 8,
      defaultValue: null,
      mapEntries: [
        { value: 0, label: 'Disabled' },
        { value: 1, label: 'Active Hi' },
        { value: 5, label: 'Sample Hi' },
      ],
    },
    ...overrides,
  };
}

function makeRow(overrides: Partial<OfflineChangeRow> = {}): OfflineChangeRow {
  return {
    changeId: 'draft-1',
    kind: 'config',
    nodeId: '05.02.01.02.03.00',
    space: 253,
    offset: '0x00000064',
    baselineValue: '1',
    plannedValue: '5',
    status: 'pending',
    ...overrides,
  };
}

const constrainedState: ConnectorConstraintState = {
  slotId: 'connector-a',
  hidden: false,
  disabled: false,
  readOnly: false,
  allowedValues: [0, 5],
  deniedValues: [],
  explanations: [],
};

describe('resolveLeafOfflineValueState', () => {
  it('prefers the effective pending row over the committed leaf value', () => {
    const leaf = makeLeaf();

    const state = resolveLeafOfflineValueState({
      leaf,
      effectiveOfflineRow: makeRow({ plannedValue: '5' }),
    });

    expect(state.offlinePlannedValue).toEqual({ type: 'int', value: 5 });
    expect(state.displayValue).toEqual({ type: 'int', value: 5 });
  });
});

describe('resolveLeafSelectViewState', () => {
  it('keeps the fallback label but clears the incompatibility message when a compatible replacement exists', () => {
    const leaf = makeLeaf();

    const state = resolveLeafSelectViewState({
      leaf,
      displayValue: { type: 'int', value: 1 },
      connectorConstraintState: constrainedState,
    });

    expect(state.selectedValue).toBe(1);
    expect(state.currentSelectFallbackLabel).toBe('Active Hi');
    expect(state.currentValueCompatibilityMessage).toBeNull();
  });

  it('clears the incompatibility message when the effective pending value is allowed', () => {
    const leaf = makeLeaf();

    const state = resolveLeafSelectViewState({
      leaf,
      displayValue: { type: 'int', value: 5 },
      connectorConstraintState: constrainedState,
    });

    expect(state.selectedValue).toBe(5);
    expect(state.currentSelectFallbackLabel).toBeNull();
    expect(state.currentValueCompatibilityMessage).toBeNull();
    expect(state.managedMapEntries.map((entry) => entry.value)).toEqual([0, 5]);
  });
});