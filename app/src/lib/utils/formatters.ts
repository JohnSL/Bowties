/**
 * Configuration Value Formatters
 * 
 * Utilities for displaying configuration values in human-readable format.
 */

import type { ConfigValue } from '$lib/api/types';

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
            return formatEventId(value.value);
        
        case 'Float':
            return value.value.toFixed(2);
        
        case 'Invalid':
            return `Error: ${value.error}`;
        
        default:
            return 'Unknown value type';
    }
}

/**
 * Format Event ID as dotted hexadecimal (T039)
 * 
 * @param bytes - 8-byte event ID array
 * @returns Dotted hex string (e.g., "05.01.01.01.03.01.00.00")
 * 
 * @example
 * ```typescript
 * formatEventId([5, 1, 1, 1, 3, 1, 0, 0])  // "05.01.01.01.03.01.00.00"
 * formatEventId([0, 0, 0, 0, 0, 0, 0, 42])  // "00.00.00.00.00.00.00.2A"
 * ```
 */
export function formatEventId(bytes: number[]): string {
    if (bytes.length !== 8) {
        return 'Invalid Event ID';
    }
    return bytes
        .map(b => b.toString(16).toUpperCase().padStart(2, '0'))
        .join('.');
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
