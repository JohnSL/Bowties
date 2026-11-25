import { describe, expect, it } from 'vitest';
import type { Facility, NodeLogicPlacement } from '$lib/api/facilities';
import { facilityBoardSlots } from '$lib/utils/facilityBoardSlots';

const BOARD = '020157000001';
const OTHER = '020157000002';

function facility(): Facility {
  return {
    facilityId: 'f-1',
    templateId: 'automatic-block-signals',
    name: 'ABS',
    slotBindings: {},
    blockSignals: {
      signals: [
        { signalId: 's-1', name: 'One', protectedBlock: 'b-1', channelId: 'c-1', resources: { nodeKey: BOARD, mast: 2, trackCircuits: [] } },
        { signalId: 's-2', name: 'Two', protectedBlock: 'b-2', channelId: 'c-2', resources: { nodeKey: BOARD, mast: 1, trackCircuits: [] } },
      ],
    },
  } as unknown as Facility;
}

describe('facilityBoardSlots', () => {
  const placement: NodeLogicPlacement[] = [{
    nodeKey: BOARD,
    groups: [
      { facilityId: 'f-1', signalId: 's-1', first: 4, length: 2 },
      { facilityId: 'other', signalId: 'x', first: 1, length: 1 },
    ],
  }];

  it('names the board, each signal mast and every Logic of the facility groups, and no one else\'s', () => {
    expect(facilityBoardSlots(facility(), placement)).toEqual([{
      nodeKey: BOARD,
      slots: [
        { kind: 'mast', index: 2 },
        { kind: 'mast', index: 1 },
        { kind: 'logic', index: 4 },
        { kind: 'logic', index: 5 },
      ],
    }]);
  });

  it('groups slots per board, in signal order, with each board own Logic', () => {
    const f = facility();
    f.blockSignals!.signals[1].resources.nodeKey = OTHER;
    f.blockSignals!.signals[0].resources.trackCircuits = [3];
    const two: NodeLogicPlacement[] = [
      { nodeKey: BOARD, groups: [{ facilityId: 'f-1', signalId: 's-1', first: 4, length: 2 }] },
      { nodeKey: OTHER, groups: [{ facilityId: 'f-1', signalId: 's-2', first: 1, length: 1 }] },
    ];
    expect(facilityBoardSlots(f, two)).toEqual([
      { nodeKey: BOARD, slots: [{ kind: 'mast', index: 2 }, { kind: 'logic', index: 4 }, { kind: 'logic', index: 5 }, { kind: 'trackCircuit', index: 3 }] },
      { nodeKey: OTHER, slots: [{ kind: 'mast', index: 1 }, { kind: 'logic', index: 1 }] },
    ]);
  });

  it('names each Track Circuit a signal holds after the masts and Logic', () => {
    const f = facility();
    f.blockSignals!.signals[0].resources.trackCircuits = [3];
    expect(facilityBoardSlots(f, placement)[0].slots).toEqual([
      { kind: 'mast', index: 2 },
      { kind: 'mast', index: 1 },
      { kind: 'logic', index: 4 },
      { kind: 'logic', index: 5 },
      { kind: 'trackCircuit', index: 3 },
    ]);
  });

  it('has no boards for a facility without a block-signals record', () => {
    expect(facilityBoardSlots({ ...facility(), blockSignals: undefined }, placement)).toEqual([]);
  });
});
