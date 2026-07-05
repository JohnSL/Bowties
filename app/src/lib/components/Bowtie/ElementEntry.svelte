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
  import { configFocusStore } from '$lib/stores/configFocus.svelte';
  import { resolveNodeParts } from '$lib/layout';
  import type { NodeDisplayParts } from '$lib/utils/nodeDisplayName';
  import NodeLabel from '$lib/components/NodeLabel.svelte';

  interface Props {
    entry: EventSlotEntry;
    isNew?: boolean;
    /** Node is not responding on the bus — show offline badge. */
    isNodeOffline?: boolean;
  }

  let { entry, isNew = false, isNodeOffline = false }: Props = $props();

  const hasDescription = $derived(!!entry.element_description);

  /** Build NodeDisplayParts using the entry's pre-resolved node_name as primary,
   *  augmented with model/manufacturer from live stores when available. */
  const nodeParts = $derived.by((): NodeDisplayParts => {
    const live = resolveNodeParts(entry.node_key);
    // If live resolution found model/manufacturer, use them with the
    // entry's node_name (which may include disambiguation suffixes).
    if (live.model || live.manufacturer) {
      return {
        name: entry.node_name,
        model: live.model,
        manufacturer: live.manufacturer,
        isUserNamed: live.isUserNamed,
      };
    }
    // Fallback: just the pre-resolved name, no product context.
    return { name: entry.node_name, model: null, manufacturer: null, isUserNamed: false };
  });
</script>

<div class="element-entry" class:has-description={hasDescription} class:offline={isNodeOffline}>
  <div class="entry-meta">
    <span class="node-name">
      <NodeLabel parts={nodeParts} orientation="inline" />
      {#if isNodeOffline}<span class="offline-indicator" title="Node offline" aria-label="offline">⚠</span>{/if}
    </span>
    <span class="element-label">
      <button
        class="element-label-link"
        onclick={() => configFocusStore.focusConfigField(entry.node_key, entry.element_path)}
        title="Go to this field in the configuration"
        aria-label="Go to {entry.element_label ?? 'field'} in configuration"
      >{entry.element_label ?? ''}</button>
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
    font-size: 0.85rem;
    color: #242424;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: flex;
    align-items: baseline;
    gap: 4px;
  }

  .element-label {
    font-size: 0.78rem;
    color: #242424;
    word-break: break-word;
  }

  .element-label-link {
    background: none;
    border: none;
    padding: 0;
    font-size: 0.78rem;
    color: #0078d4;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-align: left;
    word-break: break-word;
  }

  .element-label-link:hover {
    color: #005a9e;
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

  .element-entry.offline {
    opacity: 0.7;
  }

  .offline-indicator {
    margin-left: 4px;
    color: var(--warning-color, #f59e0b);
    font-size: 0.75rem;
  }
</style>
