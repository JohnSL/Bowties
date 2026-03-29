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
  import PillSelector from '$lib/components/PillSelector/PillSelector.svelte';

  interface PillItem { value: number; label: string; description?: string; }
  import TreeLeafRow from './TreeLeafRow.svelte';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { hasModifiedDescendant } from '$lib/types/nodeTree';
  import { pillSelections, setPillSelection, makePillKey } from '$lib/stores/pillSelection';

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
  /** Whether the parent node is offline — disables all inputs (FR-007, T050) */
  export let isNodeOffline: boolean = false;
  /** Segment origin address — forwarded to TreeLeafRow for save context */
  export let segmentOrigin: number = 0;
  /** Segment display name — forwarded to TreeLeafRow for progress labels */
  export let segmentName: string = '';

  // ── Pill-selector state ──
  // Stable key for this replicated set — persisted in pillSelections store so the
  // selected instance survives view switches (e.g. Bowties view ↔ config view).
  $: pillKey = siblings.length > 1 ? makePillKey(nodeId, siblings[0]) : '';
  $: selectedInstanceIndex = pillKey ? ($pillSelections.get(pillKey) ?? 0) : 0;

  $: pillMode = siblings.length > 1;
  $: activeGroup = pillMode ? (siblings[selectedInstanceIndex] ?? siblings[0]) : group;

  // Build pill items from siblings
  $: pillItems = siblings.map((s, idx): PillItem => ({
    value: idx,
    label: s.displayName ?? getInstanceDisplayName(s),
    description: s.instanceLabel,
  }));

  // ── Dirty-instance tracking ──
  // Check which sibling instances have modified descendant leaves.

  /**
   * Returns the set of sibling indices that have at least one modified leaf,
   * determined by checking the tree's modifiedValue on descendant leaves.
   */
  function computeDirtyInstances(sibs: GroupConfigNode[]): Set<number> {
    const result = new Set<number>();
    if (sibs.length < 2) return result;
    for (let i = 0; i < sibs.length; i++) {
      if (hasModifiedDescendant(sibs[i].children, [])) {
        result.add(i);
      }
    }
    return result;
  }

  $: dirtyInstances = computeDirtyInstances(siblings);
  $: hasDirtyInstances = dirtyInstances.size > 0;

  // ── Hideable group state ──
  // Tracks collapsed state for groups with hideable hint.
  let collapsed = group.hiddenByDefault ?? false;

  // Propagate readOnly down the tree: if this group is read-only, disable all inputs.
  $: isEffectivelyReadOnly = isNodeOffline || !!(group.readOnly);

  // Group children for the active group to handle nested replications
  $: groupedChildren = groupReplicatedChildren(activeGroup.children);

  // Cross-reference lookup: nodeId + CDI path → BowtieCard
  $: nodeSlotMap = bowtieCatalogStore.effectiveNodeSlotMap;

  /** Get the BowtieCard cross-reference for a leaf's path */
  function getUsedIn(leaf: { path: string[] }) {
    return nodeSlotMap.get(`${nodeId}:${leaf.path.join('/')}`);
  }

  function handlePillSelect(value: number) {
    if (pillKey) setPillSelection(pillKey, value);
  }
</script>

