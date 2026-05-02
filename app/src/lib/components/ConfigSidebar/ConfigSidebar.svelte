<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { nodeInfoStore } from '$lib/stores/nodeInfo';
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { configReadNodesStore } from '$lib/stores/configReadStatus';
  import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { layoutOpenInProgress } from '$lib/stores/layoutOpenLifecycle';
  import NodeEntry from './NodeEntry.svelte';
  import SegmentEntry from './SegmentEntry.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import type { SegmentInfo } from '$lib/stores/configSidebar';
  import {
    buildSidebarNodeEntries,
    getNodePendingState,
    getSegmentPendingState,
    shouldShowConfigNotReadBadge,
  } from './configSidebarPresenter';

  const dispatch = createEventDispatcher<{
    readNodeConfig: { nodeId: string };
  }>();

  /** Cached segments per nodeId — loaded on first expansion */
  let nodeSegments = $state(new Map<string, SegmentInfo[]>());
  /** Loading state per nodeId */
  let nodeLoadingMap = $state(new Map<string, boolean>());

  /** Load a node's config tree and populate nodeSegments from it. */
  async function loadSegmentsForNode(nodeId: string): Promise<void> {
    const cachedTree = nodeTreeStore.getTree(nodeId);
    if (cachedTree) {
      const segments: SegmentInfo[] = cachedTree.segments.map((seg, idx) => ({
        segmentId: `seg:${idx}`,
        segmentPath: `seg:${idx}`,
        segmentName: seg.name ?? 'Unnamed Segment',
        description: seg.description ?? null,
        space: seg.space,
        origin: seg.origin,
      }));
      nodeSegments = new Map(nodeSegments.set(nodeId, segments));
      configSidebarStore.setNodeSegments(nodeId, segments);
      return;
    }

    nodeLoadingMap = new Map(nodeLoadingMap.set(nodeId, true));
    configSidebarStore.setNodeLoading(nodeId, 'loading');
    try {
      const tree = await nodeTreeStore.loadTree(nodeId);
      if (tree) {
        const segments: SegmentInfo[] = tree.segments.map((seg, idx) => ({
          segmentId: `seg:${idx}`,
          segmentPath: `seg:${idx}`,
          segmentName: seg.name ?? 'Unnamed Segment',
          description: seg.description ?? null,
          space: seg.space,
          origin: seg.origin,
        }));
        nodeSegments = new Map(nodeSegments.set(nodeId, segments));
        configSidebarStore.setNodeSegments(nodeId, segments);
      } else {
        const err = nodeTreeStore.getError(nodeId);
        configSidebarStore.setNodeLoading(nodeId, 'error', err ?? 'Failed to load tree');
      }
    } catch (err) {
      configSidebarStore.setNodeLoading(nodeId, 'error', String(err));
    } finally {
      nodeLoadingMap = new Map(nodeLoadingMap.set(nodeId, false));
    }
  }

  // On mount, restore segments for nodes that are already expanded in the store
  // (e.g. after a route change unmounts and remounts this component).
  // Reading from the store via `get()` is reliable regardless of when $: runs.
  onMount(() => {
    const { expandedNodeIds } = get(configSidebarStore);
    for (const nodeId of expandedNodeIds) {
      if (!nodeSegments.has(nodeId)) {
        // If the tree is already cached (from a prior expansion), populate segments
        // synchronously so the user sees them immediately without a loading flash.
        const tree = nodeTreeStore.getTree(nodeId);
        if (tree) {
          const segments: SegmentInfo[] = tree.segments.map((seg, idx) => ({
            segmentId: `seg:${idx}`,
            segmentPath: `seg:${idx}`,
            segmentName: seg.name ?? 'Unnamed Segment',
            description: seg.description ?? null,
            space: seg.space,
            origin: seg.origin,
          }));
          nodeSegments = new Map(nodeSegments.set(nodeId, segments));
          configSidebarStore.setNodeSegments(nodeId, segments);
        } else {
          void loadSegmentsForNode(nodeId);
        }
      }
    }
  });

  /** Handle node toggle — expand/collapse and load segments on first expand */
  async function handleNodeToggle(nodeId: string, node: any, isCurrentlyExpanded: boolean) {
    configSidebarStore.toggleNodeExpanded(nodeId);

    // Load segments on first expansion via unified tree (Spec 007, Phase 4)
    if (!isCurrentlyExpanded && !nodeSegments.has(nodeId)) {
      await loadSegmentsForNode(nodeId);
    }
  }

  /** Handle segment selection */
  function handleSegmentSelect(nodeId: string, seg: SegmentInfo) {
    configSidebarStore.selectSegment(nodeId, seg.segmentId, seg.segmentName);
  }

  let nodes = $derived($nodeInfoStore);
  let nodeEntries = $derived(buildSidebarNodeEntries(nodes));
  let sidebarState = $derived($configSidebarStore);
  let configReadNodes = $derived($configReadNodesStore);
  let persistedRows = $derived(offlineChangesStore.persistedRows);

  // Subscribe to tree changes to enable reactive updates on hasPendingEdits
  let trees = $derived(nodeTreeStore.trees);
