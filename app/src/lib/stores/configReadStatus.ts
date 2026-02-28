/**
 * Tracks which nodes have had their configuration values successfully read.
 * Consumed by ConfigSidebar to show "not read" indicators, and by +page.svelte
 * to determine which nodes still need reading.
 */

import { writable, derived } from 'svelte/store';

/** Set of node IDs (dotted-hex) whose config values have been read */
export const configReadNodesStore = writable<Set<string>>(new Set());

/** Mark a node as having been config-read */
export function markNodeConfigRead(nodeId: string): void {
  configReadNodesStore.update(s => {
    const next = new Set(s);
    next.add(nodeId);
    return next;
  });
}

/** Clear all read status (e.g. on refresh or disconnect) */
export function clearConfigReadStatus(): void {
  configReadNodesStore.set(new Set());
}
