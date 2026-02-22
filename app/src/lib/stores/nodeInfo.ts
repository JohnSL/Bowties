/**
 * Shared store for full node SNIP data.
 * Populated by +page.svelte after discovery; consumed by NodesColumn and DetailsPanel
 * for display-name resolution, tooltips, and the node-details view.
 */

import { writable } from 'svelte/store';
import type { DiscoveredNode } from '$lib/api/tauri';

/** Map from nodeId (dotted-hex, e.g. "02.01.57.00.00.01") → full DiscoveredNode */
export const nodeInfoStore = writable<Map<string, DiscoveredNode>>(new Map());

/** Canonical dotted-hex format used as map key */
export function formatNodeIdKey(nodeId: number[]): string {
  return nodeId.map(b => b.toString(16).toUpperCase().padStart(2, '0')).join('.');
}

/** Rebuild the store from a fresh node list */
export function updateNodeInfo(nodes: DiscoveredNode[]): void {
  const map = new Map<string, DiscoveredNode>();
  for (const node of nodes) {
    map.set(formatNodeIdKey(node.node_id), node);
  }
  nodeInfoStore.set(map);
}
