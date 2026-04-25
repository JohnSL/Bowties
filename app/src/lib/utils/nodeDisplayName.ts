import type { DiscoveredNode } from '$lib/api/tauri';

export function resolveNodeDisplayName(
  nodeId: string,
  node: Pick<DiscoveredNode, 'snip_data'> | null | undefined
): string {
  const snip = node?.snip_data;
  if (!snip) return nodeId;

  const userName = snip.user_name?.trim() ?? '';
  if (userName) return userName;

  const manufacturer = snip.manufacturer?.trim() ?? '';
  const model = snip.model?.trim() ?? '';
  if (manufacturer && model) return `${manufacturer} — ${model}`;
  if (model) return model;

  return nodeId;
}