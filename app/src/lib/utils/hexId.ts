/**
 * Generic hex-encoded ID helpers, parameterized by byte count.
 *
 * Single owner of the byte ↔ hex string conversion rules used by both NodeID
 * (6 bytes / 12 hex chars) and EventID (8 bytes / 16 hex chars). Other
 * `$lib/utils` modules (`serialize.ts`, `formatters.ts`, `nodeId.ts`) wrap
 * these helpers under domain-specific names; nothing else should reimplement
 * the formatting or parsing rules.
 *
 * Canonical form: uppercase contiguous hex (no separators) — see ADR-0010.
 * Dotted form:    uppercase hex pairs separated by `.` — human display form.
 *
 * Mirror of `lcc_rs::types::{format_canonical_hex, format_dotted_hex, parse_hex_id}`.
 */

/** Format raw bytes as canonical contiguous uppercase hex (e.g. `"0102030405060708"`). */
export function formatCanonicalHex(bytes: number[]): string {
  return bytes.map((b) => b.toString(16).padStart(2, '0').toUpperCase()).join('');
}

/** Format raw bytes as dotted uppercase hex (e.g. `"01.02.03.04.05.06.07.08"`). */
export function formatDottedHex(bytes: number[]): string {
  return bytes.map((b) => b.toString(16).padStart(2, '0').toUpperCase()).join('.');
}

/**
 * Parse a hex-encoded ID string into a fixed-length byte array.
 *
 * Accepts contiguous, dotted, dashed, and space-separated forms; strips
 * `.`, `-`, and space characters before parsing. Returns `null` when the
 * stripped input is not exactly `expectedBytes * 2` hex digits.
 */
export function parseHexId(input: string, expectedBytes: number): number[] | null {
  const stripped = input.replace(/[.\- ]/g, '');
  if (stripped.length !== expectedBytes * 2) return null;
  if (!/^[0-9A-Fa-f]+$/.test(stripped)) return null;
  const out: number[] = new Array(expectedBytes);
  for (let i = 0; i < expectedBytes; i++) {
    out[i] = parseInt(stripped.substring(i * 2, i * 2 + 2), 16);
  }
  return out;
}
