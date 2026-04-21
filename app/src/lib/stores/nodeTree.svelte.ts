/**
 * Svelte 5 reactive store for the Unified Node Configuration Tree.
 *
 * Holds the per-node tree that merges CDI hierarchy, computed addresses,
 * config values, and event roles.  Populated by calling the `get_node_tree`
 * Tauri command and kept up-to-date via the `node-tree-updated` event.
 *
 * Spec: 007-unified-node-tree, Phase 3.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  NodeConfigTree,
  NodeTreeUpdatedPayload,
  ConfigNode,
  LeafConfigNode,
  SegmentNode,
} from '$lib/types/nodeTree';
import { isGroup, isLeaf, getChildrenAtPath } from '$lib/types/nodeTree';
import type { OfflineChangeRow } from '$lib/api/sync';

// ─── Store class ─────────────────────────────────────────────────────────────

class NodeTreeStore {
  /** Map of nodeId → NodeConfigTree. */
  private _trees = $state<Map<string, NodeConfigTree>>(new Map());

  /** Set of nodeIds currently being fetched (prevents duplicate requests). */
  private _loading = $state<Set<string>>(new Set());

  /** Per-node error messages. */
  private _errors = $state<Map<string, string>>(new Map());

  /** Active Tauri event listener handle. */
  private _unlisten: UnlistenFn | null = null;

  // ── Reactive getters ──────────────────────────────────────────────────────

  /** Get the full trees map (reactive). */
  get trees(): Map<string, NodeConfigTree> {
    return this._trees;
  }

  /** Check whether any tree load is in progress. */
  get isLoading(): boolean {
    return this._loading.size > 0;
  }

  /** Per-node error map (reactive). */
  get errors(): Map<string, string> {
    return this._errors;
  }

  // ── Tree access ───────────────────────────────────────────────────────────

  /** Get the tree for a specific node, or undefined if not loaded. */
  getTree(nodeId: string): NodeConfigTree | undefined {
    return this._trees.get(nodeId);
  }

  /** Whether a tree exists for the given nodeId. */
  hasTree(nodeId: string): boolean {
    return this._trees.has(nodeId);
  }

  /** Whether a specific node is currently loading. */
  isNodeLoading(nodeId: string): boolean {
    return this._loading.has(nodeId);
  }

  /** Get error for a specific node, or undefined. */
  getError(nodeId: string): string | undefined {
    return this._errors.get(nodeId);
  }

  // ── Segment helpers ───────────────────────────────────────────────────────

  /** Get the segments for a node (empty array if tree not loaded). */
  getSegments(nodeId: string): SegmentNode[] {
    return this._trees.get(nodeId)?.segments ?? [];
  }

  /** Get children at a given path within a node's tree. */
  getChildren(nodeId: string, pathKey: string[]): ConfigNode[] | null {
    const tree = this._trees.get(nodeId);
    if (!tree) return null;
    return getChildrenAtPath(tree, pathKey);
  }

  /** Find a leaf by address across a node's tree. */
  getLeaf(nodeId: string, address: number): LeafConfigNode | null {
    const tree = this._trees.get(nodeId);
    if (!tree) return null;

    for (const seg of tree.segments) {
      const found = findLeafInChildren(seg.children, address);
      if (found) return found;
    }
    return null;
  }

  // ── Tree loading ──────────────────────────────────────────────────────────

  /**
   * Load (or reload) the tree for a node by invoking `get_node_tree`.
   *
   * If a load is already in progress for this node, the call is a no-op.
   * The tree is stored and reactively available via `getTree(nodeId)`.
   */
  async loadTree(nodeId: string): Promise<NodeConfigTree | null> {
    if (this._loading.has(nodeId)) return this._trees.get(nodeId) ?? null;

    // Mark loading
    this._loading = new Set([...this._loading, nodeId]);
    this._errors = new Map(this._errors);
    this._errors.delete(nodeId);

    try {
      const tree = await invoke<NodeConfigTree>('get_node_tree', { nodeId });

      // Store tree
      this._trees = new Map(this._trees);
      this._trees.set(nodeId, tree);

      return tree;
    } catch (err) {
      const message = typeof err === 'string' ? err : String(err);
      this._errors = new Map(this._errors);
      this._errors.set(nodeId, message);
      return null;
    } finally {
      // Clear loading flag
      this._loading = new Set(this._loading);
      this._loading.delete(nodeId);
    }
  }

  /**
   * Refresh an existing tree by re-fetching from the backend.
   * This picks up any config values or event roles that were merged server-side.
   *
   * Unlike `loadTree`, this bypasses the loading guard so a fresh fetch
   * is always issued — even if another load is already in progress.
   */
  async refreshTree(nodeId: string): Promise<NodeConfigTree | null> {
    // Clear any in-progress guard so the fetch isn't skipped
    if (this._loading.has(nodeId)) {
      this._loading = new Set(this._loading);
      this._loading.delete(nodeId);
    }
    return this.loadTree(nodeId);
  }

  // ── Store in tree directly (for optimistic / incremental updates) ─────────

  /** Replace or insert a tree directly (no backend call). */
  setTree(nodeId: string, tree: NodeConfigTree): void {
    this._trees = new Map(this._trees);
    this._trees.set(nodeId, tree);
  }

  /**
   * Update the cached value for a single leaf after a successful write (FR-021).
   *
   * Locates the leaf by `fieldPath` and sets its `value` to `newValue`,
   * causing any reactive derivations from the tree to reflect the written data
   * without requiring a full re-read from the node.
   *
   * @param nodeId    The node owning the tree.
   * @param fieldPath The leaf's path array (e.g. ["seg:0", "elem:1"]).
   * @param newValue  The value just written to the node.
   */
  updateLeafValue(nodeId: string, fieldPath: string[], newValue: import('$lib/types/nodeTree').TreeConfigValue): void {
    const tree = this._trees.get(nodeId);
    if (!tree) return;

    // Deep-clone the tree so Svelte 5 reactivity detects the change
    const updatedTree = deepCloneTree(tree);
    const leaf = findLeafByPath(updatedTree, fieldPath);
    if (leaf) {
      leaf.value = newValue;
      this._trees = new Map(this._trees);
      this._trees.set(nodeId, updatedTree);
    }
  }

  /**
   * Set or clear a leaf's modifiedValue locally without backend IPC.
   * Used by offline mode to keep dirty indicators aligned with online UX.
   */
  setLeafModifiedValue(
    nodeId: string,
    fieldPath: string[],
    modifiedValue: import('$lib/types/nodeTree').TreeConfigValue | null,
  ): void {
    const tree = this._trees.get(nodeId);
    if (!tree) return;

    const updatedTree = deepCloneTree(tree);
    const leaf = findLeafByPath(updatedTree, fieldPath);
    if (!leaf) return;

    leaf.modifiedValue = modifiedValue;
    if (modifiedValue === null) {
      leaf.writeState = null;
      leaf.writeError = null;
    }

    this._trees = new Map(this._trees);
    this._trees.set(nodeId, updatedTree);
  }

  /** Clear all modifiedValue markers across all cached trees. */
  clearAllModifiedValues(): void {
    const next = new Map<string, NodeConfigTree>();
    for (const [nodeId, tree] of this._trees.entries()) {
      const updatedTree = deepCloneTree(tree);
      clearModifiedInChildren(updatedTree.segments.flatMap((s) => s.children));
      next.set(nodeId, updatedTree);
    }
    this._trees = next;
  }

  /**
   * Apply pending offline change values to the live tree.
   *
   * For each persisted offline row with `status === 'pending'`, find the
   * matching leaf (by nodeId + space + address) and set
   * `leaf.modifiedValue = plannedValue` and `leaf.isOfflinePending = true`.
   * This makes the config field show the planned value while the bus value
   * is shown in the annotation.
   *
   * Must be called after a tree is loaded/rebuilt when a layout is open.
   */
  applyOfflinePendingValues(offlineRows: OfflineChangeRow[]): void {
    const pendingRows = offlineRows.filter((r) => r.status === 'pending' && r.nodeId && r.space != null && r.offset != null);
    if (pendingRows.length === 0) return;

    const next = new Map<string, NodeConfigTree>();
    for (const [nodeId, tree] of this._trees.entries()) {
      const rows = pendingRows.filter((r) => r.nodeId === nodeId);
      if (rows.length === 0) {
        next.set(nodeId, tree);
        continue;
      }
      const updatedTree = deepCloneTree(tree);
      for (const row of rows) {
        const address = parseInt(row.offset!, 16);
        const space = row.space!;
        applyPendingInChildren(
          updatedTree.segments.flatMap((s) => s.children),
          space,
          address,
          row.plannedValue,
        );
      }
      next.set(nodeId, updatedTree);
    }
    this._trees = next;
  }

  // ── Listener lifecycle ────────────────────────────────────────────────────

  /**
   * Register a Tauri event listener for `node-tree-updated`.
   *
   * When the backend emits this event (after merging config values or
   * event roles), we automatically re-fetch the tree for the affected node.
   *
   * The optional `onTreeLoaded` callback fires after each tree load completes.
   * Callers can use this to apply post-load transformations (e.g. pending
   * offline change values) without coupling stores together.
   *
   * Safe to call multiple times — subsequent calls are no-ops.
   */
  async startListening(onTreeLoaded?: (nodeId: string) => void): Promise<void> {
    if (this._unlisten) return;

    this._unlisten = await listen<NodeTreeUpdatedPayload>(
      'node-tree-updated',
      (event) => {
        const { nodeId } = event.payload;
        // Load the tree whether or not it was previously loaded — this ensures
        // newly discovered nodes are fetched automatically after CDI scan, not
        // just nodes the user already expanded.  `loadTree` deduplicates via the
        // _loading set, and for already-loaded nodes it acts as a refresh.
        void this.loadTree(nodeId).then(() => {
          if (onTreeLoaded) onTreeLoaded(nodeId);
        });
      },
    );
  }

  /**
   * Remove the Tauri event listener.
   */
  stopListening(): void {
    if (this._unlisten) {
      this._unlisten();
      this._unlisten = null;
    }
  }

  // ── Reset ─────────────────────────────────────────────────────────────────

  /** Clear all trees, errors, and loading state (e.g. on disconnect). */
  reset(): void {
    this._trees = new Map();
    this._loading = new Set();
    this._errors = new Map();
  }
}

