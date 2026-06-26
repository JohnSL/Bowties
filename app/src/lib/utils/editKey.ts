/**
 * Canonical edit key construction and offline value conversion utilities.
 *
 * The edit key `"${normalizedNodeKey}:${space}:${address}"` is the single
 * source of truth for field identity in the changes module. The leading
 * component is a `NodeKey` (Spec 014, ADR-0008) — either a canonical 12-hex
 * live NodeID or a `placeholder:<uuidv4>`. Because placeholder NodeKeys
 * contain an internal `:`, the parser splits from the right and treats
 * everything before the last two `:` segments as the NodeKey.
 *
 * Offline change rows store addresses as hex offset strings ("0x00000064").
 * This module provides the conversion layer so the rest of the system
 * can work exclusively in decimal addresses.
 */

import { toCanonicalNodeKey } from '$lib/utils/nodeKey';
import { canonicalEventIdHex, parseEventIdHex } from '$lib/utils/serialize';
import type { TreeConfigValue } from '$lib/types/nodeTree';

// ─── Canonical key ────────────────────────────────────────────────────────────

/**
 * Build the canonical edit key for a config field.
 *
 * Format: `"${normalizedNodeKey}:${space}:${address}"`
 * - nodeKey is normalized via `toCanonicalNodeKey` (live NodeIDs uppercased and
 *   dots stripped; placeholder keys pass through unchanged).
 * - address is raw decimal (NOT hex)
 *
 * @example
 * editKeyForLeaf('05.02.01.02.03.00', 253, 100) === '050201020300:253:100'
 * editKeyForLeaf('placeholder:01234567-89ab-cdef-0123-456789abcdef', 253, 100)
 *   === 'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100'
 */
export function editKeyForLeaf(nodeKey: string, space: number, address: number): string {
  return `${toCanonicalNodeKey(nodeKey)}:${space}:${address}`;
}

/**
 * Parse an edit key back into its components.
 *
 * Inverse of `editKeyForLeaf`. Splits from the right so placeholder NodeKeys
 * (which contain an internal `:`) round-trip correctly. The legacy field name
 * `normalizedNodeId` is retained for backward compatibility with existing
 * call sites; it now holds a `NodeKey` (live NodeID or placeholder).
 */
export function parseEditKey(key: string): {
  normalizedNodeId: string;
  space: number;
  address: number;
} {
  const lastColon = key.lastIndexOf(':');
  const secondLastColon = key.lastIndexOf(':', lastColon - 1);
  return {
    normalizedNodeId: key.slice(0, secondLastColon),
    space: parseInt(key.slice(secondLastColon + 1, lastColon), 10),
    address: parseInt(key.slice(lastColon + 1), 10),
  };
}

// ─── Address ↔ hex offset ─────────────────────────────────────────────────────

/**
 * Convert a decimal address to the hex offset string format used by
 * offlineChangesStore rows.
 *
 * @example
 * addressToOffsetHex(100) === '0x00000064'
 * addressToOffsetHex(0)   === '0x00000000'
 */
export function addressToOffsetHex(address: number): string {
  return `0x${address.toString(16).toUpperCase().padStart(8, '0')}`;
}

/**
 * Parse a hex offset string from offlineChangesStore back to a decimal address.
 * Handles both "0x" and "0X" prefixes, and upper/lowercase hex digits.
 *
 * @example
 * offsetHexToAddress('0x00000064') === 100
 */
export function offsetHexToAddress(offset: string): number {
  const trimmed = offset.replace(/^0[xX]/, '');
  return parseInt(trimmed, 16);
}

// ─── Offline value string serialization ──────────────────────────────────────

/**
 * Parse a persisted offline value string into a TreeConfigValue.
 *
 * Parsing rules (in priority order):
 * 1. Digits only → int
 * 2. Digits.Digits → float
 * 3. 8 hex-byte pairs separated by dots (e.g., "01.02.03.04.05.06.07.08") → eventId
 * 4. Everything else → string
 *
 * This function is the single replacement for:
 * - `parseOfflineValueString` in nodeTree.svelte.ts
 * - `parseOfflinePlannedValue` in TreeLeafRow.svelte
 */
export function parseOfflineValueString(raw: string): TreeConfigValue {
  // Event ID check first — 16-char hex or dotted format. Must precede the
  // integer check because some canonical hex strings (e.g. "0102030405060708")
  // are all-digits and would match /^[0-9]+$/.
  const eventBytes = parseEventIdHex(raw);
  if (eventBytes) {
    return { type: 'eventId', bytes: eventBytes, hex: canonicalEventIdHex(eventBytes) };
  }
  if (/^[0-9]+$/.test(raw)) {
    return { type: 'int', value: parseInt(raw, 10) };
  }
  if (/^[0-9]+\.[0-9]+$/.test(raw)) {
    return { type: 'float', value: parseFloat(raw) };
  }
  return { type: 'string', value: raw };
}

/**
 * Serialize a TreeConfigValue to the string format stored in offline change rows.
 *
 * This is the single replacement for `valueToOfflineString` in TreeLeafRow.svelte.
 * Used only at save time when writing a row to offlineChangesStore.
 */
export function configValueToOfflineString(value: TreeConfigValue): string {
  switch (value.type) {
    case 'int':
      return String(value.value);
    case 'float':
      return String(value.value);
    case 'string':
      return value.value;
    case 'eventId':
      return value.hex;
  }
}
