<!--
  T016: ElementEntry.svelte
  Renders a single event slot entry (producer, consumer, or ambiguous).

  Props:
    entry: EventSlotEntry

  Layout (always vertical stack):
    node_name
    element_label  [● new badge if isNew]
    description    (optional, wraps below)
-->

<script lang="ts">
  import type { EventSlotEntry } from '$lib/api/tauri';

  interface Props {
    entry: EventSlotEntry;
    isNew?: boolean;
  }

  let { entry, isNew = false }: Props = $props();

  const hasDescription = $derived(!!entry.element_description);
</script>

<div class="element-entry" class:has-description={hasDescription}>
  <div class="entry-meta">
    <span class="node-name">{entry.node_name}</span>
    <span class="element-label">
      {entry.element_label ?? ''}
      {#if isNew}<span class="new-badge" aria-label="New entry">● new</span>{/if}
    </span>
  </div>
  {#if hasDescription}
    <p class="entry-description">{entry.element_description}</p>
  {/if}
</div>

<style>
  .element-entry {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 24px 6px 8px;
    border-radius: 4px;
    background: #f5f5f4;
    border: 1px solid #d1d5db;
    width: 100%;
    box-sizing: border-box;
  }

  .entry-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .node-name {
    font-weight: 600;
    font-size: 0.85rem;
    color: #242424;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .element-label {
    font-size: 0.78rem;
    color: #242424;
    word-break: break-word;
  }

  .new-badge {
    display: inline-block;
    margin-left: 5px;
    font-size: 0.68rem;
    font-weight: 600;
    color: #0b6a0b;
    background: #dff6dd;
    padding: 1px 5px;
    border-radius: 3px;
    vertical-align: middle;
    white-space: nowrap;
  }

  .entry-description {
    margin: 2px 0 0;
    font-size: 0.78rem;
    color: #605e5c;
    line-height: 1.4;
    white-space: normal;
    overflow-wrap: break-word;
  }
</style>
