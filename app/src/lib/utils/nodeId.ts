/** Normalize NodeID for comparisons across dotted/canonical forms. */
export function normalizeNodeId(nodeId?: string): string {
  return (nodeId ?? '').replace(/\./g, '').toUpperCase();
}

/** Format a 6-byte NodeID as dotted hex. */
export function formatNodeId(nodeId: number[]): string {
  return nodeId.map((byte) => byte.toString(16).toUpperCase().padStart(2, '0')).join('.');
}

/** Convert canonical or dotted NodeID text into a 6-byte array. */
export function nodeIdStringToBytes(nodeId: string): number[] {
  const pairs = normalizeNodeId(nodeId).match(/.{1,2}/g) ?? [];
  return pairs.slice(0, 6).map((pair) => parseInt(pair, 16));
}
