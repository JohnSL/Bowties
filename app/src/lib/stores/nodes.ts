// Reactive state management for discovered LCC nodes
// Uses Svelte 5 runes for reactive state

import { type DiscoveredNode } from '../api/tauri';

/**
 * Global state for discovered nodes
 */
class NodesStore {
  private _nodes = $state<DiscoveredNode[]>([]);
  private _loading = $state<boolean>(false);
  private _error = $state<string | null>(null);

  /**
   * Get current list of nodes (reactive)
   */
  get nodes() {
    return this._nodes;
  }

  /**
   * Get loading state (reactive)
   */
  get loading() {
    return this._loading;
  }

  /**
   * Get error state (reactive)
   */
  get error() {
    return this._error;
  }

  /**
   * Set the entire node list
   */
  setNodes(nodes: DiscoveredNode[]) {
    this._nodes = nodes;
    this._error = null;
  }

  /**
   * Add a newly discovered node (prevents duplicates)
   */
  addNode(node: DiscoveredNode) {
    const nodeIdString = node.node_id.map(b => b.toString(16).padStart(2, '0')).join('.');
    const exists = this._nodes.some(n => {
      const existingId = n.node_id.map(b => b.toString(16).padStart(2, '0')).join('.');
      return existingId === nodeIdString;
    });

    if (!exists) {
      this._nodes = [...this._nodes, node];
    }
  }

  /**
   * Update a specific node's data
   */
  updateNode(nodeId: number[], updatedData: Partial<DiscoveredNode>) {
    this._nodes = this._nodes.map(node => {
      const matches = node.node_id.every((byte, i) => byte === nodeId[i]);
      if (matches) {
        return { ...node, ...updatedData };
      }
      return node;
    });
  }

  /**
   * Update node by alias
   */
  updateNodeByAlias(alias: number, updatedData: Partial<DiscoveredNode>) {
    this._nodes = this._nodes.map(node => {
      if (node.alias === alias) {
        return { ...node, ...updatedData };
      }
      return node;
    });
  }

  /**
   * Set loading state
   */
  setLoading(loading: boolean) {
    this._loading = loading;
  }

  /**
   * Set error state
   */
  setError(error: string | null) {
    this._error = error;
  }

  /**
   * Clear all nodes
   */
  clear() {
    this._nodes = [];
    this._error = null;
  }
}

/**
 * Singleton instance of the nodes store
 */
export const nodesStore = new NodesStore();
