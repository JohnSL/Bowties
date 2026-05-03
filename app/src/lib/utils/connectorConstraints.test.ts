import { describe, expect, it } from 'vitest';

import type { TreeMapEntry } from '$lib/types/nodeTree';
import type { ConnectorProfileView, ConnectorSelectionDocument } from '$lib/types/connectorProfile';

import {
  evaluateConnectorConstraintsForPath,
  filterAllowedMapEntries,
} from './connectorConstraints';

const OUTPUT_FUNCTION_OPTIONS: TreeMapEntry[] = [
  { value: 0, label: 'No Function' },
  { value: 1, label: 'Steady Active Hi' },
  { value: 2, label: 'Steady Active Lo' },
  { value: 9, label: 'Sample Steady Active Hi' },
  { value: 10, label: 'Sample Steady Active Lo' },
];

const PRODUCER_TRIGGER_OPTIONS: TreeMapEntry[] = [
  { value: 0, label: 'None' },
  { value: 1, label: 'Output State On command' },
  { value: 2, label: 'Output State Off command' },
  { value: 3, label: 'Output On (Function hi)' },
  { value: 4, label: 'Output Off (Function lo)' },
  { value: 5, label: 'Input On' },
  { value: 6, label: 'Input Off' },
  { value: 7, label: 'Gated On (Non Veto Input)' },
  { value: 8, label: 'Gated Off (Non Veto Input)' },
];

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
        supportedDaughterboardIds: ['BOD4', 'FOB-A', 'OI-OB-8'],
        affectedPaths: ['Port I/O/Line#1', 'Port I/O/Line#2'],
        resolvedAffectedPaths: [
          ['seg:2', 'elem:0#1'],
          ['seg:2', 'elem:0#2'],
        ],
      },
      {
        slotId: 'connector-b',
        label: 'Connector B',
        order: 1,
        allowNoneInstalled: true,
        supportedDaughterboardIds: ['BOD4', 'FOB-A', 'OI-OB-8'],
        affectedPaths: ['Port I/O/Line#9', 'Port I/O/Line#10'],
        resolvedAffectedPaths: [
          ['seg:2', 'elem:0#9'],
          ['seg:2', 'elem:0#10'],
        ],
      },
    ],
    supportedDaughterboards: [
      {
        daughterboardId: 'BOD4',
        displayName: 'BOD4',
        validityRules: [
          {
            targetPath: 'Port I/O/Line/Output Function',
            resolvedPath: ['seg:2', 'elem:0', 'elem:1'],
            effect: 'allowValues',
            lineOrdinals: [1],
            allowedValues: [0],
            explanation: 'Detector boards do not drive Tower-LCC output functions.',
          },
          {
            targetPath: 'Port I/O/Line/Event#2/Upon this action',
            resolvedPath: ['seg:2', 'elem:0', 'elem:5', 'elem:0'],
            effect: 'allowValues',
            lineOrdinals: [1],
            allowedValues: [0, 5, 6, 7, 8],
            explanation: 'Detector lines can only send producer indications from input transitions.',
          },
          {
            targetPath: 'Port I/O/Line/Event#1',
            resolvedPath: ['seg:2', 'elem:0', 'elem:4'],
            effect: 'hide',
            explanation: 'Consumer output events do not apply to detector lines.',
          },
        ],
      },
      {
        daughterboardId: 'FOB-A',
        displayName: 'FOB-A',
        validityRules: [
          {
            targetPath: 'Port I/O/Line/Input Function',
            resolvedPath: ['seg:2', 'elem:0', 'elem:2'],
            effect: 'allowValues',
            allowedValues: [0, 1, 2, 5, 6],
          },
          {
            targetPath: 'Port I/O/Line/Event#2/Upon this action',
            resolvedPath: ['seg:2', 'elem:0', 'elem:5', 'elem:0'],
            effect: 'allowValues',
            allowedValues: [0, 5, 6, 7, 8],
          },
        ],
      },
      {
        daughterboardId: 'OI-OB-8',
        displayName: 'OI-OB-8',
        validityRules: [
          {
            targetPath: 'Port I/O/Line/Input Function',
            resolvedPath: ['seg:2', 'elem:0', 'elem:2'],
            effect: 'allowValues',
            allowedValues: [0],
          },
          {
            targetPath: 'Port I/O/Line/Event#2/Upon this action',
            resolvedPath: ['seg:2', 'elem:0', 'elem:5', 'elem:0'],
            effect: 'allowValues',
            allowedValues: [0, 1, 2, 3, 4],
          },
        ],
      },
    ],
  };
}

function makeProfileWithEmptyBehavior(): ConnectorProfileView {
  const profile = makeProfile();
  return {
    ...profile,
    slots: profile.slots.map((slot) => (
      slot.slotId === 'connector-a'
        ? {
            ...slot,
            baseBehaviorWhenEmpty: {
              effect: 'disable',
              allowedValues: [0],
            },
          }
        : slot
    )),
  };
}

function makeSelections(slotSelections: ConnectorSelectionDocument['slotSelections']): ConnectorSelectionDocument {
  return {
    nodeId: '05.02.01.02.03.00',
    carrierKey: 'rr-cirkits::tower-lcc',
    slotSelections,
  };
}

