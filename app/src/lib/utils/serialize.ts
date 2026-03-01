/**
 * Value serialization for LCC node configuration writes.
 *
 * Converts `TreeConfigValue` + schema metadata into a raw byte array
 * suitable for passing to `write_config_value` Tauri command.
 *
 * Research notes: spec 007 research.md R2.
 */

import type { TreeConfigValue, LeafType } from '$lib/types/nodeTree';

/**
 * Serialize a `TreeConfigValue` to a raw byte array for writing to node memory.
 *
 * @param value       The typed config value to serialize.
 * @param elementType The CDI element type (int, string, eventId, float).
 * @param size        The CDI field size in bytes.
 * @returns           An array of raw bytes to write.
 * @throws            If the value type doesn't match elementType, or the
 *                    value is out of bounds.
 */
export function serializeConfigValue(
  value: TreeConfigValue,
  elementType: LeafType,
  size: number
): number[] {
  switch (elementType) {
    case 'int': {
      if (value.type !== 'int') {
        throw new Error(`Expected int value, got ${value.type}`);
      }
      return serializeInt(value.value, size);
    }
    case 'string': {
      if (value.type !== 'string') {
        throw new Error(`Expected string value, got ${value.type}`);
      }
      return serializeString(value.value, size);
    }
    case 'eventId': {
      if (value.type !== 'eventId') {
        throw new Error(`Expected eventId value, got ${value.type}`);
      }
      return serializeEventId(value.bytes);
    }
    case 'float': {
      if (value.type !== 'float') {
        throw new Error(`Expected float value, got ${value.type}`);
      }
      return serializeFloat(value.value, size);
    }
    default:
      throw new Error(`Unsupported element type for serialization: ${elementType}`);
  }
}

// ─── Type-specific serializers ────────────────────────────────────────────────

/**
 * Serialize an integer value to big-endian bytes.
 *
 * Supports 1, 2, and 4 byte sizes.
 */
function serializeInt(value: number, size: number): number[] {
  const buf = new ArrayBuffer(size);
  const view = new DataView(buf);

  switch (size) {
    case 1:
      view.setUint8(0, value & 0xff);
      break;
    case 2:
      view.setUint16(0, value & 0xffff, /* littleEndian */ false);
      break;
    case 4:
      view.setUint32(0, value >>> 0, /* littleEndian */ false);
      break;
    default:
      // For unusual sizes (3, 5, 6, 7, 8) write as many big-endian bytes as needed
      for (let i = size - 1; i >= 0; i--) {
        view.setUint8(i, value & 0xff);
        value = Math.floor(value / 256);
      }
  }

  return Array.from(new Uint8Array(buf));
}

/**
 * Serialize a string value to UTF-8 bytes with a null terminator.
 *
 * Only writes `utf8.length + 1` bytes — NOT padded to full field width.
 * (Per research.md R2, matching OpenLCB_Java StringEntry.setValue().)
 *
 * @param value The string to encode.
 * @param size  The CDI field size in bytes (constrains max length).
 */
function serializeString(value: string, size: number): number[] {
  const encoder = new TextEncoder();
  const utf8 = encoder.encode(value);

  // Truncate to fit within field (leave 1 byte for null terminator)
  const maxContent = size - 1;
  const content = utf8.length > maxContent ? utf8.slice(0, maxContent) : utf8;

  const result = new Uint8Array(content.length + 1);
  result.set(content, 0);
  result[content.length] = 0x00; // null terminator

  return Array.from(result);
}

/**
 * Serialize an event ID from its pre-computed bytes array.
 *
 * The `bytes` field of `TreeConfigValue.eventId` contains exactly 8 bytes
 * decoded from the dotted-hex representation.
 *
 * @param bytes 8-element array of raw byte values.
 */
function serializeEventId(bytes: number[]): number[] {
  if (bytes.length !== 8) {
    throw new Error(`Event ID must be exactly 8 bytes, got ${bytes.length}`);
  }
  return [...bytes];
}

/**
 * Serialize a float value to IEEE 754 big-endian bytes.
 *
 * @param value The floating-point number to encode.
 * @param size  4 for single precision (f32), 8 for double precision (f64).
 */
function serializeFloat(value: number, size: number): number[] {
  const buf = new ArrayBuffer(size);
  const view = new DataView(buf);

  if (size === 4) {
    view.setFloat32(0, value, /* littleEndian */ false);
  } else if (size === 8) {
    view.setFloat64(0, value, /* littleEndian */ false);
  } else {
    throw new Error(`Unsupported float size: ${size} (expected 4 or 8)`);
  }

  return Array.from(new Uint8Array(buf));
}

// ─── Event ID utilities ───────────────────────────────────────────────────────

/**
 * Parse a dotted-hex event ID string into an 8-byte array.
 *
 * Valid format: `HH.HH.HH.HH.HH.HH.HH.HH` where each `HH` is a 2-digit hex pair.
 * Examples:
 *   `05.01.01.01.22.00.00.FF` → [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xFF]
 *   `00.00.00.00.00.00.00.00` → [0, 0, 0, 0, 0, 0, 0, 0]
 *
 * @param dottedHex  The dotted-hex string to parse.
 * @returns          An 8-element number array, or `null` if the input is invalid.
 */
export function parseEventIdHex(dottedHex: string): number[] | null {
  const PATTERN = /^[0-9A-Fa-f]{2}(\.[0-9A-Fa-f]{2}){7}$/;
  if (!PATTERN.test(dottedHex)) return null;

  const parts = dottedHex.split('.');
  if (parts.length !== 8) return null;

  return parts.map(p => parseInt(p, 16));
}

/**
 * Format an 8-byte array as a dotted-hex event ID string.
 *
 * @param bytes  Array of 8 bytes.
 * @returns      Dotted-hex string, e.g. `05.01.01.01.22.00.00.FF`.
 */
export function formatEventIdHex(bytes: number[]): string {
  return bytes.map(b => b.toString(16).padStart(2, '0').toUpperCase()).join('.');
}
