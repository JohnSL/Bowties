/**
 * T017: Vitest unit tests for serializeConfigValue()
 *
 * Tests byte-level correctness for all supported CDI element types:
 * - int: 1, 2, 4-byte big-endian, boundary values
 * - string: UTF-8, null terminator (NOT full-padded), truncation
 * - eventId: 8 raw bytes from TreeConfigValue.bytes
 * - float: 4-byte and 8-byte IEEE 754 big-endian
 */

import { describe, it, expect } from 'vitest';
import { serializeConfigValue, parseEventIdHex } from '$lib/utils/serialize';
import type { TreeConfigValue } from '$lib/types/nodeTree';

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

// ─── Int serialization ────────────────────────────────────────────────────────

describe('serializeConfigValue — int', () => {
  it('serializes 1-byte int (0)', () => {
    expect(serializeConfigValue(intVal(0), 'int', 1)).toEqual([0x00]);
  });

  it('serializes 1-byte int (255)', () => {
    expect(serializeConfigValue(intVal(255), 'int', 1)).toEqual([0xff]);
  });

  it('serializes 2-byte int (0)', () => {
    expect(serializeConfigValue(intVal(0), 'int', 2)).toEqual([0x00, 0x00]);
  });

  it('serializes 2-byte int (1000) as big-endian', () => {
    // 1000 = 0x03E8 → [0x03, 0xE8]
    expect(serializeConfigValue(intVal(1000), 'int', 2)).toEqual([0x03, 0xe8]);
  });

  it('serializes 2-byte int (65535) as big-endian', () => {
    expect(serializeConfigValue(intVal(65535), 'int', 2)).toEqual([0xff, 0xff]);
  });

  it('serializes 4-byte int (0)', () => {
    expect(serializeConfigValue(intVal(0), 'int', 4)).toEqual([0x00, 0x00, 0x00, 0x00]);
  });

  it('serializes 4-byte int (65536) as big-endian', () => {
    // 65536 = 0x00010000 → [0x00, 0x01, 0x00, 0x00]
    expect(serializeConfigValue(intVal(65536), 'int', 4)).toEqual([0x00, 0x01, 0x00, 0x00]);
  });

  it('serializes 4-byte int (0xDEADBEEF)', () => {
    expect(serializeConfigValue(intVal(0xdeadbeef), 'int', 4)).toEqual([
      0xde, 0xad, 0xbe, 0xef,
    ]);
  });
});

// ─── String serialization ─────────────────────────────────────────────────────

describe('serializeConfigValue — string', () => {
  it('serializes empty string as single null byte', () => {
    expect(serializeConfigValue(strVal(''), 'string', 16)).toEqual([0x00]);
  });

  it('serializes ASCII string + null terminator (NOT full-padded)', () => {
    // "Hi" in UTF-8 → [0x48, 0x69, 0x00] — only 3 bytes, not 16
    expect(serializeConfigValue(strVal('Hi'), 'string', 16)).toEqual([0x48, 0x69, 0x00]);
  });

  it('serializes UTF-8 multi-byte character', () => {
    // "é" = 0xC3A9 in UTF-8
    const result = serializeConfigValue(strVal('é'), 'string', 16);
    expect(result).toEqual([0xc3, 0xa9, 0x00]);
  });

  it('truncates string that would exceed field size (leaves room for null)', () => {
    // size=4 → max 3 UTF-8 content bytes + 1 null
    const result = serializeConfigValue(strVal('Hello'), 'string', 4);
    // "Hel" + null
    expect(result).toEqual([0x48, 0x65, 0x6c, 0x00]);
    expect(result.length).toBe(4);
  });

  it('includes null terminator as last byte', () => {
    const result = serializeConfigValue(strVal('Test'), 'string', 16);
    expect(result[result.length - 1]).toBe(0x00);
  });

  it('does NOT pad to full field width', () => {
    // "A" (1 byte) + null = 2 bytes written, NOT 16
    const result = serializeConfigValue(strVal('A'), 'string', 16);
    expect(result.length).toBe(2);
  });
});

// ─── Event ID serialization ───────────────────────────────────────────────────