{#if pillMode}
  <!-- Replicated group with pill selector — label + pill on left, always expanded -->
  <div class="pill-section" style="--depth: {depth}; --field-label-width: {depth >= 3 ? '100px' : '120px'}">
    <div class="pill-section-header">
      <span class="pill-section-name" class:pill-section-name--dirty={hasDirtyInstances}>{group.replicationOf}</span>
      <PillSelector
        items={pillItems}
        selected={selectedInstanceIndex}
        onSelect={handlePillSelect}
        dirtyValues={dirtyInstances}
      />
    </div>

    <div class="pill-section-body" style="--gap: {depth >= 3 ? '4px' : '8px'}">
      {#if activeGroup.description}
        <p class="section-description">{activeGroup.description}</p>
      {/if}

      {#each groupedChildren as item}
        {#if item.type === 'leaf'}
          <TreeLeafRow leaf={item.node} usedIn={getUsedIn(item.node)} {depth} {nodeId} {segmentOrigin} {segmentName} {isNodeOffline} />
        {:else if item.type === 'group'}
          <svelte:self
            group={item.node}
            {nodeId}
            depth={depth + 1}
            {segmentOrigin}
            {segmentName}
            {isNodeOffline}
          />
        {:else if item.type === 'replicatedSet'}
          <svelte:self
            group={item.instances[0]}
            {nodeId}
            depth={depth + 1}
            siblings={item.instances}
            {segmentOrigin}
            {segmentName}
            {isNodeOffline}
          />
        {/if}
      {/each}
    </div>
  </div>

{:else}
  <!-- Non-replicated group — subtle label header, always visible -->
  <div class="inline-section" style="--depth: {depth}; --field-label-width: {depth >= 3 ? '100px' : '120px'}">
    <div class="inline-header">
      {#if group.hideable}
        <button
          class="inline-name inline-toggle-btn"
          aria-expanded={!collapsed}
          onclick={() => { collapsed = !collapsed; }}
        >
          <span class="toggle-arrow" class:collapsed>{collapsed ? '▶' : '▼'}</span>
          {group.displayName ?? group.instanceLabel}
        </button>
      {:else}
        <span class="inline-name">{group.displayName ?? group.instanceLabel}</span>
      {/if}
      {#if group.description}
        <p class="section-description">{group.description}</p>
      {/if}
    </div>

    {#if !collapsed}
      {#each groupedChildren as item}
        {#if item.type === 'leaf'}
          <TreeLeafRow leaf={item.node} usedIn={getUsedIn(item.node)} {depth} {nodeId} {segmentOrigin} {segmentName} isNodeOffline={isEffectivelyReadOnly} />
        {:else if item.type === 'group'}
          <svelte:self
            group={item.node}
            {nodeId}
            depth={depth + 1}
            {segmentOrigin}
            {segmentName}
            isNodeOffline={isEffectivelyReadOnly}
          />
        {:else if item.type === 'replicatedSet'}
          <svelte:self
            group={item.instances[0]}
            {nodeId}
            depth={depth + 1}
            siblings={item.instances}
            {segmentOrigin}
            {segmentName}
            isNodeOffline={isEffectivelyReadOnly}
          />
        {/if}
      {/each}
    {/if}
  </div>
{/if}

<style>
  /* ══════════════════════════════════════════
     Fluent UI Design — TreeGroupAccordion
     ══════════════════════════════════════════ */

  /* ── Pill-section (replicated group with pill selector) ── */
  .pill-section {
    margin-top: 6px;
    padding-top: 10px;
    border-top: 1px solid #e1dfdd;                 /* subtle section divider */
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  /* No divider above the very first section in its parent */
  .pill-section:first-child {
    border-top: none;
    padding-top: 0;
  }

  .pill-section-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 0 4px;
  }

  .pill-section-name {
    font-size: 13px;
    font-weight: 600;
    color: #323130;                                /* colorNeutralForeground1 — warmer */
    white-space: nowrap;
    position: relative;
    padding-left: 0;
    transition: padding-left 0.1s ease;
  }

  /* Amber vertical bar — indicates unsaved changes exist in this replicated group */
  .pill-section-name--dirty {
    padding-left: 9px;
  }

  .pill-section-name--dirty::before {
    content: '';
    position: absolute;
    left: 0;
    top: 1px;
    bottom: 1px;
    width: 3px;
    border-radius: 1.5px;
    background: #ca8500;                           /* amber — matches sidebar pending-edits dot */
  }

  .pill-section-body {
    padding: 4px 4px 6px;
    display: flex;
    flex-direction: column;
    gap: var(--gap, 4px);
    background: #f8f8f7;                           /* soft grouping background */
    border-radius: 6px;
    margin-top: 2px;
  }

  /* ── Inline section (non-replicated groups) ── */
  .inline-section {
    margin-top: 6px;
    padding-top: 10px;
    border-top: 1px solid #e1dfdd;                 /* subtle section divider */
    padding-left: calc(var(--depth, 0) * 8px);
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  .inline-section:first-child {
    border-top: none;
    padding-top: 0;
  }

  .inline-header {
    margin-bottom: 4px;
    padding: 4px 0 2px;
  }

  .inline-name {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: #323130;                                /* colorNeutralForeground1 */
  }

  .inline-toggle-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    color: #323130;
    text-align: left;
  }

  .inline-toggle-btn:hover {
    color: #0078d4;
  }

  .toggle-arrow {
    font-size: 10px;
    transition: transform 0.15s;
  }

  .toggle-arrow.collapsed {
    transform: rotate(-90deg);
  }

  /* ── Shared ── */
  .section-description {
    margin: 2px 0 2px;
    font-size: 12px;
    color: #605e5c;                                /* colorNeutralForeground2 */
    line-height: 1.5;
    font-weight: 400;
    white-space: pre-wrap;                         /* preserve newlines from CDI descriptions */
  }
</style>
