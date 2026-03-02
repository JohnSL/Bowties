<!--
  T016: ElementEntry.svelte
  Renders a single event slot entry (producer, consumer, or ambiguous).

  Props:
    entry: EventSlotEntry

  Layout:
    Without description: node_name stacked above element_label.
    With description:    entry-meta (min 160px) | entry-description (flex:1)
-->

<script lang="ts">
  import type { EventSlotEntry } from '$lib/api/tauri';

  interface Props {
    entry: EventSlotEntry;
  }

  let { entry }: Props = $props();

  const hasDescription = $derived(!!entry.element_description);
</script>

<div class="element-entry" class:has-description={hasDescription}>
  <div class="entry-meta">
    <span class="node-name">{entry.node_name}</span>
    <span class="element-label">{entry.element_label}</span>
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
    padding: 6px 8px;
    border-radius: 4px;
    background: #f5f5f4;
    border: 1px solid #d1d5db;
  }

  .element-entry.has-description {
    flex-direction: row;
    align-items: flex-start;
    gap: 12px;
  }

  .entry-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 160px;
    flex-shrink: 0;
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
    color: #605e5c;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .entry-description {
    flex: 1;
    margin: 0;
    font-size: 0.78rem;
    color: #605e5c;
    line-height: 1.4;
    white-space: normal;
    overflow-wrap: break-word;
  }
</style>
