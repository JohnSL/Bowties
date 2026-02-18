<script lang="ts">
  import { millerColumnsStore, type NavigationStep } from '$lib/stores/millerColumns';

  // Subscribe to breadcrumb from store
  $: breadcrumb = $millerColumnsStore.breadcrumb;

  /**
   * Handle clicking a breadcrumb segment to navigate back
   * T091: Call removeColumnsAfter to trim columns
   */
  function handleSegmentClick(step: NavigationStep) {
    // Remove all columns after this depth
    millerColumnsStore.removeColumnsAfter(step.depth);
    
    // Clear element details if navigating back from an element
    if (step.itemType !== 'element') {
      millerColumnsStore.setElementDetails(null);
    }
  }

  /**
   * T095: Truncate breadcrumb if > 6 levels
   * Shows first + last 2-3 segments with "..." in middle
   */
  function getTruncatedBreadcrumb(breadcrumb: NavigationStep[]): {
    items: NavigationStep[];
    isTruncated: boolean;
  } {
    if (breadcrumb.length <= 6) {
      return { items: breadcrumb, isTruncated: false };
    }

    // Show first 2 segments, "...", and last 3 segments
    const first = breadcrumb.slice(0, 2);
    const last = breadcrumb.slice(-3);
    
    return {
      items: [...first, ...last],
      isTruncated: true,
    };
  }

  /**
   * T096: Get full path as string for tooltip
   */
  function getFullPath(breadcrumb: NavigationStep[]): string {
    return breadcrumb.map(step => step.label).join(' › ');
  }

  $: truncatedBreadcrumb = getTruncatedBreadcrumb(breadcrumb);
  $: fullPath = getFullPath(breadcrumb);
</script>

<div class="breadcrumb" title={truncatedBreadcrumb.isTruncated ? fullPath : undefined}>
  {#if breadcrumb.length === 0}
    <span class="breadcrumb-empty">No selection</span>
  {:else}
    {#each truncatedBreadcrumb.items as step, index (step.depth)}
      {@const isLast = index === truncatedBreadcrumb.items.length - 1}
      {@const showEllipsis = truncatedBreadcrumb.isTruncated && index === 1}
      
      {#if showEllipsis && index > 0}
        <span class="breadcrumb-separator">›</span>
        <span class="breadcrumb-ellipsis" title={fullPath}>...</span>
      {/if}
      
      {#if index > 0 && !showEllipsis}
        <span class="breadcrumb-separator">›</span>
      {/if}
      
      <button
        class="breadcrumb-segment"
        class:active={isLast}
        on:click={() => handleSegmentClick(step)}
        title={step.label}
        aria-label="Navigate to {step.label}"
      >
        {step.label}
      </button>
    {/each}
  {/if}
</div>

<style>
  .breadcrumb {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    background-color: var(--breadcrumb-bg, #f5f5f5);
    border-bottom: 1px solid var(--border-color, #ddd);
    overflow-x: auto;
    white-space: nowrap;
    gap: 4px;
  }

  .breadcrumb-empty {
    font-size: 13px;
    color: var(--text-secondary, #666);
    font-style: italic;
  }

  .breadcrumb-separator {
    font-size: 14px;
    color: var(--text-secondary, #666);
    margin: 0 4px;
    user-select: none;
  }

  .breadcrumb-ellipsis {
    font-size: 13px;
    color: var(--text-secondary, #666);
    padding: 4px 8px;
    cursor: help;
  }

  .breadcrumb-segment {
    background: none;
    border: none;
    font-size: 13px;
    color: var(--primary-color, #1976d2);
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
    transition: background-color 0.15s;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .breadcrumb-segment:hover {
    background-color: var(--breadcrumb-hover, #e3f2fd);
  }

  .breadcrumb-segment:active {
    background-color: var(--breadcrumb-active, #bbdefb);
  }

  .breadcrumb-segment.active {
    color: var(--text-primary, #333);
    font-weight: 600;
    cursor: default;
  }

  .breadcrumb-segment.active:hover {
    background-color: transparent;
  }

  /* Scrollbar styling for horizontal overflow */
  .breadcrumb::-webkit-scrollbar {
    height: 4px;
  }

  .breadcrumb::-webkit-scrollbar-track {
    background: var(--scrollbar-track, #f1f1f1);
  }

  .breadcrumb::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb, #888);
    border-radius: 2px;
  }

  .breadcrumb::-webkit-scrollbar-thumb:hover {
    background: var(--scrollbar-thumb-hover, #555);
  }
</style>
