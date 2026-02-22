<script lang="ts">
  import { millerColumnsStore } from '$lib/stores/millerColumns';
  import NodesColumn from './NodesColumn.svelte';
  import NavigationColumn from './NavigationColumn.svelte';
  import DetailsPanel from './DetailsPanel.svelte';
  import { onMount } from 'svelte';

  // Subscribe to store
  $: columns = $millerColumnsStore.columns;
  $: isLoading = $millerColumnsStore.isLoading;
  $: error = $millerColumnsStore.error;

  // T100: Error boundary for CDI parsing errors
  let hasRenderError = false;
  let renderErrorMessage = '';

  // T109: Horizontal scroll indicators
  let columnsContainer: HTMLDivElement;
  let showLeftScroll = false;
  let showRightScroll = false;

  // Reference to NodesColumn for triggering refresh
  let nodesColumn: NodesColumn;

  /**
   * T109: Update scroll indicators based on scroll position
   */
  function updateScrollIndicators() {
    if (!columnsContainer) return;
    
    const { scrollLeft, scrollWidth, clientWidth } = columnsContainer;
    showLeftScroll = scrollLeft > 0;
    showRightScroll = scrollLeft + clientWidth < scrollWidth - 1;
  }

  /**
   * Get selected item ID for a column
   */
  function getSelectedItemId(columnDepth: number): string | null {
    const breadcrumb = $millerColumnsStore.breadcrumb;
    const step = breadcrumb.find(s => s.depth === columnDepth);
    return step ? step.itemId : null;
  }

  /**
   * T100: Global error handler for parsing errors
   */
  onMount(() => {
    const errorHandler = (event: ErrorEvent) => {
      const message = event.message || String(event.error);
      if (message.includes('CDI') || message.includes('parse') || message.includes('XML')) {
        hasRenderError = true;
        renderErrorMessage = message;
        event.preventDefault();
      }
    };

    window.addEventListener('error', errorHandler);
    
    // T109: Set up scroll listener
    if (columnsContainer) {
      columnsContainer.addEventListener('scroll', updateScrollIndicators);
      // Initial check
      updateScrollIndicators();
    }
    
    return () => {
      window.removeEventListener('error', errorHandler);
      if (columnsContainer) {
        columnsContainer.removeEventListener('scroll', updateScrollIndicators);
      }
    };
  });

  // T109: Update scroll indicators when columns change
  $: if (columns && columnsContainer) {
    setTimeout(() => updateScrollIndicators(), 200);
  }

  /**
   * T100: Clear render error and reset
   */
  function clearRenderError() {
    hasRenderError = false;
    renderErrorMessage = '';
    millerColumnsStore.reset();
  }

  /**
   * Refresh nodes column data
   * Called by parent when node discovery/refresh completes
   */
  export async function refreshNodes() {
    if (nodesColumn) {
      await nodesColumn.refresh();
    }
  }

  /**
   * Open CDI XML viewer for the currently selected node.
   * Called from the app menu bar "Tools" item.
   */
  export async function viewCdiXmlForSelectedNode() {
    if (nodesColumn) await nodesColumn.viewCdiXmlForSelectedNode();
  }

  /**
   * Force-download CDI for the currently selected node.
   * Called from the app menu bar "Tools" item.
   */
  export async function downloadCdiForSelectedNode() {
    if (nodesColumn) await nodesColumn.downloadCdiForSelectedNode();
  }
</script>

