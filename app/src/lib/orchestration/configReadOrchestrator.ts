import type { DiscoveredNode } from '$lib/api/tauri';
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