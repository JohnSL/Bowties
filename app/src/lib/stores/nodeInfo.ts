/**
 * Shared store for full node SNIP data.
 * Populated by +page.svelte after discovery; consumed by NodesColumn and DetailsPanel
 * for display-name resolution, tooltips, and the node-details view.
 */

import { writable } from 'svelte/store';
import type { DiscoveredNode } from '$lib/api/tauri';
import { formatNodeId } from '$lib/utils/nodeId';

/** Map from nodeId (dotted-hex, e.g. "02.01.57.00.00.01") → full DiscoveredNode */
export const nodeInfoStore = writable<Map<string, DiscoveredNode>>(new Map());

/** Canonical dotted-hex format used as map key */
export function formatNodeIdKey(nodeId: number[]): string {
  return formatNodeId(nodeId);
}

/** Rebuild the store from a fresh node list */
export function updateNodeInfo(nodes: DiscoveredNode[]): void {
  const map = new Map<string, DiscoveredNode>();
  for (const node of nodes) {
    map.set(formatNodeIdKey(node.node_id), node);
  }
  nodeInfoStore.set(map);
}

/**
 * Patch a single SNIP string field for one node without a full re-discovery.
 *
 * Used by SaveControls after writing to ACDI User space (0xFB) so the
 * sidebar node name and tooltip update immediately without a network round-trip.
 *
 * ACDI User space layout (from research.md R8):
 *   offset 1   → user_name         (space 0xFB)
 *   offset 64  → user_description  (space 0xFB)
 */
export function updateNodeSnipField(
  nodeId: string,
  field: 'user_name' | 'user_description',
  value: string
): void {
  nodeInfoStore.update(map => {
    const node = map.get(nodeId);
    if (!node) return map;
    const updated: DiscoveredNode = {
      ...node,
      snip_data: node.snip_data
        ? { ...node.snip_data, [field]: value }
        : null,
    };
    const next = new Map(map);
    next.set(nodeId, updated);
    return next;
  });
}
