<script lang="ts">
  import { millerColumnsStore, type Constraint } from '$lib/stores/millerColumns';

  $: elementDetails = $millerColumnsStore.selectedElementDetails;

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
      <div class="empty">
        <p>Select an element to view details</p>
      </div>
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
</style>
