<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let segmentId: string;
  export let segmentName: string;
  export let description: string | null = null;
  export let isSelected: boolean = false;
  /** Whether this segment has unsaved pending edits (FR-012b) */
  export let hasPendingEdits: boolean = false;
  /** Whether this segment has saved offline changes pending apply */
  export let hasPendingApply: boolean = false;

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
  <span class="segment-name-row">
    <span class="segment-name">{segmentName}</span>
    {#if hasPendingEdits}
      <span
        class="pending-edits-dot"
        title="Unsaved changes"
        aria-label="Unsaved changes"
      ></span>
    {/if}
    {#if hasPendingApply}
      <span
        class="pending-apply-dot"
        title="Saved in layout, pending apply to node"
        aria-label="Saved in layout, pending apply to node"
      ></span>
    {/if}
  </span>
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

  .segment-name-row {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }

  .pending-edits-dot {
    flex-shrink: 0;
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background-color: #ca8500;                     /* amber — unsaved changes (distinct from selection blue) */
    border: 1.5px solid #fff;
    box-shadow: 0 0 0 1px rgba(202, 133, 0, 0.35);
  }

  .pending-apply-dot {
    flex-shrink: 0;
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background-color: #0f766e;
    border: 1.5px solid #fff;
    box-shadow: 0 0 0 1px rgba(15, 118, 110, 0.35);
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
