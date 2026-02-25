<script lang="ts">
  /**
   * TreeGroupAccordion — renders a GroupConfigNode from the unified tree.
   *
   * Two modes (no accordion / collapse controls):
   * 1. **Replicated set with PillSelector** — when `siblings` provided with 2+ instances,
   *    renders a section label + pill on the left to navigate instances.
   *    Children of the selected instance are always visible.
   * 2. **Inline section** — non-replicated groups show a subtle label header,
   *    children always visible.
   *
   * Recursively renders children using `<svelte:self>` for nested groups and
   * TreeLeafRow for leaves. Groups replicated children via `groupReplicatedChildren`.
   *
   * Spec: plan-cdiConfigNavigator.
   */
  import type { GroupConfigNode, ConfigNode, GroupedChild } from '$lib/types/nodeTree';
  import { isGroup, isLeaf, groupReplicatedChildren, getInstanceDisplayName } from '$lib/types/nodeTree';
  import type { PillItem } from '$lib/components/PillSelector/PillSelector.svelte';
  import PillSelector from '$lib/components/PillSelector/PillSelector.svelte';
  import TreeLeafRow from './TreeLeafRow.svelte';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

  /** The group node from the unified tree */
  export let group: GroupConfigNode;
  /** Node ID for cross-reference lookups */
  export let nodeId: string;
  /** Current nesting depth — used to indent deeper levels */
  export let depth: number = 0;
  /**
   * Sibling replicated instances for pill-selector mode.
   * When provided with 2+ items, PillSelector replaces per-instance accordions.
   */
  export let siblings: GroupConfigNode[] = [];

  // ── Pill-selector state ──
  let selectedInstanceIndex = 0;

  $: pillMode = siblings.length > 1;
  $: activeGroup = pillMode ? (siblings[selectedInstanceIndex] ?? siblings[0]) : group;

  // Build pill items from siblings
  $: pillItems = siblings.map((s, idx): PillItem => ({
    value: idx,
    label: getInstanceDisplayName(s),
    description: s.instanceLabel,
  }));

  // Group children for the active group to handle nested replications
  $: groupedChildren = groupReplicatedChildren(activeGroup.children);

  // Cross-reference lookup: nodeId + CDI path → BowtieCard
  $: nodeSlotMap = bowtieCatalogStore.nodeSlotMap;

  /** Get the BowtieCard cross-reference for a leaf's path */
  function getUsedIn(leaf: { path: string[] }) {
    return nodeSlotMap.get(`${nodeId}:${leaf.path.join('/')}`);
  }

  function handlePillSelect(value: number) {
    selectedInstanceIndex = value;
  }
</script>

{#if pillMode}
  <!-- Replicated group with pill selector — label + pill on left, always expanded -->
  <div class="pill-section" style="--depth: {depth}; --field-label-width: {depth >= 3 ? '100px' : '120px'}">
    <div class="pill-section-header">
      <span class="pill-section-name">{group.replicationOf}</span>
      <PillSelector
        items={pillItems}
        selected={selectedInstanceIndex}
        onSelect={handlePillSelect}
      />
    </div>

    <div class="pill-section-body" style="--gap: {depth >= 3 ? '4px' : '8px'}">
      {#if activeGroup.description}
        <p class="section-description">{activeGroup.description}</p>
      {/if}

      {#each groupedChildren as item}
        {#if item.type === 'leaf'}
          <TreeLeafRow leaf={item.node} usedIn={getUsedIn(item.node)} {depth} />
        {:else if item.type === 'group'}
          <svelte:self
            group={item.node}
            {nodeId}
            depth={depth + 1}
          />
        {:else if item.type === 'replicatedSet'}
          <svelte:self
            group={item.instances[0]}
            {nodeId}
            depth={depth + 1}
            siblings={item.instances}
          />
        {/if}
      {/each}
    </div>
  </div>

{:else}
  <!-- Non-replicated group — subtle label header, always visible -->
  <div class="inline-section" style="--depth: {depth}; --field-label-width: {depth >= 3 ? '100px' : '120px'}">
    <div class="inline-header">
      <span class="inline-name">{group.instanceLabel}</span>
      {#if group.description}
        <p class="section-description">{group.description}</p>
      {/if}
    </div>

    {#each groupedChildren as item}
      {#if item.type === 'leaf'}
        <TreeLeafRow leaf={item.node} usedIn={getUsedIn(item.node)} {depth} />
      {:else if item.type === 'group'}
        <svelte:self
          group={item.node}
          {nodeId}
          depth={depth + 1}
        />
      {:else if item.type === 'replicatedSet'}
        <svelte:self
          group={item.instances[0]}
          {nodeId}
          depth={depth + 1}
          siblings={item.instances}
        />
      {/if}
    {/each}
  </div>
{/if}

<style>
  /* ══════════════════════════════════════════
     Fluent UI Design — TreeGroupAccordion
     ══════════════════════════════════════════ */

  /* ── Pill-section (replicated group with pill selector) ── */
  .pill-section {
    margin-top: 8px;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  .pill-section-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0 4px;
    border-bottom: 1px solid #e1dfdd;              /* colorNeutralStroke2 */
  }

  .pill-section-name {
    font-size: 12px;
    font-weight: 600;
    color: #605e5c;                                /* colorNeutralForeground2 */
    text-transform: uppercase;
    letter-spacing: 0.04em;
    white-space: nowrap;
  }

  .pill-section-body {
    padding: 4px 0 6px;
    display: flex;
    flex-direction: column;
    gap: var(--gap, 6px);
  }

  /* ── Inline section (non-replicated groups) ── */
  .inline-section {
    margin-top: 8px;
    padding-left: calc(var(--depth, 0) * 12px);
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  .inline-header {
    margin-bottom: 4px;
    border-bottom: 1px solid #e1dfdd;              /* colorNeutralStroke2 */
    padding: 6px 0 4px;
  }

  .inline-name {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: #605e5c;                                /* colorNeutralForeground2 */
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  /* ── Shared ── */
  .section-description {
    margin: 3px 0 4px;
    font-size: 12px;
    color: #605e5c;                                /* colorNeutralForeground2 */
    line-height: 1.5;
    font-weight: 400;
  }
</style>
