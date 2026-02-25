<script lang="ts">
  /**
   * TreeGroupAccordion — renders a GroupConfigNode from the unified tree.
   *
   * Uses instanceLabel for display (fixes the 12 flat "Event" accordions bug).
   * Recursively renders child groups and leaf nodes.
   *
   * Spec: 007-unified-node-tree, Phase 4.
   */
  import type { GroupConfigNode, ConfigNode } from '$lib/types/nodeTree';
  import { isGroup, isLeaf } from '$lib/types/nodeTree';
  import TreeLeafRow from './TreeLeafRow.svelte';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

  /** The group node from the unified tree */
  export let group: GroupConfigNode;
  /** Node ID for cross-reference lookups */
  export let nodeId: string;
  /** Current nesting depth — used to indent deeper levels */
  export let depth: number = 0;
  /**
   * Whether this group is collapsible (accordion).
   * True for replicated groups (replicationCount > 1).
   * False for non-replicated groups — rendered inline, always visible.
   */
  export let collapsible: boolean = group.replicationCount > 1;

  let expanded = false;

  // Cross-reference lookup: nodeId + CDI path → BowtieCard
  $: nodeSlotMap = bowtieCatalogStore.nodeSlotMap;

  /** Get the BowtieCard cross-reference for a leaf's path */
  function getUsedIn(leaf: { path: string[] }) {
    return nodeSlotMap.get(`${nodeId}:${leaf.path.join('/')}`);
  }
</script>

{#if collapsible}
  <!-- Replicated group — collapsible accordion (collapsed by default) -->
  <div class="subgroup-accordion" style="--depth: {depth}">
    <button
      class="subgroup-header"
      class:expanded
      on:click={() => (expanded = !expanded)}
      aria-expanded={expanded}
    >
      <span class="expand-icon" aria-hidden="true">{expanded ? '▾' : '▸'}</span>
      <span class="subgroup-name">{group.instanceLabel}</span>
    </button>

    {#if expanded}
      <div class="subgroup-body">
        {#if group.description}
          <p class="subgroup-description">{group.description}</p>
        {/if}

        {#each group.children as child (child.kind === 'group' ? child.path.join('/') : child.path.join('/'))}
          {#if isLeaf(child)}
            <TreeLeafRow leaf={child} usedIn={getUsedIn(child)} />
          {:else if isGroup(child)}
            <svelte:self
              group={child}
              {nodeId}
              depth={depth + 1}
              collapsible={child.replicationCount > 1}
            />
          {/if}
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <!-- Non-replicated group — inline section, always visible -->
  <div class="inline-section" style="--depth: {depth}">
    <div class="inline-header">
      <span class="inline-name">{group.instanceLabel}</span>
      {#if group.description}
        <p class="subgroup-description">{group.description}</p>
      {/if}
    </div>

    {#each group.children as child (child.kind === 'group' ? child.path.join('/') : child.path.join('/'))}
      {#if isLeaf(child)}
        <TreeLeafRow leaf={child} usedIn={getUsedIn(child)} />
      {:else if isGroup(child)}
        <svelte:self
          group={child}
          {nodeId}
          depth={depth + 1}
          collapsible={child.replicationCount > 1}
        />
      {/if}
    {/each}
  </div>
{/if}

<style>
  /* ── Accordion (collapsible / replicated groups) ── */
  .subgroup-accordion {
    margin-top: 4px;
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    overflow: hidden;
  }

  .subgroup-header {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 7px 12px;
    background: var(--subgroup-header-bg, #f5f5f5);
    border: none;
    cursor: pointer;
    text-align: left;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #333);
    transition: background-color 0.1s;
  }

  .subgroup-header:hover {
    background-color: var(--hover-bg, #ebebeb);
  }

  .subgroup-header.expanded {
    border-bottom: 1px solid var(--border-color, #e0e0e0);
  }

  .expand-icon {
    flex-shrink: 0;
    font-size: 22px;
    color: var(--text-secondary, #666);
    width: 18px;
    line-height: 1;
  }

  .subgroup-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .subgroup-body {
    padding: 0 12px 8px;
    background: var(--card-bg, #fff);
  }

  /* ── Inline section (non-replicated groups) ── */
  .inline-section {
    margin-top: 8px;
    padding-left: calc(var(--depth, 0) * 8px);
  }

  .inline-header {
    margin-bottom: 4px;
  }

  .inline-name {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary, #555);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  /* ── Shared ── */
  .subgroup-description {
    margin: 3px 0 4px;
    font-size: 11px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
  }
</style>
