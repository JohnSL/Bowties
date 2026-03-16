<!--
  PickerTreeNode.svelte — Recursive tree node for ElementPicker.

  Renders a single ConfigNode from the unified NodeConfigTree:
  - GroupConfigNode  → expand/collapse toggle with displayName ?? name;
                       children rendered recursively via <svelte:self>
  - LeafConfigNode (eventId) → selectable slot button with ▲/▼/? role badge
  - Other leaf types → not rendered

  Visibility rules:
  - roleFilter filters out leaves whose eventRole doesn't match (null/Ambiguous
    leaves always pass, satisfying T035 until classification is applied)
  - When searchQuery is non-empty, a group only renders if it has a matching
    descendant (or its own label contains the query)
  - When a group's label matches the query, all role-matching descendants are
    shown (childQuery is forced to '' for recursive calls)

  Phase 1+3 fix: eventRole === null is treated as Ambiguous — unclassified
  slots are shown in both pickers with a '?' badge.
-->

<script module lang="ts">
  import type { ConfigNode, EventRole, GroupConfigNode } from '$lib/types/nodeTree';
  import { isGroup, isLeaf, getInstanceDisplayName } from '$lib/types/nodeTree';

  /** Single source of truth for how a group's label is displayed in the picker. */
  export function pickerGroupLabel(group: GroupConfigNode): string {
    return group.displayName ?? getInstanceDisplayName(group);
  }

  /**
   * Returns true if `children` contain any event-ID leaf matching `roleFilter`
   * and `query` (already lowercased and trimmed).
   *
   * Group-label search: when a group's displayName/name contains `query`, all
   * role-matching descendants of that group are considered matches (query is
   * treated as '' for descendants).
   */
  export function hasMatchingDescendant(
    children: ConfigNode[],
    query: string,
    roleFilter: EventRole | null,
    nodeName: string = '',
  ): boolean {
    for (const child of children) {
      if (isLeaf(child) && child.elementType === 'eventId') {
        const matchesRole =
          roleFilter === null ||
          child.eventRole === roleFilter ||
          child.eventRole === 'Ambiguous' ||
          child.eventRole === null;
        if (!matchesRole) continue;
        if (query === '') return true;
        if (child.name.toLowerCase().includes(query)) return true;
        if ((child.description ?? '').toLowerCase().includes(query)) return true;
        if (child.path.join('/').toLowerCase().includes(query)) return true;
        if (nodeName && nodeName.toLowerCase().includes(query)) return true;
      } else if (isGroup(child)) {
        const groupLabel = pickerGroupLabel(child).toLowerCase();
        if (query !== '' && groupLabel.includes(query)) {
          // Group label matches — show any role-matching descendant
          if (hasMatchingDescendant(child.children, '', roleFilter, nodeName)) return true;
        } else {
          if (hasMatchingDescendant(child.children, query, roleFilter, nodeName)) return true;
        }
      }
    }
    return false;
  }
</script>

<script lang="ts">
  import type { LeafConfigNode } from '$lib/types/nodeTree';
  import PickerTreeNode from './PickerTreeNode.svelte';

  interface Props {
    /** The config tree node to render. */
    node: ConfigNode;
    /** Tree depth — drives inline indentation (depth 2 = first child of segment). */
    depth: number;
    roleFilter: EventRole | null;
    /**
     * Raw search query (not lowercased).
     * Parent passes '' when its own label matched, so all descendants show.
     */
    searchQuery: string;
    /** Unique path prefix used to build expand/collapse keys. */
    pathKey: string;
    expandedNodes: Set<string>;
    onToggle: (key: string) => void;
    onSelect: (leaf: LeafConfigNode, nodeId: string) => void;
    isSlotFree: (leaf: LeafConfigNode) => boolean;
    isSelected: (leaf: LeafConfigNode) => boolean;
    nodeId: string;
    nodeName: string;
  }

  let {
    node,
    depth,
    roleFilter,
    searchQuery,
    pathKey,
    expandedNodes,
    onToggle,
    onSelect,
    isSlotFree,
    isSelected,
    nodeId,
    nodeName,
  }: Props = $props();
</script>

