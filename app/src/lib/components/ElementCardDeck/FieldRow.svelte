<script lang="ts">
  import type { CardField } from '$lib/stores/configSidebar';
  import type { ConfigValue } from '$lib/api/types';
  import { createEventDispatcher } from 'svelte';

  export let field: CardField;
  /** Current config value; null if not yet read */
  export let value: ConfigValue | null = null;
  /** True while a single-field refresh is in progress */
  export let isRefreshing: boolean = false;
  /** Error from last refresh attempt; null on success */
  export let refreshError: string | null = null;

  const dispatch = createEventDispatcher<{ refresh: { field: CardField } }>();

  let showDescription = false;

  function handleRefresh() {
    dispatch('refresh', { field });
  }

  function formatValue(v: ConfigValue | null): string {
    if (v === null) return '—';
    switch (v.type) {
      case 'Int': return String(v.value);
      case 'String': return v.value || '(empty)';
      case 'Float': return v.value.toFixed(4);
      case 'EventId': return v.value.map(b => b.toString(16).padStart(2, '0')).join('.');
      case 'Invalid': return `(error: ${v.error})`;
    }
  }
</script>

<div class="field-row" role="listitem">
  <div class="field-header">
    <span class="field-label">
      {field.name}
      {#if field.description}
        <button
          class="description-toggle"
          title="Show/hide description"
          aria-label="Toggle field description"
          on:click={() => (showDescription = !showDescription)}
        >?</button>
      {/if}
    </span>

    <span class="field-value" class:loading={isRefreshing}>
      {#if isRefreshing}
        <span role="status" aria-label="Refreshing…">⋯</span>
      {:else}
        {formatValue(value)}
      {/if}
    </span>

    <button
      class="action-btn"
      title="Refresh value"
      aria-label="Refresh {field.name}"
      on:click={handleRefresh}
      disabled={isRefreshing}
    >[R]</button>
  </div>

  {#if showDescription && field.description}
    <div class="field-description">{field.description}</div>
  {/if}

  {#if refreshError}
    <div class="refresh-error">{refreshError}</div>
  {/if}
</div>

<style>
  .field-row {
    padding: 6px 0;
    border-bottom: 1px solid var(--border-light, #f5f5f5);
  }

  .field-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .field-label {
    flex: 1;
    min-width: 0;
    color: var(--text-primary, #333);
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .description-toggle {
    background: none;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 50%;
    width: 16px;
    height: 16px;
    font-size: 10px;
    cursor: pointer;
    color: var(--text-secondary, #666);
    padding: 0;
    line-height: 1;
    flex-shrink: 0;
  }

  .field-value {
    color: var(--text-secondary, #555);
    font-family: monospace;
    font-size: 12px;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-value.loading {
    opacity: 0.5;
  }

  .action-btn {
    background: none;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 3px;
    padding: 1px 5px;
    font-size: 11px;
    cursor: pointer;
    color: var(--text-secondary, #666);
    flex-shrink: 0;
  }

  .action-btn:hover:not(:disabled) {
    background-color: var(--hover-bg, #f5f5f5);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .field-description {
    margin-top: 4px;
    font-size: 11px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
    padding-left: 4px;
  }

  .refresh-error {
    margin-top: 2px;
    font-size: 11px;
    color: var(--error-color, #c62828);
  }
</style>
