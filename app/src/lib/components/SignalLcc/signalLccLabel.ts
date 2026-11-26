import { resolveNodeParts } from '$lib/layout';
import { nodeIdToDisplayHex } from '$lib/utils/nodeId';

/** Common node naming: display name over "<model> · <node address>". */
export function signalLccLabel(nodeKey: string): { name: string; detail: string } {
  const parts = resolveNodeParts(nodeKey);
  const address = nodeIdToDisplayHex(nodeKey) || nodeKey;
  return {
    name: parts.name || address,
    detail: [parts.model, address].filter(Boolean).join(' · '),
  };
}

export const signalLccName = (nodeKey: string): string => signalLccLabel(nodeKey).name;
