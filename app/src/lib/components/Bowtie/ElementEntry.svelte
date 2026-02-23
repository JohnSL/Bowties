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
    background: var(--entry-bg, rgba(255, 255, 255, 0.04));
    border: 1px solid var(--entry-border, rgba(255, 255, 255, 0.08));
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
    color: var(--text-primary, #e2e8f0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .element-label {
    font-size: 0.78rem;
    color: var(--text-secondary, #94a3b8);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .entry-description {
    flex: 1;
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-secondary, #94a3b8);
    line-height: 1.4;
    white-space: normal;
    overflow-wrap: break-word;
  }
</style>
