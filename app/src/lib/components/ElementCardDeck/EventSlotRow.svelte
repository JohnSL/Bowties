<script lang="ts">
  import type { CardField } from '$lib/stores/configSidebar';
  import type { BowtieCard } from '$lib/api/tauri';
  import { bowtieName } from '$lib/api/tauri';
  import { goto } from '$app/navigation';
  import { createEventDispatcher } from 'svelte';

  export let field: CardField;
  /** Raw event ID bytes (8 bytes); null if not yet read */
  export let bytes: number[] | null = null;
  /** True while a single-field refresh is in progress */
  export let isRefreshing: boolean = false;
  /** Error from last refresh attempt */
  export let refreshError: string | null = null;
  /**
   * Cross-reference: the BowtieCard this event ID slot participates in.
   * When present, renders a "Used in: …" navigable link (FR-008, FR-009).
   */
  export let usedIn: BowtieCard | undefined = undefined;

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

  function handleNavigateToBowties() {
    if (usedIn) {
      goto('/bowties?highlight=' + usedIn.event_id_hex);
    }
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

  {#if usedIn}
    <div class="used-in">
      Used in:
      <button
        class="used-in-link"
        on:click={handleNavigateToBowties}
        title="View bowtie for event {usedIn.event_id_hex}"
        aria-label="View bowtie connection for {bowtieName(usedIn)}"
      >{bowtieName(usedIn)}</button>
    </div>
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

  .used-in {
    margin-top: 3px;
    font-size: 11px;
    color: var(--text-tertiary, #888);
  }

  .used-in-link {
    background: none;
    border: none;
    padding: 0;
    font-size: 11px;
    color: var(--link-color, #1565c0);
    cursor: pointer;
    text-decoration: underline;
  }

  .used-in-link:hover {
    color: var(--link-hover-color, #0d47a1);
  }
</style>
