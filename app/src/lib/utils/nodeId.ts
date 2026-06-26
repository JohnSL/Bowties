import { formatDottedHex, parseHexId } from '$lib/utils/hexId';

/** Normalize NodeID for comparisons across dotted/canonical forms. */
export function normalizeNodeId(nodeId?: string): string {
  return (nodeId ?? '').replace(/\./g, '').toUpperCase();
}

/** Format a 6-byte NodeID as dotted hex. */
export function formatNodeId(nodeId: number[]): string {
  return formatDottedHex(nodeId);
}

/** Convert any NodeID string (canonical or dotted) to dotted-hex display form. */
export function nodeIdToDisplayHex(nodeId: string): string {
  const canonical = normalizeNodeId(nodeId);
  if (canonical.length === 0) return '';
  return (canonical.match(/.{1,2}/g) ?? []).join('.');
}

/**
 * Convert canonical or dotted NodeID text into a 6-byte array.
 *
 * Returns a 6-element array of zero bytes when the input is not a valid
 * 6-byte hex ID (matches the legacy lenient behavior callers rely on).
 */
export function nodeIdStringToBytes(nodeId: string): number[] {
  return parseHexId(nodeId, 6) ?? [0, 0, 0, 0, 0, 0];
}
