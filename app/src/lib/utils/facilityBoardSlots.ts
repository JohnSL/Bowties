import type { Facility, NodeLogicPlacement } from '$lib/api/facilities';

/** A board slot a facility occupies, for display. `index` is the 1-based number on the board. */
export interface BoardSlot {
  kind: 'mast' | 'logic' | 'trackCircuit';
  index: number;
}

/** The slots a facility occupies on one board. */
export interface BoardSlotGroup {
  nodeKey: string;
  slots: BoardSlot[];
}

/**
 * Each board an Automatic Block Signals facility runs on (in signal order) and
 * the masts, Logic and Track Circuits it occupies there: each signal's mast and
 * circuits from its resources, each Logic of the facility's groups from that
 * board's placement. Pure derivation; ownership of both sources stays with the
 * facilities store.
 */
export function facilityBoardSlots(
  facility: Facility,
  logicPlacement: readonly NodeLogicPlacement[],
): BoardSlotGroup[] {
  if (!facility.blockSignals) return [];
  const signals = facility.blockSignals.signals;
  const nodeKeys = [...new Set(signals.map((signal) => signal.resources.nodeKey))];
  return nodeKeys.map((nodeKey) => {
    const onBoard = signals.filter((signal) => signal.resources.nodeKey === nodeKey);
    const slots: BoardSlot[] = onBoard.map((signal) => ({ kind: 'mast', index: signal.resources.mast }));
    for (const board of logicPlacement) {
      if (board.nodeKey !== nodeKey) continue;
      for (const group of board.groups) {
        if (group.facilityId !== facility.facilityId) continue;
        for (let offset = 0; offset < group.length; offset++) slots.push({ kind: 'logic', index: group.first + offset });
      }
    }
    for (const signal of onBoard) {
      for (const index of signal.resources.trackCircuits) slots.push({ kind: 'trackCircuit', index });
    }
    return { nodeKey, slots };
  });
}
