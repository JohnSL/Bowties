<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let segmentId: string;
  export let segmentName: string;
  export let description: string | null = null;
  export let isSelected: boolean = false;

  const dispatch = createEventDispatcher<{ select: { segmentId: string; segmentName: string } }>();

  function handleClick() {
    dispatch('select', { segmentId, segmentName });
  }
</script>

<button
  class="segment-entry"
  class:selected={isSelected}
  on:click={handleClick}
  aria-pressed={isSelected}
>
  <span class="segment-name">{segmentName}</span>
  {#if description}
    <span class="segment-description">{description}</span>
  {/if}
</button>

<style>
  .segment-entry {
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: 6px 12px 6px 40px;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    font-size: 12px;
    color: var(--text-primary, #444);
    border-bottom: 1px solid var(--border-color, #f0f0f0);
    transition: background-color 0.1s;
  }

  .segment-entry:hover {
    background-color: var(--hover-bg, #f5f5f5);
  }

  .segment-entry.selected {
    background-color: var(--selected-bg, #e3f2fd);
    color: var(--primary-color, #1976d2);
    font-weight: 500;
    position: relative;
  }

  .segment-entry.selected::before {
    content: '';
    position: absolute;
    left: 32px;
    top: 4px;
    bottom: 4px;
    width: 3px;
    border-radius: 1.5px;
    background-color: var(--primary-color, #1976d2);
  }

  .segment-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .segment-description {
    font-size: 11px;
    color: var(--text-secondary, #777);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 1px;
  }
</style>
