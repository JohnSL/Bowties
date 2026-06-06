import { describe, it, expect } from 'vitest';
import {
  nodeKey,
  nodeKeyEquals,
  nodeKeyToString,
  nodeKeyToDisplay,
  toCanonicalNodeKey,
  isPlaceholderInput,
} from './nodeKey';

describe('isPlaceholderInput', () => {
  it('returns true for placeholder-prefixed strings', () => {
    expect(
      isPlaceholderInput('placeholder:01234567-89ab-cdef-0123-456789abcdef'),
    ).toBe(true);
  });

  it('returns true for branded placeholder NodeKey', () => {
    expect(isPlaceholderInput(nodeKey('placeholder:abc'))).toBe(true);
  });

  it('returns false for canonical live NodeIDs', () => {
    expect(isPlaceholderInput('05010101148A')).toBe(false);
    expect(isPlaceholderInput('05.01.01.01.14.8A')).toBe(false);
  });

  it('returns false for branded live NodeKey', () => {
    expect(isPlaceholderInput(nodeKey('05010101148A'))).toBe(false);
  });

  it('returns false for empty / null / undefined', () => {
    expect(isPlaceholderInput('')).toBe(false);
    expect(isPlaceholderInput(undefined)).toBe(false);
    expect(isPlaceholderInput(null)).toBe(false);
  });

  it('uses exact prefix match (mirrors backend, case-sensitive)', () => {
    expect(isPlaceholderInput('Placeholder:abc')).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Branded NodeKey factory + operators (Step 5 — frontend mirror of the
// backend `NodeKey` sum type, ADR-0010). Tests-first contract:
//
//   - `nodeKey(input)` is the only public constructor.
//   - Live inputs parse from dotted OR canonical form and normalize to the
//     canonical 12-hex id (matching backend `NodeKey::Live` wire form).
//   - Placeholder inputs preserve the uuid after the `placeholder:` prefix.
//   - Invalid input throws. There is no `as NodeKey` shortcut.
//   - `nodeKeyEquals` compares by kind + id, not by reference.
//   - `nodeKeyToString` round-trips with the backend wire form.
//   - `nodeKeyToDisplay` returns dotted form for live, placeholder verbatim.
// ---------------------------------------------------------------------------

describe('nodeKey factory', () => {
  it('parses dotted live form to canonical id', () => {
    const k = nodeKey('02.01.57.00.02.D9');
    expect(k.kind).toBe('live');
    expect(k.id).toBe('0201570002D9');
  });

  it('parses canonical live form unchanged', () => {
    const k = nodeKey('0201570002D9');
    expect(k.kind).toBe('live');
    expect(k.id).toBe('0201570002D9');
  });

  it('uppercases lowercase hex live form', () => {
    const k = nodeKey('02.01.57.00.02.d9');
    expect(k.kind).toBe('live');
    expect(k.id).toBe('0201570002D9');
  });

  it('parses placeholder form preserving the uuid', () => {
    const k = nodeKey('placeholder:01234567-89ab-cdef-0123-456789abcdef');
    expect(k.kind).toBe('placeholder');
    expect(k.id).toBe('01234567-89ab-cdef-0123-456789abcdef');
  });

  it('throws on garbage input', () => {
    expect(() => nodeKey('garbage')).toThrow();
    expect(() => nodeKey('')).toThrow();
    expect(() => nodeKey('02.01.57.00.02')).toThrow(); // too few hex pairs
    expect(() => nodeKey('02.01.57.00.02.ZZ')).toThrow(); // not hex
    expect(() => nodeKey('placeholder:')).toThrow(); // empty placeholder id
  });
});

describe('nodeKeyEquals', () => {
  it('treats dotted and canonical live forms as equal', () => {
    const a = nodeKey('02.01.57.00.02.D9');
    const b = nodeKey('0201570002d9');
    expect(nodeKeyEquals(a, b)).toBe(true);
  });

  it('is false across kinds even when ids match strings', () => {
    const live = nodeKey('0201570002D9');
    const placeholder = nodeKey('placeholder:0201570002D9');
    expect(nodeKeyEquals(live, placeholder)).toBe(false);
  });

  it('is true for matching placeholder uuids', () => {
    const a = nodeKey('placeholder:abc-123');
    const b = nodeKey('placeholder:abc-123');
    expect(nodeKeyEquals(a, b)).toBe(true);
  });

  it('is false for different placeholder uuids', () => {
    const a = nodeKey('placeholder:abc-123');
    const b = nodeKey('placeholder:def-456');
    expect(nodeKeyEquals(a, b)).toBe(false);
  });
});

describe('nodeKeyToString (wire form)', () => {
  it('returns canonical 12-hex for live keys', () => {
    expect(nodeKeyToString(nodeKey('02.01.57.00.02.D9'))).toBe('0201570002D9');
    expect(nodeKeyToString(nodeKey('0201570002d9'))).toBe('0201570002D9');
  });

  it('returns the placeholder:<uuid> form for placeholders', () => {
    const wire = 'placeholder:01234567-89ab-cdef-0123-456789abcdef';
    expect(nodeKeyToString(nodeKey(wire))).toBe(wire);
  });

  it('round-trips through the factory', () => {
    const original = nodeKey('02.01.57.00.02.D9');
    const round = nodeKey(nodeKeyToString(original));
    expect(nodeKeyEquals(original, round)).toBe(true);
  });
});

describe('nodeKeyToDisplay', () => {
  it('returns dotted form for live keys', () => {
    expect(nodeKeyToDisplay(nodeKey('0201570002D9'))).toBe('02.01.57.00.02.D9');
  });

  it('returns the placeholder verbatim for placeholders', () => {
    const wire = 'placeholder:01234567-89ab-cdef-0123-456789abcdef';
    expect(nodeKeyToDisplay(nodeKey(wire))).toBe(wire);
  });
});

describe('toCanonicalNodeKey', () => {
  it('canonicalizes dotted strings to wire form', () => {
    expect(toCanonicalNodeKey('02.01.57.00.02.d9')).toBe('0201570002D9');
  });

  it('preserves placeholder strings verbatim', () => {
    const wire = 'placeholder:01234567-89ab-cdef-0123-456789abcdef';
    expect(toCanonicalNodeKey(wire)).toBe(wire);
  });

  it('serialises branded live keys to wire form', () => {
    expect(toCanonicalNodeKey(nodeKey('02.01.57.00.02.D9'))).toBe('0201570002D9');
  });

  it('serialises branded placeholder keys to wire form', () => {
    const wire = 'placeholder:abc-123';
    expect(toCanonicalNodeKey(nodeKey(wire))).toBe(wire);
  });
});