describe('evaluateConnectorConstraintsForPath', () => {
  it('applies the selected slot rules only to the governed line instance', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD4', status: 'selected' },
      { slotId: 'connector-b', selectedDaughterboardId: 'FOB-A', status: 'selected' },
    ]);

    const lineOneOutput = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:1'],
    );
    const lineNineOutput = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#9', 'elem:1'],
    );

    expect(lineOneOutput.slotId).toBe('connector-a');
    expect(lineOneOutput.hidden).toBe(false);
    expect(lineOneOutput.disabled).toBe(false);
    expect(lineOneOutput.allowedValues).toEqual([0]);

    expect(lineNineOutput.slotId).toBe('connector-b');
    expect(lineNineOutput.allowedValues).toBeNull();
  });

  it('applies slot-relative line ordinal rules only to the matching affected line', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD4', status: 'selected' },
      { slotId: 'connector-b', selectedDaughterboardId: undefined, status: 'none' },
    ]);

    const lineOneOutput = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:1'],
    );
    const lineTwoOutput = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#2', 'elem:1'],
    );

    expect(lineOneOutput.allowedValues).toEqual([0]);
    expect(lineTwoOutput.allowedValues).toBeNull();
  });

  it('applies no additional constraints when no daughterboard is installed and no empty behavior is authored', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: undefined, status: 'none' },
      { slotId: 'connector-b', selectedDaughterboardId: 'FOB-A', status: 'selected' },
    ]);

    const state = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:1'],
    );

    expect(state.slotId).toBe('connector-a');
    expect(state.hidden).toBe(false);
    expect(state.disabled).toBe(false);
    expect(state.allowedValues).toBeNull();
  });

  it('falls back to the slot empty behavior only when the profile explicitly authors it', () => {
    const profile = makeProfileWithEmptyBehavior();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: undefined, status: 'none' },
      { slotId: 'connector-b', selectedDaughterboardId: 'FOB-A', status: 'selected' },
    ]);

    const state = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:1'],
    );

    expect(state.slotId).toBe('connector-a');
    expect(state.hidden).toBe(false);
    expect(state.disabled).toBe(true);
    expect(state.allowedValues).toEqual([0]);
  });

  it('marks governed sections hidden when the selected daughterboard hides that target', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD4', status: 'selected' },
      { slotId: 'connector-b', selectedDaughterboardId: undefined, status: 'none' },
    ]);

    const state = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:4'],
    );

    expect(state.hidden).toBe(true);
    expect(state.explanations).toContain(
      'Consumer output events do not apply to detector lines.',
    );
  });

  it('filters input-only producer trigger actions without hiding the indicator field', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD4', status: 'selected' },
      { slotId: 'connector-b', selectedDaughterboardId: undefined, status: 'none' },
    ]);

    const triggerState = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:5', 'elem:0'],
    );
    const indicatorState = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:5', 'elem:1'],
    );

    expect(triggerState.allowedValues).toEqual([0, 5, 6, 7, 8]);
    expect(indicatorState.hidden).toBe(false);
    expect(indicatorState.allowedValues).toBeNull();
  });

  it('filters output-only producer trigger actions without hiding the indicator field', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: undefined, status: 'none' },
      { slotId: 'connector-b', selectedDaughterboardId: 'OI-OB-8', status: 'selected' },
    ]);

    const triggerState = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#9', 'elem:5', 'elem:0'],
    );
    const indicatorState = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#9', 'elem:5', 'elem:1'],
    );

    expect(triggerState.allowedValues).toEqual([0, 1, 2, 3, 4]);
    expect(indicatorState.hidden).toBe(false);
    expect(indicatorState.allowedValues).toBeNull();
  });
});

describe('filterAllowedMapEntries', () => {
  it('narrows enum options to the values allowed by the active connector rule', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD4', status: 'selected' },
      { slotId: 'connector-b', selectedDaughterboardId: 'FOB-A', status: 'selected' },
    ]);

    const state = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:1'],
    );

    expect(filterAllowedMapEntries(OUTPUT_FUNCTION_OPTIONS, state)).toEqual([
      { value: 0, label: 'No Function' },
    ]);
  });

  it('narrows producer-trigger options to the input-driven values for detector boards', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD4', status: 'selected' },
      { slotId: 'connector-b', selectedDaughterboardId: undefined, status: 'none' },
    ]);

    const state = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#1', 'elem:5', 'elem:0'],
    );

    expect(filterAllowedMapEntries(PRODUCER_TRIGGER_OPTIONS, state)).toEqual([
      { value: 0, label: 'None' },
      { value: 5, label: 'Input On' },
      { value: 6, label: 'Input Off' },
      { value: 7, label: 'Gated On (Non Veto Input)' },
      { value: 8, label: 'Gated Off (Non Veto Input)' },
    ]);
  });

  it('narrows producer-trigger options to the output-driven values for output-only boards', () => {
    const profile = makeProfile();
    const document = makeSelections([
      { slotId: 'connector-a', selectedDaughterboardId: undefined, status: 'none' },
      { slotId: 'connector-b', selectedDaughterboardId: 'OI-OB-8', status: 'selected' },
    ]);

    const state = evaluateConnectorConstraintsForPath(
      profile,
      document,
      ['seg:2', 'elem:0#9', 'elem:5', 'elem:0'],
    );

    expect(filterAllowedMapEntries(PRODUCER_TRIGGER_OPTIONS, state)).toEqual([
      { value: 0, label: 'None' },
      { value: 1, label: 'Output State On command' },
      { value: 2, label: 'Output State Off command' },
      { value: 3, label: 'Output On (Function hi)' },
      { value: 4, label: 'Output Off (Function lo)' },
    ]);
  });
});