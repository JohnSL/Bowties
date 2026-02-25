/**
 * Unified Node Configuration Tree — TypeScript type definitions.
 *
 * Mirrors the Rust types in `app/src-tauri/src/node_tree.rs`.
 * All field names use camelCase (matching Rust's `#[serde(rename_all = "camelCase")]`).
 *
 * Spec: 007-unified-node-tree, Phase 3.
 */

// ─── Existing type re-used from tauri.ts ─────────────────────────────────────

/** Event role classification — matches `lcc_rs::cdi::EventRole`. */
export type EventRole = 'Producer' | 'Consumer' | 'Ambiguous';

// ─── Identification ──────────────────────────────────────────────────────────

/** CDI `<identification>` element — matches `lcc_rs::cdi::Identification`. */
export interface Identification {
  manufacturer: string | null;
  model: string | null;
  hardwareVersion: string | null;
  softwareVersion: string | null;
}

// ─── Tree node types ─────────────────────────────────────────────────────────

/** Root of the unified configuration tree for a single LCC node. */
export interface NodeConfigTree {
  /** Node identifier (dotted-hex, e.g. "05.02.01.02.03.00") */
  nodeId: string;
  /** Optional identification from CDI `<identification>` element */
  identity: Identification | null;
  /** Top-level segments mirroring CDI `<segment>` elements */
  segments: SegmentNode[];
}

/** One CDI segment — a contiguous memory space. */
export interface SegmentNode {
  /** Segment display name */
  name: string;
  /** Optional description */
  description: string | null;
  /** Starting address in memory space */
  origin: number;
  /** Memory space number (e.g. 253 for configuration) */
  space: number;
  /** Child nodes (groups and leaves) */
  children: ConfigNode[];
}

/**
 * A node in the configuration tree — either a group or a leaf element.
 *
 * Rust serializes this as `{ "kind": "group"|"leaf", ...fields }`.
 */
export type ConfigNode = GroupConfigNode | LeafConfigNode;

/** Discriminated union tag for ConfigNode. */
interface ConfigNodeBase {
  kind: 'group' | 'leaf';
}

/** A (possibly replicated) group of child config nodes. */
export interface GroupConfigNode extends ConfigNodeBase {
  kind: 'group';
  /** Display name for this group instance */
  name: string;
  /** Optional description */
  description: string | null;
  /** 1-based replication instance number (1 when not replicated) */
  instance: number;
  /** Computed instance label (e.g. "Event 3") */
  instanceLabel: string;
  /** Original group name before replication (for sibling disambiguation) */
  replicationOf: string;
  /** Total number of replications for this group template */
  replicationCount: number;
  /** Index-based path identifying this group (e.g. ["seg:0", "elem:2#3"]) */
  path: string[];
  /** Child nodes */
  children: ConfigNode[];
}

/** Leaf element types — matches Rust `LeafType`. */
export type LeafType = 'int' | 'string' | 'eventId' | 'float' | 'action' | 'blob';

/**
 * Typed config value — discriminated union matching Rust `ConfigValue`.
 *
 * Rust serializes with `{ "type": "int"|"string"|"eventId"|"float", ...data }`.
 */
export type TreeConfigValue =
  | { type: 'int'; value: number }
  | { type: 'string'; value: string }
  | { type: 'eventId'; bytes: number[]; hex: string }
  | { type: 'float'; value: number };

/** Optional constraints on a leaf element. */
export interface LeafConstraints {
  min: number | null;
  max: number | null;
  defaultValue: string | null;
  mapEntries: TreeMapEntry[] | null;
}

/** Value→label mapping entry. */
export interface TreeMapEntry {
  value: number;
  label: string;
}

/** A leaf configuration element (int, string, eventid, float, action, blob). */
export interface LeafConfigNode extends ConfigNodeBase {
  kind: 'leaf';
  /** Display name */
  name: string;
  /** Optional description */
  description: string | null;
  /** Type discriminator */
  elementType: LeafType;
  /** Absolute memory address (origin + computed offset) */
  address: number;
  /** Size in bytes */
  size: number;
  /** Memory space number */
  space: number;
  /** Index-based path identifying this element */
  path: string[];
  /** Current configuration value (populated after config read) */
  value: TreeConfigValue | null;
  /** Classified event role (only meaningful for EventId leaves) */
  eventRole: EventRole | null;
  /** Constraints (min, max, default, map entries) */
  constraints: LeafConstraints | null;
}

