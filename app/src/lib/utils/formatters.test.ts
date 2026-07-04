/**
 * Tests for formatters utility functions.
 *
 * Covers:
 * - formatTreeConfigValue for all TreeConfigValue variants
 * - map-entry label resolution for int values
 * - null input handling
 */

import { describe, it, expect } from 'vitest';
import { formatTreeConfigValue } from '$lib/utils/formatters';
import type { TreeConfigValue, TreeMapEntry } from '$lib/types/nodeTree';

// ─── Helpers ─────────────────────────────────────────────────────────────────

function intVal(value: number): TreeConfigValue {
  return { type: 'int', value };
}

function strVal(value: string): TreeConfigValue {
  return { type: 'string', value };
}

function eventVal(bytes: number[]): TreeConfigValue {
  const hex = bytes.map((b) => b.toString(16).padStart(2, '0').toUpperCase()).join('.');
  return { type: 'eventId', bytes, hex };
}

function floatVal(value: number): TreeConfigValue {
  return { type: 'float', value };
}

function mapEntry(value: number, label: string): TreeMapEntry {
  return { value, label };
}

// ─── formatTreeConfigValue ────────────────────────────────────────────────────

describe('formatTreeConfigValue — int', () => {
  it('formats a plain int as decimal string when no map entries', () => {
    expect(formatTreeConfigValue(intVal(42))).toBe('42');
    expect(formatTreeConfigValue(intVal(0))).toBe('0');
    expect(formatTreeConfigValue(intVal(255))).toBe('255');
  });

  it('returns the map-entry label when value matches', () => {
    const map: TreeMapEntry[] = [mapEntry(0, 'None'), mapEntry(1, 'Steady'), mapEntry(2, 'Pulse')];
    expect(formatTreeConfigValue(intVal(0), map)).toBe('None');
    expect(formatTreeConfigValue(intVal(1), map)).toBe('Steady');
    expect(formatTreeConfigValue(intVal(2), map)).toBe('Pulse');
  });

  it('falls back to decimal string when int value has no matching map entry', () => {
    const map: TreeMapEntry[] = [mapEntry(1, 'Steady'), mapEntry(2, 'Pulse')];
    expect(formatTreeConfigValue(intVal(5), map)).toBe('5');
  });

  it('formats int as decimal when map entries array is empty', () => {
    expect(formatTreeConfigValue(intVal(7), [])).toBe('7');
  });

  it('formats int as decimal when map entries is null', () => {
    expect(formatTreeConfigValue(intVal(7), null)).toBe('7');
  });

  it('formats int as decimal when map entries is undefined', () => {
    expect(formatTreeConfigValue(intVal(7), undefined)).toBe('7');
  });
});

describe('formatTreeConfigValue — string', () => {
  it('returns the string value as-is', () => {
    expect(formatTreeConfigValue(strVal('Tower LCC'))).toBe('Tower LCC');
    expect(formatTreeConfigValue(strVal(''))).toBe('');
  });

  it('ignores map entries for string type', () => {
    const map: TreeMapEntry[] = [mapEntry(0, 'Ignored')];
    expect(formatTreeConfigValue(strVal('hello'), map)).toBe('hello');
  });
});

describe('formatTreeConfigValue — eventId', () => {
  it('returns dotted-hex uppercase string', () => {
    const v = eventVal([1, 2, 3, 4, 5, 6, 7, 8]);
    expect(formatTreeConfigValue(v)).toBe('01.02.03.04.05.06.07.08');
  });

  it('converts undotted hex to dotted display form', () => {
    const v: TreeConfigValue = {
      type: 'eventId',
      bytes: [2, 1, 0x57, 0x10, 0x09, 0x97, 2, 0xc1],
      hex: '02015710099702C1',
    };
    expect(formatTreeConfigValue(v)).toBe('02.01.57.10.09.97.02.C1');
  });

  it('returns already-dotted hex unchanged', () => {
    const v: TreeConfigValue = {
      type: 'eventId',
      bytes: [5, 1, 1, 1, 3, 1, 0, 0],
      hex: '05.01.01.01.03.01.00.00',
    };
    expect(formatTreeConfigValue(v)).toBe('05.01.01.01.03.01.00.00');
  });
});

describe('formatTreeConfigValue — float', () => {
  it('formats float with two decimal places', () => {
    expect(formatTreeConfigValue(floatVal(3.14))).toBe('3.14');
    expect(formatTreeConfigValue(floatVal(1.5))).toBe('1.50');
    expect(formatTreeConfigValue(floatVal(0.0))).toBe('0.00');
  });
});

describe('formatTreeConfigValue — null', () => {
  it('returns empty string for null value', () => {
    expect(formatTreeConfigValue(null)).toBe('');
  });

  it('returns empty string for null value with map entries', () => {
    const map: TreeMapEntry[] = [mapEntry(1, 'Steady')];
    expect(formatTreeConfigValue(null, map)).toBe('');
  });
});
