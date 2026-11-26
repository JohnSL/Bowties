import { describe, it, expect } from 'vitest';
import { removalConfirmationGroups } from './facilityRemovalText';
import type { FacilityRemovalClaim } from '$lib/api/facilities';

const names: Record<string, string> = { A: 'Board A', B: 'Board B' };
const resolvers = {
  boardName: (nodeKey: string) => names[nodeKey] ?? nodeKey,
  pinLabel: (_nodeKey: string, lampSelection: number) => `#${lampSelection} H3-G`,
};

describe('removalConfirmationGroups', () => {
  it('lists each claim under its board in first-seen order, labelled as the panel card labels them', () => {
    const claims: FacilityRemovalClaim[] = [
      { nodeKey: 'B', kind: 'mast', index: 2, lampSelection: null },
      { nodeKey: 'A', kind: 'logic', index: 1, lampSelection: null },
      { nodeKey: 'B', kind: 'trackCircuit', index: 3, lampSelection: null },
      { nodeKey: 'A', kind: 'directLamp', index: 4, lampSelection: 9 },
      { nodeKey: 'A', kind: 'directLamp', index: 5, lampSelection: null },
    ];

    expect(removalConfirmationGroups(claims, resolvers)).toEqual([
      { board: 'Board B', nodeKey: 'B', items: ['Mast 2', 'Track Circuit 3'] },
      { board: 'Board A', nodeKey: 'A', items: ['Logic 1', '#9 H3-G', 'row 5 (no pin set)'] },
    ]);
  });
});
