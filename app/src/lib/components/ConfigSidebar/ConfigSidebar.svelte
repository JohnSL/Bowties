<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { nodeInfoStore } from '$lib/stores/nodeInfo';
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { configReadNodesStore } from '$lib/stores/configReadStatus';
  import NodeEntry from './NodeEntry.svelte';
  import SegmentEntry from './SegmentEntry.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import type { SegmentInfo } from '$lib/stores/configSidebar';

  const dispatch = createEventDispatcher<{ readNodeConfig: { nodeId: string } }>();

  /** Cached segments per nodeId — loaded on first expansion */
  let nodeSegments = new Map<string, SegmentInfo[]>();
  /** Loading state per nodeId */
  let nodeLoadingMap = new Map<string, boolean>();

  /** Get display name for a discovered node */
  function getNodeDisplayName(node: any): string {
    const snip = node.snip_data;
    if (!snip) return `Node ${node.node_id}`;
    if (snip.user_name && snip.user_name.length > 0) return snip.user_name;
    if (snip.user_description && snip.user_description.length > 0) return snip.user_description;
    if (snip.manufacturer && snip.model) return `${snip.manufacturer} ${snip.model}`;
    return `Node`;
  }

  /** Get secondary detail for disambiguation (manufacturer/model) */
  function getNodeDetail(node: any): string | null {
    const snip = node.snip_data;
    if (!snip) return null;
    if (snip.user_name && snip.manufacturer && snip.model) {
      return `${snip.manufacturer} ${snip.model}`;
    }
    return null;
  }

  /** Build multi-line hover tooltip with full SNIP details */
  function getNodeTooltip(nodeId: string, node: any): string {
    const parts: string[] = [`Node ID: ${nodeId}`];
    if (node.alias != null) {
      parts.push(`Alias: 0x${node.alias.toString(16).toUpperCase().padStart(3, '0')}`);
    }
    const snip = node.snip_data;
    if (snip) {
      if (snip.manufacturer)     parts.push(`Manufacturer: ${snip.manufacturer}`);
      if (snip.model)            parts.push(`Model: ${snip.model}`);
      if (snip.hardware_version) parts.push(`Hardware: ${snip.hardware_version}`);
      if (snip.software_version) parts.push(`Software: ${snip.software_version}`);
      if (snip.user_name)        parts.push(`User Name: ${snip.user_name}`);
      if (snip.user_description) parts.push(`Description: ${snip.user_description}`);
    }
    return parts.join('\n');
  }

  /** Check whether a node is offline */
  function isNodeOffline(node: any): boolean {
    return node.connection_status === 'NotResponding';
  }

  /** Handle node toggle — expand/collapse and load segments on first expand */
  async function handleNodeToggle(nodeId: string, node: any, isCurrentlyExpanded: boolean) {
    configSidebarStore.toggleNodeExpanded(nodeId);

    // Load segments on first expansion via unified tree (Spec 007, Phase 4)
    if (!isCurrentlyExpanded && !nodeSegments.has(nodeId)) {
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
  }

  /** Handle segment selection */
  function handleSegmentSelect(nodeId: string, seg: SegmentInfo) {
    configSidebarStore.selectSegment(nodeId, seg.segmentId, seg.segmentName, seg.segmentPath);
  }

  $: nodes = $nodeInfoStore;
  $: sidebarState = $configSidebarStore;
  $: configReadNodes = $configReadNodesStore;
</script>

<aside class="config-sidebar">
  {#if nodes.size === 0}
    <div class="empty-state">
      <p>No nodes discovered — use Discover Nodes to scan the network</p>
    </div>
  {:else}
    <nav class="node-list" aria-label="Discovered nodes">
      {#each [...nodes.entries()].sort((a, b) => getNodeDisplayName(a[1]).localeCompare(getNodeDisplayName(b[1]))) as [nodeId, node]}
        {@const isExpanded = sidebarState.expandedNodeIds.includes(nodeId)}
        {@const isOffline = isNodeOffline(node)}
        {@const isLoading = nodeLoadingMap.get(nodeId) ?? false}
        {@const segments = nodeSegments.get(nodeId) ?? []}
        {@const nodeError = sidebarState.nodeErrors[nodeId] ?? null}
        {@const hasSelectedSegment = sidebarState.selectedSegment?.nodeId === nodeId}
        {@const isConfigNotRead = node.snip_data !== null && !configReadNodes.has(nodeId)}

        <div class="node-group" class:child-selected={hasSelectedSegment}>
          <NodeEntry
            {nodeId}
            nodeName={getNodeDisplayName(node)}
            nodeDetail={getNodeDetail(node)}
            nodeTooltip={getNodeTooltip(nodeId, node)}
            {isExpanded}
            {isOffline}
            {isLoading}
            configNotRead={isConfigNotRead}
            on:toggle={() => handleNodeToggle(nodeId, node, isExpanded)}
            on:readConfig={() => dispatch('readNodeConfig', { nodeId })}
          />

          {#if isExpanded}
            <div class="segment-list" role="list" aria-label="Segments for {getNodeDisplayName(node)}">
              {#if nodeError}
                <div class="segment-error" role="alert">{nodeError}</div>
              {:else if isLoading}
                <div class="segment-loading">
                  <span role="status" aria-label="Loading segments">Loading segments…</span>
                </div>
              {:else if segments.length === 0}
                <p class="segment-empty">No segments available</p>
              {:else}
                {#each segments as seg}
                  {@const isSelected =
                    sidebarState.selectedSegment?.nodeId === nodeId &&
                    sidebarState.selectedSegment?.segmentId === seg.segmentId}
                  <SegmentEntry
                    segmentId={seg.segmentId}
                    segmentName={seg.segmentName}
                    description={seg.description}
                    {isSelected}
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