// ─── Internal helper ─────────────────────────────────────────────────────────

function findLeafInChildren(
  children: ConfigNode[],
  address: number,
): LeafConfigNode | null {
  for (const child of children) {
    if (isLeaf(child) && child.address === address) return child;
    if (isGroup(child)) {
      const found = findLeafInChildren(child.children, address);
      if (found) return found;
    }
  }
  return null;
}

// ─── Singleton export ────────────────────────────────────────────────────────

/**
 * Singleton reactive store for node configuration trees.
 *
 * Usage in a Svelte component:
 * ```svelte
 * <script>
 *   import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
 *   import { onMount, onDestroy } from 'svelte';
 *
 *   onMount(() => nodeTreeStore.startListening());
 *   onDestroy(() => nodeTreeStore.stopListening());
 *
 *   // Load tree when a node is selected
 *   async function onNodeSelect(nodeId: string) {
 *     await nodeTreeStore.loadTree(nodeId);
 *   }
 *
 *   // Reactive access
 *   let tree = $derived(nodeTreeStore.getTree(selectedNodeId));
 *   let segments = $derived(nodeTreeStore.getSegments(selectedNodeId));
 * </script>
 * ```
 */
export const nodeTreeStore = new NodeTreeStore();

// ─── Helpers for updateLeafValue ──────────────────────────────────────────────

