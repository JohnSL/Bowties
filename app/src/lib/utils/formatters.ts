/**
 * Configuration Value Formatters
 * 
 * Utilities for displaying configuration values in human-readable format.
 */

import type { ConfigValue } from '$lib/api/types';
import type { TreeConfigValue, TreeMapEntry } from '$lib/types/nodeTree';
import { formatEventIdHex } from '$lib/utils/serialize';

/**
 * Format a configuration value for display (T038)
 * 
 * @param value - Configuration value to format
 * @returns Human-readable string representation
 * 
 * @example
 * ```typescript
 * formatConfigValue({ type: 'Int', value: 42, size_bytes: 1 })  // "42"
 * formatConfigValue({ type: 'String', value: 'Tower LCC', size_bytes: 32 })  // "Tower LCC"
 * formatConfigValue({ type: 'EventId', value: [5,1,1,1,3,1,0,0] })  // "05.01.01.01.03.01.00.00"
 * formatConfigValue({ type: 'Invalid', error: 'Node timeout' })  // "Error: Node timeout"
 * ```
 */
export function formatConfigValue(value: ConfigValue): string {
    switch (value.type) {
        case 'Int':
            return value.value.toString();
        
        case 'String':
            return value.value;
        
        case 'EventId':
            return value.value.length === 8 ? formatEventIdHex(value.value) : 'Invalid Event ID';
        
        case 'Float':
            return value.value.toFixed(2);
        
        case 'Invalid':
            return `Error: ${value.error}`;
        
        default:
            return 'Unknown value type';
    }
}

/**
 * Convert a canonical contiguous event ID hex string to dotted display form.
 *
 * @param hex  16-char uppercase hex, e.g. `"010000000000FFFF"`.
 * @returns    Dotted display form, e.g. `"01.00.00.00.00.00.FF.FF"`.
 *             Returns the input unchanged if it doesn't look like a 16-char hex string.
 */
export function displayEventIdHex(hex: string): string {
    // Already dotted? Return as-is.
    if (hex.includes('.')) return hex;
    if (hex.length !== 16) return hex;
    return hex.match(/.{2}/g)!.join('.');
}

/**
 * Get data type label for display
 * 
 * @param value - Configuration value
 * @returns Human-readable type label
 */
export function getValueTypeLabel(value: ConfigValue): string {
    switch (value.type) {
        case 'Int':
            return `Integer (${value.size_bytes} byte${value.size_bytes > 1 ? 's' : ''})`;
        case 'String':
            return `String (max ${value.size_bytes} bytes)`;
        case 'EventId':
            return 'Event ID (8 bytes)';
        case 'Float':
            return 'Float (4 bytes)';
        case 'Invalid':
            return 'Invalid';
        default:
            return 'Unknown';
    }
}

// ── Well-Known Event IDs (LCC Spec) ──────────────────────────────────────────

const WELL_KNOWN_EVENT_HEX_SET = new Set<string>([
  '010000000000FFFF', // Emergency Off
  '010000000000FFFE', // Clear Emergency Off
  '010000000000FFFD', // Emergency Stop
  '010000000000FFFC', // Clear Emergency Stop
  '010000000000FFF8', // New Log Entry
  '010000000000FE00', // Ident Button Pressed
  '010000000000FD01', // Link Error 1
  '010000000000FD02', // Link Error 2
  '010000000000FD03', // Link Error 3
  '010000000000FD04', // Link Error 4
  '0101000000000201', // Duplicate Node ID
  '0101000000000303', // Is Train
  '0101000000000304', // Is Traction Proxy
]);

/**
 * Returns true when the given canonical-hex event ID is a well-known LCC event.
 * Well-known events do not require producers/consumers to be catalogued.
 *
 * @param hex  Canonical contiguous hex (16 uppercase chars, e.g. "010000000000FFFF").
 */
export function isWellKnownEvent(hex: string): boolean {
  return WELL_KNOWN_EVENT_HEX_SET.has(hex);
}

// ─── TreeConfigValue formatter ────────────────────────────────────────────────

/**
 * Format a TreeConfigValue for display, resolving int values to their map-entry
 * labels when available.
 *
 * Used by annotations and dirty indicators in the config editor UI.
 *
 * @param value - TreeConfigValue to format, or null for an empty string.
 * @param mapEntries - Optional map entries from LeafConstraints. When provided
 *   and the value is an int, the matching label is returned instead of the
 *   raw number (e.g. "Steady" instead of "1").
 *
 * @example
 * formatTreeConfigValue({ type: 'int', value: 1 }, [{ value: 1, label: 'Steady' }]) // "Steady"
 * formatTreeConfigValue({ type: 'int', value: 5 }, [{ value: 1, label: 'Steady' }]) // "5"
 * formatTreeConfigValue({ type: 'string', value: 'Tower LCC' })                     // "Tower LCC"
 * formatTreeConfigValue({ type: 'eventId', bytes: [...], hex: '05.01...' })          // "05.01..."
 * formatTreeConfigValue({ type: 'float', value: 1.5 })                              // "1.50"
 * formatTreeConfigValue(null)                                                        // ""
 */
export function formatTreeConfigValue(
  value: TreeConfigValue | null,
  mapEntries?: TreeMapEntry[] | null,
): string {
  if (value === null || value === undefined) return '';
  switch (value.type) {
    case 'int': {
      if (mapEntries && mapEntries.length > 0) {
        const entry = mapEntries.find((e) => e.value === value.value);
        if (entry) return entry.label;
      }
      return String(value.value);
    }
    case 'string':
      return value.value;
    case 'eventId':
      return displayEventIdHex(value.hex);
    case 'float':
      return value.value.toFixed(2);
  }
}
