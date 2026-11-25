import type { Facility, NodeLogicPlacement } from '$lib/api/facilities';

/** A board slot a facility occupies, for display. `index` is the 1-based number on the board. */
export interface BoardSlot {
  kind: 'mast' | 'logic' | 'trackCircuit';
  index: number;
}

/**
 * The board an Automatic Block Signals facility runs on and the masts,
 * Logic and Track Circuits it occupies: each signal's mast and circuits from
 * its resources, each Logic of the facility's groups from the board placement. Pure derivation; ownership of
 * both sources stays with the facilities store.
 */
export function facilityBoardSlots(
  facility: Facility,
  logicPlacement: readonly NodeLogicPlacement[],
): { nodeKey: string | undefined; slots: BoardSlot[] } {
  if (!facility.blockSignals) return { nodeKey: undefined, slots: [] };
  const signals = facility.blockSignals.signals;
  const nodeKey = signals[0]?.resources.nodeKey;
  const slots: BoardSlot[] = signals.map((signal) => ({ kind: 'mast', index: signal.resources.mast }));
  for (const board of logicPlacement) {
    for (const group of board.groups) {
      if (group.facilityId !== facility.facilityId) continue;
      for (let offset = 0; offset < group.length; offset++) slots.push({ kind: 'logic', index: group.first + offset });
    }
  }
  for (const signal of signals) {
    for (const index of signal.resources.trackCircuits) slots.push({ kind: 'trackCircuit', index });
  }
  return { nodeKey, slots };
}
