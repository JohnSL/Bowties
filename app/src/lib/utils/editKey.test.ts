/**
 * Tests for editKey utilities.
 *
 * Covers:
 * - editKeyForLeaf canonical key construction
 * - parseEditKey round-trip with editKeyForLeaf
 * - addressToOffsetHex / offsetHexToAddress conversion
 * - parseOfflineValueString for all TreeConfigValue variants
 * - configValueToOfflineString round-trip for all variants
 */

import { describe, it, expect } from 'vitest';
import {
  editKeyForLeaf,
  parseEditKey,
  addressToOffsetHex,
  offsetHexToAddress,
  parseOfflineValueString,
  configValueToOfflineString,
} from '$lib/utils/editKey';
import type { TreeConfigValue } from '$lib/types/nodeTree';

// ─── Helpers ─────────────────────────────────────────────────────────────────

function intVal(value: number): TreeConfigValue {
  return { type: 'int', value };
}

function strVal(value: string): TreeConfigValue {
  return { type: 'string', value };
}

function eventVal(bytes: number[]): TreeConfigValue {
  const hex = bytes.map((b) => b.toString(16).padStart(2, '0').toUpperCase()).join('');
  return { type: 'eventId', bytes, hex };
}

function floatVal(value: number): TreeConfigValue {
  return { type: 'float', value };
}

// ─── editKeyForLeaf ───────────────────────────────────────────────────────────

describe('editKeyForLeaf', () => {
  it('produces the canonical key for a dotted nodeId', () => {
    expect(editKeyForLeaf('05.02.01.02.03.00', 253, 100)).toBe('050201020300:253:100');
  });

  it('produces the canonical key for an already-normalized nodeId', () => {
    expect(editKeyForLeaf('050201020300', 253, 100)).toBe('050201020300:253:100');
  });

  it('normalizes dotted and undotted node IDs identically', () => {
    const dotted = editKeyForLeaf('05.02.01.02.03.00', 253, 100);
    const undotted = editKeyForLeaf('050201020300', 253, 100);
    expect(dotted).toBe(undotted);
  });

  it('normalizes node IDs case-insensitively', () => {
    const lower = editKeyForLeaf('0a.0b.0c.0d.0e.0f', 253, 0);
    const upper = editKeyForLeaf('0A.0B.0C.0D.0E.0F', 253, 0);
    expect(lower).toBe(upper);
  });

  it('includes space in key', () => {
    const key1 = editKeyForLeaf('050201020300', 253, 100);
    const key2 = editKeyForLeaf('050201020300', 254, 100);
    expect(key1).not.toBe(key2);
    expect(key1).toContain(':253:');
  });

  it('includes address as decimal in key', () => {
    expect(editKeyForLeaf('050201020300', 253, 100)).toContain(':100');
    expect(editKeyForLeaf('050201020300', 253, 0)).toContain(':0');
  });
});

// ─── parseEditKey ─────────────────────────────────────────────────────────────

describe('parseEditKey', () => {
  it('round-trips with editKeyForLeaf', () => {
    const key = editKeyForLeaf('05.02.01.02.03.00', 253, 100);
    const parsed = parseEditKey(key);
    expect(parsed.normalizedNodeId).toBe('050201020300');
    expect(parsed.space).toBe(253);
    expect(parsed.address).toBe(100);
  });

  it('parses zero address', () => {
    const key = editKeyForLeaf('050201020300', 253, 0);
    const parsed = parseEditKey(key);
    expect(parsed.address).toBe(0);
  });

  it('parses large address', () => {
    const key = editKeyForLeaf('050201020300', 253, 65535);
    const parsed = parseEditKey(key);
    expect(parsed.address).toBe(65535);
  });

  it('parses space correctly across different spaces', () => {
    expect(parseEditKey(editKeyForLeaf('050201020300', 254, 1)).space).toBe(254);
    expect(parseEditKey(editKeyForLeaf('050201020300', 255, 1)).space).toBe(255);
  });

  // Spec 014, ADR-0008 — placeholder NodeKeys contain an internal ':', so
  // round-tripping must split from the right.
  it('round-trips a placeholder NodeKey unchanged', () => {
    const placeholderKey = 'placeholder:01234567-89ab-cdef-0123-456789abcdef';
    const key = editKeyForLeaf(placeholderKey, 253, 42);
    expect(key).toBe(`${placeholderKey}:253:42`);
    const parsed = parseEditKey(key);
    expect(parsed.normalizedNodeId).toBe(placeholderKey);
    expect(parsed.space).toBe(253);
    expect(parsed.address).toBe(42);
  });
});

// ─── addressToOffsetHex ───────────────────────────────────────────────────────

describe('addressToOffsetHex', () => {
  it('converts 100 to "0x00000064"', () => {
    expect(addressToOffsetHex(100)).toBe('0x00000064');
  });

  it('converts 0 to "0x00000000"', () => {
    expect(addressToOffsetHex(0)).toBe('0x00000000');
  });

  it('converts 256 to "0x00000100"', () => {
    expect(addressToOffsetHex(256)).toBe('0x00000100');
  });

  it('pads to 8 hex digits', () => {
    expect(addressToOffsetHex(1)).toBe('0x00000001');
    expect(addressToOffsetHex(0xffffff)).toBe('0x00FFFFFF');
  });

  it('uses uppercase hex digits', () => {
    expect(addressToOffsetHex(0xabcdef)).toMatch(/^0x[0-9A-F]+$/);
  });
});