<div class="miller-columns-nav" role="main" aria-label="Miller Columns CDI Navigator">
  {#if hasRenderError}
    <!-- T100: Error boundary for parsing errors -->
    <div class="fatal-error" role="alert">
      <div class="fatal-error-content">
        <div class="fatal-error-icon">🚫</div>
        <h2>Error Loading CDI Data</h2>
        <p class="error-detail">{renderErrorMessage}</p>
        <p class="error-help">
          This may be caused by malformed CDI XML or parsing issues. 
          Please verify the CDI data is valid.
        </p>
        <button class="reset-button" on:click={clearRenderError}>
          Reset and Try Again
        </button>
      </div>
    </div>
  {:else}
    {#if error}
      <div class="error-banner" role="alert">
        <span class="error-icon">⚠️</span>
        <span class="error-message">{error}</span>
        <button
          class="error-dismiss"
          on:click={() => millerColumnsStore.setError(null)}
          aria-label="Dismiss error"
        >
          ✕
        </button>
      </div>
    {/if}

    <div 
      class="columns-container" 
      bind:this={columnsContainer}
      role="region"
      aria-label="Configuration navigation columns"
    >
      <!-- T109: Left scroll indicator -->
      {#if showLeftScroll}
        <div class="scroll-indicator left" aria-hidden="true">
          ‹
        </div>
      {/if}
    <!-- Nodes Column (leftmost, always visible) -->
    <NodesColumn bind:this={nodesColumn} />

    <!-- T094: Dynamic Columns with removal animation (<150ms) -->
    {#each columns as column (column.depth)}
      <div class="column-wrapper">
        <NavigationColumn {column} selectedItemId={getSelectedItemId(column.depth)} />
      </div>
    {/each}

    <!-- Details Panel (rightmost, always visible) -->
    <DetailsPanel />

      <!-- T109: Right scroll indicator -->
      {#if showRightScroll}
        <div class="scroll-indicator right" aria-hidden="true">
          ›
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .miller-columns-nav {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background-color: var(--bg-color, #fff);
  }

  .error-banner {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background-color: var(--error-bg, #ffebee);
    border-bottom: 1px solid var(--error-border, #f44336);
    color: var(--error-color, #d32f2f);
  }

  .error-icon {
    font-size: 18px;
  }

  .error-message {
    flex: 1;
    font-size: 14px;
  }

  .error-dismiss {
    background: none;
    border: none;
    font-size: 20px;
    color: var(--error-color, #d32f2f);
    cursor: pointer;
    padding: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: background-color 0.15s;
  }

  .error-dismiss:hover {
    background-color: var(--error-dismiss-hover, rgba(0, 0, 0, 0.1));
  }

  .columns-container {
    display: flex;
    flex-direction: row;
    flex: 1;
    overflow-x: auto;
    overflow-y: hidden;
    position: relative;
    min-height: 0; /* Allow flex children to shrink */
  }

  /* T094: Column wrapper for smooth animations */
  .column-wrapper {
    display: flex;
    height: 100%;
  }

  /* Scrollbar styling */
  .columns-container::-webkit-scrollbar {
    height: 8px;
  }

  .columns-container::-webkit-scrollbar-track {
    background: var(--scrollbar-track, #f1f1f1);
  }

  .columns-container::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb, #888);
    border-radius: 4px;
  }

  .columns-container::-webkit-scrollbar-thumb:hover {
    background: var(--scrollbar-thumb-hover, #555);
  }

  /* T100: Fatal error boundary styling */
  .fatal-error {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    width: 100%;
    padding: 32px;
    background-color: var(--bg-color, #fff);
  }

  .fatal-error-content {
    text-align: center;
    max-width: 500px;
  }

  .fatal-error-icon {
    font-size: 64px;
    margin-bottom: 24px;
  }

  .fatal-error-content h2 {
    margin: 0 0 16px 0;
    font-size: 24px;
    font-weight: 600;
    color: var(--error-color, #d32f2f);
  }

  .error-detail {
    margin: 0 0 16px 0;
    padding: 16px;
    background-color: var(--error-bg-light, #ffebee);
    border-radius: 4px;
    font-size: 13px;
    color: var(--text-primary, #333);
    font-family: 'Consolas', 'Monaco', monospace;
    word-break: break-word;
  }

  .error-help {
    margin: 0 0 24px 0;
    font-size: 14px;
    color: var(--text-secondary, #666);
    line-height: 1.5;
  }

  .reset-button {
    padding: 12px 24px;
    font-size: 14px;
    font-weight: 600;
    color: #fff;
    background-color: var(--primary-color, #1976d2);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .reset-button:hover {
    background-color: var(--primary-color-dark, #1565c0);
  }

  .reset-button:active {
    background-color: var(--primary-color-darker, #0d47a1);
  }

  /* T109: Scroll indicators */
  .scroll-indicator {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 40px;
    height: 100px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 32px;
    color: var(--primary-color, #1976d2);
    background: linear-gradient(to right, 
      rgba(255, 255, 255, 0.95) 0%, 
      rgba(255, 255, 255, 0) 100%);
    pointer-events: none;
    z-index: 5;
  }

  .scroll-indicator.left {
    left: 0;
  }

  .scroll-indicator.right {
    right: 0;
    background: linear-gradient(to left, 
      rgba(255, 255, 255, 0.95) 0%, 
      rgba(255, 255, 255, 0) 100%);
  }
</style>
