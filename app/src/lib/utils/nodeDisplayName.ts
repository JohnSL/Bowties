import type { DiscoveredNode } from '$lib/api/tauri';
import {
  isGroup,
  isLeaf,
  type ConfigNode,
  type LeafConfigNode,
  type NodeConfigTree,
  type TreeConfigValue,
} from '$lib/types/nodeTree';

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

/**
 * ACDI user-info memory space (0xFB). The user-assigned node name and
 * description live here; the User Name is the lowest-address string leaf.
 * This is the editable equivalent of `snip_data.user_name`.
 */
const ACDI_USER_SPACE = 251;

/**
 * Locate the editable User Name leaf in a node's config tree: the
 * lowest-address `string` leaf in the ACDI user space (251). The ACDI user
 * space contains only the user name (offset 1) and user description
 * (offset 64), so lowest-address reliably selects the name. Returns null when
 * the node does not expose the ACDI user space.
 */
export function findUserNameLeaf(
  tree: NodeConfigTree | null | undefined,
): LeafConfigNode | null {
  if (!tree) return null;

  let best: LeafConfigNode | null = null;
  const visit = (nodes: ConfigNode[]): void => {
    for (const node of nodes) {
      if (isLeaf(node)) {
        if (node.space === ACDI_USER_SPACE && node.elementType === 'string') {
          if (!best || node.address < best.address) best = node;
        }
      } else if (isGroup(node)) {
        visit(node.children);
      }
    }
  };
  for (const segment of tree.segments) visit(segment.children);
  return best;
}

/**
 * Resolve the effective User Name from the edit layer — the editable ACDI
 * User Name leaf resolved through the draft → offline → baseline waterfall
 * (ADR-0003 point 4). Returns the trimmed name when present, or null when the
 * node has no User Name leaf or the resolved value is empty.
 *
 * `resolveValue` is the leaf-value resolver (e.g. `makeValueResolver(nodeId)`
 * or an inline `configChangesStore.overrideValue(...) ?? leaf.value`). Keeping
 * it injected keeps this helper pure and store-free.
 */
export function resolveEffectiveUserName(
  tree: NodeConfigTree | null | undefined,
  resolveValue: (leaf: LeafConfigNode) => TreeConfigValue | null,
): string | null {
  const leaf = findUserNameLeaf(tree);
  if (!leaf) return null;

  const value = resolveValue(leaf);
  if (value?.type !== 'string') return null;

  const trimmed = value.value.trim();
  return trimmed ? trimmed : null;
}