{#if isLeaf(node)}
  {#if node.elementType === 'eventId'}
    {@const q = searchQuery.toLowerCase().trim()}
    {@const matchesRole =
      roleFilter === null ||
      node.eventRole === roleFilter ||
      node.eventRole === 'Ambiguous' ||
      node.eventRole === null}
    {@const matchesSearch =
      q === '' ||
      node.name.toLowerCase().includes(q) ||
      (node.description ?? '').toLowerCase().includes(q) ||
      node.path.join('/').toLowerCase().includes(q) ||
      nodeName.toLowerCase().includes(q)}
    {#if matchesRole && matchesSearch}
      {@const isFree = isSlotFree(node)}
      {@const selected = isSelected(node)}
      <button
        class="tree-slot"
        style="padding-left: {4 + depth * 16}px"
        class:selected
        class:unavailable={!isFree}
        disabled={!isFree}
        onclick={() => onSelect(node, nodeId)}
        title={isFree ? `Select ${node.name}` : 'Slot already in use'}
      >
        <span
          class="role-icon"
          class:role-producer={node.eventRole === 'Producer'}
          class:role-consumer={node.eventRole === 'Consumer'}
          class:role-ambiguous={node.eventRole === 'Ambiguous' || node.eventRole === null}
        >
          {node.eventRole === 'Producer' ? '▲' : node.eventRole === 'Consumer' ? '▼' : '?'}
        </span>
        <span class="slot-name">{node.name}</span>
        {#if !isFree}
          <span class="slot-used" aria-label="In use">(in use)</span>
        {/if}
      </button>
    {/if}
  {/if}
{:else if isGroup(node)}
  {@const q = searchQuery.toLowerCase().trim()}
  {@const groupLabel = pickerGroupLabel(node)}
  {@const groupNameMatches = q !== '' && groupLabel.toLowerCase().includes(q)}
  {@const hasMatch =
    groupNameMatches ||
    hasMatchingDescendant(node.children, q, roleFilter, nodeName)}
  {#if hasMatch}
    {@const childQuery = groupNameMatches ? '' : searchQuery}
    {@const key = `${pathKey}:${node.path.join('/')}`}
    {@const expanded = expandedNodes.has(key)}
    <div class="tree-group" role="treeitem" aria-expanded={expanded} aria-selected={false}>
      <button
        class="tree-toggle"
        style="padding-left: {4 + depth * 16}px"
        onclick={() => onToggle(key)}
      >
        <span class="toggle-icon">{expanded ? '▾' : '▸'}</span>
        <span class="group-label">{groupLabel}</span>
      </button>
      {#if expanded}
        {#each node.children as child (child.path.join('/'))}
          <PickerTreeNode
            node={child}
            depth={depth + 1}
            {roleFilter}
            searchQuery={childQuery}
            pathKey={key}
            {expandedNodes}
            {onToggle}
            {onSelect}
            {isSlotFree}
            {isSelected}
            {nodeId}
            {nodeName}
          />
        {/each}
      {/if}
    </div>
  {/if}
{/if}

<style>
  .tree-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    padding: 4px 8px;
    background: none;
    border: none;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.82rem;
    text-align: left;
    color: #374151;
  }

  .tree-toggle:hover {
    background: #f3f4f6;
  }

  .toggle-icon {
    flex-shrink: 0;
    width: 12px;
    font-size: 0.7rem;
    color: #9ca3af;
  }

  .group-label {
    color: #4b5563;
  }

  .tree-slot {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 3px 8px;
    background: none;
    border: none;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.8rem;
    text-align: left;
    color: #374151;
    transition: background 0.1s;
  }

  .tree-slot:hover:not(:disabled) {
    background: #eff6ff;
  }

  .tree-slot.selected {
    background: #dbeafe;
    font-weight: 500;
  }

  .tree-slot.unavailable {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .role-icon {
    flex-shrink: 0;
    width: 14px;
    height: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.6rem;
    border-radius: 50%;
    font-weight: 700;
  }

  .role-producer { color: #0b6a0b; background: #dff6dd; }
  .role-consumer { color: #0078d4; background: #deecf9; }
  .role-ambiguous { color: #ca5010; background: #fff4e6; }

  .slot-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .slot-used {
    font-size: 0.72rem;
    color: #9ca3af;
    font-style: italic;
    flex-shrink: 0;
  }
</style>
