import type {
  DiscoveredNode,
  QueryPipResponse,
  QuerySnipResponse,
} from '$lib/api/tauri';
import { formatNodeId, nodeIdStringToBytes } from '$lib/utils/nodeId';
import {
  nodeKey,
  nodeKeyToString,
  toCanonicalNodeKey,
  type NodeKeyInput,
} from '$lib/utils/nodeKey';

/**
 * Accepted identifier form for orchestrator inputs (Spec 014 Step 6c).
 *
 * The events router now emits `NodeKey` payloads in canonical wire form
 * (no dots, uppercase) rather than dotted strings. Callers may also pass
 * legacy dotted strings or already-branded `NodeKey` values; all
 * three are canonicalized at the boundary so internal comparisons stop
 * drifting and SNIP/PIP merges hit the existing entry instead of
 * appending duplicates.
 */
export type { NodeKeyInput };

function toCanonical(input: NodeKeyInput): string {
  return toCanonicalNodeKey(input);
}

/** Canonical wire-form key for a `DiscoveredNode`. */
function keyOf(node: DiscoveredNode): string {
  return nodeKeyToString(nodeKey(formatNodeId(node.node_id)));
}

interface DiscoveryOrchestratorArgs {
  currentNodes: DiscoveredNode[];
  getCurrentNodes?: () => DiscoveredNode[];
  nodeId: NodeKeyInput;
  alias: number;
  registerNode: (nodeId: string, alias: number) => Promise<void>;
  querySnip: (alias: number) => Promise<QuerySnipResponse>;
  queryPip: (alias: number) => Promise<QueryPipResponse>;
  publishNodes: (nodes: DiscoveredNode[]) => void;
  now?: () => string;
  warn?: (message: string, error: unknown) => void;
}

interface ReinitializedNodeArgs {
  currentNodes: DiscoveredNode[];
  getCurrentNodes?: () => DiscoveredNode[];
  nodeId: NodeKeyInput;
  alias: number;
  querySnip: (alias: number) => Promise<QuerySnipResponse>;
  queryPip: (alias: number) => Promise<QueryPipResponse>;
  publishNodes: (nodes: DiscoveredNode[]) => void;
  warn?: (message: string, error: unknown) => void;
}

interface DiscoveryUpdateResult {
  nodes: DiscoveredNode[];
  skipped: boolean;
}

interface ReconcileRefreshArgs {
  currentNodes: DiscoveredNode[];
  staleNodeIds: NodeKeyInput[];
  selectedNodeId?: NodeKeyInput | null;
  nodesWithCdi?: Iterable<NodeKeyInput>;
}

interface ReconcileRefreshResult {
  nodes: DiscoveredNode[];
  removedNodeIds: string[];
  nodesWithCdi: Set<string>;
  shouldResetSidebar: boolean;
}

function isDiscoveryComplete(node: DiscoveredNode): boolean {
  return node.snip_status === 'Complete' &&
    node.snip_data !== null &&
    node.pip_status === 'Complete' &&
    node.pip_flags !== null;
}

function findNodeIndex(nodes: DiscoveredNode[], nodeId: NodeKeyInput): number {
  const target = toCanonical(nodeId);
  return nodes.findIndex((node) => keyOf(node) === target);
}

function createSkeletonNode(nodeId: NodeKeyInput, alias: number, timestamp: string): DiscoveredNode {
  // `nodeIdStringToBytes` accepts both dotted and canonical live forms;
  // it does not handle placeholder keys, but skeletons are only built for
  // live discoveries.
  return {
    node_id: nodeIdStringToBytes(toCanonical(nodeId)),
    alias,
    snip_data: null,
    snip_status: 'Unknown',
    connection_status: 'Connected',
    last_verified: null,
    last_seen: timestamp,
    cdi: null,
    pip_flags: null,
    pip_status: 'Unknown',
  };
}

function updateNodeById(
  nodes: DiscoveredNode[],
  nodeId: NodeKeyInput,
  updater: (node: DiscoveredNode) => DiscoveredNode,
): DiscoveredNode[] {
  const target = toCanonical(nodeId);
  return nodes.map((node) => keyOf(node) === target ? updater(node) : node);
}

function updateOrInsertNodeById(
  nodes: DiscoveredNode[],
  nodeId: NodeKeyInput,
  fallbackNode: DiscoveredNode,
  updater: (node: DiscoveredNode) => DiscoveredNode,
): DiscoveredNode[] {
  if (findNodeIndex(nodes, nodeId) >= 0) {
    return updateNodeById(nodes, nodeId, updater);
  }

  return [...nodes, updater(fallbackNode)];
}

function mergeDiscoveryQueries(
  node: DiscoveredNode,
  alias: number,
  snipResult: QuerySnipResponse,
  pipResult: QueryPipResponse,
): DiscoveredNode {
  return {
    ...node,
    alias,
    snip_data: snipResult.snip_data ?? node.snip_data,
    snip_status: snipResult.status,
    pip_flags: pipResult.pip_flags ?? node.pip_flags,
    pip_status: pipResult.status,
  };
}

