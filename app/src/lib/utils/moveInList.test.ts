import { describe, expect, it } from 'vitest';
import { moveItem } from './moveInList';
import * as reorder from './moveInList';

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

describe('moveItemTo', () => {
  it.each([
    { from: 0, to: 2, expected: ['b', 'c', 'a', 'd'], moved: true },
    { from: 3, to: 0, expected: ['d', 'a', 'b', 'c'], moved: true },
    { from: 1, to: 2, expected: ['a', 'c', 'b', 'd'], moved: true },
    { from: 2, to: 2, expected: ['a', 'b', 'c', 'd'], moved: false },
    { from: -1, to: 1, expected: ['a', 'b', 'c', 'd'], moved: false },
    { from: 4, to: 1, expected: ['a', 'b', 'c', 'd'], moved: false },
    { from: 1, to: -1, expected: ['a', 'b', 'c', 'd'], moved: false },
    { from: 1, to: 4, expected: ['a', 'b', 'c', 'd'], moved: false },
    { from: 1.5, to: 2, expected: ['a', 'b', 'c', 'd'], moved: false },
    { from: 1, to: 2.5, expected: ['a', 'b', 'c', 'd'], moved: false },
    { from: Number.NaN, to: 2, expected: ['a', 'b', 'c', 'd'], moved: false },
  ])('moves index $from to $to without mutating its input', ({ from, to, expected, moved }) => {
    expect(reorder).toHaveProperty('moveItemTo', expect.any(Function));
    const items = ['a', 'b', 'c', 'd'];
    const result = reorder.moveItemTo(items, from, to);
    expect(result).toEqual(expected);
    expect(items).toEqual(['a', 'b', 'c', 'd']);
    if (moved) expect(result).not.toBe(items);
    else expect(result).toBe(items);
  });
});

describe('dropSlot', () => {
  it.each([
    { midpoints: [], y: 10, expected: 0 },
    { midpoints: [10, 20, 30], y: 5, expected: 0 },
    { midpoints: [10, 20, 30], y: 10, expected: 1 },
    { midpoints: [10, 20, 30], y: 15, expected: 1 },
    { midpoints: [10, 20, 30], y: 20, expected: 2 },
    { midpoints: [10, 20, 30], y: 30, expected: 3 },
    { midpoints: [10, 20, 30], y: 35, expected: 3 },
    { midpoints: [20, 10, 20], y: 20, expected: 3 },
  ])('returns slot $expected for y $y among $midpoints', ({ midpoints, y, expected }) => {
    expect(reorder).toHaveProperty('dropSlot', expect.any(Function));
    expect(reorder.dropSlot(midpoints, y)).toBe(expected);
  });
});
