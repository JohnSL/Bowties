<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let nodeId: string;
  export let nodeName: string;
  /** Optional secondary detail (e.g. manufacturer/model for disambiguation) */
  export let nodeDetail: string | null = null;
  /** Full node details for the hover tooltip */
  export let nodeTooltip: string | null = null;
  export let isExpanded: boolean = false;
  export let isOffline: boolean = false;
  export let isLoading: boolean = false;

  const dispatch = createEventDispatcher<{ toggle: { nodeId: string } }>();

  function handleClick() {
    dispatch('toggle', { nodeId });
  }
</script>

<button
  class="node-entry"
  class:expanded={isExpanded}
  class:offline={isOffline}
  on:click={handleClick}
  aria-expanded={isExpanded}
  title={nodeTooltip ?? undefined}
>
  <span class="expand-icon" aria-hidden="true">{isExpanded ? '▾' : '▸'}</span>

  <span class="node-info">
    <span class="node-name">{nodeName}</span>
    {#if nodeDetail}
      <span class="node-detail">{nodeDetail}</span>
    {/if}
  </span>

  {#if isOffline}
    <span class="offline-indicator" title="Offline" aria-label="offline">⚠</span>
  {/if}

  {#if isLoading}
    <span role="status" class="loading" aria-label="Loading segments…">
      <span class="spinner" aria-hidden="true">⋯</span>
    </span>
  {/if}
</button>

<style>
  .node-entry {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 8px 12px;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    gap: 6px;
    font-size: 13px;
    color: var(--text-primary, #333);
    border-bottom: 1px solid var(--border-color, #eee);
    transition: background-color 0.1s;
  }

  .node-entry:hover {
    background-color: var(--hover-bg, #f5f5f5);
  }

  .node-entry.offline {
    opacity: 0.7;
  }

  .expand-icon {
    flex-shrink: 0;
    width: 18px;
    font-size: 22px;
    line-height: 1;
    color: var(--text-secondary, #666);
  }

  .node-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .node-name {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .node-detail {
    font-size: 11px;
    color: var(--text-secondary, #666);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .offline-indicator {
    flex-shrink: 0;
    color: var(--warning-color, #f59e0b);
    font-size: 12px;
  }

  .loading {
    flex-shrink: 0;
    color: var(--text-secondary, #666);
    font-size: 11px;
  }

  .spinner {
    display: inline-block;
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 1; }
  }
</style>