function mergeReinitializedQueries(
  node: DiscoveredNode,
  alias: number,
  snipResult: QuerySnipResponse,
  pipResult: QueryPipResponse,
): DiscoveredNode {
  return {
    ...node,
    alias,
    snip_data: snipResult.snip_data,
    snip_status: snipResult.status,
    pip_flags: pipResult.pip_flags,
    pip_status: pipResult.status,
    cdi: null,
  };
}

export function reconcileRefreshState({
  currentNodes,
  staleNodeIds,
  selectedNodeId = null,
  nodesWithCdi = [],
}: ReconcileRefreshArgs): ReconcileRefreshResult {
  if (staleNodeIds.length === 0) {
    return {
      nodes: currentNodes,
      removedNodeIds: [],
      nodesWithCdi: new Set([...nodesWithCdi].map((id) => typeof id === 'string' ? id : nodeKeyToString(id))),
      shouldResetSidebar: false,
    };
  }

  // Canonicalize for internal matching only; preserve the caller's
  // original string form in the returned ids so downstream consumers
  // (e.g. configReadStatus) that key off the same form still match.
  const staleCanonical = new Set(staleNodeIds.map(toCanonical));
  const staleOriginal = staleNodeIds.map((id) => typeof id === 'string' ? id : nodeKeyToString(id));
  const selectedCanonical = selectedNodeId ? toCanonical(selectedNodeId) : null;

  return {
    nodes: currentNodes.filter((node) => !staleCanonical.has(keyOf(node))),
    removedNodeIds: staleOriginal,
    nodesWithCdi: new Set(
      [...nodesWithCdi]
        .map((id) => typeof id === 'string' ? id : nodeKeyToString(id))
        .filter((nodeId) => !staleCanonical.has(toCanonical(nodeId))),
    ),
    shouldResetSidebar: !!selectedCanonical && staleCanonical.has(selectedCanonical),
  };
}

export async function handleDiscoveredNode({
  currentNodes,
  getCurrentNodes,
  nodeId,
  alias,
  registerNode,
  querySnip,
  queryPip,
  publishNodes,
  now = () => new Date().toISOString(),
  warn = (message, error) => console.warn(message, error),
}: DiscoveryOrchestratorArgs): Promise<DiscoveryUpdateResult> {
  // Preserve the caller's original string form for downstream IPC and log
  // messages — Step 6d owns migrating `registerNode` to take `NodeKey`.
  const nodeIdStr = typeof nodeId === 'string' ? nodeId : nodeKeyToString(nodeId);
  const existingIdx = findNodeIndex(currentNodes, nodeId);
  let nextNodes = currentNodes;

  if (existingIdx >= 0) {
    const existing = currentNodes[existingIdx];
    if (existing.alias === alias && isDiscoveryComplete(existing)) {
      return { nodes: currentNodes, skipped: true };
    }

    if (existing.alias !== alias) {
      nextNodes = currentNodes.map((node, index) => index !== existingIdx ? node : {
        ...node,
        alias,
        connection_status: 'Connected',
        last_seen: now(),
      });
      publishNodes(nextNodes);
    }
  } else {
    nextNodes = [...currentNodes, createSkeletonNode(nodeId, alias, now())];
    publishNodes(nextNodes);
  }

  try {
    await registerNode(nodeIdStr, alias);
    const [snipResult, pipResult] = await Promise.all([
      querySnip(alias),
      queryPip(alias),
    ]);

    const latestNodes = getCurrentNodes ? getCurrentNodes() : nextNodes;
    nextNodes = updateOrInsertNodeById(latestNodes, nodeId, createSkeletonNode(nodeId, alias, now()), (node) => (
      mergeDiscoveryQueries(node, alias, snipResult, pipResult)
    ));
    publishNodes(nextNodes);
  } catch (error) {
    warn(`Failed to query node ${nodeIdStr}:`, error);
  }

  return { nodes: nextNodes, skipped: false };
}

export async function refreshReinitializedNode({
  currentNodes,
  getCurrentNodes,
  nodeId,
  alias,
  querySnip,
  queryPip,
  publishNodes,
  warn = (message, error) => console.warn(message, error),
}: ReinitializedNodeArgs): Promise<DiscoveryUpdateResult> {
  const nodeIdStr = typeof nodeId === 'string' ? nodeId : nodeKeyToString(nodeId);
  if (findNodeIndex(currentNodes, nodeId) < 0) {
    return { nodes: currentNodes, skipped: true };
  }

  try {
    const [snipResult, pipResult] = await Promise.all([
      querySnip(alias),
      queryPip(alias),
    ]);

    const latestNodes = getCurrentNodes ? getCurrentNodes() : currentNodes;
    const nextNodes = updateOrInsertNodeById(latestNodes, nodeId, createSkeletonNode(nodeId, alias, new Date().toISOString()), (node) => (
      mergeReinitializedQueries(node, alias, snipResult, pipResult)
    ));
    publishNodes(nextNodes);
    return { nodes: nextNodes, skipped: false };
  } catch (error) {
    warn(`Failed to refresh reinitialized node ${nodeIdStr}:`, error);
    return { nodes: currentNodes, skipped: false };
  }
}