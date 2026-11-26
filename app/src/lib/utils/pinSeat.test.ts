import { describe, it, expect } from 'vitest';
import { seatPin } from './pinSeat';

type Pins = Record<'a' | 'b' | 'c', number | null>;

describe('seatPin', () => {
  it('seats the claimant on a free pin', () => {
    const r = seatPin<'a' | 'b' | 'c', number>({ a: null, b: 2, c: null }, 'a', 1, { taken: 'swap' });
    expect(r.assignments).toEqual({ a: 1, b: 2, c: null });
    expect(r.displaced).toBeNull();
  });

  it('moves the claimant off its old pin when seating on a free pin', () => {
    const r = seatPin<'a' | 'b', number>({ a: 1, b: null }, 'a', 3, { taken: 'swap' });
    expect(r.assignments).toEqual({ a: 3, b: null });
  });

  it('clears the claimant when it claims its own pin', () => {
    const r = seatPin<'a' | 'b', number>({ a: 1, b: 2 }, 'a', 1, { taken: 'swap' });
    expect(r.assignments).toEqual({ a: null, b: 2 });
    expect(r.displaced).toBeNull();
  });

  it('swap: the other claimant takes the claimant\'s old pin and is named displaced', () => {
    const r = seatPin<'a' | 'b', number>({ a: 1, b: 2 }, 'a', 2, { taken: 'swap' });
    expect(r.assignments).toEqual({ a: 2, b: 1 });
    expect(r.displaced).toBe('b');
  });

  it('swap: the other claimant ends with no pin when the claimant had none', () => {
    const r = seatPin<'a' | 'b', number>({ a: null, b: 2 }, 'a', 2, { taken: 'swap' });
    expect(r.assignments).toEqual({ a: 2, b: null });
    expect(r.displaced).toBe('b');
  });

  it('move: the other claimant ends with no pin even when the claimant had one', () => {
    const r = seatPin<'a' | 'b', number>({ a: 1, b: 2 }, 'a', 2, { taken: 'move' });
    expect(r.assignments).toEqual({ a: 2, b: null });
    expect(r.displaced).toBe('b');
  });

  it('identifies pins with a custom samePin comparator', () => {
    type P = { node: string; out: number };
    const same = (x: P, y: P) => x.node === y.node && x.out === y.out;
    const r = seatPin<'a' | 'b', P>(
      { a: null, b: { node: 'N', out: 1 } },
      'a',
      { node: 'N', out: 1 },
      { taken: 'move', samePin: same },
    );
    expect(r.assignments.a).toEqual({ node: 'N', out: 1 });
    expect(r.assignments.b).toBeNull();
    expect(r.displaced).toBe('b');
  });

  it('does not mutate the input', () => {
    const input: Pins = { a: 1, b: 2, c: null };
    const snapshot = { ...input };
    const r = seatPin(input, 'a', 2, { taken: 'swap' });
    expect(input).toEqual(snapshot);
    expect(r.assignments).not.toBe(input);
  });
});