</script>

<aside class="config-sidebar">
  {#if nodes.size === 0}
    <div class="empty-state">
      <p>No nodes discovered — use Discover Nodes to scan the network</p>
    </div>
  {:else}
    <nav class="node-list" aria-label="Discovered nodes">
      {#each nodeEntries as entry (entry.nodeId)}
        {@const { nodeId, node } = entry}
        {@const isExpanded = sidebarState.expandedNodeIds.includes(nodeId)}
        {@const isLoading = nodeLoadingMap.get(nodeId) ?? false}
        {@const segments = nodeSegments.get(nodeId) ?? []}
        {@const nodeError = sidebarState.nodeErrors[nodeId] ?? null}
        {@const hasSelectedSegment = sidebarState.selectedSegment?.nodeId === nodeId}
        {@const isNodeSelected = sidebarState.selectedNodeId === nodeId && !hasSelectedSegment}
        {@const tree = nodeTreeStore.getTree(nodeId) ?? null}
        {@const nodePending = getNodePendingState(nodeId, tree, $layoutOpenInProgress, persistedRows)}
        {@const isConfigNotRead = shouldShowConfigNotReadBadge({
          configReadNodes,
          layoutIsOfflineMode: layoutStore.isOfflineMode,
          layoutOpenInProgress: $layoutOpenInProgress,
          node,
          nodeId,
        })}

        <div class="node-group" class:child-selected={hasSelectedSegment}>
          <NodeEntry
            {nodeId}
            nodeName={entry.nodeName}
            nodeDetail={entry.nodeDetail}
            nodeTooltip={entry.nodeTooltip}
            {isExpanded}
            isOffline={entry.isOffline}
            {isLoading}
            configNotRead={isConfigNotRead}
            isSelected={isNodeSelected}
            hasPendingEdits={nodePending.hasPendingEdits}
            hasPendingApply={nodePending.hasPendingApply}
            on:toggle={() => handleNodeToggle(nodeId, node, isExpanded)}
            on:readConfig={() => dispatch('readNodeConfig', { nodeId })}
          />

          {#if isExpanded}
            <div class="segment-list" role="list" aria-label="Segments for {entry.nodeName}">
              {#if nodeError}
                {#if nodeError.includes('CdiUnavailable') || nodeError.includes('CdiNotRetrieved')}
                  {#if isConfigNotRead}
                    <p class="segment-empty">Configuration has not been read from this node yet</p>
                  {:else}
                    <p class="segment-empty">Configuration not supported by this node</p>
                  {/if}
                {:else}
                  <div class="segment-error" role="alert">{nodeError}</div>
                {/if}
              {:else if isLoading}
                <div class="segment-loading">
                  <span role="status" aria-label="Loading segments">Loading segments…</span>
                </div>
              {:else if segments.length === 0}
                <p class="segment-empty">No segments available</p>
              {:else}
                {#each segments as seg}
                  {@const segmentPending = getSegmentPendingState(nodeId, tree, seg.origin, $layoutOpenInProgress, persistedRows)}
                  {@const isSelected =
                    sidebarState.selectedSegment?.nodeId === nodeId &&
                    sidebarState.selectedSegment?.segmentId === seg.segmentId}
                  <SegmentEntry
                    segmentId={seg.segmentId}
                    segmentName={seg.segmentName}
                    description={seg.description}
                    {isSelected}
                    hasPendingEdits={segmentPending.hasPendingEdits}
                    hasPendingApply={segmentPending.hasPendingApply}
                    on:select={() => handleSegmentSelect(nodeId, seg)}
                  />
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </nav>
  {/if}
</aside>

<style>
  .config-sidebar {
    width: 240px;
    min-width: 240px;
    height: 100%;
    overflow-y: auto;
    background-color: var(--sidebar-bg, #fafafa);
    border-right: 1px solid var(--border-color, #ddd);
    display: flex;
    flex-direction: column;
  }

  .empty-state {
    padding: 20px 16px;
    color: var(--text-secondary, #666);
    font-size: 13px;
    line-height: 1.5;
  }

  .empty-state p {
    margin: 0;
  }

  .node-list {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .node-group {
    display: flex;
    flex-direction: column;
  }

  .node-group.child-selected > :global(.node-entry) {
    border-left: 3px solid var(--primary-color, #1976d2);
    padding-left: 9px;
  }

  .segment-list {
    background-color: var(--segment-list-bg, #f5f5f5);
    border-bottom: 1px solid var(--border-color, #eee);
  }

  .segment-error {
    padding: 8px 12px 8px 40px;
    font-size: 12px;
    color: var(--error-color, #c62828);
  }

  .segment-loading {
    padding: 8px 12px 8px 40px;
    font-size: 12px;
    color: var(--text-secondary, #666);
  }

  .segment-empty {
    padding: 8px 12px 8px 40px;
    font-size: 12px;
    color: var(--text-secondary, #999);
    margin: 0;
    font-style: italic;
  }
</style>
