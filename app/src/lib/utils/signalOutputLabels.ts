/**
 * Lamp names for a Signal LCC output ordinal, read from the node's
 * "Lamp Selection" enum (`#3 H1-R`). Pure translation: falls back to `#N`
 * when the node configuration is not available.
 */
import type { ConfigNode, NodeConfigTree } from '$lib/types/nodeTree';
import { replicationInstances } from '$lib/types/nodeTree';

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

export interface SignalOutputCell<T> {
  output: T;
  /** The label relative to its header row, e.g. `H1-G`. */
  short: string;
}

export interface SignalOutputRow<T> {
  /** Header name (`H1`), or '' when the labels do not follow the `#N Hx-Y` form. */
  header: string;
  cells: SignalOutputCell<T>[];
}

const OUTPUT_LABEL = /^#\d+\s+(H\d+)-\S+$/;

/** Group outputs into one row per head header, in ordinal order. */
export function signalOutputRows<T extends { ordinal: number; label: string }>(outputs: T[]): SignalOutputRow<T>[] {
  const ordered = [...outputs].sort((a, b) => a.ordinal - b.ordinal);
  const parsed = ordered.map((output) => ({ output, match: OUTPUT_LABEL.exec(output.label) }));
  if (parsed.length === 0) return [];
  if (parsed.some((p) => !p.match)) {
    return [{ header: '', cells: ordered.map((output) => ({ output, short: output.label })) }];
  }
  const rows: SignalOutputRow<T>[] = [];
  for (const { output, match } of parsed) {
    const header = match![1];
    let row = rows.find((r) => r.header === header);
    if (!row) rows.push((row = { header, cells: [] }));
    row.cells.push({ output, short: output.label.replace(/^#\d+\s+/, '') });
  }
  return rows;
}

function descriptionIn(nodes: ConfigNode[], leafName: string, instanceNumber: number, groupName: string): string | undefined {
  const group = replicationInstances(nodes, groupName).find((candidate) => candidate.instance === instanceNumber);
  const leaf = group?.children.find((child) => child.kind === 'leaf' && child.name === leafName);
  if (leaf?.kind !== 'leaf' || leaf.value?.type !== 'string') return undefined;
  return leaf.value.value.trim() || undefined;
}

/**
 * The user's own description of a Signal-LCC output owner ("Mast N" or
 * "Direct lamp N", as serialized by the backend), read from its
 * `Mast Description` / `Lamp Description` leaf.
 */
export function signalOutputOwnerDescription(tree: NodeConfigTree | undefined, owner: string): string | undefined {
  if (!tree) return undefined;
  const mast = /^Mast (\d+)$/.exec(owner);
  const lamp = /^Direct lamp (\d+)$/.exec(owner);
  const [segmentName, groupName, leafName, n] = mast
    ? ['Rule to Aspect', 'Mast', 'Mast Description', Number(mast[1])]
    : lamp
      ? ['Direct Lamp Control', 'Lamp', 'Lamp Description', Number(lamp[1])]
      : [];
  if (!segmentName) return undefined;
  const segment = tree.segments.find((candidate) => candidate.name === segmentName);
  return segment ? descriptionIn(segment.children, leafName as string, n as number, groupName as string) : undefined;
}
