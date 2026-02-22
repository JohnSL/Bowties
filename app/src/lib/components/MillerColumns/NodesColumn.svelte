<script lang="ts">
  import { getDiscoveredNodes, getCdiXml, downloadCdi, type DiscoveredNode } from '$lib/api/cdi';
  import { millerColumnsStore } from '$lib/stores/millerColumns';
  import { getCdiStructure } from '$lib/api/cdi';
  import { nodeInfoStore } from '$lib/stores/nodeInfo';
  import CdiXmlViewer from '$lib/components/CdiXmlViewer.svelte';
  import { getCdiErrorMessage, isCdiError } from '$lib/types/cdi';
  import type { ViewerStatus } from '$lib/types/cdi';

  let nodes: DiscoveredNode[] = [];
  let loading = false;
  let error: string | null = null;
  let selectedNodeId: string | null = null;

  $: if ($millerColumnsStore.selectedNode) {
    selectedNodeId = $millerColumnsStore.selectedNode.nodeId;
  }

  // --- Context menu state ---
  let contextMenuVisible = false;
  let contextMenuX = 0;
  let contextMenuY = 0;
  let contextMenuNode: DiscoveredNode | null = null;

  // --- CDI Viewer state ---
  let viewerVisible = false;
  let viewerNodeId: string | null = null;
  let viewerXmlContent: string | null = null;
  let viewerStatus: ViewerStatus = 'idle';
  let viewerErrorMessage: string | null = null;

  // ----------------------------------------------------------------
  // Exported public methods (called by MillerColumnsNav → +page.svelte)
  // ----------------------------------------------------------------

  /** Refresh nodes from backend state. */
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

  /** Open CDI XML viewer for the currently selected node (called from menu bar). */
  export async function viewCdiXmlForSelectedNode() {
    if (!selectedNodeId) return;
    await openCdiViewer(selectedNodeId, false);
  }

  /** Force-download CDI for the currently selected node (called from menu bar). */
  export async function downloadCdiForSelectedNode() {
    if (!selectedNodeId) return;
    await openCdiViewer(selectedNodeId, true);
  }

  // ----------------------------------------------------------------
  // Node selection
  // ----------------------------------------------------------------

  async function handleNodeSelect(node: DiscoveredNode) {
    if (!node.hasCdi) {
      alert(`Node "${getNodeDisplayName(node)}" does not have CDI data available. Please download CDI first.`);
      return;
    }
    try {
      millerColumnsStore.setLoading(true);
      millerColumnsStore.selectNode(node.nodeId, getNodeDisplayName(node));
      const cdiStructure = await getCdiStructure(node.nodeId);
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

  // ----------------------------------------------------------------
  // Display name & tooltip
  // ----------------------------------------------------------------

  /**
   * Display name priority: user_name > user_description > manufacturer+model > nodeId
   */
  function getNodeDisplayName(node: DiscoveredNode): string {
    const full = $nodeInfoStore.get(node.nodeId);
    if (full?.snip_data) {
      const s = full.snip_data;
      if (s.user_name?.trim()) return s.user_name.trim();
      if (s.user_description?.trim()) return s.user_description.trim();
      const parts = [s.manufacturer?.trim(), s.model?.trim()].filter(Boolean);
      if (parts.length) return parts.join(' ');
    }
    return node.nodeName || node.nodeId;
  }

  /** Build multi-line tooltip string from full SNIP data. */
  function getNodeTooltip(node: DiscoveredNode): string {
    const lines: string[] = [`Node ID: ${node.nodeId}`];
    const full = $nodeInfoStore.get(node.nodeId);
    if (full) {
      lines.push(`Alias: 0x${full.alias.toString(16).toUpperCase().padStart(3, '0')}`);
      const s = full.snip_data;
      if (s) {
        if (s.manufacturer)     lines.push(`Manufacturer: ${s.manufacturer}`);
        if (s.model)            lines.push(`Model: ${s.model}`);
        if (s.hardware_version) lines.push(`Hardware: ${s.hardware_version}`);
        if (s.software_version) lines.push(`Software: ${s.software_version}`);
        if (s.user_name)        lines.push(`User Name: ${s.user_name}`);
        if (s.user_description) lines.push(`Description: ${s.user_description}`);
      }
    }
    return lines.join('\n');
  }

  // ----------------------------------------------------------------
  // Context menu
  // ----------------------------------------------------------------

  function handleContextMenu(event: MouseEvent, node: DiscoveredNode) {
    event.preventDefault();
    contextMenuNode = node;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    contextMenuVisible = true;
  }

  function closeContextMenu() {
    contextMenuVisible = false;
    contextMenuNode = null;
  }

  function handleWindowClick(event: MouseEvent) {
    if (contextMenuVisible && !(event.target as Element)?.closest('.nodes-context-menu')) {
      closeContextMenu();
    }
  }

  function handleViewCdiXml() {
    if (!contextMenuNode) return;
    const nodeId = contextMenuNode.nodeId;
    closeContextMenu();
    openCdiViewer(nodeId, false);
  }

  function handleDownloadCdi() {
    if (!contextMenuNode) return;
    const nodeId = contextMenuNode.nodeId;
    closeContextMenu();
    openCdiViewer(nodeId, true);
  }

  // ----------------------------------------------------------------
  // CDI Viewer
  // ----------------------------------------------------------------

  async function openCdiViewer(nodeId: string, forceDownload: boolean) {
    viewerVisible = true;
    viewerNodeId = nodeId;
    viewerXmlContent = null;
    viewerStatus = 'loading';
    viewerErrorMessage = forceDownload ? 'Downloading CDI from node...' : 'Checking cache...';

    try {
      let response;
      if (forceDownload) {
        response = await downloadCdi(nodeId);
      } else {
        try {
          response = await getCdiXml(nodeId);
        } catch (cacheError: any) {
          if (isCdiError(cacheError, 'CdiNotRetrieved')) {
            viewerErrorMessage = 'Downloading CDI from node...';
            await new Promise(resolve => setTimeout(resolve, 0));
            response = await downloadCdi(nodeId);
          } else {
            throw cacheError;
          }
        }
      }

      if (response.xmlContent) {
        viewerXmlContent = response.xmlContent;
        viewerStatus = 'success';
        viewerErrorMessage = null;
      } else {
        viewerStatus = 'error';
        viewerErrorMessage = 'No CDI data available for this node.';
      }
    } catch (err) {
      viewerStatus = 'error';
      viewerErrorMessage = getCdiErrorMessage(err);
    }
  }

  function closeCdiViewer() {
    viewerVisible = false;
    viewerNodeId = null;
    viewerXmlContent = null;
    viewerStatus = 'idle';
    viewerErrorMessage = null;
  }
</script>

<svelte:window onclick={handleWindowClick} />

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
        <button onclick={refresh}>Retry</button>
      </div>
    {:else if nodes.length === 0}
      <div class="empty">
        <p>No nodes discovered</p>
        <p class="hint">Use Discover Nodes to scan the network</p>
      </div>
    {:else}
      <ul class="items-list">
        {#each nodes as node (node.nodeId)}
          <button
            class="item"
            class:selected={selectedNodeId === node.nodeId}
            class:unavailable={!node.hasCdi}
            onclick={() => handleNodeSelect(node)}
            oncontextmenu={(e) => handleContextMenu(e, node)}
            type="button"
            title={getNodeTooltip(node)}
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

<!-- Context Menu -->
{#if contextMenuVisible && contextMenuNode}
  <div
    class="nodes-context-menu"
    style="left: {contextMenuX}px; top: {contextMenuY}px;"
    role="menu"
  >
    <button class="ctx-item" onclick={handleViewCdiXml} role="menuitem">
      📄 View CDI XML
    </button>
    <button class="ctx-item" onclick={handleDownloadCdi} role="menuitem">
      🔄 Download CDI from Node
    </button>
  </div>
{/if}

<!-- CDI XML Viewer Modal -->
<CdiXmlViewer
  visible={viewerVisible}
  nodeId={viewerNodeId}
  xmlContent={viewerXmlContent}
  status={viewerStatus}
  errorMessage={viewerErrorMessage}
  onClose={closeCdiViewer}
/>

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

  /* Context menu */
  .nodes-context-menu {
    position: fixed;
    z-index: 1000;
    background: #fff;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.15);
    min-width: 200px;
    padding: 4px 0;
  }

  .ctx-item {
    display: block;
    width: 100%;
    padding: 8px 16px;
    background: none;
    border: none;
    text-align: left;
    font-size: 13px;
    cursor: pointer;
    color: #333;
  }

  .ctx-item:hover {
    background-color: #f0f4ff;
  }
</style>
