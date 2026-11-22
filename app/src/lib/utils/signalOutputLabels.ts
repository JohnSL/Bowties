/**
 * Lamp names for a Signal LCC output ordinal, read from the node's
 * "Lamp Selection" enum (`#3 H1-R`). Pure translation: falls back to `#N`
 * when the node configuration is not available.
 */
import type { ConfigNode, NodeConfigTree } from '$lib/types/nodeTree';

const LAMP_SELECTION = 'Lamp Selection';

function findLabel(nodes: ConfigNode[], ordinal: number): string | undefined {
  for (const node of nodes) {
    if (node.kind === 'group') {
      const hit = findLabel(node.children, ordinal);
      if (hit) return hit;
    } else if (node.name === LAMP_SELECTION) {
      const entry = node.constraints?.mapEntries?.find((e) => e.value === ordinal);
      if (entry) return entry.label;
    }
  }
  return undefined;
}

export function signalOutputLabel(tree: NodeConfigTree | undefined, ordinal: number): string {
  if (!tree) return `#${ordinal}`;
  for (const segment of tree.segments) {
    const hit = findLabel(segment.children, ordinal);
    if (hit) return hit;
  }
  return `#${ordinal}`;
}
