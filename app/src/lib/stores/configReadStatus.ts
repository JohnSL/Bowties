/**
 * Tracks which nodes have had their configuration values successfully read.
 * Consumed by ConfigSidebar to show "not read" indicators, and by +page.svelte
 * to determine which nodes still need reading.
 *
 * Spec 014 Step 6e: stores canonical NodeKey wire form (uppercase 12-hex
 * for live, `placeholder:<id>` for placeholders). All mutators
 * canonicalize at the boundary so legacy dotted-string callers and
 * branded-key callers converge on the same set entry.
 */

import { writable, derived } from 'svelte/store';
import { toCanonicalNodeKey, type NodeKeyInput } from '$lib/utils/nodeKey';

/** Set of canonical NodeKey wire-form strings whose config values have been read. */
export const configReadNodesStore = writable<Set<string>>(new Set());

/** Mark a node as having been config-read. */
export function markNodeConfigRead(nodeId: NodeKeyInput): void {
  const key = toCanonicalNodeKey(nodeId);
  configReadNodesStore.update(s => {
    const next = new Set(s);
    next.add(key);
    return next;
  });
}

/** Clear all read status (e.g. on disconnect). */
export function clearConfigReadStatus(): void {
  configReadNodesStore.set(new Set());
}

/** Remove read status for a specific set of node IDs (e.g. nodes removed after refresh). */
export function removeNodesConfigRead(nodeIds: NodeKeyInput[]): void {
  if (nodeIds.length === 0) return;
  const staleSet = new Set(nodeIds.map(toCanonicalNodeKey));
  configReadNodesStore.update(s => {
    const next = new Set(s);
    for (const id of staleSet) next.delete(id);
    return next;
  });
}
