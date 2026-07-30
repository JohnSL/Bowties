/**
 * channelConfigNavigation.ts — resolves a channel's binding to its underlying
 * CDI config target(s), for discoverable navigation from the Railroad tab.
 *
 * A `connectorInput` binding resolves to a single group (the connector
 * slot's resolved affected path for that input). A `lampRow` binding
 * resolves to one target per row the channel's style claims (styles that
 * span multiple rows, e.g. `2-led-bicolor-aspect`, produce N targets).
 *
 * Mirrors the CDI-path resolution done backend-side in
 * `bowties-core::channel_events` (connector profile `resolvedAffectedPaths`
 * / `Direct Lamp Control` replication instances) but reads from the
 * frontend's already-cached `NodeConfigTree` instead of re-walking on the
 * backend.
 */

import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import { replicationInstances, buildPathLabel } from '$lib/types/nodeTree';
import { getStyleRowCount } from '$lib/utils/channelStyles';

/** A single navigable config-tree location for a channel. */
export interface ConfigTarget {
  label: string;
  nodeId: string;
  elementPath: string[];
}

/** Resolve the config target(s) a channel's binding points to. */
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
  const slot = tree.connectorProfile?.slots?.find((s) => s.slotId === connector);
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
  const { rowOrdinal } = channel.binding;
  const segment = tree.segments.find((s) => s.name === 'Direct Lamp Control');
  if (!segment) return [];
  const instances = replicationInstances(segment.children, 'Lamp');
  const rowCount = Math.max(getStyleRowCount(channel.style), 1);

  const targets: ConfigTarget[] = [];
  for (let offset = 0; offset < rowCount; offset++) {
    const ordinal = rowOrdinal + offset;
    const instance = instances.find((inst) => inst.instance === ordinal);
    if (!instance) continue;
    const label = buildPathLabel(tree, instance.path) || `Lamp #${ordinal}`;
    targets.push({ label, nodeId: tree.nodeId, elementPath: instance.path });
  }
  return targets;
}
