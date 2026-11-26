import { describe, expect, it } from 'vitest';
import { moveItem } from './moveInList';

describe('moveItem', () => {
  it.each([
    { index: 0, delta: 1 as const, expected: ['b', 'a', 'c'], moved: true },
    { index: 2, delta: -1 as const, expected: ['a', 'c', 'b'], moved: true },
    { index: -1, delta: 1 as const, expected: ['a', 'b', 'c'], moved: false },
    { index: 3, delta: -1 as const, expected: ['a', 'b', 'c'], moved: false },
    { index: 0, delta: -1 as const, expected: ['a', 'b', 'c'], moved: false },
    { index: 2, delta: 1 as const, expected: ['a', 'b', 'c'], moved: false },
  ])('moves index $index by $delta without mutating its input', ({ index, delta, expected, moved }) => {
    const items = ['a', 'b', 'c'];
    const result = moveItem(items, index, delta);
    expect(result).toEqual(expected);
    expect(items).toEqual(['a', 'b', 'c']);
    if (moved) expect(result).not.toBe(items);
    else expect(result).toBe(items);
  });
});
