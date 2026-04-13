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
  /** Whether this node's config values have NOT been read yet */
  export let configNotRead: boolean = false;
  /** Whether this node has unsaved pending edits (FR-012a) */
  export let hasPendingEdits: boolean = false;
  /** Whether this node has saved offline changes still pending apply to device */
  export let hasPendingApply: boolean = false;
  /** Whether this node is selected (node-level selection, no segment) */
  export let isSelected: boolean = false;

  const dispatch = createEventDispatcher<{
    toggle: { nodeId: string };
    readConfig: { nodeId: string };
  }>();

  function handleClick() {
    dispatch('toggle', { nodeId });
  }

  function handleReadConfig(event: Event) {
    event.stopPropagation();
    dispatch('readConfig', { nodeId });
  }
</script>

<button
  class="node-entry"
  class:expanded={isExpanded}
  class:offline={isOffline}
  class:selected={isSelected}
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

  {#if hasPendingEdits}
    <span
      class="pending-edits-dot"
      title="Unsaved changes"
      aria-label="Unsaved changes"
    ></span>
  {/if}

  {#if hasPendingApply}
    <span
      class="pending-apply-dot"
      title="Saved in layout, pending apply to node"
      aria-label="Saved in layout, pending apply to node"
    ></span>
  {/if}

  {#if isLoading}
    <span role="status" class="loading" aria-label="Loading segments…">
      <span class="spinner" aria-hidden="true">⋯</span>
    </span>
  {/if}

  {#if configNotRead && !isOffline}
    <span
      class="config-not-read"
      role="button"
      tabindex="-1"
      on:click={handleReadConfig}
      on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleReadConfig(e); }}
      title="Configuration not yet read — click to read"
      aria-label="Read configuration for {nodeName}"
    >
      <span class="not-read-dot" aria-hidden="true"></span>
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
    border-left: 3px solid transparent;
    transition: background-color 0.1s, border-left-color 0.1s;
  }

  .node-entry:hover {
    background-color: var(--hover-bg, #f5f5f5);
  }

  .node-entry.selected {
    border-left-color: #0078d4;
    background-color: rgba(0, 120, 212, 0.06);
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

  .config-not-read {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    margin: 0;
    background: none;
    border: none;
    cursor: pointer;
    border-radius: 50%;
    transition: background-color 0.15s;
  }

  .config-not-read:hover {
    background-color: rgba(0, 0, 0, 0.08);
  }

  .not-read-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: var(--warning-color, #f59e0b);
    border: 1.5px solid #fff;
    box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.3);
  }

  .pending-edits-dot {
    flex-shrink: 0;
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: #ca8500;                     /* amber — unsaved changes (distinct from selection blue) */
    border: 1.5px solid #fff;
    box-shadow: 0 0 0 1px rgba(202, 133, 0, 0.35);
  }

  .pending-apply-dot {
    flex-shrink: 0;
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: #0f766e;
    border: 1.5px solid #fff;
    box-shadow: 0 0 0 1px rgba(15, 118, 110, 0.35);
  }
</style>
