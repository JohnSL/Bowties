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
  // Unit 6 supports the currently shipped one-row lamp style. Multi-row
  // signal-aspect styles are introduced with the target-independent signal
  // capability in Unit 8 and can extend this at that owner seam.
  const target = replicatedInstanceTarget(
    tree, 'Direct Lamp Control', 'Lamp', channel.binding.rowOrdinal, `Lamp #${channel.binding.rowOrdinal}`,
  );
  return target ? [target] : [];
}

function replicatedInstanceTarget(
  tree: NodeConfigTree,
  segmentName: string,
  groupName: string,
  ordinal: number,
  fallbackLabel: string,
): ConfigTarget | undefined {
  const segment = tree.segments.find((candidate) => candidate.name === segmentName);
  if (!segment) return undefined;
  const instance = replicationInstances(segment.children, groupName).find((candidate) => candidate.instance === ordinal);
  if (!instance) return undefined;
  const label = buildPathLabel(tree, instance.path) || fallbackLabel;
  return { label, nodeId: tree.nodeId, elementPath: instance.path };
}

/**
 * Resolve a Signal-LCC output owner label ("Mast N" or "Direct lamp N", as
 * serialized by the backend) to the configuration group that drives the pin.
 */
export function resolveSignalOutputOwnerTarget(
  tree: NodeConfigTree | undefined,
  owner: string,
): ConfigTarget | undefined {
  if (!tree) return undefined;
  const mast = /^Mast (\d+)$/.exec(owner);
  if (mast) return replicatedInstanceTarget(tree, 'Rule to Aspect', 'Mast', Number(mast[1]), owner);
  const lamp = /^Direct lamp (\d+)$/.exec(owner);
  if (lamp) return replicatedInstanceTarget(tree, 'Direct Lamp Control', 'Lamp', Number(lamp[1]), owner);
  return undefined;
}
