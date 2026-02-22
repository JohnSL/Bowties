<script lang="ts">
  import { millerColumnsStore, type Constraint } from '$lib/stores/millerColumns';
  import { readConfigValue } from '$lib/api/cdi';
  import { formatConfigValue, getValueTypeLabel } from '$lib/utils/formatters';
  import type { ConfigValueWithMetadata } from '$lib/api/types';
  import { nodeInfoStore } from '$lib/stores/nodeInfo';

  $: elementDetails = $millerColumnsStore.selectedElementDetails;
  $: selectedNode = $millerColumnsStore.selectedNode;

  // T043-T044: Config value state
  let currentValue: ConfigValueWithMetadata | null = null;
  let valueError: string | null = null;

  // T071, T073: Refresh state
  let isRefreshing = false;
  let isStale = false;

  // Track the last element key we loaded so the reactive only fires when the
  // element actually changes, not on every unrelated store update (e.g. when
  // setConfigValue updates configValues after a refresh).
  let currentElementKey: string | null = null;

  // T043: Load config value when element selected.
  // Only re-runs loadConfigValue when the selected element path actually changes.
  $: {
    const nextKey = (elementDetails && selectedNode)
      ? `${selectedNode.nodeId}:${elementDetails.elementPath.join('/')}`
      : null;
    if (nextKey !== currentElementKey) {
      currentElementKey = nextKey;
      if (nextKey && elementDetails && selectedNode) {
        loadConfigValue();
      } else {
        currentValue = null;
        valueError = null;
        isStale = false;
      }
    }
  }

  // Read the cached value for the currently selected element.
  // Values are pre-populated by read_all_config_values during discovery; we do
  // NOT issue individual node reads here.  Use the Refresh button to explicitly
  // re-read a single value from the node.
  function loadConfigValue() {
    if (!elementDetails || !selectedNode) return;

    // Check if elementPath exists
    if (!elementDetails.elementPath || !Array.isArray(elementDetails.elementPath)) {
      console.warn('Element details missing elementPath:', elementDetails);
      currentValue = null;
      valueError = 'Invalid element path';
      return;
    }

    // Only show values for readable element types (skip Action and Blob)
    const dataTypeLower = elementDetails.dataType.toLowerCase();
    const isReadable = dataTypeLower.startsWith('int') ||
                      dataTypeLower.startsWith('string') ||
                      dataTypeLower.startsWith('eventid') ||
                      dataTypeLower.startsWith('float');

    if (!isReadable) {
      currentValue = null;
      valueError = null;
      return;
    }

    isStale = false;

    // Read from cache only (T041).  Values must have been pre-loaded by
    // read_all_config_values; if the entry is absent the user can refresh.
    const cacheKey = `${selectedNode.nodeId}:${elementDetails.elementPath.join('/')}`;
    const cached = $millerColumnsStore.configValues.get(cacheKey);

    if (cached) {
      currentValue = cached;
      valueError = null;
    } else {
      currentValue = null;
      valueError = null;
    }
  }

  // T070: Handle refresh value button click
  async function handleRefreshValue() {
    if (!elementDetails || !selectedNode || isRefreshing) return;

    // Capture path at the start so that if the user navigates away during the
    // async read the result is still cached under the correct key.
    const nodeId = selectedNode.nodeId;
    const elementPath = elementDetails.elementPath;

    isRefreshing = true;
    valueError = null;

    try {
      const value = await readConfigValue(nodeId, elementPath);
      currentValue = value;
      isStale = false; // Clear staleness on successful refresh

      // Update cache (T040)
      millerColumnsStore.setConfigValue(nodeId, elementPath, value);
    } catch (error) {
      // T072: On failure, keep stale value but mark as stale
      valueError = String(error);
      if (currentValue) {
        isStale = true; // T073: Mark as stale
      }
    } finally {
      isRefreshing = false;
    }
  }

  /**
   * Format constraint for display
   */
  function formatConstraint(constraint: Constraint): string {
    switch (constraint.type) {
      case 'range':
        return `Range: ${constraint.value.min ?? '−∞'}-${constraint.value.max ?? '∞'}`;
      
      case 'map':
        if (constraint.value.entries) {
          return constraint.value.entries
            .map(e => `${e.value} = ${e.label}`)
            .join(', ');
        }
        return 'Value mapping';
      
      case 'length':
        return `Max length: ${constraint.value.maxLength} bytes`;
      
      default:
        return constraint.description;
    }
  }

  /**
   * Format memory address
   */
  function formatMemoryAddress(address: number): string {
    return `0x${address.toString(16).toUpperCase().padStart(4, '0')}`;
  }
