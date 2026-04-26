<!--
  T024: ElementPicker.svelte — Browsable tree of nodes → segments → groups → event slots.

  Used in the NewConnectionDialog to select producer/consumer elements.
  Filters by role (producer/consumer), supports text search,
  and grays out elements with no free slots (FR-012).

  Props:
    roleFilter: 'Producer' | 'Consumer' | null — filter elements by classified role
    onSelect: callback when an element is selected
    selectedElement: currently selected element (for highlighting)
-->

<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { editableBowtiePreviewStore } from '$lib/stores/bowties.svelte';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { nodeInfoStore } from '$lib/stores/nodeInfo';
  import { isPlaceholderEventId } from '$lib/utils/eventIds';
  import { resolveNodeDisplayName } from '$lib/utils/nodeDisplayName';
  import {
    type SegmentNode,
    type ConfigNode,
    type LeafConfigNode,
    buildElementLabel,
    isGroup,
    isLeaf,
  } from '$lib/types/nodeTree';
  import type { ElementSelection } from '$lib/types/bowtie';
  import type { EventRole } from '$lib/types/nodeTree';
  import PickerTreeNode, { hasMatchingDescendant } from './PickerTreeNode.svelte';
  import RoleClassifyPrompt from './RoleClassifyPrompt.svelte';

  interface Props {
    /** Filter elements by event role. null = show all. */
    roleFilter?: EventRole | null;
    /** Callback when user selects an element. */
    onSelect?: (selection: ElementSelection) => void;
    /** Currently selected element (for highlight). */
    selectedElement?: ElementSelection | null;
  }

  let {
    roleFilter = null,
    onSelect,
    selectedElement = null,
  }: Props = $props();

  let searchQuery = $state('');
  let expandedNodes = $state<Set<string>>(new Set());

  // T035: pending ambiguous leaf waiting for role classification
  let pendingAmbiguous = $state<{ leaf: LeafConfigNode; nodeId: string } | null>(null);

  // Get all node trees
  let trees = $derived(nodeTreeStore.trees);

  // Build set of connected event IDs from the editable preview so offline layouts
  // can still reserve slots that already participate in saved bowties.
  let connectedEventIds = $derived.by(() => {
    const ids = new Set<string>();
    for (const bowtie of editableBowtiePreviewStore.preview.bowties) {
      ids.add(bowtie.eventIdHex);
    }
    return ids;
  });

  // Load (or refresh) trees for all discovered nodes on mount.
  // Always refresh even for already-loaded trees: after CDI reads complete, profiles
  // are applied server-side but the frontend copy may be pre-profile (event roles
  // show as '?' and element names are missing).  loadTree() only skips nodes that
  // are currently mid-fetch, so this is safe and deduplicated automatically.
  onMount(() => {
    const nodes = get(nodeInfoStore);
    for (const nodeId of nodes.keys()) {
      nodeTreeStore.loadTree(nodeId);
    }
  });

  /** Get display name for a node using SNIP data from nodeInfoStore. */
  function getNodeDisplayName(nodeId: string): string {
    const nodes = get(nodeInfoStore);
    return resolveNodeDisplayName(nodeId, nodes.get(nodeId));
  }

  /** Build the picker tree data directly from NodeConfigTree — no flattening. */
  let pickerTreeData = $derived.by(() => {
    const result: Array<{ nodeId: string; nodeName: string; segments: SegmentNode[] }> = [];
    const query = searchQuery.toLowerCase().trim();

    for (const [nodeId, tree] of trees) {
      const nodeName = getNodeDisplayName(nodeId);
      const segments = tree.segments.filter(seg =>
        hasMatchingDescendant(seg.children, query, roleFilter, nodeName)
      );
      if (segments.length > 0) {
        result.push({ nodeId, nodeName, segments });
      }
    }

    return result;
  });

  /**
   * Check if a slot is free (not participating in an existing bowtie connection).
   * A slot is free if its event ID does not appear in any bowtie in the catalog.
   * Slots with no value or all-zero event IDs are also free.
   */
  function isSlotFree(leaf: LeafConfigNode): boolean {
    if (!leaf.value || leaf.value.type !== 'eventId') return true;
    const hex = leaf.value.hex;
    // Placeholder (leading-zero) event IDs can never be connected
    if (isPlaceholderEventId(hex)) return false;
    // Free if this event ID is not part of any existing bowtie connection
    return !connectedEventIds.has(hex);
  }

  function toggleNode(key: string): void {
    const next = new Set(expandedNodes);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    expandedNodes = next;
  }

  function handleSelect(leaf: LeafConfigNode, nodeId: string): void {
    if (!isSlotFree(leaf)) return; // FR-012: can't select occupied slots

    // Phase 3: when the picker has a definite role filter and the slot is
    // ambiguous/unclassified, auto-classify it and skip the prompt entirely.
    if ((leaf.eventRole === 'Ambiguous' || leaf.eventRole === null) &&
        (roleFilter === 'Producer' || roleFilter === 'Consumer')) {
      const key = `${nodeId}:${leaf.path.join('/')}`;
      bowtieMetadataStore.classifyRole(key, roleFilter);
      doSelect(leaf, nodeId);
      return;
    }

    // T035: intercept ambiguous/null role slots — ask user to classify first
    if (leaf.eventRole === 'Ambiguous' || leaf.eventRole === null) {
      pendingAmbiguous = { leaf, nodeId };
      return;
    }

    doSelect(leaf, nodeId);
  }

  function doSelect(leaf: LeafConfigNode, nodeId: string): void {
    const tree = nodeTreeStore.getTree(nodeId);
    const selection: ElementSelection = {
      nodeId,
      nodeName: getNodeDisplayName(nodeId),
      elementPath: leaf.path,
      elementLabel: tree ? buildElementLabel(tree, leaf) : leaf.name,
      address: leaf.address,
      space: leaf.space,
      currentEventId: leaf.value?.type === 'eventId' ? leaf.value.hex : '00.00.00.00.00.00.00.00',
    };
    onSelect?.(selection);
  }

  function handleAmbiguousClassify(role: 'Producer' | 'Consumer'): void {
    if (!pendingAmbiguous) return;
    const { leaf, nodeId } = pendingAmbiguous;

    // Persist the classification
    const key = `${nodeId}:${leaf.path.join('/')}`;
    bowtieMetadataStore.classifyRole(key, role);

    // Forward the selection
    doSelect(leaf, nodeId);
    pendingAmbiguous = null;
  }

  function isSelectedLeaf(leaf: LeafConfigNode, nodeId: string): boolean {
    if (!selectedElement) return false;
    return (
      selectedElement.nodeId === nodeId &&
      selectedElement.address === leaf.address &&
      selectedElement.space === leaf.space
    );
  }
