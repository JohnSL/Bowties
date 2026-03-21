<script lang="ts">
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import type { CardData } from '$lib/stores/configSidebar';
  import ElementCard from './ElementCard.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { resolveCardTitle } from '$lib/utils/cardTitle';

  export let nodeId: string;

  $: deck = $configSidebarStore.cardDeck;

  // Track which segment we last loaded so we don't re-fetch on unrelated store updates
  let lastLoadedSegmentId: string | null = null;

  // Reactive: load top-level groups whenever selectedSegment changes (FR-005, FR-006)
  $: {
    const sel = $configSidebarStore.selectedSegment;
    if (sel && sel.segmentId !== lastLoadedSegmentId) {
      loadCardDeck(sel.nodeId, sel.segmentId, sel.segmentId);
    }
  }

  async function loadCardDeck(nId: string, segId: string, segmentPath: string) {
    lastLoadedSegmentId = segId;
    try {
      const response = await invoke<{ items: any[]; column_type: string }>('get_column_items', {
        nodeId: nId,
        parentPath: [segmentPath],
        depth: 1,
      });

      const cards: CardData[] = response.items.map((item: any) => {
        const meta = item.metadata ?? {};
        const isReplicated: boolean = meta.replicated ?? false;
        const instanceIndex: number | null = meta.instanceNumber ?? null;
        const itemPathId: string = meta.pathId ?? item.id;
        const groupPath = [segmentPath, itemPathId];

        const cardTitle = resolveCardTitle(
          { cdGroupName: item.name, isReplicated, instanceIndex, fields: [] },
          nId,
          new Map(),
        );

        return {
          cardId: item.id,
          groupPath,
          cdGroupName: item.name,
          isReplicated,
          instanceIndex,
          cardTitle,
          elements: null,
          isLoading: false,
          loadError: null,
        } satisfies CardData;
      });

      configSidebarStore.setCards(nId, segId, cards);
    } catch (err) {
      configSidebarStore.setCardDeckLoading(false, String(err));
    }
  }
</script>

<div class="element-card-deck">
  {#if !deck}
    <!-- No segment selected -->
    <div class="empty-prompt">
      <p>Select a segment from the sidebar to view its configuration</p>
    </div>
  {:else if deck.isLoading}
    <!-- Loading deck -->
    <div class="deck-loading">
      <span role="status" aria-label="Loading configuration cards">Loading…</span>
    </div>
  {:else if deck.error}
    <!-- Deck-level error -->
    <div class="deck-error" role="alert">
      {deck.error}
    </div>
  {:else if deck.cards.length === 0}
    <!-- Segment has no top-level groups -->
    <div class="deck-empty">
      <p>(This segment has no configurable groups)</p>
    </div>
  {:else}
    <!-- Card deck — one card per top-level CDI group (FR-006) -->
    <div class="cards-container">
      <div class="deck-header">
        <span class="deck-segment-name">{deck.segmentName}</span>
        <span class="deck-count">{deck.cards.length} item{deck.cards.length !== 1 ? 's' : ''}</span>
      </div>

      {#each deck.cards as card (card.cardId)}
        <ElementCard {card} {nodeId} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .element-card-deck {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    background-color: var(--main-bg, #f8f9fa);
    min-height: 0;
  }

  .empty-prompt {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-secondary, #999);
    font-size: 14px;
    text-align: center;
  }

  .empty-prompt p {
    margin: 0;
    max-width: 280px;
  }

  .deck-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px;
    color: var(--text-secondary, #666);
    font-size: 13px;
  }

  .deck-error {
    padding: 12px 16px;
    background-color: var(--error-bg, #fdf2f2);
    border: 1px solid var(--error-border, #f5c6c6);
    border-radius: 6px;
    color: var(--error-color, #c62828);
    font-size: 13px;
  }

  .deck-empty {
    padding: 24px 0;
    color: var(--text-secondary, #999);
    font-size: 13px;
    font-style: italic;
    text-align: center;
  }

  .deck-empty p {
    margin: 0;
  }

  .cards-container {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .deck-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-color, #ddd);
  }

  .deck-segment-name {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary, #333);
  }

  .deck-count {
    font-size: 12px;
    color: var(--text-secondary, #999);
  }
</style>
