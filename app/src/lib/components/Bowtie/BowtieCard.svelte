<!--
  T018: BowtieCard.svelte
  Renders a single bowtie card for one shared event ID (FR-002, FR-004, FR-005, FR-014).

  Props:
    card: BowtieCard — the card data to render

  Layout (FR-004):
    ┌──────────────────────────────────────────────────────────┐
    │ [card header: card.name ?? card.event_id_hex]             │
    ├──────────────────────────────────────────────────────────┤
    │  Producers column  │ → event_id │  Consumers column       │
    │  [ElementEntry]    │            │  [ElementEntry]          │
    │  [ElementEntry]    │            │  [ElementEntry]          │
    ├──────────────────────────────────────────────────────────┤
    │ [Ambiguous section — only when ambiguous_entries non-empty]│
    └──────────────────────────────────────────────────────────┘
-->

<script lang="ts">
  import type { BowtieCard as BowtieCardType } from '$lib/api/tauri';
  import ElementEntry from './ElementEntry.svelte';
  import ConnectorArrow from './ConnectorArrow.svelte';

  /** Write feedback state for a bowtie card (FR-030) */
  type WriteStatus = 'idle' | 'writing' | 'success' | 'error' | 'rolled-back' | 'rollback-failed';

  interface Props {
    card: BowtieCardType;
    /** Pass true to visually highlight (e.g. when navigated via cross-reference) */
    highlighted?: boolean;
    /** T021: Set of dirty field names for unsaved-change indicators */
    dirtyFields?: Set<string>;
    /** T021: Whether this card has any unsaved changes */
    isDirty?: boolean;
    /** T029: Write operation feedback state */
    writeStatus?: WriteStatus;
    /** T029: Error message when writeStatus is 'error' or 'rollback-failed' */
    writeError?: string | null;
    /** T029: Callback for retry button */
    onRetry?: (() => void) | null;
  }

  let {
    card,
    highlighted = false,
    dirtyFields,
    isDirty = false,
    writeStatus = 'idle',
    writeError = null,
    onRetry = null,
  }: Props = $props();

  let hasAmbiguous = $derived(card.ambiguous_entries.length > 0);

  // Auto-dismiss success feedback after 3s
  let showSuccess = $state(false);
  $effect(() => {
    if (writeStatus === 'success') {
      showSuccess = true;
      const timer = setTimeout(() => { showSuccess = false; }, 3000);
      return () => clearTimeout(timer);
    } else {
      showSuccess = false;
    }
  });
</script>

<div
  class="bowtie-card"
  class:highlighted
  class:is-dirty={isDirty}
  aria-label="Bowtie card for event {card.event_id_hex}"
  data-event-id={card.event_id_hex}