// ─── Event payloads ──────────────────────────────────────────────────────────

/** Payload emitted with the `node-tree-updated` Tauri event. */
export interface NodeTreeUpdatedPayload {
  nodeId: string;
  leafCount: number;
}

// ─── Type guards ─────────────────────────────────────────────────────────────

/** Narrow a ConfigNode to GroupConfigNode. */
export function isGroup(node: ConfigNode): node is GroupConfigNode {
  return node.kind === 'group';
}

/** Narrow a ConfigNode to LeafConfigNode. */
export function isLeaf(node: ConfigNode): node is LeafConfigNode {
  return node.kind === 'leaf';
}

// ─── Tree traversal helpers ──────────────────────────────────────────────────

/**
 * Find the children for a given path within a tree.
 *
 * Path segments match `SegmentNode.name` (first level) then
 * group `path` arrays (deeper levels). An empty path returns
 * the segment list as "children" (for the top-level column).
 */
export function getChildrenAtPath(
  tree: NodeConfigTree,
  pathKey: string[],
): ConfigNode[] | null {
  if (pathKey.length === 0) return null;

  // First component selects a segment by index (e.g. "seg:0")
  const segKey = pathKey[0];
  const segIdx = parseSegIndex(segKey);
  if (segIdx === null || segIdx >= tree.segments.length) return null;

  let children = tree.segments[segIdx].children;

  // Walk deeper path components
  for (let i = 1; i < pathKey.length; i++) {
    const step = pathKey[i];
    const found = children.find(
      (c) => isGroup(c) && c.path[c.path.length - 1] === step,
    );
    if (!found || !isGroup(found)) return null;
    children = found.children;
  }

  return children;
}

/**
 * Find a leaf node by its absolute memory address.
 * Returns `null` if not found.
 */
export function findLeafByAddress(
  tree: NodeConfigTree,
  address: number,
): LeafConfigNode | null {
  for (const seg of tree.segments) {
    const found = findLeafInChildren(seg.children, address);
    if (found) return found;
  }
  return null;
}

function findLeafInChildren(
  children: ConfigNode[],
  address: number,
): LeafConfigNode | null {
  for (const child of children) {
    if (isLeaf(child)) {
      if (child.address === address) return child;
    } else if (isGroup(child)) {
      const found = findLeafInChildren(child.children, address);
      if (found) return found;
    }
  }
  return null;
}

/**
 * Count all leaf nodes in the tree.
 */
export function countLeaves(tree: NodeConfigTree): number {
  let count = 0;
  for (const seg of tree.segments) {
    count += countLeavesInChildren(seg.children);
  }
  return count;
}

function countLeavesInChildren(children: ConfigNode[]): number {
  let count = 0;
  for (const child of children) {
    if (isLeaf(child)) {
      count++;
    } else if (isGroup(child)) {
      count += countLeavesInChildren(child.children);
    }
  }
  return count;
}

/**
 * Collect all EventId leaves from the tree.
 * Useful for cross-referencing with the bowtie catalog.
 */
export function collectEventIdLeaves(tree: NodeConfigTree): LeafConfigNode[] {
  const results: LeafConfigNode[] = [];
  for (const seg of tree.segments) {
    collectEventIdLeavesInChildren(seg.children, results);
  }
  return results;
}

function collectEventIdLeavesInChildren(
  children: ConfigNode[],
  results: LeafConfigNode[],
): void {
  for (const child of children) {
    if (isLeaf(child) && child.elementType === 'eventId') {
      results.push(child);
    } else if (isGroup(child)) {
      collectEventIdLeavesInChildren(child.children, results);
    }
  }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/** Parse "seg:N" into N, or null on failure. */
function parseSegIndex(key: string): number | null {
  const match = key.match(/^seg:(\d+)$/);
  return match ? parseInt(match[1], 10) : null;
}
