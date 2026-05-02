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
        supportedDaughterboardIds: ['BOD4', 'FOB-A'],
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
        supportedDaughterboardIds: ['BOD4', 'FOB-A'],
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
            allowedValues: [0, 9, 10],
            explanation: 'Detector boards only allow sample output modes.',
          },
          {
            targetPath: 'Port I/O/Line/Event#2',
            resolvedPath: ['seg:2', 'elem:0', 'elem:5'],
            effect: 'hide',
            explanation: 'Detector boards do not emit producer events for governed lines.',
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
    expect(lineOneOutput.allowedValues).toEqual([0, 9, 10]);

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

    expect(lineOneOutput.allowedValues).toEqual([0, 9, 10]);
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
      ['seg:2', 'elem:0#1', 'elem:5'],
    );

    expect(state.hidden).toBe(true);
    expect(state.explanations).toContain(
      'Detector boards do not emit producer events for governed lines.',
    );
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
      { value: 9, label: 'Sample Steady Active Hi' },
      { value: 10, label: 'Sample Steady Active Lo' },
    ]);
  });
});