</script>

<div class="details-panel">
  <div class="panel-header">
    <h3>Details</h3>
  </div>

  <div class="panel-content">
    {#if $millerColumnsStore.selectedNode && !$millerColumnsStore.selectedNode.nodeId}
      <!-- T097: No CDI data available message -->
      <div class="no-cdi-message">
        <div class="icon">📭</div>
        <h4>No CDI Data Available</h4>
        <p>This node does not provide Configuration Description Information (CDI).</p>
      </div>
    {:else if !elementDetails}
      {#if $millerColumnsStore.selectedNode}
        {@const nodeId = $millerColumnsStore.selectedNode.nodeId}
        {@const full = $nodeInfoStore.get(nodeId)}
        <div class="node-info">
          <div class="node-info-header">
            <h4>Node Details</h4>
          </div>
          <div class="node-info-row">
            <span class="ni-label">Node ID</span>
            <span class="ni-value ni-mono">{nodeId}</span>
          </div>
          {#if full}
            <div class="node-info-row">
              <span class="ni-label">Alias</span>
              <span class="ni-value ni-mono">0x{full.alias.toString(16).toUpperCase().padStart(3, '0')}</span>
            </div>
            {#if full.snip_data}
              {@const s = full.snip_data}
              {#if s.manufacturer}
                <div class="node-info-row">
                  <span class="ni-label">Manufacturer</span>
                  <span class="ni-value">{s.manufacturer}</span>
                </div>
              {/if}
              {#if s.model}
                <div class="node-info-row">
                  <span class="ni-label">Model</span>
                  <span class="ni-value">{s.model}</span>
                </div>
              {/if}
              {#if s.hardware_version}
                <div class="node-info-row">
                  <span class="ni-label">Hardware Ver.</span>
                  <span class="ni-value">{s.hardware_version}</span>
                </div>
              {/if}
              {#if s.software_version}
                <div class="node-info-row">
                  <span class="ni-label">Software Ver.</span>
                  <span class="ni-value">{s.software_version}</span>
                </div>
              {/if}
              {#if s.user_name}
                <div class="node-info-row">
                  <span class="ni-label">User Name</span>
                  <span class="ni-value">{s.user_name}</span>
                </div>
              {/if}
              {#if s.user_description}
                <div class="node-info-row">
                  <span class="ni-label">Description</span>
                  <span class="ni-value">{s.user_description}</span>
                </div>
              {/if}
            {/if}
          {/if}
          <p class="ni-hint">Select a segment to begin configuring this node</p>
        </div>
      {:else}
        <div class="empty">
          <p>Select a node to get started</p>
        </div>
      {/if}
    {:else}
      <div class="details">
        <!-- Element Name -->
        <div class="detail-section">
          <h4 class="section-title">Name</h4>
          <p class="element-name">{elementDetails.name}</p>
        </div>

        <!-- Data Type -->
        <div class="detail-section">
          <h4 class="section-title">Data Type</h4>
          <p class="data-type">{elementDetails.dataType}</p>
        </div>

        <!-- Description -->
        {#if elementDetails.description}
          <div class="detail-section">
            <h4 class="section-title">Description</h4>
            <p class="description">{elementDetails.description}</p>
          </div>
        {/if}

        <!-- Current Value (T043: Display config value) -->
        <div class="detail-section">
          <div class="section-header">
            <h4 class="section-title">Current Value</h4>
            <!-- T069: Refresh Value button - always visible for readable elements -->
            {#if elementDetails.dataType.toLowerCase().startsWith('int') ||
                 elementDetails.dataType.toLowerCase().startsWith('string') ||
                 elementDetails.dataType.toLowerCase().startsWith('eventid') ||
                 elementDetails.dataType.toLowerCase().startsWith('float')}
              <button
                class="refresh-btn"
                onclick={handleRefreshValue}
                disabled={isRefreshing}
                title="Refresh value from node"
              >
                {isRefreshing ? '⏳' : '🔄'}
              </button>
            {/if}
          </div>
          {#if valueError}
            <div class="value-error">
              <span class="error-icon">⚠️</span>
              <span>{valueError}</span>
            </div>
          {:else if currentValue}
            <div class="current-value">
              <!-- T073: Staleness indicator -->
              {#if isStale}
                <div class="stale-indicator">
                  <span class="stale-icon">⚠️</span>
                  <span class="stale-text">Stale - refresh failed</span>
                </div>
              {/if}
              <p class="value-display">{formatConfigValue(currentValue.value)}</p>
              <p class="value-type">{getValueTypeLabel(currentValue.value)}</p>
            </div>
          {:else if !(elementDetails.dataType.toLowerCase().startsWith('int') ||
                      elementDetails.dataType.toLowerCase().startsWith('string') ||
                      elementDetails.dataType.toLowerCase().startsWith('eventid') ||
                      elementDetails.dataType.toLowerCase().startsWith('float'))}
            <p class="value-placeholder">This element type does not store readable values</p>
          {:else}
            <p class="value-placeholder">Value not yet loaded — click 🔄 to read from node</p>
          {/if}
        </div>

        <!-- Default Value -->
        {#if elementDetails.defaultValue}
          <div class="detail-section">
            <h4 class="section-title">Default Value</h4>
            <p class="default-value">{elementDetails.defaultValue}</p>
          </div>
        {/if}

        <!-- Constraints -->
        {#if elementDetails.constraints.length > 0}
          <div class="detail-section">
            <h4 class="section-title">Constraints</h4>
            <ul class="constraints-list">
              {#each elementDetails.constraints as constraint}
                <li class="constraint-item">
                  {formatConstraint(constraint)}
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        <!-- Memory Address -->
        <div class="detail-section">
          <h4 class="section-title">Memory Address</h4>
          <p class="memory-address">{formatMemoryAddress(elementDetails.memoryAddress)}</p>
        </div>

        <!-- Full Path Breadcrumb -->
        <div class="detail-section">
          <h4 class="section-title">Full Path</h4>
          <p class="full-path">{elementDetails.fullPath}</p>
        </div>

        <!-- Read-only Note -->
        <div class="detail-section note">
          <p class="note-text">
            ℹ️ Structure metadata only - values not retrieved
          </p>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .details-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 300px;
    max-width: 400px;
    border-left: 1px solid var(--border-color, #ddd);
    background-color: var(--panel-bg, #fff);
  }

  .panel-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color, #ddd);
    background-color: var(--header-bg, #fff);
  }

  .panel-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #333);
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
  }

  .empty {
    text-align: center;
    color: var(--text-secondary, #666);
    padding: 24px 0;
  }

  .details {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .detail-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .detail-section.note {
    margin-top: 8px;
    padding: 12px;
    background-color: var(--note-bg, #f5f5f5);
    border-radius: 4px;
  }

  .section-title {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-secondary, #666);
    letter-spacing: 0.5px;
  }

  .element-name {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary, #333);
  }

  .data-type {
    margin: 0;
    font-size: 14px;
    color: var(--text-primary, #333);
    font-family: 'Consolas', 'Monaco', monospace;
    background-color: var(--code-bg, #f0f0f0);
    padding: 6px 10px;
    border-radius: 4px;
  }

  .description {
    margin: 0;
    font-size: 13px;
    color: var(--text-primary, #333);
    line-height: 1.5;
  }

  .default-value {
    margin: 0;
    font-size: 14px;
    color: var(--text-primary, #333);
    font-family: 'Consolas', 'Monaco', monospace;
    background-color: var(--code-bg, #f0f0f0);
    padding: 6px 10px;
    border-radius: 4px;
  }

  .constraints-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .constraint-item {
    font-size: 13px;
    color: var(--text-primary, #333);
    padding: 8px 12px;
    background-color: var(--constraint-bg, #f9f9f9);
    border-left: 3px solid var(--primary-color, #1976d2);
    border-radius: 2px;
  }

  .memory-address {
    margin: 0;
    font-size: 14px;
    color: var(--text-primary, #333);
    font-family: 'Consolas', 'Monaco', monospace;
    background-color: var(--code-bg, #f0f0f0);
    padding: 6px 10px;
    border-radius: 4px;
  }

  .full-path {
    margin: 0;
    font-size: 12px;
    color: var(--text-secondary, #666);
    line-height: 1.6;
    word-break: break-word;
  }

  .note-text {
    margin: 0;
    font-size: 12px;
    color: var(--text-secondary, #666);
    font-style: italic;
  }

  /* T097: No CDI message styling */
  .no-cdi-message {
    text-align: center;
    padding: 32px 16px;
  }

  .no-cdi-message .icon {
    font-size: 48px;
    margin-bottom: 16px;
  }

  .no-cdi-message h4 {
    margin: 0 0 8px 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary, #333);
  }

  .no-cdi-message p {
    margin: 0;
    font-size: 13px;
    color: var(--text-secondary, #666);
    line-height: 1.5;
  }

  /* T043-T044: Current value styling */
  .value-loading,
  .value-error,
  .value-placeholder {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-radius: 4px;
    font-size: 13px;
  }

  .value-loading {
    background-color: var(--info-bg, #e3f2fd);
    color: var(--info-text, #1976d2);
  }

  .value-error {
    background-color: var(--error-bg, #ffebee);
    color: var(--error-text, #c62828);
  }

  .value-placeholder {
    color: var(--text-secondary, #666);
    font-style: italic;
  }

  .loading-spinner {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .current-value {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .value-display {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--primary-color, #1976d2);
    font-family: 'Consolas', 'Monaco', monospace;
    background-color: var(--value-bg, #f0f8ff);
    padding: 10px 12px;
    border-radius: 4px;
    border: 1px solid var(--primary-color-light, #bbdefb);
    word-break: break-all;
  }

  .value-type {
    margin: 0;
    font-size: 11px;
    color: var(--text-secondary, #666);
  }

  /* T069: Refresh button styles */
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .refresh-btn {
    background: none;
    border: 1px solid #ddd;
    border-radius: 4px;
    padding: 4px 8px;
    cursor: pointer;
    font-size: 14px;
    transition: all 0.2s;
  }

  .refresh-btn:hover:not(:disabled) {
    background: #f0f0f0;
    border-color: #667eea;
  }

  .refresh-btn:active:not(:disabled) {
    transform: scale(0.95);
  }

  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* T073: Staleness indicator styles */
  .stale-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: #fff3cd;
    border: 1px solid #ffc107;
    border-radius: 4px;
    margin-bottom: 8px;
  }

  .stale-icon {
    font-size: 14px;
  }

  .stale-text {
    font-size: 12px;
    color: #856404;
    font-weight: 500;
  }

  /* Node info view (shown when node selected, no element chosen yet) */
  .node-info {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .node-info-header h4 {
    margin: 0 0 12px 0;
    font-size: 13px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-secondary, #666);
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-color, #ddd);
  }

  .node-info-row {
    display: flex;
    flex-direction: column;
    padding: 8px 0;
    border-bottom: 1px solid var(--item-border, #f0f0f0);
  }

  .ni-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-secondary, #888);
    margin-bottom: 2px;
  }

  .ni-value {
    font-size: 13px;
    color: var(--text-primary, #333);
    word-break: break-all;
  }

  .ni-mono {
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 12px;
  }

  .ni-hint {
    margin: 16px 0 0 0;
    font-size: 12px;
    color: var(--text-secondary, #888);
    font-style: italic;
    text-align: center;
  }
</style>