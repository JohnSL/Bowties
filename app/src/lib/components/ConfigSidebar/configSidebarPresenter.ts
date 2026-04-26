import type { DiscoveredNode } from '$lib/api/tauri';
import type { OfflineChangeRow } from '$lib/api/sync';
import { pipConfirmsNoCdi } from '$lib/orchestration/configReadOrchestrator';
import {
  hasModifiedDescendant,
  hasModifiedLeaves,
  isGroup,
  isLeaf,
  type ConfigNode,
  type NodeConfigTree,
} from '$lib/types/nodeTree';
import { resolveNodeDisplayName } from '$lib/utils/nodeDisplayName';

export interface SidebarNodeEntry {
  isOffline: boolean;
  node: DiscoveredNode;
  nodeDetail: string | null;
  nodeId: string;
  nodeName: string;
  nodeTooltip: string;
}

export interface SidebarPendingState {
  hasPendingApply: boolean;
  hasPendingEdits: boolean;
}

export function buildSidebarNodeEntries(nodes: Map<string, DiscoveredNode>): SidebarNodeEntry[] {
  const baseNames = new Map<string, number>();

  for (const [nodeId, node] of nodes.entries()) {
    const baseName = resolveNodeDisplayName(nodeId, node);
    baseNames.set(baseName, (baseNames.get(baseName) ?? 0) + 1);
  }

  return [...nodes.entries()]
    .map(([nodeId, node]) => {
      const baseName = resolveNodeDisplayName(nodeId, node);
      const duplicateCount = baseNames.get(baseName) ?? 0;
      const nodeName = duplicateCount > 1
        ? `${baseName} (${nodeId.split('.').slice(-2).join('.')})`
        : baseName;

      return {
        isOffline: node.connection_status === 'NotResponding',
        node,
        nodeDetail: getNodeDetail(node),
        nodeId,
        nodeName,
        nodeTooltip: getNodeTooltip(nodeId, node),
      };
    })
    .sort((left, right) => left.nodeName.localeCompare(right.nodeName));
}

export function shouldShowConfigNotReadBadge(args: {
  configReadNodes: Set<string>;
  layoutIsOfflineMode: boolean;
  layoutOpenInProgress: boolean;
  node: DiscoveredNode;
  nodeId: string;
}): boolean {
  const { configReadNodes, layoutIsOfflineMode, layoutOpenInProgress, node, nodeId } = args;

  return !layoutIsOfflineMode
    && !layoutOpenInProgress
    && node.snip_data !== null
    && !pipConfirmsNoCdi(node)
    && !configReadNodes.has(nodeId);
}

export function getNodePendingState(
  nodeId: string,
  tree: NodeConfigTree | null,
  layoutOpenInProgress: boolean,
  persistedRows: OfflineChangeRow[],
): SidebarPendingState {
  if (layoutOpenInProgress || !tree) {
    return { hasPendingApply: false, hasPendingEdits: false };
  }

  return {
    hasPendingApply: hasPendingApplyForNode(nodeId, persistedRows),
    hasPendingEdits: hasModifiedLeaves(tree),
  };
}

export function getSegmentPendingState(
  nodeId: string,
  tree: NodeConfigTree | null,
  segmentOrigin: number,
  layoutOpenInProgress: boolean,
  persistedRows: OfflineChangeRow[],
): SidebarPendingState {
  if (layoutOpenInProgress || !tree) {
    return { hasPendingApply: false, hasPendingEdits: false };
  }

  const segment = tree.segments.find((candidate) => candidate.origin === segmentOrigin);
  if (!segment) {
    return { hasPendingApply: false, hasPendingEdits: false };
  }

  return {
    hasPendingApply: hasPendingApplyInChildren(nodeId, segment.children, persistedRows),
    hasPendingEdits: hasModifiedDescendant(segment.children, []),
  };
}

function getNodeDetail(node: DiscoveredNode): string | null {
  const snip = node.snip_data;
  if (!snip) return null;
  if (snip.user_name && snip.manufacturer && snip.model) {
    return `${snip.manufacturer} ${snip.model}`;
  }
  return null;
}

function getNodeTooltip(nodeId: string, node: DiscoveredNode): string {
  const parts: string[] = [`Node ID: ${nodeId}`];
  if (node.alias != null) {
    parts.push(`Alias: 0x${node.alias.toString(16).toUpperCase().padStart(3, '0')}`);
  }

  const snip = node.snip_data;
  if (snip) {
    if (snip.manufacturer) parts.push(`Manufacturer: ${snip.manufacturer}`);
    if (snip.model) parts.push(`Model: ${snip.model}`);
    if (snip.hardware_version) parts.push(`Hardware: ${snip.hardware_version}`);
    if (snip.software_version) parts.push(`Software: ${snip.software_version}`);
    if (snip.user_name) parts.push(`User Name: ${snip.user_name}`);
    if (snip.user_description) parts.push(`Description: ${snip.user_description}`);
  }

  return parts.join('\n');
}

function canonicalNodeId(nodeId: string): string {
  return nodeId.replace(/\./g, '').toUpperCase();
}

function offsetFromAddress(address: number): string {
  return `0x${address.toString(16).toUpperCase().padStart(8, '0')}`;
}

function hasPendingApplyForNode(nodeId: string, persistedRows: OfflineChangeRow[]): boolean {
  const canonical = canonicalNodeId(nodeId);
  return persistedRows.some(
    (row) => row.kind === 'config' && row.status === 'pending' && canonicalNodeId(row.nodeId ?? '') === canonical,
  );
}

function hasPendingApplyInChildren(
  nodeId: string,
  children: ConfigNode[],
  persistedRows: OfflineChangeRow[],
): boolean {
  for (const child of children) {
    if (isLeaf(child)) {
      const hasMatch = persistedRows.some((row) => (
        row.kind === 'config'
        && row.status === 'pending'
        && canonicalNodeId(row.nodeId ?? '') === canonicalNodeId(nodeId)
        && row.space === child.space
        && row.offset === offsetFromAddress(child.address)
      ));
      if (hasMatch) {
        return true;
      }
      continue;
    }

    if (isGroup(child) && hasPendingApplyInChildren(nodeId, child.children, persistedRows)) {
      return true;
    }
  }

  return false;
}