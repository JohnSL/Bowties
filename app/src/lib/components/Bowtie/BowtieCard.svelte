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
  import { bowtieName } from '$lib/api/tauri';
  import ElementEntry from './ElementEntry.svelte';
  import ConnectorArrow from './ConnectorArrow.svelte';

  interface Props {
    card: BowtieCardType;
    /** Pass true to visually highlight (e.g. when navigated via cross-reference) */
    highlighted?: boolean;
  }

  let { card, highlighted = false }: Props = $props();

  let hasAmbiguous = $derived(card.ambiguous_entries.length > 0);
</script>

<div
  class="bowtie-card"
  class:highlighted
  aria-label="Bowtie card for event {card.event_id_hex}"
  data-event-id={card.event_id_hex}
>
  <!-- Header: name (if set) or event_id_hex as fallback (FR-014) -->
  <header class="card-header">
    <h3 class="card-title">{bowtieName(card)}</h3>
  </header>

  <!-- Three-column layout: Producers | Arrow | Consumers (FR-004) -->
  <div class="card-body">
    <!-- Producers column -->
    <section class="column producers-column" aria-label="Producers">
      <h4 class="column-label">Producers</h4>
      {#each card.producers as entry (entry.node_id + entry.element_path.join('/'))}
        <ElementEntry {entry} />
      {/each}
    </section>

    <!-- Centre connector arrow (FR-005) -->
    <ConnectorArrow eventIdHex={card.event_id_hex} />

    <!-- Consumers column -->
    <section class="column consumers-column" aria-label="Consumers">
      <h4 class="column-label">Consumers</h4>
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
    background: var(--card-bg, #1e293b);
    border: 1px solid var(--card-border, rgba(255, 255, 255, 0.08));
    border-radius: 8px;
    overflow: hidden;
    transition: box-shadow 0.15s ease;
  }

  .bowtie-card.highlighted {
    box-shadow: 0 0 0 2px var(--highlight-color, #3b82f6);
  }

  .card-header {
    padding: 10px 14px 8px;
    border-bottom: 1px solid var(--card-border, rgba(255, 255, 255, 0.08));
    background: var(--card-header-bg, rgba(255, 255, 255, 0.03));
  }

  .card-title {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary, #e2e8f0);
    font-family: 'ui-monospace', monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-body {
    display: flex;
    align-items: flex-start;
    gap: 0;
    padding: 12px;
  }

  .column {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .column-label {
    margin: 0 0 6px;
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted, #64748b);
  }

  .ambiguous-section {
    border-top: 1px solid var(--card-border, rgba(255, 255, 255, 0.08));
    padding: 10px 12px;
    background: var(--ambiguous-bg, rgba(251, 191, 36, 0.05));
  }

  .ambiguous-label {
    margin: 0 0 8px;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--warning-color, #f59e0b);
  }

  .ambiguous-entries {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
</style>
