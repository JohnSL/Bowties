<!--
  BowtieCatalogPanel — in-page tab panel for the bowtie catalog.

  Rendered inside +page.svelte when activeTab === 'bowties'.
  Replaces the former /bowties route page, preserving all catalog display
  logic without full-page navigation (FR-003, FR-010, SC-004).
-->

<script lang="ts">
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import BowtieCard from '$lib/components/Bowtie/BowtieCard.svelte';
  import EmptyState from '$lib/components/Bowtie/EmptyState.svelte';

  // Optional: event ID hex to scroll to and highlight (FR-009)
  let { highlightedEventIdHex = null }: { highlightedEventIdHex?: string | null } = $props();

  // Store access
  let catalog = $derived(bowtieCatalogStore.catalog);
  let readComplete = $derived(bowtieCatalogStore.readComplete);

  // Scroll to highlighted card when it becomes available (FR-009)
  $effect(() => {
    if (highlightedEventIdHex) {
      const id = highlightedEventIdHex;
      requestAnimationFrame(() => {
        const el = document.querySelector(`[data-event-id="${id}"]`);
        el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      });
    }
  });
</script>

<div class="bowties-panel">
  <!-- Panel header: stats summary -->
  {#if catalog}
    <div class="panel-header">
      <span class="catalog-meta">
        {catalog.bowties.length} connection{catalog.bowties.length !== 1 ? 's' : ''}
        · {catalog.source_node_count} node{catalog.source_node_count !== 1 ? 's' : ''}
      </span>
    </div>
  {/if}

  <!-- Content area -->
  <div class="panel-content">
    {#if !readComplete}
      <div class="not-ready">
        <p>Bowties will be available after CDI reads complete.</p>
        <p class="hint">Discover nodes and read their configuration from the toolbar.</p>
      </div>

    {:else if !catalog || catalog.bowties.length === 0}
      <EmptyState />

    {:else}
      <!-- FR-003, FR-010: scrollable list of bowtie cards -->
      <div class="card-list" role="list" aria-label="Bowtie connections">
        {#each catalog.bowties as card (card.event_id_hex)}
          <div role="listitem">
            <BowtieCard
              {card}
              highlighted={highlightedEventIdHex === card.event_id_hex}
            />
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .bowties-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 6px 16px;
    border-bottom: 1px solid #e5e7eb;
    background: #fff;
    flex-shrink: 0;
  }

  .catalog-meta {
    font-size: 0.78rem;
    color: #6b7280;
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    background: #f9fafb;
  }

  .not-ready {
    text-align: center;
    padding: 48px 24px;
    color: #6b7280;
  }

  .not-ready .hint {
    font-size: 0.85rem;
    margin-top: 8px;
    color: #9ca3af;
  }

  .card-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 900px;
    margin: 0 auto;
  }
</style>
