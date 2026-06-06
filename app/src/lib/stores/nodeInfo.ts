/**
 * Shared store for full node SNIP data.
 * Populated by +page.svelte after discovery; consumed by NodesColumn and DetailsPanel
 * for display-name resolution, tooltips, and the node-details view.
 *
 * Map keys are canonical NodeKey wire form (uppercase 12-hex for live
 * nodes, `placeholder:<id>` for placeholders). Spec 014 Step 6e:
 * `updateNodeInfo` and `updateNodeSnipField` canonicalize at the
 * boundary so legacy dotted-form inputs converge on the same key the
 * rest of the system (`nodeRoster`, `discoveryOrchestrator`) writes.
 */

import { writable } from 'svelte/store';
import type { DiscoveredNode } from '$lib/api/tauri';
import { formatNodeId } from '$lib/utils/nodeId';
import {
  nodeKey,
  nodeKeyToString,
  toCanonicalNodeKey,
  type NodeKeyInput,
} from '$lib/utils/nodeKey';

/** Map from canonical NodeKey wire form → full DiscoveredNode. */
export const nodeInfoStore = writable<Map<string, DiscoveredNode>>(new Map());

/** Canonical NodeKey wire form for a node's 6-byte NodeID. */
export function formatNodeIdKey(nodeId: number[]): string {
  return nodeKeyToString(nodeKey(formatNodeId(nodeId)));
}

/** Rebuild the store from a fresh node list, keyed by canonical wire form. */
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
  nodeId: NodeKeyInput,
  field: 'user_name' | 'user_description',
  value: string
): void {
  const key = toCanonicalNodeKey(nodeId);
  nodeInfoStore.update(map => {
    const node = map.get(key);
    if (!node) return map;
    const updated: DiscoveredNode = {
      ...node,
      snip_data: node.snip_data
        ? { ...node.snip_data, [field]: value }
        : null,
    };
    const next = new Map(map);
    next.set(key, updated);
    return next;
  });
}
