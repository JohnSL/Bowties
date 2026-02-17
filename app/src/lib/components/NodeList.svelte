<script lang="ts">
  import type { DiscoveredNode } from '$lib/api/tauri';
  import NodeStatus from './NodeStatus.svelte';

  interface Props {
    nodes: DiscoveredNode[];
    isRefreshing?: boolean;
  }

  let { nodes, isRefreshing = false }: Props = $props();

  /**
   * Format Node ID as hex string with dots
   */
  function formatNodeId(nodeId: number[]): string {
    return nodeId
      .map(byte => byte.toString(16).padStart(2, '0').toUpperCase())
      .join('.');
  }

  /**
   * Format alias as 3-digit hex
   */
  function formatAlias(alias: number): string {
    return alias.toString(16).padStart(3, '0').toUpperCase();
  }

  /**
   * Get friendly name for display based on priority:
   * 1. user_name (if present)
   * 2. manufacturer + model (if available)
   * 3. Node ID (fallback)
   */
  function getFriendlyName(node: DiscoveredNode): string {
    // Priority 1: User-assigned name
    if (node.snip_data?.user_name && node.snip_data.user_name.trim()) {
      return node.snip_data.user_name;
    }

    // Priority 2: Manufacturer + Model
    if (node.snip_data?.manufacturer || node.snip_data?.model) {
      const manufacturer = node.snip_data.manufacturer || '';
      const model = node.snip_data.model || '';
      const combined = `${manufacturer} ${model}`.trim();
      if (combined) {
        return combined;
      }
    }

    // Priority 3: Node ID
    return formatNodeId(node.node_id);
  }

  /**
   * Get secondary info for display (shown below friendly name)
   */
  function getSecondaryInfo(node: DiscoveredNode): string {
    const parts: string[] = [];

    // If friendly name is user_name, show manufacturer+model
    if (node.snip_data?.user_name && node.snip_data.user_name.trim()) {
      const manufacturer = node.snip_data.manufacturer || '';
      const model = node.snip_data.model || '';
      const combined = `${manufacturer} ${model}`.trim();
      if (combined) {
        parts.push(combined);
      }
    }

    // Add software version if available
    if (node.snip_data?.software_version) {
      parts.push(`v${node.snip_data.software_version}`);
    }

    return parts.join(' • ');
  }

  /**
   * Get tooltip text with full details
   */
  function getTooltip(node: DiscoveredNode): string {
    const lines: string[] = [];

    lines.push(`Node ID: ${formatNodeId(node.node_id)}`);
    lines.push(`Alias: 0x${formatAlias(node.alias)}`);

    if (node.snip_data) {
      if (node.snip_data.manufacturer) {
        lines.push(`Manufacturer: ${node.snip_data.manufacturer}`);
      }
      if (node.snip_data.model) {
        lines.push(`Model: ${node.snip_data.model}`);
      }
      if (node.snip_data.software_version) {
        lines.push(`Software: ${node.snip_data.software_version}`);
      }
      if (node.snip_data.hardware_version) {
        lines.push(`Hardware: ${node.snip_data.hardware_version}`);
      }
      if (node.snip_data.user_description) {
        lines.push(`Description: ${node.snip_data.user_description}`);
      }
    } else if (node.snip_status === 'NotSupported') {
      lines.push('SNIP not supported');
    }

    return lines.join('\n');
  }

  /**
   * Truncate long text with ellipsis
   */
  function truncate(text: string, maxLength: number): string {
    if (text.length <= maxLength) {
      return text;
    }
    return text.substring(0, maxLength - 1) + '…';
  }

  /**
   * Check for duplicate names and add disambiguation
   */
  function getDisplayName(node: DiscoveredNode, index: number): string {
    const baseName = getFriendlyName(node);
    
    // Count how many nodes have the same friendly name
    const duplicates = nodes.filter(n => getFriendlyName(n) === baseName);
    
    if (duplicates.length > 1) {
      // Append partial Node ID to disambiguate
      const nodeIdSuffix = node.node_id
        .slice(-2)
        .map(b => b.toString(16).padStart(2, '0').toUpperCase())
        .join('');
      return `${baseName} (${nodeIdSuffix})`;
    }
    
    return baseName;
  }

  /**
   * Format timestamp as relative time
   */
  function formatLastSeen(timestamp: string): string {
    try {
      const date = new Date(timestamp);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffSec = Math.floor(diffMs / 1000);
      
      if (diffSec < 60) {
        return `${diffSec} seconds ago`;
      } else if (diffSec < 3600) {
        return `${Math.floor(diffSec / 60)} minutes ago`;
      } else if (diffSec < 86400) {
        return `${Math.floor(diffSec / 3600)} hours ago`;
      } else {
        return `${Math.floor(diffSec / 86400)} days ago`;
      }
    } catch {
      return timestamp;
    }
  }
</script>

