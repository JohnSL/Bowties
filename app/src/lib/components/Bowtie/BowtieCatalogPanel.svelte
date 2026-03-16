<!--
  BowtieCatalogPanel — in-page tab panel for the bowtie catalog.

  Rendered inside +page.svelte when activeTab === 'bowties'.
  Replaces the former /bowties route page, preserving all catalog display
  logic without full-page navigation (FR-003, FR-010, SC-004).
-->

<script lang="ts">
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { editableBowtiePreviewStore } from '$lib/stores/bowties.svelte';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { setModifiedValue } from '$lib/api/config';
  import BowtieCard from '$lib/components/Bowtie/BowtieCard.svelte';
  import EmptyState from '$lib/components/Bowtie/EmptyState.svelte';
  import NewConnectionDialog from '$lib/components/Bowtie/NewConnectionDialog.svelte';
  import type { ElementSelection, EventIdResolution, PreviewBowtieCard } from '$lib/types/bowtie';
  import type { BowtieCard as BowtieCardType } from '$lib/api/tauri';
  import { findLeafByAddress } from '$lib/types/nodeTree';

  // Optional: event ID hex to scroll to and highlight (FR-009)
  let { highlightedEventIdHex = null }: { highlightedEventIdHex?: string | null } = $props();

  // Store access
  let catalog = $derived(bowtieCatalogStore.catalog);
  let readComplete = $derived(bowtieCatalogStore.readComplete);
  let preview = $derived(editableBowtiePreviewStore.preview);
  let previewCards = $derived(preview.bowties);

  // New Connection dialog state
  let showNewConnectionDialog = $state(false);

  /** Convert a PreviewBowtieCard to the BowtieCard shape expected by the BowtieCard component. */
  function toBowtieCard(p: PreviewBowtieCard): BowtieCardType {
    return {
      event_id_hex: p.eventIdHex,
      event_id_bytes: p.eventIdBytes,
      producers: p.producers,
      consumers: p.consumers,
      ambiguous_entries: p.ambiguousEntries,
      name: p.name ?? null,
      tags: p.tags,
      state: p.state === 'active' ? 'Active' : p.state === 'incomplete' ? 'Incomplete' : 'Planning',
    };
  }

  // Scroll to highlighted card when it becomes available (FR-009)
  $effect(() => {
    if (highlightedEventIdHex) {
      const id = highlightedEventIdHex;
      requestAnimationFrame(() => {
        const el = document.querySelector(`[data-event-id="${CSS.escape(id)}"]`);
        el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      });
    }
  });

  /**
   * Handle new connection creation from the dialog.
   * Sets modified values on tree leaves and metadata in bowtieMetadataStore.
   */
  function handleNewConnection(
    producer: ElementSelection | null,
    consumer: ElementSelection | null,
    name: string,
    resolution: EventIdResolution,
  ): void {
    showNewConnectionDialog = false;

    if (!producer || !consumer) return;

    const eventIdHex = resolution.eventIdHex;

    // Create pending edit for the side(s) that need writing
    if (resolution.writeTo === 'consumer' || resolution.writeTo === 'both') {
      setEventIdOnLeaf(consumer, eventIdHex);
    }
    if (resolution.writeTo === 'producer' || resolution.writeTo === 'both') {
      setEventIdOnLeaf(producer, eventIdHex);
    }

    // Track bowtie metadata
    bowtieMetadataStore.createBowtie(eventIdHex, name || undefined);
  }

  /**
   * Set a modified event ID value on a leaf via the Rust tree.
   */
  function setEventIdOnLeaf(
    element: ElementSelection,
    eventIdHex: string,
  ): void {
    const tree = nodeTreeStore.getTree(element.nodeId);
    if (!tree) {
      console.warn('[BowtieCatalogPanel] setEventIdOnLeaf: tree not found for node', element.nodeId);
      return;
    }

    const leaf = findLeafByAddress(tree, element.address);
    if (!leaf) {
      console.warn('[BowtieCatalogPanel] setEventIdOnLeaf: leaf not found at address', element.address, 'in node', element.nodeId);
      return;
    }

    // Parse event ID hex string to bytes
    const eventIdBytes = eventIdHex.split('.').map(h => parseInt(h, 16));

    setModifiedValue(element.nodeId, element.address, element.space, {
      type: 'eventId',
      bytes: eventIdBytes,
      hex: eventIdHex,
    });
  }
</script>

<div class="bowties-panel">
  <!-- Panel header: stats summary + new connection button -->
  {#if catalog}
    <div class="panel-header">
      <button
        class="new-connection-btn"
        onclick={() => { showNewConnectionDialog = true; }}
        title="Create a new bowtie connection"
      >
        + New Connection
      </button>
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

    {:else if previewCards.length === 0 && (!catalog || catalog.bowties.length === 0)}
      <EmptyState />

    {:else}
      <!-- FR-003, FR-010: scrollable list of bowtie cards with dirty indicators -->
      <div class="card-list" role="list" aria-label="Bowtie connections">
        {#each previewCards as previewCard (previewCard.eventIdHex)}
          <div role="listitem">
            <BowtieCard
              card={toBowtieCard(previewCard)}
              highlighted={highlightedEventIdHex === previewCard.eventIdHex}
              isDirty={previewCard.isDirty}
              dirtyFields={previewCard.dirtyFields}
            />
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- New Connection dialog -->
  <NewConnectionDialog
    visible={showNewConnectionDialog}
    onConfirm={handleNewConnection}
    onCancel={() => { showNewConnectionDialog = false; }}
  />

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
    justify-content: space-between;
    padding: 6px 16px;
    border-bottom: 1px solid #e5e7eb;
    background: #fff;
    flex-shrink: 0;
  }

  .new-connection-btn {
    padding: 4px 12px;
    font-size: 0.82rem;
    font-weight: 500;
    color: #fff;
    background: #2563eb;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .new-connection-btn:hover {
    background: #1d4ed8;
  }

  .new-connection-btn:active {
    background: #1e40af;
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
