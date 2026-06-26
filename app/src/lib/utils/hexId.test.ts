/**
 * Tests for the generic HexId helpers — pin the contract that NodeID
 * (6 bytes) and EventID (8 bytes) wrappers depend on.
 */

import { describe, it, expect } from 'vitest';
import { formatCanonicalHex, formatDottedHex, parseHexId } from './hexId';

describe('formatCanonicalHex', () => {
  it('formats 8 bytes as 16-char uppercase contiguous hex', () => {
    expect(formatCanonicalHex([0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff])).toBe('05010101220000FF');
  });

  it('formats 6 bytes as 12-char uppercase contiguous hex', () => {
    expect(formatCanonicalHex([0x05, 0x02, 0x01, 0x02, 0x00, 0xff])).toBe('0502010200FF');
  });

  it('zero-pads single-digit bytes', () => {
    expect(formatCanonicalHex([0, 1, 15, 16, 255, 170])).toBe('00010F10FFAA');
  });

  it('returns empty string for empty input', () => {
    expect(formatCanonicalHex([])).toBe('');
  });
});

describe('formatDottedHex', () => {
  it('formats 8 bytes as dotted uppercase hex', () => {
    expect(formatDottedHex([0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff])).toBe(
      '05.01.01.01.22.00.00.FF',
    );
  });

  it('formats 6 bytes as dotted uppercase hex', () => {
    expect(formatDottedHex([0x05, 0x02, 0x01, 0x02, 0x00, 0xff])).toBe('05.02.01.02.00.FF');
  });

  it('returns empty string for empty input', () => {
    expect(formatDottedHex([])).toBe('');
  });
});

describe('parseHexId', () => {
  it('parses dotted 8-byte hex', () => {
    expect(parseHexId('05.01.01.01.22.00.00.FF', 8)).toEqual([
      0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff,
    ]);
  });

  it('parses canonical contiguous 8-byte hex', () => {
    expect(parseHexId('0501010122000000', 8)).toEqual([
      0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0x00,
    ]);
  });

  it('parses dotted 6-byte hex', () => {
    expect(parseHexId('05.02.01.02.00.FF', 6)).toEqual([0x05, 0x02, 0x01, 0x02, 0x00, 0xff]);
  });

  it('parses canonical 6-byte hex', () => {
    expect(parseHexId('0502010200FF', 6)).toEqual([0x05, 0x02, 0x01, 0x02, 0x00, 0xff]);
  });

  it('accepts lowercase, uppercase, and mixed case', () => {
    expect(parseHexId('aabbccddeeff', 6)).toEqual([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    expect(parseHexId('AABBCCDDEEFF', 6)).toEqual([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    expect(parseHexId('AaBbCcDdEeFf', 6)).toEqual([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
  });

  it('strips dashes and spaces in addition to dots', () => {
    expect(parseHexId('05-01-01-01-22-00-00-FF', 8)).toEqual([
      0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff,
    ]);
    expect(parseHexId('05 01 01 01 22 00 00 FF', 8)).toEqual([
      0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff,
    ]);
  });

  it('returns null for wrong byte count', () => {
    expect(parseHexId('05.01.01.01', 8)).toBeNull();
    expect(parseHexId('05.01.01.01.22.00.00.FF.AA', 8)).toBeNull();
    expect(parseHexId('0502010200FF', 8)).toBeNull();
  });

  it('returns null for non-hex characters', () => {
    expect(parseHexId('05.ZZ.01.01.22.00.00.FF', 8)).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(parseHexId('', 8)).toBeNull();
    expect(parseHexId('', 6)).toBeNull();
  });

  it('returns null for colon separators (not in the strip set)', () => {
    expect(parseHexId('05:01:01:01:22:00:00:FF', 8)).toBeNull();
  });
});
