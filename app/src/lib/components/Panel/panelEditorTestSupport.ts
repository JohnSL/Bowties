/** Test support for the Panel editor components: API boundary mocks and a wiring-facts fixture. */
import type { PanelWiringFacts, PinOwner } from '$lib/api/panels';

export const BOARD_NODE = '050101010101';
export const OTHER_BOARD_NODE = '050101010102';

const HEADS = ['H1', 'H2', 'H3', 'H4'];
const COLORS = ['G', 'Y', 'R', 'L'];

/** Two 16-pin boards labelled #1 H1-G … #16 H4-L, with the given owners on the first board. */
export function wiringFacts(owners: Record<number, PinOwner> = {}): PanelWiringFacts {
  const pins = (own: Record<number, PinOwner>) =>
    HEADS.flatMap((group, h) =>
      COLORS.map((color, c) => {
        const output = h * 4 + c + 1;
        return { output, label: `#${output} ${group}-${color}`, group, owner: own[output] ?? ({ kind: 'free' } as PinOwner) };
      }),
    );
  return {
    boards: [
      { nodeKey: BOARD_NODE, name: 'Eagle Creek Fascia', pins: pins(owners) },
      { nodeKey: OTHER_BOARD_NODE, name: 'West Signals', pins: pins({}) },
    ],
    issues: [],
  };
}
