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
   * Collapse a group that has exactly one visible event-ID leaf (after applying
   * roleFilter + query) into a single "Group.Leaf" slot button, mirroring
   * VS Code's compact-folders behaviour.
   *
   * This intentionally ignores non-eventId siblings (string/int config leaves)
   * in the raw CDI tree — it counts only the leaves that would actually be
   * rendered for the current filter state.
   *
   * Returns { combinedLabel, terminal } where:
   *   - terminal === node           → render as normal expandable group
   *   - isLeaf(terminal)            → render as collapsed slot button;
   *                                    combinedLabel already includes the leaf name
   *   - isGroup(terminal) ≠ node   → chain of single-child groups (legacy path)
   */
  export function collapseGroupChain(
    node: GroupConfigNode,
    query: string = '',
    roleFilter: EventRole | null = null,
    nodeName: string = '',
  ): { combinedLabel: string; terminal: ConfigNode } {
    const label = pickerGroupLabel(node);

    // Count visible event-ID leaves (respecting role filter + search query)
    const visibleLeaves = findVisibleEventIdLeaves(node.children, query, roleFilter, nodeName);
    if (visibleLeaves.length === 1 && isLeaf(visibleLeaves[0])) {
      const leaf = visibleLeaves[0];
      // Build full label: outerGroupName[.intermediateGroups].leafName
      const labelParts: string[] = [label];
      appendGroupNamesToLeaf(node.children, leaf.address, labelParts);
      labelParts.push(leaf.name);
      return { combinedLabel: labelParts.join('.'), terminal: leaf };
    }

    // Legacy: chain of single raw-child groups (no eventId siblings)
    if (node.children.length === 1 && isGroup(node.children[0])) {
      const child = node.children[0] as GroupConfigNode;
      const r = collapseGroupChain(child, query, roleFilter, nodeName);
      return { combinedLabel: `${label}.${r.combinedLabel}`, terminal: r.terminal };
    }

    return { combinedLabel: label, terminal: node };
  }

  /** Collect all event-ID leaf nodes visible under children for the given filter state. */
  function findVisibleEventIdLeaves(
    children: ConfigNode[],
    query: string,
    roleFilter: EventRole | null,
    nodeName: string,
  ): ConfigNode[] {
    const results: ConfigNode[] = [];
    for (const child of children) {
      if (isLeaf(child) && child.elementType === 'eventId') {
        const matchesRole =
          roleFilter === null ||
          child.eventRole === roleFilter ||
          child.eventRole === 'Ambiguous' ||
          child.eventRole === null;
        if (!matchesRole) continue;
        if (
          query === '' ||
          child.name.toLowerCase().includes(query) ||
          (child.description ?? '').toLowerCase().includes(query) ||
          child.path.join('/').toLowerCase().includes(query) ||
          nodeName.toLowerCase().includes(query)
        ) {
          results.push(child);
        }
      } else if (isGroup(child)) {
        results.push(...findVisibleEventIdLeaves(child.children, query, roleFilter, nodeName));
      }
    }
    return results;
  }

  /**
   * Walk children searching for a leaf at `address`, appending intermediate
   * group labels to `labelParts` along the way (backtracking if not found).
   */
  function appendGroupNamesToLeaf(
    children: ConfigNode[],
    address: number,
    labelParts: string[],
  ): boolean {
    for (const child of children) {
      if (isLeaf(child) && child.address === address) return true;
      if (isGroup(child)) {
        labelParts.push(pickerGroupLabel(child));
        if (appendGroupNamesToLeaf(child.children, address, labelParts)) return true;
        labelParts.pop();
      }
    }
    return false;
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
  {@const { combinedLabel, terminal } = collapseGroupChain(node, q, roleFilter, nodeName)}
  {@const groupNameMatches = q !== '' && combinedLabel.toLowerCase().includes(q)}
  {@const hasMatch =
    groupNameMatches ||
    hasMatchingDescendant(node.children, q, roleFilter, nodeName)}
  {#if hasMatch}
    {@const childQuery = groupNameMatches ? '' : searchQuery}
    {@const key = `${pathKey}:${node.path.join('/')}`}

    {#if terminal !== (node as ConfigNode) && isLeaf(terminal) && terminal.elementType === 'eventId'}
      <!-- Collapsed to a single eventId leaf: combinedLabel already includes the leaf name -->
      {@const leafLabel = combinedLabel}
      {@const leafQ = childQuery.toLowerCase().trim()}
      {@const matchesRole =
        roleFilter === null ||
        terminal.eventRole === roleFilter ||
        terminal.eventRole === 'Ambiguous' ||
        terminal.eventRole === null}
      {@const matchesSearch =
        leafQ === '' ||
        leafLabel.toLowerCase().includes(leafQ) ||
        terminal.name.toLowerCase().includes(leafQ) ||
        (terminal.description ?? '').toLowerCase().includes(leafQ) ||
        nodeName.toLowerCase().includes(leafQ)}
      {#if matchesRole && matchesSearch}
        {@const isFree = isSlotFree(terminal)}
        {@const selected = isSelected(terminal)}
        <button
          class="tree-slot"
          style="padding-left: {4 + depth * 16}px"
          class:selected
          class:unavailable={!isFree}
          disabled={!isFree}
          onclick={() => onSelect(terminal, nodeId)}
          title={isFree ? `Select ${leafLabel}` : 'Slot already in use'}
        >
          <span
            class="role-icon"
            class:role-producer={terminal.eventRole === 'Producer'}
            class:role-consumer={terminal.eventRole === 'Consumer'}
            class:role-ambiguous={terminal.eventRole === 'Ambiguous' || terminal.eventRole === null}
          >
            {terminal.eventRole === 'Producer' ? '▲' : terminal.eventRole === 'Consumer' ? '▼' : '?'}
          </span>
          <span class="slot-name">{leafLabel}</span>
          {#if !isFree}
            <span class="slot-used" aria-label="In use">(in use)</span>
          {/if}
        </button>
      {/if}
    {:else}
      <!-- Normal group or collapsed multi-child group -->
      {@const terminalChildren = isGroup(terminal) && terminal !== (node as ConfigNode) ? terminal.children : node.children}
      {@const expanded = expandedNodes.has(key)}
      <div class="tree-group" role="treeitem" aria-expanded={expanded} aria-selected={false}>
        <button
          class="tree-toggle"
          style="padding-left: {4 + depth * 16}px"
          onclick={() => onToggle(key)}
        >
          <span class="toggle-icon">{expanded ? '▾' : '▸'}</span>
          <span class="group-label">{combinedLabel}</span>
        </button>
        {#if expanded}
          {#each terminalChildren as child (child.path.join('/'))}
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
