/**
 * Resolves a channel binding to its underlying CDI configuration target(s).
 * Pure translation only: callers own navigation and UI state.
 */
import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import { buildPathLabel, replicationInstances } from '$lib/types/nodeTree';

export interface ConfigTarget {
  label: string;
  nodeId: string;
  elementPath: string[];
}

export function resolveConfigTargets(
  channel: InformationChannel,
  tree: NodeConfigTree | undefined,
): ConfigTarget[] {
  if (!tree) return [];
  if (channel.binding.kind === 'connectorInput') {
    return resolveConnectorInputTargets(channel, tree);
  }
  if (channel.binding.kind === 'lampRow') {
    return resolveLampRowTargets(channel, tree);
  }
  return [];
}

function resolveConnectorInputTargets(
  channel: InformationChannel,
  tree: NodeConfigTree,
): ConfigTarget[] {
  if (channel.binding.kind !== 'connectorInput') return [];
  const { connector, input } = channel.binding;
  const slot = tree.connectorProfile?.slots?.find((candidate) => candidate.slotId === connector);
  const path = slot?.resolvedAffectedPaths?.[input - 1];
  if (!path) return [];
  const label = buildPathLabel(tree, path) || channel.name;
  return [{ label, nodeId: tree.nodeId, elementPath: path }];
}

function resolveLampRowTargets(
  channel: InformationChannel,
  tree: NodeConfigTree,
): ConfigTarget[] {
  if (channel.binding.kind !== 'lampRow') return [];
  const segment = tree.segments.find((candidate) => candidate.name === 'Direct Lamp Control');
  if (!segment) return [];

  const instances = replicationInstances(segment.children, 'Lamp');
  const targets: ConfigTarget[] = [];

  // Unit 6 supports the currently shipped one-row lamp style. Multi-row
  // signal-aspect styles are introduced with the target-independent signal
  // capability in Unit 8 and can extend this loop at that owner seam.
  for (let offset = 0; offset < 1; offset++) {
    const ordinal = channel.binding.rowOrdinal + offset;
    const instance = instances.find((candidate) => candidate.instance === ordinal);
    if (!instance) continue;
    const label = buildPathLabel(tree, instance.path) || `Lamp #${ordinal}`;
    targets.push({ label, nodeId: tree.nodeId, elementPath: instance.path });
  }

  return targets;
}
