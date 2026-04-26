import type {
  DiscoveredNode,
  QueryPipResponse,
  QuerySnipResponse,
} from '$lib/api/tauri';
import { formatNodeId, nodeIdStringToBytes } from '$lib/utils/nodeId';

interface DiscoveryOrchestratorArgs {
  currentNodes: DiscoveredNode[];
  getCurrentNodes?: () => DiscoveredNode[];
  nodeId: string;
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
  nodeId: string;
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
  staleNodeIds: string[];
  selectedNodeId?: string | null;
  nodesWithCdi?: Iterable<string>;
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

function findNodeIndex(nodes: DiscoveredNode[], nodeId: string): number {
  return nodes.findIndex((node) => formatNodeId(node.node_id) === nodeId);
}

function createSkeletonNode(nodeId: string, alias: number, timestamp: string): DiscoveredNode {
  return {
    node_id: nodeIdStringToBytes(nodeId),
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
  nodeId: string,
  updater: (node: DiscoveredNode) => DiscoveredNode,
): DiscoveredNode[] {
  return nodes.map((node) => formatNodeId(node.node_id) === nodeId ? updater(node) : node);
}

function updateOrInsertNodeById(
  nodes: DiscoveredNode[],
  nodeId: string,
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
      nodesWithCdi: new Set(nodesWithCdi),
      shouldResetSidebar: false,
    };
  }

  const staleSet = new Set(staleNodeIds);

  return {
    nodes: currentNodes.filter((node) => !staleSet.has(formatNodeId(node.node_id))),
    removedNodeIds: staleNodeIds,
    nodesWithCdi: new Set([...nodesWithCdi].filter((nodeId) => !staleSet.has(nodeId))),
    shouldResetSidebar: !!selectedNodeId && staleSet.has(selectedNodeId),
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
    await registerNode(nodeId, alias);
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
    warn(`Failed to query node ${nodeId}:`, error);
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
    warn(`Failed to refresh reinitialized node ${nodeId}:`, error);
    return { nodes: currentNodes, skipped: false };
  }
}