/** Deep-clone a NodeConfigTree (used for immutable update in updateLeafValue). */
function deepCloneTree(tree: NodeConfigTree): NodeConfigTree {
  return JSON.parse(JSON.stringify(tree)) as NodeConfigTree;
}

/**
 * Find a LeafConfigNode in a tree by its path array.
 *
 * Path follows the format: `["seg:N", "elem:M", ...]`. The first segment
 * `seg:N` identifies the segment by index; subsequent segments navigate groups.
 */
function findLeafByPath(tree: NodeConfigTree, path: string[]): LeafConfigNode | null {
  if (path.length === 0) return null;

  // Parse "seg:N"
  const segMatch = path[0].match(/^seg:(\d+)$/);
  if (!segMatch) return null;
  const segIdx = parseInt(segMatch[1], 10);
  const segment = tree.segments[segIdx];
  if (!segment) return null;

  return findLeafByPathInChildren(segment.children, path.slice(1));
}

/**
 * Find a child node by matching the last path component string, not by array index.
 *
 * This is necessary because Rust's `build_node_config_tree` encodes the CDI element
 * index `i` in path strings (e.g. `"elem:2"`), but spacer groups hit a `continue`
 * before `children.push(...)`, so the CDI index and the array index can diverge.
 * Matching on `path.at(-1)` is authoritative.
 */