>
  <!-- Header: name (if set) or event_id_hex as fallback (FR-014) -->
  <header class="card-header">
    <h3 class="card-title">
      {#if card.name}
        {card.name} <span class="event-id-suffix">({card.event_id_hex})</span>
      {:else}
        {card.event_id_hex}
      {/if}
      {#if isDirty}
        <span class="dirty-dot" title="Unsaved changes" aria-label="Unsaved changes">●</span>
      {/if}
    </h3>
    <div class="header-badges">
      {#if dirtyFields && dirtyFields.size > 0}
        <span class="dirty-badge" aria-label="Modified fields: {[...dirtyFields].join(', ')}">
          modified
        </span>
      {/if}
      <!-- T029: Write operation feedback (FR-030) -->
      {#if writeStatus === 'writing'}
        <span class="write-status write-status-writing" aria-label="Writing to nodes">
          <span class="spinner"></span>
          Writing…
        </span>
      {:else if showSuccess}
        <span class="write-status write-status-success" aria-label="Write successful">
          ✓ Saved
        </span>
      {:else if writeStatus === 'error' || writeStatus === 'rollback-failed'}
        <span class="write-status write-status-error" aria-label="Write failed: {writeError ?? 'unknown error'}">
          ✗ {writeStatus === 'rollback-failed' ? 'Rollback failed' : 'Write failed'}
          {#if onRetry}
            <button class="retry-btn" onclick={onRetry} title="Retry write">Retry</button>
          {/if}
        </span>
      {:else if writeStatus === 'rolled-back'}
        <span class="write-status write-status-rolledback" aria-label="Write rolled back">
          ↺ Rolled back
        </span>
      {/if}
    </div>
  </header>

  <!-- Three-column layout: Producers | Arrow | Consumers (FR-004) -->
  <!-- Labels row -->
  <div class="labels-row">
    <div class="label-column">
      <span class="column-label producers-label">Producers</span>
    </div>
    <div class="label-spacer"></div>
    <div class="label-column">
      <span class="column-label consumers-label">Consumers</span>
    </div>
  </div>

  <!-- Entries and arrow row -->
  <div class="card-body">
    <!-- Producers entries -->
    <section class="column producers-column" aria-label="Producers">
      {#each card.producers as entry (entry.node_id + entry.element_path.join('/'))}
        <ElementEntry {entry} />
      {/each}
    </section>

    <!-- Centre connector arrow (FR-005) -->
    <ConnectorArrow />

    <!-- Consumers entries -->
    <section class="column consumers-column" aria-label="Consumers">
      {#each card.consumers as entry (entry.node_id + entry.element_path.join('/'))}
        <ElementEntry {entry} />
      {/each}
    </section>
  </div>

  <!-- Ambiguous entries section (only rendered when non-empty) -->
  {#if hasAmbiguous}
    <div class="ambiguous-section" aria-label="Unknown role entries">
      <h4 class="ambiguous-label">Unknown role — needs clarification</h4>
      <div class="ambiguous-entries">
        {#each card.ambiguous_entries as entry (entry.node_id + entry.element_path.join('/'))}
          <ElementEntry {entry} />
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .bowtie-card {
    background: #ffffff;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    overflow: hidden;
    transition: box-shadow 0.15s ease;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  .bowtie-card.highlighted {
    box-shadow: 0 0 0 2px #0078d4;
  }

  .bowtie-card.is-dirty {
    border-color: #ca5010;
    box-shadow: 0 0 0 1px rgba(202, 80, 16, 0.2);
  }

  .dirty-dot {
    color: #ca5010;
    font-size: 0.6rem;
    vertical-align: super;
    margin-left: 4px;
  }

  .dirty-badge {
    font-size: 0.68rem;
    font-weight: 500;
    color: #ca5010;
    background: #fff4e6;
    padding: 1px 6px;
    border-radius: 3px;
    border: 1px solid #ffe0b2;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px 8px;
    border-bottom: 1px solid #d1d5db;
    background: #ffffff;
  }

  .header-badges {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .card-title {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    color: #242424;
    font-family: 'ui-monospace', monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1 1 0;
    min-width: 0;
  }

  .event-id-suffix {
    color: #6b7280;
    font-weight: 400;
    font-size: 0.82rem;
  }

  .card-body {
    display: flex;
    align-items: center;
    gap: 0;
    padding: 12px;
  }

  .labels-row {
    display: flex;
    align-items: flex-start;
    gap: 0;
    padding: 0 12px;
    padding-top: 8px;
    padding-bottom: 2px;
  }

  .label-column {
    flex: 1;
    display: flex;
    min-width: 0;
  }

  .label-spacer {
    flex-shrink: 0;
    width: 60px;
  }

  .column {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .column-label {
    margin: 0;
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 4px 8px;
    border-radius: 4px;
    display: inline-block;
    width: fit-content;
  }

  .producers-label {
    color: #0b6a0b;
    background: #dff6dd;
  }

  .consumers-label {
    color: #0078d4;
    background: #deecf9;
  }

  .ambiguous-section {
    border-top: 1px solid #d1d5db;
    padding: 10px 12px;
    background: #fdf8f4;
  }

  .ambiguous-label {
    margin: 0 0 8px;
    font-size: 0.78rem;
    font-weight: 600;
    color: #ca5010;
  }

  .ambiguous-entries {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  /* T029: Write operation feedback styles (FR-030) */
  .write-status {
    font-size: 0.72rem;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 3px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }

  .write-status-writing {
    color: #0078d4;
    background: #deecf9;
    border: 1px solid #b4d6fa;
  }

  .write-status-success {
    color: #0b6a0b;
    background: #dff6dd;
    border: 1px solid #b7e1cd;
  }

  .write-status-error {
    color: #a4262c;
    background: #fde7e9;
    border: 1px solid #f1bbbc;
  }

  .write-status-rolledback {
    color: #8a6d3b;
    background: #fcf8e3;
    border: 1px solid #f5e79e;
  }

  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid #b4d6fa;
    border-top-color: #0078d4;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .retry-btn {
    margin-left: 4px;
    padding: 1px 6px;
    font-size: 0.68rem;
    font-weight: 600;
    color: #a4262c;
    background: #fff;
    border: 1px solid #a4262c;
    border-radius: 3px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .retry-btn:hover {
    background: #fde7e9;
  }
</style>
