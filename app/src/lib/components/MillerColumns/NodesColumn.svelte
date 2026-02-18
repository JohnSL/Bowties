<script lang="ts">
  import { getDiscoveredNodes, type DiscoveredNode } from '$lib/api/cdi';
  import { millerColumnsStore } from '$lib/stores/millerColumns';
  import { getCdiStructure } from '$lib/api/cdi';

  let nodes: DiscoveredNode[] = [];
  let loading = false;
  let error: string | null = null;
  let selectedNodeId: string | null = null;

  // Subscribe to store to track selected node
  $: if ($millerColumnsStore.selectedNode) {
    selectedNodeId = $millerColumnsStore.selectedNode.nodeId;
  }

  /**
   * Refresh nodes from backend state
   * Called by parent component when node data has been updated
   */
  export async function refresh() {
    loading = true;
    error = null;
    
    try {
      const response = await getDiscoveredNodes();
      nodes = response.nodes;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      console.error('[NodesColumn] Failed to load nodes:', err);
    } finally {
      loading = false;
    }
  }

  async function handleNodeSelect(node: DiscoveredNode) {
    if (!node.hasCdi) {
      alert(`Node "${node.nodeName}" does not have CDI data available. Please download CDI first.`);
      return;
    }

    try {
      millerColumnsStore.setLoading(true);
      
      // Select the node in store (resets columns and breadcrumb)
      millerColumnsStore.selectNode(node.nodeId, node.nodeName);
      
      // Load CDI structure to populate segments column
      const cdiStructure = await getCdiStructure(node.nodeId);
      
      // Add segments column
      millerColumnsStore.addColumn({
        depth: 1,
        type: 'segments',
        items: cdiStructure.segments.map(seg => ({
          id: seg.id,
          name: seg.name || `Space ${seg.space}`,
          fullName: seg.description || undefined,
          type: undefined,
          hasChildren: seg.hasGroups || seg.hasElements,
          metadata: {
            ...seg.metadata,
            space: seg.space,
            hasGroups: seg.hasGroups,
            hasElements: seg.hasElements,
          } as Record<string, unknown>,
        })),
        parentPath: [],
      });
      
      millerColumnsStore.setLoading(false);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      millerColumnsStore.setError(`Failed to load CDI structure: ${errorMsg}`);
      console.error('[NodesColumn] Failed to load CDI structure:', err);
    }
  }

  /**
   * Get display name for a node
   * Priority: user name > SNIP (manufacturer + model) > fallback to Node ID
   */
  function getNodeDisplayName(node: DiscoveredNode): string {
    // For now, use nodeName from backend (already implements the priority logic)
    return node.nodeName;
  }
</script>

<div class="nodes-column">
  <div class="column-header">
    <h3>Nodes</h3>
    {#if nodes.length > 0}
      <span class="count">{nodes.length}</span>
    {/if}
  </div>

  <div class="column-content">
    {#if loading}
      <div class="loading">Loading nodes...</div>
    {:else if error}
      <div class="error">
        <p>⚠️ {error}</p>
        <button on:click={refresh}>Retry</button>
      </div>
    {:else if nodes.length === 0}
      <div class="empty">
        <p>No nodes discovered</p>
        <p class="hint">Start network discovery to find nodes</p>
      </div>
    {:else}
      <ul class="items-list">
        {#each nodes as node (node.nodeId)}
          <button
            class="item"
            class:selected={selectedNodeId === node.nodeId}
            class:unavailable={!node.hasCdi}
            on:click={() => handleNodeSelect(node)}
            type="button"
            title={node.hasCdi ? node.nodeName : `${node.nodeName} (CDI unavailable)`}
          >
            <span class="item-name">
              {getNodeDisplayName(node)}
            </span>
            {#if !node.hasCdi}
              <span class="warning-icon" title="CDI not available">⚠️</span>
            {/if}
          </button>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .nodes-column {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 200px;
    max-width: 300px;
    border-right: 1px solid var(--border-color, #ddd);
    background-color: var(--column-bg, #fafafa);
  }

  .column-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color, #ddd);
    background-color: var(--header-bg, #fff);
  }

  .column-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #333);
  }

  .count {
    font-size: 12px;
    color: var(--text-secondary, #666);
    background-color: var(--badge-bg, #e0e0e0);
    padding: 2px 8px;
    border-radius: 10px;
  }

  .column-content {
    flex: 1;
    overflow-y: auto;
  }

  .loading,
  .error,
  .empty {
    padding: 24px 16px;
    text-align: center;
    color: var(--text-secondary, #666);
  }

  .error {
    color: var(--error-color, #d32f2f);
  }

  .error button {
    margin-top: 12px;
    padding: 6px 16px;
    background-color: var(--primary-color, #1976d2);
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .error button:hover {
    background-color: var(--primary-dark, #1565c0);
  }

  .empty .hint {
    font-size: 12px;
    margin-top: 8px;
  }

  .items-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 10px 16px;
    cursor: pointer;
    border: none;
    border-bottom: 1px solid var(--item-border, #efefef);
    background-color: transparent;
    text-align: left;
    transition: background-color 0.15s;
  }

  .item:hover {
    background-color: var(--item-hover, #f0f0f0);
  }

  .item.selected {
    background-color: var(--item-selected, #e3f2fd);
    border-left: 3px solid var(--primary-color, #1976d2);
    padding-left: 13px;
  }

  .item.unavailable {
    color: var(--text-disabled, #999);
    cursor: not-allowed;
  }

  .item.unavailable:hover {
    background-color: var(--item-unavailable-hover, #f9f9f9);
  }

  .item-name {
    flex: 1;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .warning-icon {
    margin-left: 8px;
    font-size: 16px;
  }
</style>