function findChildByComponent(children: ConfigNode[], component: string): ConfigNode | undefined {
  return children.find(c => c.path.at(-1) === component);
}

function findLeafByPathInChildren(children: ConfigNode[], path: string[]): LeafConfigNode | null {
  if (path.length === 0) return null;

  const segment = path[0];

  // Could be "elem:N" or "elem:N#M"
  const elemMatch = segment.match(/^elem:(\d+)(?:#(\d+))?$/);
  if (!elemMatch) return null;

  const instanceNum = elemMatch[2] ? parseInt(elemMatch[2], 10) : undefined;

  if (instanceNum !== undefined) {
    // elem:N#M — find the wrapper group by its base path component "elem:N",
    // then navigate to instance M (1-based → 0-based)
    const wrapperComponent = `elem:${elemMatch[1]}`;
    const wrapper = findChildByComponent(children, wrapperComponent);
    if (!wrapper || !isGroup(wrapper)) return null;
    const instanceNode = wrapper.children[instanceNum - 1];
    if (!instanceNode) return null;
    if (path.length === 1) return isLeaf(instanceNode) ? instanceNode : null;
    if (isGroup(instanceNode)) return findLeafByPathInChildren(instanceNode.children, path.slice(1));
    return null;
  }

  // Plain "elem:N" — find by path component matching
  const node = findChildByComponent(children, segment);
  if (!node) return null;

  if (path.length === 1) {
    return isLeaf(node) ? node : null;
  }

  if (isGroup(node)) {
    return findLeafByPathInChildren(node.children, path.slice(1));
  }

  return null;
}

function clearModifiedInChildren(children: ConfigNode[]): void {
  for (const child of children) {
    if (isLeaf(child)) {
      child.modifiedValue = null;
      child.isOfflinePending = false;
      child.writeState = null;
      child.writeError = null;
      continue;
    }
    if (isGroup(child)) {
      clearModifiedInChildren(child.children);
    }
  }
}

function parseOfflineValueString(value: string): import('$lib/types/nodeTree').TreeConfigValue {
  if (/^[0-9]+$/.test(value)) return { type: 'int', value: parseInt(value, 10) };
  if (/^[0-9]+\.[0-9]+$/.test(value)) return { type: 'float', value: parseFloat(value) };
  if (/^([0-9A-F]{2}\.){7}[0-9A-F]{2}$/i.test(value)) {
    const bytes = value.split('.').map((b) => parseInt(b, 16));
    return { type: 'eventId', bytes, hex: value.toUpperCase() };
  }
  return { type: 'string', value };
}

function applyPendingInChildren(children: ConfigNode[], space: number, address: number, plannedValue: string): boolean {
  for (const child of children) {
    if (isLeaf(child) && child.space === space && child.address === address) {
      child.modifiedValue = parseOfflineValueString(plannedValue);
      child.isOfflinePending = true;
      return true;
    }
    if (isGroup(child)) {
      if (applyPendingInChildren(child.children, space, address, plannedValue)) return true;
    }
  }
  return false;
}

