import type { DiscoveredNode } from '$lib/api/tauri';
import type { NodeReadState, ReadAllConfigValuesResponse } from '$lib/api/types';
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

interface ExecuteConfigReadCandidatesArgs {
  nodes: ConfigReadNodeCandidate[];
  hasCachedCdi?: (nodeId: string) => Promise<boolean>;
  markNodeConfigRead: (nodeId: string) => void;
  readAllConfigValues: (
    nodeId: string,
    nodeIndex: number,
    totalNodes: number,
  ) => Promise<ReadAllConfigValuesResponse>;
  reloadTree: (nodeId: string) => Promise<unknown>;
  setNodeReadStates: (states: NodeReadState[]) => void;
  warn: (message: string, error?: unknown) => void;
}

export interface ConfigReadExecutionFailure {
  error?: unknown;
  nodeId: string;
  nodeName: string;
  status: 'failed' | 'no-cdi';
}

export interface ConfigReadExecutionResult {
  failures: ConfigReadExecutionFailure[];
  nodeReadStates: NodeReadState[];
}

function updateNodeReadState(
  states: NodeReadState[],
  nodeIndex: number,
  patch: Partial<NodeReadState>,
): NodeReadState[] {
  return states.map((state, index) => (
    index === nodeIndex ? { ...state, ...patch } : state
  ));
}

export async function executeConfigReadCandidates({
  nodes,
  hasCachedCdi,
  markNodeConfigRead,
  readAllConfigValues,
  reloadTree,
  setNodeReadStates,
  warn,
}: ExecuteConfigReadCandidatesArgs): Promise<ConfigReadExecutionResult> {
  let nodeReadStates = createWaitingNodeReadStates(nodes);
  const failures: ConfigReadExecutionFailure[] = [];
  setNodeReadStates(nodeReadStates);

  for (let nodeIndex = 0; nodeIndex < nodes.length; nodeIndex++) {
    const { nodeId, nodeName } = nodes[nodeIndex];
    try {
      if (hasCachedCdi) {
        const hasCdi = await hasCachedCdi(nodeId);
        if (!hasCdi) {
          nodeReadStates = updateNodeReadState(nodeReadStates, nodeIndex, { status: 'no-cdi' });
          failures.push({ nodeId, nodeName, status: 'no-cdi' });
          setNodeReadStates(nodeReadStates);
          continue;
        }
      }

      const result = await readAllConfigValues(nodeId, nodeIndex, nodes.length);
      if (result.abortError) {
        nodeReadStates = updateNodeReadState(nodeReadStates, nodeIndex, { status: 'failed' });
        failures.push({ error: result.abortError, nodeId, nodeName, status: 'failed' });
        setNodeReadStates(nodeReadStates);
        throw new Error(result.abortError);
      }

      if (result.failedReads === 0) {
        markNodeConfigRead(nodeId);
      } else {
        warn(
          `Config read for ${nodeName}: ${result.failedReads}/${result.totalElements} elements failed — node not marked as read`,
        );
        nodeReadStates = updateNodeReadState(nodeReadStates, nodeIndex, { status: 'failed' });
        failures.push({
          error: `${result.failedReads}/${result.totalElements} elements failed`,
          nodeId,
          nodeName,
          status: 'failed',
        });
        setNodeReadStates(nodeReadStates);
      }

      await reloadTree(nodeId);
      nodeReadStates = updateNodeReadState(nodeReadStates, nodeIndex, {
        percentage: 100,
        status: 'complete',
      });
      setNodeReadStates(nodeReadStates);
    } catch (error) {
      warn(`Failed to read config values from ${nodeName}:`, error);
      nodeReadStates = updateNodeReadState(nodeReadStates, nodeIndex, { status: 'failed' });
      if (!failures.some((failure) => failure.nodeId === nodeId && failure.status === 'failed')) {
        failures.push({ error, nodeId, nodeName, status: 'failed' });
      }
      setNodeReadStates(nodeReadStates);
    }
  }

  return { failures, nodeReadStates };
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