</script>

<div class="element-picker">
  <!-- Search bar -->
  <div class="picker-search">
    <input
      type="text"
      placeholder="Search elements…"
      bind:value={searchQuery}
      class="search-input"
      aria-label="Search event slots"
    />
    {#if searchQuery}
      <button class="search-clear" onclick={() => { searchQuery = ''; }} aria-label="Clear search">✕</button>
    {/if}
  </div>

  <!-- Tree -->
  <div class="picker-tree" role="tree" aria-label="Event slot picker">
    {#if pickerTreeData.length === 0}
      <div class="picker-empty">
        {#if searchQuery}
          No matching event slots found.
        {:else}
          No nodes with event slots available.
        {/if}
      </div>
    {/if}

    {#each pickerTreeData as node (node.nodeId)}
      {@const nodeKey = `node:${node.nodeId}`}
      {@const nodeExpanded = expandedNodes.has(nodeKey)}
      <div class="tree-node" role="treeitem" aria-expanded={nodeExpanded} aria-selected={false}>
        <button class="tree-toggle" onclick={() => toggleNode(nodeKey)}>
          <span class="toggle-icon">{nodeExpanded ? '▾' : '▸'}</span>
          <span class="node-label">{node.nodeName}</span>
          <span class="node-id">{node.nodeId}</span>
        </button>

        {#if nodeExpanded}
          {#each node.segments as seg, si (`${seg.space}:${seg.origin}`)}
            {@const segKey = `${nodeKey}:seg:${si}`}
            {@const segExpanded = expandedNodes.has(segKey)}
            <div class="tree-segment" role="treeitem" aria-expanded={segExpanded} aria-selected={false}>
              <button class="tree-toggle indent-1" onclick={() => toggleNode(segKey)}>
                <span class="toggle-icon">{segExpanded ? '▾' : '▸'}</span>
                <span class="seg-label">{seg.name}</span>
              </button>

              {#if segExpanded}
                {#each seg.children as child (child.path.join('/'))}
                  <PickerTreeNode
                    node={child}
                    depth={2}
                    {roleFilter}
                    {searchQuery}
                    pathKey={segKey}
                    {expandedNodes}
                    onToggle={toggleNode}
                    onSelect={handleSelect}
                    {isSlotFree}
                    isSelected={(leaf) => isSelectedLeaf(leaf, node.nodeId)}
                    nodeId={node.nodeId}
                    nodeName={node.nodeName}
                  />
                {/each}
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/each}
  </div>

  <!-- T035: Ambiguous classification prompt -->
  {#if pendingAmbiguous}
    <div class="ambiguous-overlay">
      <RoleClassifyPrompt
        elementName={pendingAmbiguous.leaf.name}
        onClassify={handleAmbiguousClassify}
        onCancel={() => { pendingAmbiguous = null; }}
      />
    </div>
  {/if}

  <!-- Selection preview -->
  {#if selectedElement}
    <div class="selection-preview">
      <h4 class="preview-title">Selected</h4>
      <div class="preview-detail">
        <span class="preview-node">{selectedElement.nodeName}</span>
        <span class="preview-path">{selectedElement.elementLabel}</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .element-picker {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 200px;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    background: #fff;
    overflow: hidden;
  }

  .picker-search {
    display: flex;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid #e5e7eb;
    background: #fafafa;
    gap: 4px;
  }

  .search-input {
    flex: 1;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 0.82rem;
    font-family: inherit;
    outline: none;
  }

  .search-input:focus {
    border-color: #0078d4;
    box-shadow: 0 0 0 1px rgba(0, 120, 212, 0.3);
  }

  .search-clear {
    background: none;
    border: none;
    cursor: pointer;
    color: #6b7280;
    font-size: 0.8rem;
    padding: 2px 4px;
  }

  .picker-tree {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  .picker-empty {
    padding: 24px 16px;
    text-align: center;
    color: #9ca3af;
    font-size: 0.85rem;
  }

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

  .indent-1 { padding-left: 20px; }

  .toggle-icon {
    flex-shrink: 0;
    width: 12px;
    font-size: 0.7rem;
    color: #9ca3af;
  }

  .node-label {
    font-weight: 600;
    color: #1f2937;
  }

  .node-id {
    font-family: 'ui-monospace', monospace;
    font-size: 0.72rem;
    color: #9ca3af;
    margin-left: 6px;
  }

  .seg-label {
    font-weight: 500;
    color: #4b5563;
  }

  .selection-preview {
    border-top: 1px solid #e5e7eb;
    padding: 8px 10px;
    background: #f0f9ff;
  }

  .preview-title {
    margin: 0 0 4px;
    font-size: 0.72rem;
    font-weight: 600;
    color: #0078d4;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .preview-detail {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .preview-node {
    font-size: 0.78rem;
    font-weight: 500;
    color: #1f2937;
  }

  .preview-path {
    font-size: 0.72rem;
    color: #6b7280;
    font-family: 'ui-monospace', monospace;
  }

  .ambiguous-overlay {
    border-top: 1px solid #e5e7eb;
    padding: 8px 10px;
    background: #fffbf0;
  }
</style>