// ─── offsetHexToAddress ───────────────────────────────────────────────────────

describe('offsetHexToAddress', () => {
  it('converts "0x00000064" to 100', () => {
    expect(offsetHexToAddress('0x00000064')).toBe(100);
  });

  it('converts "0x00000000" to 0', () => {
    expect(offsetHexToAddress('0x00000000')).toBe(0);
  });

  it('handles lowercase hex', () => {
    expect(offsetHexToAddress('0x00000064')).toBe(100);
    expect(offsetHexToAddress('0x0000ffff')).toBe(65535);
  });

  it('handles uppercase hex', () => {
    expect(offsetHexToAddress('0x0000FFFF')).toBe(65535);
  });

  it('handles "0X" prefix (capital X)', () => {
    expect(offsetHexToAddress('0X00000064')).toBe(100);
  });

  it('round-trips with addressToOffsetHex', () => {
    const addresses = [0, 1, 100, 256, 65535, 0x00123456];
    for (const addr of addresses) {
      expect(offsetHexToAddress(addressToOffsetHex(addr))).toBe(addr);
    }
  });
});

// ─── parseOfflineValueString ──────────────────────────────────────────────────

describe('parseOfflineValueString', () => {
  it('parses integer strings as int', () => {
    expect(parseOfflineValueString('3')).toEqual(intVal(3));
    expect(parseOfflineValueString('0')).toEqual(intVal(0));
    expect(parseOfflineValueString('255')).toEqual(intVal(255));
    expect(parseOfflineValueString('65535')).toEqual(intVal(65535));
  });

  it('parses decimal strings as float', () => {
    expect(parseOfflineValueString('3.14')).toEqual(floatVal(3.14));
    expect(parseOfflineValueString('0.0')).toEqual(floatVal(0.0));
    expect(parseOfflineValueString('1.5')).toEqual(floatVal(1.5));
  });

  it('parses dotted-hex 8-byte strings as eventId', () => {
    const result = parseOfflineValueString('01.02.03.04.05.06.07.08');
    expect(result.type).toBe('eventId');
    if (result.type === 'eventId') {
      expect(result.bytes).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
      expect(result.hex).toBe('0102030405060708');
    }
  });

  it('parses dotted-hex event IDs case-insensitively (normalizes to uppercase)', () => {
    const result = parseOfflineValueString('01.02.03.04.05.06.07.0a');
    expect(result.type).toBe('eventId');
    if (result.type === 'eventId') {
      expect(result.bytes).toEqual([1, 2, 3, 4, 5, 6, 7, 10]);
      expect(result.hex).toBe('010203040506070A');
    }
  });

  it('parses arbitrary strings as string', () => {
    expect(parseOfflineValueString('hello world')).toEqual(strVal('hello world'));
    expect(parseOfflineValueString('')).toEqual(strVal(''));
    expect(parseOfflineValueString('Tower LCC')).toEqual(strVal('Tower LCC'));
  });

  it('does not parse 7-segment dotted-hex as eventId (wrong length)', () => {
    const result = parseOfflineValueString('01.02.03.04.05.06.07');
    expect(result.type).toBe('string');
  });

  it('does not parse a decimal-looking string with no fractional as float', () => {
    // "3" is int, not float
    expect(parseOfflineValueString('3').type).toBe('int');
  });
});

// ─── configValueToOfflineString ───────────────────────────────────────────────

describe('configValueToOfflineString', () => {
  it('serializes int as decimal string', () => {
    expect(configValueToOfflineString(intVal(3))).toBe('3');
    expect(configValueToOfflineString(intVal(0))).toBe('0');
    expect(configValueToOfflineString(intVal(255))).toBe('255');
  });

  it('serializes float as decimal string', () => {
    expect(configValueToOfflineString(floatVal(3.14))).toBe('3.14');
    expect(configValueToOfflineString(floatVal(1.5))).toBe('1.5');
  });

  it('serializes string as-is', () => {
    expect(configValueToOfflineString(strVal('hello world'))).toBe('hello world');
    expect(configValueToOfflineString(strVal(''))).toBe('');
  });

  it('serializes eventId as canonical contiguous hex (ADR-0010)', () => {
    const v = eventVal([1, 2, 3, 4, 5, 6, 7, 8]);
    expect(configValueToOfflineString(v)).toBe('0102030405060708');
  });

  it('round-trips int with parseOfflineValueString', () => {
    const original = intVal(42);
    const serialized = configValueToOfflineString(original);
    expect(parseOfflineValueString(serialized)).toEqual(original);
  });

  it('round-trips float with parseOfflineValueString', () => {
    const original = floatVal(3.14);
    const serialized = configValueToOfflineString(original);
    expect(parseOfflineValueString(serialized)).toEqual(original);
  });

  it('round-trips string with parseOfflineValueString', () => {
    const original = strVal('Tower LCC');
    const serialized = configValueToOfflineString(original);
    expect(parseOfflineValueString(serialized)).toEqual(original);
  });

  it('round-trips eventId with parseOfflineValueString', () => {
    const original = eventVal([1, 2, 3, 4, 5, 6, 7, 8]);
    const serialized = configValueToOfflineString(original);
    const parsed = parseOfflineValueString(serialized);
    expect(parsed.type).toBe('eventId');
    expect(parsed).toEqual(original);
  });
});
