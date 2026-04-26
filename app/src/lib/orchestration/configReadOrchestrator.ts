import type { DiscoveredNode } from '$lib/api/tauri';
import type { NodeReadState } from '$lib/api/types';
import { getCdiErrorMessage, isCdiError } from '$lib/types/cdi';
import { resolveNodeDisplayName } from '$lib/utils/nodeDisplayName';
import { formatNodeId } from '$lib/utils/nodeId';

export interface ConfigReadNodeCandidate {
  nodeId: string;
  nodeName: string;
}

export interface FailedCdiPreflightNode extends ConfigReadNodeCandidate {
  reason: string;
}

export interface ConfigReadPreflightResolution {
  failureMessage: string | null;
  failedNodeIds: Set<string>;
  missingNodes: ConfigReadNodeCandidate[];
  nodesWithCdi: Set<string>;
  pendingNodes: ConfigReadNodeCandidate[];
}

export function pipConfirmsNoCdi(node: Pick<DiscoveredNode, 'pip_status' | 'pip_flags'>): boolean {
  if (node.pip_status !== 'Complete') return false;
  if (!node.pip_flags) return false;
  return !node.pip_flags.cdi && !node.pip_flags.memory_configuration;
}

export function getUnreadConfigEligibleNodes(
  nodes: DiscoveredNode[],
  readNodeIds: Set<string>,
): DiscoveredNode[] {
  return nodes.filter((node) => {
    if (!node.snip_data) return false;
    if (pipConfirmsNoCdi(node)) return false;
    return !readNodeIds.has(formatNodeId(node.node_id));
  });
}

export function toConfigReadCandidate(node: DiscoveredNode): ConfigReadNodeCandidate {
  const nodeId = formatNodeId(node.node_id);
  return {
    nodeId,
    nodeName: resolveNodeDisplayName(nodeId, node),
  };
}

export function formatCdiPreflightFailureMessage(
  failedNodes: FailedCdiPreflightNode[],
  fallbackPrefix: string,
): string {
  if (failedNodes.length === 0) return fallbackPrefix;
  if (failedNodes.length === 1) {
    const [{ nodeName, reason }] = failedNodes;
    return `${fallbackPrefix} for ${nodeName}: ${reason}`;
  }

  const details = failedNodes
    .map(({ nodeName, reason }) => `${nodeName}: ${reason}`)
    .join(' | ');
  return `${fallbackPrefix} for ${failedNodes.length} nodes: ${details}`;
}

export function createWaitingNodeReadStates(nodes: ConfigReadNodeCandidate[]): NodeReadState[] {
  return nodes.map(({ nodeId, nodeName }) => ({
    nodeId,
    name: nodeName,
    percentage: 0,
    status: 'waiting' as const,
  }));
}

export async function partitionNodesByCdiAvailability(
  nodes: DiscoveredNode[],
  hasCachedCdi: (nodeId: string) => Promise<boolean>,
): Promise<{
  nodesWithCdi: Set<string>;
  missingNodes: ConfigReadNodeCandidate[];
  failedNodes: FailedCdiPreflightNode[];
}> {
  const nodesWithCdi = new Set<string>();
  const missingNodes: ConfigReadNodeCandidate[] = [];
  const failedNodes: FailedCdiPreflightNode[] = [];

  for (const node of nodes) {
    const candidate = toConfigReadCandidate(node);
    try {
      if (await hasCachedCdi(candidate.nodeId)) {
        nodesWithCdi.add(candidate.nodeId);
      } else {
        missingNodes.push(candidate);
      }
    } catch (error) {
      if (isCdiError(error, 'CdiNotRetrieved')) {
        missingNodes.push(candidate);
      } else {
        failedNodes.push({
          ...candidate,
          reason: getCdiErrorMessage(error),
        });
      }
    }
  }

  return { nodesWithCdi, missingNodes, failedNodes };
}

export async function resolveConfigReadPreflight(
  nodes: DiscoveredNode[],
  hasCachedCdi: (nodeId: string) => Promise<boolean>,
  prefix: string,
): Promise<ConfigReadPreflightResolution> {
  const { nodesWithCdi, missingNodes, failedNodes } = await partitionNodesByCdiAvailability(
    nodes,
    hasCachedCdi,
  );

  const failedNodeIds = new Set(failedNodes.map(({ nodeId }) => nodeId));

  return {
    failureMessage: failedNodes.length > 0
      ? formatCdiPreflightFailureMessage(failedNodes, prefix)
      : null,
    failedNodeIds,
    missingNodes,
    nodesWithCdi,
    pendingNodes: nodes
      .map((node) => toConfigReadCandidate(node))
      .filter(({ nodeId }) => !failedNodeIds.has(nodeId)),
  };
}