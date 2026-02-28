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

  // ── Listener lifecycle ────────────────────────────────────────────────────

  /**
   * Register a Tauri event listener for `node-tree-updated`.
   *
   * When the backend emits this event (after merging config values or
   * event roles), we automatically re-fetch the tree for the affected node.
   *
   * Safe to call multiple times — subsequent calls are no-ops.
   */
  async startListening(): Promise<void> {
    if (this._unlisten) return;

    this._unlisten = await listen<NodeTreeUpdatedPayload>(
      'node-tree-updated',
      (event) => {
        const { nodeId } = event.payload;
        // Only refresh if we already have this tree loaded.
        // If the frontend never requested this tree, skip the fetch.
        if (this._trees.has(nodeId)) {
          this.refreshTree(nodeId);
        }
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
