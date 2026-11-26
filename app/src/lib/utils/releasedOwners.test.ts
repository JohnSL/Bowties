import { describe, it, expect } from 'vitest';
import { withReleasedOwner } from './releasedOwners';

describe('withReleasedOwner', () => {
  it('adds an owner once, treating differently formatted node keys as the same node', () => {
    const first = withReleasedOwner([], { nodeKey: '050101010001', owner: 'Mast 1' });
    const again = withReleasedOwner(first, { nodeKey: '05.01.01.01.00.01', owner: 'Mast 1' });
    expect(again).toHaveLength(1);

    const other = withReleasedOwner(again, { nodeKey: '050101010001', owner: 'Direct lamp 2' });
    expect(other.map((r) => r.owner)).toEqual(['Mast 1', 'Direct lamp 2']);
  });
});
