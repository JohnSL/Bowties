/**
 * Canonical edit key construction and offline value conversion utilities.
 *
 * The edit key `"${normalizedNodeId}:${space}:${address}"` is the single
 * source of truth for field identity in the changes module.
 *
 * Offline change rows store addresses as hex offset strings ("0x00000064").
 * This module provides the conversion layer so the rest of the system
 * can work exclusively in decimal addresses.
 */

import { normalizeNodeId } from '$lib/utils/nodeId';
import type { TreeConfigValue } from '$lib/types/nodeTree';

// ─── Canonical key ────────────────────────────────────────────────────────────

/**
 * Build the canonical edit key for a config field.
 *
 * Format: `"${normalizedNodeId}:${space}:${address}"`
 * - nodeId is normalized (uppercase, no dots) via normalizeNodeId
 * - address is raw decimal (NOT hex)
 *
 * @example
 * editKeyForLeaf('05.02.01.02.03.00', 253, 100) === '050201020300:253:100'
 */
export function editKeyForLeaf(nodeId: string, space: number, address: number): string {
  return `${normalizeNodeId(nodeId)}:${space}:${address}`;
}

/**
 * Parse an edit key back into its components.
 *
 * Inverse of editKeyForLeaf. Returns the normalized node ID, space, and
 * decimal address.
 */
export function parseEditKey(key: string): {
  normalizedNodeId: string;
  space: number;
  address: number;
} {
  const parts = key.split(':');
  return {
    normalizedNodeId: parts[0],
    space: parseInt(parts[1], 10),
    address: parseInt(parts[2], 10),
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
  if (/^[0-9]+$/.test(raw)) {
    return { type: 'int', value: parseInt(raw, 10) };
  }
  if (/^[0-9]+\.[0-9]+$/.test(raw)) {
    return { type: 'float', value: parseFloat(raw) };
  }
  if (/^([0-9A-F]{2}\.){7}[0-9A-F]{2}$/i.test(raw)) {
    const bytes = raw.split('.').map((b) => parseInt(b, 16));
    return { type: 'eventId', bytes, hex: raw.toUpperCase() };
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