<div class="node-list">
  <!-- Refreshing Overlay -->
  {#if isRefreshing}
    <div class="refreshing-overlay">
      <div class="refreshing-spinner"></div>
      <p class="refreshing-text">Refreshing node status...</p>
    </div>
  {/if}

  {#if nodes.length === 0}
    <div class="empty-state">
      <p class="text-gray-500 dark:text-gray-400 text-center py-8">
        No nodes discovered. Click "Discover Nodes" to scan the network.
      </p>
    </div>
  {:else}
    <div class="table-container">
      <table class="nodes-table">
        <thead>
          <tr>
            <th class="status-col">Status</th>
            <th class="manufacturer-col">Manufacturer</th>
            <th class="model-col">Model</th>
            <th class="hw-col">Hardware</th>
            <th class="sw-col">Software</th>
            <th class="user-name-col">User Name</th>
            <th class="user-desc-col">Description</th>
          </tr>
        </thead>
        <tbody>
          {#each nodes as node, index (formatNodeId(node.node_id))}
            <tr 
              class="node-row"
              title={getTooltip(node)}
              role="row"
            >
              <td class="status-cell">
                <NodeStatus 
                  connectionStatus={node.connection_status} 
                  snipStatus={node.snip_status} 
                />
              </td>
              <td class="text-cell">
                {node.snip_data?.manufacturer || '—'}
              </td>
              <td class="text-cell">
                {node.snip_data?.model || '—'}
              </td>
              <td class="text-cell version-cell">
                {node.snip_data?.hardware_version || '—'}
              </td>
              <td class="text-cell version-cell">
                {node.snip_data?.software_version || '—'}
              </td>
              <td class="text-cell user-name-cell">
                {node.snip_data?.user_name || '—'}
              </td>
              <td class="text-cell description-cell">
                {node.snip_data?.user_description || '—'}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .node-list {
    width: 100%;
    position: relative;
  }

  .table-container {
    overflow-x: auto;
    border-radius: 8px;
    border: 1px solid #e5e7eb;
  }

  :global(.dark) .table-container {
    border-color: #374151;
  }

  .nodes-table {
    width: 100%;
    border-collapse: collapse;
    background-color: white;
    font-size: 0.875rem;
  }

  :global(.dark) .nodes-table {
    background-color: #1f2937;
  }

  .nodes-table thead {
    background-color: #f9fafb;
    border-bottom: 2px solid #e5e7eb;
  }

  :global(.dark) .nodes-table thead {
    background-color: #111827;
    border-bottom-color: #374151;
  }

  .nodes-table th {
    padding: 0.75rem 1rem;
    text-align: left;
    font-weight: 600;
    color: #374151;
    white-space: nowrap;
  }

  :global(.dark) .nodes-table th {
    color: #d1d5db;
  }

  .nodes-table tbody tr {
    border-bottom: 1px solid #f3f4f6;
    transition: background-color 0.15s ease;
  }

  :global(.dark) .nodes-table tbody tr {
    border-bottom-color: #374151;
  }

  .node-row {
    cursor: pointer;
  }

  .node-row:hover {
    background-color: #f9fafb;
  }

  :global(.dark) .node-row:hover {
    background-color: #374151;
  }

  .nodes-table td {
    padding: 0.75rem 1rem;
    vertical-align: middle;
  }

  .status-col {
    width: 80px;
  }

  .status-cell {
    text-align: center;
  }

  .manufacturer-col {
    min-width: 150px;
    max-width: 200px;
  }

  .model-col {
    min-width: 150px;
    max-width: 200px;
  }

  .hw-col, .sw-col {
    width: 100px;
  }

  .user-name-col {
    min-width: 120px;
    max-width: 180px;
  }

  .user-desc-col {
    min-width: 200px;
    max-width: 300px;
  }

  .text-cell {
    color: #374151;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.dark) .text-cell {
    color: #d1d5db;
  }

  .version-cell {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.8125rem;
    color: #6b7280;
  }

  :global(.dark) .version-cell {
    color: #9ca3af;
  }

  .user-name-cell {
    font-weight: 500;
    color: #111827;
  }

  :global(.dark) .user-name-cell {
    color: #f3f4f6;
  }

  .description-cell {
    color: #6b7280;
    font-style: italic;
  }

  :global(.dark) .description-cell {
    color: #9ca3af;
  }

  .refreshing-overlay {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(255, 255, 255, 0.85);
    backdrop-filter: blur(2px);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    z-index: 10;
    border-radius: 8px;
  }

  :global(.dark) .refreshing-overlay {
    background-color: rgba(0, 0, 0, 0.85);
  }

  .refreshing-spinner {
    width: 40px;
    height: 40px;
    border: 4px solid rgba(0, 0, 0, 0.1);
    border-left-color: #0284c7;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  :global(.dark) .refreshing-spinner {
    border-color: rgba(255, 255, 255, 0.1);
    border-left-color: #38bdf8;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .refreshing-text {
    font-size: 0.875rem;
    font-weight: 500;
    color: #0284c7;
  }

  :global(.dark) .refreshing-text {
    color: #38bdf8;
  }
</style>
