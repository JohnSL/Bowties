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
  /**
   * True when the CDI group had an explicit `<name>` element.
   * When false (or absent) the UI should suppress the group section header.
   * Absent means true (backward-compatible default).
   */
  hasName?: boolean;
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
  /**
   * Profile-supplied display-name override.
   * When non-null, the UI renders this instead of `name`.
   * Use `displayName ?? name` everywhere a group title is shown.
   */
  displayName: string | null;
  /** When true, this group can be toggled hidden/visible by the user. */
  hideable?: boolean;
  /** Initial hidden state when hideable is true. */
  hiddenByDefault?: boolean;
  /** When true, all fields within this group should be read-only. */
  readOnly?: boolean;
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

/** Write lifecycle state for a pending modification — matches Rust `WriteState`. */
export type LeafWriteState = 'dirty' | 'writing' | 'error';

/** Slider hint for an integer field. */
export interface SliderHints {
  /** When true, value is applied immediately on drag. */
  immediate: boolean;
  /** Spacing between tick marks (0 = no ticks). */
  tickSpacing: number;
  /** When true, the current value is displayed alongside the slider. */
  showValue: boolean;
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
  /** For action elements: the label to display on the trigger button. */
  buttonText?: string | null;
  /** For action elements: confirmation dialog text (shown before triggering). */
  dialogText?: string | null;
  /** For action elements: the value written when triggered. */
  actionValue?: number;
  /** Slider display hint for int fields. */
  hintSlider?: SliderHints | null;
  /** When true, render int field as radio buttons (one per map entry). */
  hintRadio?: boolean;
  /** User-modified value not yet written to the node. */
  modifiedValue?: TreeConfigValue | null;
  /** Write lifecycle state. Absent when no modification is pending. */
  writeState?: LeafWriteState | null;
  /** Error message from the last failed write attempt. */
  writeError?: string | null;
  /**
   * Set to true at runtime when the device rejects a write with 0x1083
   * (address is read-only). Disables the control for the rest of the session.
   */
  readOnly?: boolean;
  /**
   * Set to true when a persisted offline change is pending for this leaf.
   * The `modifiedValue` is set to the planned value so the field shows what
   * will be written. These leaves are excluded from `countModifiedLeaves` so
   * they don't trigger the SaveControls dirty indicator.
   */
  isOfflinePending?: boolean;
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

/** Return the effective value for display: modifiedValue if present, else value. */
export function effectiveValue(leaf: LeafConfigNode): TreeConfigValue | null {
  return leaf.modifiedValue ?? leaf.value;
}

/** Count the number of leaves with a pending `modifiedValue` in a tree. */
export function countModifiedLeaves(tree: NodeConfigTree): number {
  let count = 0;
  for (const seg of tree.segments) {
    count += countModifiedInChildren(seg.children);
  }
  return count;
}

function countModifiedInChildren(children: ConfigNode[]): number {
  let count = 0;
  for (const child of children) {
    if (isLeaf(child)) {
      if (child.modifiedValue != null && !child.isOfflinePending) count++;
    } else if (isGroup(child)) {
      count += countModifiedInChildren(child.children);
    }
  }
  return count;
}

/** Check whether any leaf in a tree has a pending `modifiedValue`. */
export function hasModifiedLeaves(tree: NodeConfigTree): boolean {
  for (const seg of tree.segments) {
    if (hasModifiedInChildren(seg.children)) return true;
  }
  return false;
}

function hasModifiedInChildren(children: ConfigNode[]): boolean {
  for (const child of children) {
    if (isLeaf(child)) {
      if (child.modifiedValue != null) return true;
    } else if (isGroup(child)) {
      if (hasModifiedInChildren(child.children)) return true;
    }
  }
  return false;
}

/** Check whether any child has a pending modification (for a subtree path). */
export function hasModifiedDescendant(children: ConfigNode[], path: string[]): boolean {
  for (const child of children) {
    if (isLeaf(child)) {
      if (child.modifiedValue != null && childIsDescendant(child.path, path)) return true;
    } else if (isGroup(child)) {
      if (hasModifiedDescendant(child.children, path)) return true;
    }
  }
  return false;
}

function childIsDescendant(childPath: string[], ancestorPath: string[]): boolean {
  if (childPath.length < ancestorPath.length) return false;
  return ancestorPath.every((seg, i) => childPath[i] === seg);
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

export function findLeafInChildren(
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
 * Find a leaf node by its path array (matching `LeafConfigNode.path`).
 * Used to resolve address/space from an EventSlotEntry.element_path.
 * Returns `undefined` if not found.
 */
export function findLeafByPath(
  tree: NodeConfigTree,
  path: string[],
): LeafConfigNode | undefined {
  const target = path.join('/');
  for (const seg of tree.segments) {
    const found = findLeafInChildrenByPath(seg.children, target);
    if (found) return found;
  }
  return undefined;
}

function findLeafInChildrenByPath(
  children: ConfigNode[],
  targetPath: string,
): LeafConfigNode | undefined {
  for (const child of children) {
    if (isLeaf(child)) {
      if (child.path.join('/') === targetPath) return child;
    } else if (isGroup(child)) {
      const found = findLeafInChildrenByPath(child.children, targetPath);
      if (found) return found;
    }
  }
  return undefined;
}

/**
 * Build a dot-joined element label for a leaf, mirroring the Rust `element_label()` logic.
 * Walks from segment root to the leaf, collecting non-empty group names as ancestors.
 *
 * Format: `Ancestor1.Ancestor2.LeafLabel`
 * Leaf label priority: `name` → first sentence of `description` → last path component.
 */
export function buildElementLabel(
  tree: NodeConfigTree,
  leaf: LeafConfigNode,
): string {
  // Find the leaf's ancestors by walking the tree
  const ancestors: string[] = [];
  for (const seg of tree.segments) {
    if (collectAncestorNames(seg.children, leaf.address, ancestors)) {
      break;
    }
  }

  // Resolve leaf label: name → first sentence of description → last path component
  let leafLabel = leaf.name?.trim() || '';
  if (!leafLabel && leaf.description) {
    const sentence = leaf.description.split('.')[0]?.trim() || '';
    if (sentence) leafLabel = sentence;
  }
  if (!leafLabel) {
    leafLabel = leaf.path[leaf.path.length - 1] ?? '';
  }

  const parts = [...ancestors.filter(n => n.length > 0), leafLabel];
  return parts.join('.');
}

/**
 * Walk children to find a leaf by address, collecting group names along the path.
 * Returns true if the leaf was found in this subtree.
 */
function collectAncestorNames(
  children: ConfigNode[],
  address: number,
  ancestors: string[],
): boolean {
  for (const child of children) {
    if (isLeaf(child) && child.address === address) {
      return true;
    }
    if (isGroup(child)) {
      const name = (child.displayName ?? getInstanceDisplayName(child)).trim();
      ancestors.push(name);
      if (collectAncestorNames(child.children, address, ancestors)) {
        return true;
      }
      ancestors.pop();
    }
  }
  return false;
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

// ─── Display helpers ─────────────────────────────────────────────────────────

/**
 * Derive a human-friendly display name for a replicated group instance.
 *
 * Searches the group's immediate children for the first string-type leaf
 * with a non-empty value (e.g. "Line Description"). Returns:
 *   - `"${description} (${instance})"` if a description is found
 *   - `instanceLabel` otherwise (e.g. "Line 3")
 */
export function getInstanceDisplayName(group: GroupConfigNode): string {
  for (const child of group.children) {
    const ev = effectiveValue(child as LeafConfigNode);
    if (
      isLeaf(child) &&
      child.elementType === 'string' &&
      ev !== null &&
      ev.type === 'string' &&
      ev.value.trim() !== ''
    ) {
      return `${ev.value.trim()} (${group.instance})`;
    }
  }
  return group.instanceLabel;
}

// ─── Child grouping ──────────────────────────────────────────────────────────

/**
 * A child item after grouping replicated siblings together.
 *
 * - `leaf`: a single LeafConfigNode (unchanged)
 * - `group`: a single non-replicated GroupConfigNode (unchanged)
 * - `replicatedSet`: consecutive replicated siblings collapsed into one entry
 */
export type GroupedChild =
  | { type: 'leaf'; node: LeafConfigNode }
  | { type: 'group'; node: GroupConfigNode }
  | { type: 'replicatedSet'; templateName: string; instances: GroupConfigNode[] };

/**
 * Group consecutive replicated siblings in a children array.
 *
 * Scans `children` in order:
 * - Leaf nodes pass through as `{ type: 'leaf' }`
 * - Non-replicated groups pass through as `{ type: 'group' }`
 * - Consecutive groups sharing the same `replicationOf` value (and replicationCount > 1)
 *   are merged into `{ type: 'replicatedSet', instances: [...] }`
 */
export function groupReplicatedChildren(children: ConfigNode[]): GroupedChild[] {
  const result: GroupedChild[] = [];
  let i = 0;

  while (i < children.length) {
    const child = children[i];

    if (isLeaf(child)) {
      result.push({ type: 'leaf', node: child });
      i++;
    } else if (isGroup(child) && child.replicationCount > 1) {
      // Collect all consecutive siblings with same replicationOf
      const instances: GroupConfigNode[] = [child];
      let j = i + 1;
      while (j < children.length) {
        const next = children[j];
        if (isGroup(next) && next.replicationOf === child.replicationOf) {
          instances.push(next);
          j++;
        } else {
          break;
        }
      }
      if (instances.length > 1) {
        result.push({
          type: 'replicatedSet',
          templateName: child.replicationOf,
          instances,
        });
      } else {
        // Single instance with replicationCount > 1 — treat as normal group
        result.push({ type: 'group', node: child });
      }
      i = j;
    } else {
      result.push({ type: 'group', node: child as GroupConfigNode });
      i++;
    }
  }

  return result;
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/** Parse "seg:N" into N, or null on failure. */
function parseSegIndex(key: string): number | null {
  const match = key.match(/^seg:(\d+)$/);
  return match ? parseInt(match[1], 10) : null;
}

// ─── Pill navigation helper ──────────────────────────────────────────────────

/**
 * Return all consecutive wrappers in `children` that belong to the same
 * sibling group as `wrapper` (same replicationOf, same replicationCount > 1).
 *
 * Mirrors the contiguous-grouping logic of `groupReplicatedChildren`.
 */
function findWrapperSiblings(children: ConfigNode[], wrapper: GroupConfigNode): GroupConfigNode[] {
  const idx = children.findIndex(c => c === wrapper);
  if (idx === -1) return [wrapper];

  // Walk backwards to find the start of the contiguous block
  let start = idx;
  while (start > 0) {
    const prev = children[start - 1];
    if (isGroup(prev) && prev.replicationOf === wrapper.replicationOf && prev.replicationCount > 1) {
      start--;
    } else {
      break;
    }
  }

  // Collect the full contiguous block
  const siblings: GroupConfigNode[] = [];
  let i = start;
  while (i < children.length) {
    const c = children[i];
    if (isGroup(c) && c.replicationOf === wrapper.replicationOf && c.replicationCount > 1) {
      siblings.push(c as GroupConfigNode);
      i++;
    } else {
      break;
    }
  }
  return siblings;
}

/**
 * Walk `elementPath` from `seg.children`, computing the pill-store entries
 * that must be set so every replicated-group ancestor of the target field shows
 * the correct instance.
 *
 * Returns a Map<pillKey, 0-based selectedIndex> ready to write to pillSelections.
 *
 * Uses path-component matching (not array-index) so spacers are handled correctly.
 *
 * Each `elem:N#K` path component may produce up to two entries:
 *  1. An **outer entry** (when wrapper_N is one of multiple same-named sibling
 *     wrappers) — key uses the first sibling wrapper's path (no # suffix),
 *     selects which wrapper.
 *  2. An **inner entry** — key uses the first instance inside wrapper_N,
 *     selects which instance within that wrapper.
 *
 * @param nodeId       - LCC node ID string (used to build pill keys)
 * @param seg          - The SegmentNode to walk into
 * @param elementPath  - Full element path, e.g. ["seg:0", "elem:0#2", "elem:1#3", "elem:2"]
 */
export function resolvePillSelectionsForPath(
  nodeId: string,
  seg: SegmentNode,
  elementPath: string[],
): Map<string, number> {
  const result = new Map<string, number>();
  let currentChildren = seg.children;

  for (let pi = 1; pi < elementPath.length; pi++) {
    const component = elementPath[pi];
    const rm = component.match(/^elem:(\d+)#(\d+)$/);

    if (rm) {
      const instNum = parseInt(rm[2], 10); // 1-based
      // Find the wrapper group by matching path component "elem:N" (without instance suffix)
      const wrapperComponent = `elem:${rm[1]}`;
      const wrapper = currentChildren.find(
        c => isGroup(c) && c.path.at(-1) === wrapperComponent,
      ) as GroupConfigNode | undefined;
      if (!wrapper) break;

      // Check whether wrapper_N is one of multiple same-named sibling wrappers.
      // When it is, `groupReplicatedChildren` groups THOSE WRAPPERS into a
      // replicatedSet, so the pill key uses the first WRAPPER's path (no # suffix).
      const wrapperSiblings = findWrapperSiblings(currentChildren, wrapper);
      if (wrapperSiblings.length > 1) {
        const firstWrapperSibling = wrapperSiblings[0];
        const wrapperIndexInSiblings = wrapperSiblings.indexOf(wrapper);
        result.set(`${nodeId}:${firstWrapperSibling.path.join('/')}`, wrapperIndexInSiblings);
      }

      // Inner pill: selects which INSTANCE within wrapper_N.
      // The pill key uses the first instance's path (has # suffix).
      const firstInstance = wrapper.children[0];
      if (!firstInstance || !isGroup(firstInstance)) break;
      result.set(`${nodeId}:${firstInstance.path.join('/')}`, instNum - 1);

      const selectedInst = wrapper.children[instNum - 1];
      if (!selectedInst || !isGroup(selectedInst)) break;
      currentChildren = selectedInst.children;
    } else {
      // Non-replicated group — navigate into it without setting a pill
      const nm = component.match(/^elem:(\d+)$/);
      if (!nm) break;
      const node = currentChildren.find(
        c => isGroup(c) && c.path.at(-1) === component,
      ) as GroupConfigNode | undefined;
      if (!node) break;
      currentChildren = node.children;
    }
  }

  return result;
}

// ─── Spec 007: Edit & Write Types ─────────────────────────────────────────────

/** Write lifecycle state for a pending edit. */
export type WriteState = 'dirty' | 'writing' | 'error' | 'clean';

/** State of an overall save operation. */
export type SaveState = 'idle' | 'saving' | 'completed' | 'partial-failure';

/**
 * Outcome of a write operation for a single field.
 * Mirrors the Rust `WriteResponse` struct (camelCase via serde).
 */
export interface WriteResult {
  /** Memory address that was written */
  address: number;
  /** Address space byte that was written */
  space: number;
  /** Whether the write succeeded */
  success: boolean;
  /** Protocol-level error code if the write failed */
  errorCode: number | null;
  /** Human-readable error message if the write failed */
  errorMessage: string | null;
  /** Number of attempts made (1–3) */
  retryCount: number;
}

/**
 * Tracks the overall progress of a save operation in the UI.
 */
export interface SaveProgress {
  /** Current save lifecycle state */
  state: SaveState;
  /** Total number of fields to write in this save batch */
  total: number;
  /** Number of fields written successfully so far */
  completed: number;
  /** Number of fields that failed during this save */
  failed: number;
  /** Label of the field currently being written, or null when idle */
  currentFieldLabel: string | null;
}