describe('serializeConfigValue — eventId', () => {
  it('serializes 8 bytes from bytes array unchanged', () => {
    const bytes = [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff];
    expect(serializeConfigValue(eventVal(bytes), 'eventId', 8)).toEqual(bytes);
  });

  it('serializes all-zeros event ID', () => {
    const bytes = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    expect(serializeConfigValue(eventVal(bytes), 'eventId', 8)).toEqual(bytes);
  });

  it('serializes all-0xFF event ID', () => {
    const bytes = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    expect(serializeConfigValue(eventVal(bytes), 'eventId', 8)).toEqual(bytes);
  });

  it('throws if bytes array length is not 8', () => {
    const bad = eventVal([0x01, 0x02]);
    (bad as { bytes: number[] }).bytes = [0x01, 0x02]; // keep type, force short array
    expect(() => serializeConfigValue(bad, 'eventId', 8)).toThrow();
  });
});

// ─── Float serialization ──────────────────────────────────────────────────────

describe('serializeConfigValue — float (4-byte)', () => {
  it('serializes 0.0 as 4-byte IEEE 754', () => {
    expect(serializeConfigValue(floatVal(0.0), 'float', 4)).toEqual([0x00, 0x00, 0x00, 0x00]);
  });

  it('serializes 1.0 as 4-byte IEEE 754 big-endian (0x3F800000)', () => {
    expect(serializeConfigValue(floatVal(1.0), 'float', 4)).toEqual([0x3f, 0x80, 0x00, 0x00]);
  });

  it('serializes -1.0 as 4-byte IEEE 754 (0xBF800000)', () => {
    expect(serializeConfigValue(floatVal(-1.0), 'float', 4)).toEqual([0xbf, 0x80, 0x00, 0x00]);
  });

  it('serializes 3.14 (approx) as 4-byte IEEE 754', () => {
    // 3.14 as f32 = 0x4048F5C3
    expect(serializeConfigValue(floatVal(3.14), 'float', 4)).toEqual([0x40, 0x48, 0xf5, 0xc3]);
  });
});

describe('serializeConfigValue — float (8-byte)', () => {
  it('serializes 1.0 as 8-byte IEEE 754 big-endian (0x3FF0000000000000)', () => {
    expect(serializeConfigValue(floatVal(1.0), 'float', 8)).toEqual([
      0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
  });

  it('serializes 0.0 as 8-byte IEEE 754', () => {
    expect(serializeConfigValue(floatVal(0.0), 'float', 8)).toEqual([
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
  });
});

// ─── Type mismatch errors ─────────────────────────────────────────────────────

describe('serializeConfigValue — type mismatches', () => {
  it('throws when int value passed for string elementType', () => {
    expect(() => serializeConfigValue(intVal(1), 'string', 16)).toThrow(/Expected string value/);
  });

  it('throws when string value passed for int elementType', () => {
    expect(() => serializeConfigValue(strVal('hi'), 'int', 1)).toThrow(/Expected int value/);
  });
});

// ─── T033: parseEventIdHex ────────────────────────────────────────────────────

describe('T033: parseEventIdHex', () => {
  it('parses a valid lowercase dotted-hex event ID', () => {
    expect(parseEventIdHex('05.01.01.01.22.00.00.ff')).toEqual([0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff]);
  });

  it('parses a valid uppercase dotted-hex event ID', () => {
    expect(parseEventIdHex('05.01.01.01.22.00.00.FF')).toEqual([0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff]);
  });

  it('parses all-zeros event ID', () => {
    expect(parseEventIdHex('00.00.00.00.00.00.00.00')).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
  });

  it('parses all-FF event ID', () => {
    expect(parseEventIdHex('FF.FF.FF.FF.FF.FF.FF.FF')).toEqual([255, 255, 255, 255, 255, 255, 255, 255]);
  });

  it('returns null for wrong byte count (only 4 bytes)', () => {
    expect(parseEventIdHex('05.01.01.01')).toBeNull();
  });

  it('returns null for wrong byte count (9 bytes)', () => {
    expect(parseEventIdHex('05.01.01.01.22.00.00.FF.AA')).toBeNull();
  });

  it('returns null for non-hex characters', () => {
    expect(parseEventIdHex('05.ZZ.01.01.22.00.00.FF')).toBeNull();
  });

  it('returns null when dots are missing (run-on hex)', () => {
    expect(parseEventIdHex('0501010122000000')).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(parseEventIdHex('')).toBeNull();
  });

  it('returns null for single-digit hex pairs', () => {
    expect(parseEventIdHex('5.1.1.1.22.0.0.FF')).toBeNull();
  });

  it('returns null when separators are colons instead of dots', () => {
    expect(parseEventIdHex('05:01:01:01:22:00:00:FF')).toBeNull();
  });
});
