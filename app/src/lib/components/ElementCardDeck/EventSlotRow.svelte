<script lang="ts">
  import type { CardField } from '$lib/stores/configSidebar';
  import { createEventDispatcher } from 'svelte';

  export let field: CardField;
  /** Raw event ID bytes (8 bytes); null if not yet read */
  export let bytes: number[] | null = null;
  /** True while a single-field refresh is in progress */
  export let isRefreshing: boolean = false;
  /** Error from last refresh attempt */
  export let refreshError: string | null = null;

  const dispatch = createEventDispatcher<{ refresh: { field: CardField } }>();

  /** From FR-014: all-zero event IDs are labeled "(free)" */
  function formatEventId(eventBytes: number[] | null): string {
    if (eventBytes === null) return '—';
    if (eventBytes.every(b => b === 0)) return '(free)';
    return eventBytes.map(b => b.toString(16).padStart(2, '0')).join('.');
  }

  function handleRefresh() {
    dispatch('refresh', { field });
  }
</script>

<div class="event-slot-row" role="listitem">
  <div class="slot-header">
    <span class="slot-label">{field.name}</span>

    <span class="slot-value" class:loading={isRefreshing}>
      {#if isRefreshing}
        <span role="status" aria-label="Refreshing…">⋯</span>
      {:else}
        <span class="event-id" class:free={bytes !== null && bytes.every(b => b === 0)}>
          {formatEventId(bytes)}
        </span>
      {/if}
    </span>

    <button
      class="action-btn"
      title="Refresh event ID"
      aria-label="Refresh {field.name}"
      on:click={handleRefresh}
      disabled={isRefreshing}
    >[R]</button>
  </div>

  {#if refreshError}
    <div class="refresh-error">{refreshError}</div>
  {/if}
</div>

<style>
  .event-slot-row {
    padding: 6px 0;
    border-bottom: 1px solid var(--border-light, #f5f5f5);
  }

  .slot-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .slot-label {
    flex: 1;
    min-width: 0;
    color: var(--text-primary, #333);
  }

  .slot-value {
    font-family: monospace;
    font-size: 12px;
    color: var(--text-secondary, #555);
  }

  .slot-value.loading {
    opacity: 0.5;
  }

  .event-id {
    letter-spacing: 0.3px;
  }

  .event-id.free {
    color: var(--text-tertiary, #999);
    font-style: italic;
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

  .refresh-error {
    margin-top: 2px;
    font-size: 11px;
    color: var(--error-color, #c62828);
  }
</style>
