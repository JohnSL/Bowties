import { describe, expect, it } from 'vitest';
import {
  buildConnectorSlotSelectors,
  buildSegmentConnectorSlotSelectors,
} from './connectorSlotSelectors';

describe('buildConnectorSlotSelectors', () => {
  it('maps connector profiles and saved selections into ordered selector view models', () => {
    const selectors = buildConnectorSlotSelectors(
      {
        nodeId: '02.01.57.00.00.01',
        carrierKey: 'rr-cirkits::tower-lcc',
        slots: [
          {
            slotId: 'connector-b',
            label: 'Connector B',
            order: 1,
            allowNoneInstalled: true,
            supportedDaughterboardIds: ['zeta-board', 'alpha-board', 'middle-board'],
            affectedPaths: ['Port I/O/Line'],
          },
          {
            slotId: 'connector-a',
            label: 'Connector A',
            order: 0,
            allowNoneInstalled: false,
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
          {
            daughterboardId: 'zeta-board',
            displayName: 'Zeta Board',
            description: 'Late alphabet board',
          },
          {
            daughterboardId: 'alpha-board',
            displayName: 'Alpha Board',
            description: 'Early alphabet board',
          },
          {
            daughterboardId: 'middle-board',
            displayName: 'Middle Board',
            description: 'Middle alphabet board',
          },
        ],
      },
      {
        nodeId: '02.01.57.00.00.01',
        carrierKey: 'rr-cirkits::tower-lcc',
        slotSelections: [
          {
            slotId: 'connector-a',
            selectedDaughterboardId: 'BOD4-CP',
            status: 'selected',
          },
        ],
      },
    );

    expect(selectors.map((selector) => selector.slotId)).toEqual(['connector-a', 'connector-b']);
    expect(selectors[0].selectedDaughterboardId).toBe('BOD4-CP');
    expect(selectors[1].options[0]).toEqual({
      value: '',
      label: 'None installed',
      description: null,
    });
    expect(selectors[1].options.slice(1).map((option) => option.label)).toEqual([
      'Alpha Board',
      'Middle Board',
      'Zeta Board',
    ]);
  });
});

describe('buildSegmentConnectorSlotSelectors', () => {
  it('filters selector view models to slots that affect the selected segment', () => {
    const selectors = buildSegmentConnectorSlotSelectors(
      {
        nodeId: '02.01.57.00.00.01',
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
          {
            slotId: 'serial-expansion',
            label: 'Serial Expansion',
            order: 1,
            allowNoneInstalled: true,
            supportedDaughterboardIds: ['SER-8'],
            affectedPaths: ['Serial Port'],
          },
        ],
        supportedDaughterboards: [
          {
            daughterboardId: 'BOD4-CP',
            displayName: 'BOD4-CP',
          },
          {
            daughterboardId: 'SER-8',
            displayName: 'SER-8',
          },
        ],
      },
      null,
      'Port I/O',
    );

    expect(selectors.map((selector) => selector.slotId)).toEqual(['connector-a']);
  });
});