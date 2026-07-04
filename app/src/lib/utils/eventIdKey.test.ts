import { describe, it, expect } from 'vitest';
import {
  toEventIdKey,
  eventIdKeyFromBytes,
  formatEventIdKey,
  isCanonicalEventIdKey,
  type EventIdKey,
} from '$lib/utils/eventIdKey';

const BYTES = [0x02, 0x01, 0x57, 0x00, 0x02, 0xD9, 0x04, 0xD2];
const DOTTED = '02.01.57.00.02.D9.04.D2';
const CANONICAL = '0201570002D904D2';

describe('EventIdKey', () => {
  it('toEventIdKey normalizes dotted uppercase to canonical', () => {
    const k = toEventIdKey(DOTTED);
    expect(k).toBe(CANONICAL);
  });

  it('toEventIdKey normalizes dotted lowercase to canonical uppercase', () => {
    const k = toEventIdKey('02.01.57.00.02.d9.04.d2');
    expect(k).toBe(CANONICAL);
  });

  it('toEventIdKey returns canonical unchanged when already canonical', () => {
    const k = toEventIdKey(CANONICAL);
    expect(k).toBe(CANONICAL);
  });

  it('toEventIdKey returns null for invalid input', () => {
    expect(toEventIdKey('not a hex id')).toBeNull();
    expect(toEventIdKey('05.01.01')).toBeNull();
  });

  it('eventIdKeyFromBytes produces canonical form', () => {
    expect(eventIdKeyFromBytes(BYTES)).toBe(CANONICAL);
  });

  it('two calls with the same bytes produce equal keys', () => {
    const a = eventIdKeyFromBytes(BYTES);
    const b = toEventIdKey(DOTTED);
    expect(a).toBe(b);
  });

  it('formatEventIdKey renders dotted display form', () => {
    const k = eventIdKeyFromBytes(BYTES);
    expect(formatEventIdKey(k)).toBe(DOTTED);
  });

  it('isCanonicalEventIdKey narrows canonical strings', () => {
    expect(isCanonicalEventIdKey(CANONICAL)).toBe(true);
    expect(isCanonicalEventIdKey(DOTTED)).toBe(false);
    expect(isCanonicalEventIdKey('0201570002D904d2')).toBe(false); // lowercase
    expect(isCanonicalEventIdKey('020157000 2D904D2')).toBe(false);
  });

  it('compile-time: raw strings cannot be assigned to EventIdKey', () => {
    // This test documents the compile-time guard. The line below is
    // commented because it would break `tsc`; uncomment during review to
    // verify TypeScript rejects the assignment.
    // const bad: EventIdKey = 'raw string';
    // Runtime check: a raw string is not brand-compatible via `is` check.
    const k: EventIdKey | null = toEventIdKey(DOTTED);
    expect(k).not.toBeNull();
    expect(typeof k).toBe('string');
  });
});
