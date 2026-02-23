<script lang="ts">
  import type { CardData, CardElementTree, CardField, CardSubGroup } from '$lib/stores/configSidebar';
  import FieldRow from './FieldRow.svelte';
  import EventSlotRow from './EventSlotRow.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

  export let card: CardData;
  export let nodeId: string;

  // Local expansion state — starts expanded when loading or error is already set
  let localExpanded: boolean = card.isLoading || card.loadError !== null;
  // Local copies that stay in sync with prop but can also be overridden by invoke result
  let localElements: CardElementTree | null = card.elements;
  let localLoading: boolean = card.isLoading;
  let localError: string | null = card.loadError;

  // Keep local state in sync with external card prop updates
  $: localElements = card.elements;
  $: localLoading = card.isLoading;
  $: localError = card.loadError;
  // Auto-expand when loading or error is present (only happens after a prior toggle)
  $: if (localLoading || localError) localExpanded = true;

  // Cross-reference lookup: nodeId + CDI path → BowtieCard (FR-008, SC-005)
  $: nodeSlotMap = bowtieCatalogStore.nodeSlotMap;

  async function handleToggle() {
    localExpanded = !localExpanded;

    if (localExpanded && card.elements === null && !card.isLoading && !localLoading) {
      localLoading = true;
      localError = null;
      try {
        const result = await invoke<CardElementTree>('get_card_elements', {
          nodeId,
          groupPath: card.groupPath,
        });
        localElements = result;
      } catch (err) {
        localError = String(err);
      } finally {
        localLoading = false;
      }
    }
  }

  /** Recursively collect all leaf fields from element tree (FR-011: inline, fully expanded) */
  function collectFields(tree: CardElementTree | CardSubGroup): CardField[] {
    const result: CardField[] = [...tree.fields];
    for (const sub of tree.subGroups) {
      result.push(...collectFields(sub));
    }
    return result;
  }
</script>

<!-- Card header (always visible) -->
<div class="element-card">
  <button
    class="card-header"
    class:expanded={localExpanded}
    on:click={handleToggle}
    aria-expanded={localExpanded}
  >
    <span class="card-title">{card.cardTitle}</span>
    <span class="expand-icon" aria-hidden="true">{localExpanded ? '▾' : '▸'}</span>
  </button>

  <!-- Loading spinner — shown when loading is active -->
  {#if localLoading}
    <div role="status" aria-label="Loading card elements" class="card-loading">
      <span class="spinner" aria-hidden="true">⋯</span>
      Loading…
    </div>
  {/if}

  <!-- Error state — shown when element load has failed -->
  {#if localError && !localLoading}
    <div class="card-error" role="alert">
      {localError}
    </div>
  {/if}

  <!-- Card body — revealed when expanded and not loading -->
  {#if localExpanded && !localLoading && !localError}
    <div role="region" aria-label="card body" class="card-body">
      {#if localElements !== null}
        {@const allFields = collectFields(localElements)}
        {#if allFields.length === 0}
          <p class="no-fields">(no configurable fields)</p>
        {:else}
          <div class="fields-list" role="list">
            {#each allFields as field (field.elementPath.join('/'))}
              {#if field.dataType === 'eventid'}
                <EventSlotRow
                  {field}
                  usedIn={nodeSlotMap.get(`${nodeId}:${field.elementPath.join('/')}`)}
                />
              {:else}
                <FieldRow {field} />
              {/if}
            {/each}
          </div>
        {/if}
      {:else}
        <!-- elements not yet fetched and not loading — can happen if invoke is still in flight -->
        <p class="no-fields">—</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .element-card {
    background: var(--card-bg, #fff);
    border: 1px solid var(--border-color, #ddd);
    border-radius: 6px;
    margin-bottom: 6px;
    overflow: hidden;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 10px 14px;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary, #333);
    transition: background-color 0.1s;
  }

  .card-header:hover {
    background-color: var(--hover-bg, #f5f5f5);
  }

  .card-header.expanded {
    border-bottom: 1px solid var(--border-color, #ddd);
    background-color: var(--header-expanded-bg, #fafafa);
  }

  .card-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .expand-icon {
    flex-shrink: 0;
    margin-left: 8px;
    color: var(--text-secondary, #666);
    font-size: 11px;
  }

  .card-loading {
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-secondary, #666);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .spinner {
    display: inline-block;
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.3; }
    50% { opacity: 1; }
  }

  .card-error {
    padding: 8px 14px;
    font-size: 12px;
    color: var(--error-color, #c62828);
    background-color: var(--error-bg, #fdf2f2);
    border-top: 1px solid var(--error-border, #f5c6c6);
  }

  .card-body {
    padding: 8px 14px 12px;
  }

  .fields-list {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .no-fields {
    font-size: 12px;
    color: var(--text-secondary, #999);
    font-style: italic;
    margin: 0;
    padding: 6px 0;
  }
</style>
