<script lang="ts">
  /**
   * SegmentView — renders the configuration tree for a selected segment.
   *
   * Phase 4 migration (Spec 007): reads from the unified nodeTreeStore
   * instead of calling `get_segment_elements`. Values are embedded in
   * leaf nodes, so no separate configValues lookup is needed.
   *
   * Updated for plan-cdiConfigNavigator: uses groupReplicatedChildren
   * to collapse sibling replicated groups into pill-selectable sections.
   */
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { nodeInfoStore } from '$lib/stores/nodeInfo';
  import type { SegmentNode, ConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
  import { isGroup, isLeaf, groupReplicatedChildren } from '$lib/types/nodeTree';
  import TreeGroupAccordion from './TreeGroupAccordion.svelte';
  import TreeLeafRow from './TreeLeafRow.svelte';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

  let selectedSegment = $derived($configSidebarStore.selectedSegment);

  // Cross-reference lookup for event ID → bowtie card
  let nodeSlotMap = $derived(bowtieCatalogStore.nodeSlotMap);

  // Access trees reactively so segment derivation re-runs when tree data changes
  // (e.g. after node-tree-updated event merges config values)
  let trees = $derived(nodeTreeStore.trees);

  /**
   * Derive the SegmentNode from the tree store whenever the selection or tree changes.
   * The segment index is encoded in segmentPath as "seg:N".
   */
  let segment = $derived(deriveSegment(selectedSegment, trees));

  /** Whether the tree for the selected node is still loading */
  let isLoading = $derived(selectedSegment ? nodeTreeStore.isNodeLoading(selectedSegment.nodeId) : false);

  /** Error from tree loading */
  let loadError = $derived(selectedSegment ? nodeTreeStore.getError(selectedSegment.nodeId) ?? null : null);

  /** Whether the selected node is offline — disables all inputs (FR-007) */
  let isNodeOffline = $derived(
    selectedSegment
      ? ($nodeInfoStore.get(selectedSegment.nodeId)?.connection_status === 'NotResponding')
      : false
  );

  function deriveSegment(
    sel: { nodeId: string; segmentId: string; segmentPath: string } | null,
    _trees: Map<string, any>,  // reactive dependency; value used via nodeTreeStore
  ): SegmentNode | null {
    if (!sel) return null;
    const tree = nodeTreeStore.getTree(sel.nodeId);
    if (!tree) return null;

    // Parse "seg:N" from segmentPath
    const match = sel.segmentPath.match(/^seg:(\d+)$/);
    if (!match) return null;
    const idx = parseInt(match[1], 10);
    return tree.segments[idx] ?? null;
  }

  /** Format a TreeConfigValue for inline display */
  function formatTreeValue(v: TreeConfigValue | null): string {
    if (v === null) return '—';
    switch (v.type) {
      case 'int':     return String(v.value);
      case 'string':  return v.value || '(empty)';
      case 'float':   return v.value.toFixed(4);
      case 'eventId': return v.bytes.every((b: number) => b === 0)
        ? '(free)'
        : v.bytes.map((b: number) => b.toString(16).padStart(2, '0')).join('.');
    }
  }

  /** Get the BowtieCard cross-reference for a leaf's path */
  function getUsedIn(nodeId: string, leaf: { path: string[] }) {
    return nodeSlotMap.get(`${nodeId}:${leaf.path.join('/')}`);
  }
</script>

<div class="segment-view">
  {#if !selectedSegment}
    <div class="empty-prompt">
      <p>Select a segment from the sidebar to view its configuration</p>
    </div>

  {:else if segment}
    {@const nodeId = selectedSegment.nodeId}
    {@const groupedChildren = groupReplicatedChildren(segment.children)}
    <div class="segment-content">

      <h2 class="segment-heading">{segment.name}</h2>
      {#if segment.description}
        <p class="segment-description">{segment.description}</p>
      {/if}
      {#each groupedChildren as item, idx (idx)}
        {#if item.type === 'leaf'}
          <!-- Direct leaf field at segment level (e.g. User Info fields) -->
          <div class="segment-leaf">
            <TreeLeafRow leaf={item.node} usedIn={getUsedIn(nodeId, item.node)} depth={0} {nodeId} segmentOrigin={segment.origin} segmentName={segment.name} {isNodeOffline} />
          </div>
        {:else if item.type === 'replicatedSet'}
          <!-- Replicated group → pill-selectable section -->
          <TreeGroupAccordion
            group={item.instances[0]}
            {nodeId}
            depth={0}
            siblings={item.instances}
            segmentOrigin={segment.origin}
            segmentName={segment.name}
            {isNodeOffline}
          />
        {:else if item.type === 'group'}
          {#if item.node.replicationCount > 1}
            <!-- Single replicated instance (shouldn't normally happen after grouping) -->
            <TreeGroupAccordion group={item.node} {nodeId} depth={0} segmentOrigin={segment.origin} segmentName={segment.name} {isNodeOffline} />
          {:else}
            <!-- Non-replicated group → section header with children -->
            {@const innerGrouped = groupReplicatedChildren(item.node.children)}
            <section class="group-section">
              <div class="group-header">
                <span class="group-name">{item.node.instanceLabel}</span>
                {#if item.node.description}
                  <p class="group-description">{item.node.description}</p>
                {/if}
              </div>

              {#each innerGrouped as inner, innerIdx (innerIdx)}
                {#if inner.type === 'leaf'}
                  <TreeLeafRow leaf={inner.node} usedIn={getUsedIn(nodeId, inner.node)} depth={1} {nodeId} segmentOrigin={segment.origin} segmentName={segment.name} {isNodeOffline} />
                {:else if inner.type === 'replicatedSet'}
                  <TreeGroupAccordion
                    group={inner.instances[0]}
                    {nodeId}
                    depth={1}
                    siblings={inner.instances}
                    segmentOrigin={segment.origin}
                    segmentName={segment.name}
                    {isNodeOffline}
                  />
                {:else if inner.type === 'group'}
                  <TreeGroupAccordion
                    group={inner.node}
                    {nodeId}
                    depth={1}
                    segmentOrigin={segment.origin}
                    segmentName={segment.name}
                    {isNodeOffline}
                  />
                {/if}
              {/each}
            </section>
          {/if}
        {/if}
      {/each}

    </div>

  {:else if isLoading}
    <!-- Initial load — segment not yet available -->
    <div class="loading" role="status" aria-label="Loading segment">
      <span aria-hidden="true">⋯</span> Loading…
    </div>

  {:else if loadError}
    <!-- Load error on initial fetch -->
    <div class="load-error" role="alert">
      {loadError}
    </div>

  {:else}
    <!-- Tree loaded but segment not found — unusual edge case -->
    <div class="empty-prompt">
      <p>Segment data not available</p>
    </div>
  {/if}
</div>

<style>
  /* ══════════════════════════════════════════
     Fluent UI Design — SegmentView
     ══════════════════════════════════════════ */

  .segment-view {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    background-color: #faf9f8;                     /* colorNeutralBackground2 */
    min-height: 0;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  /* ── Empty / loading / error states ── */
  .empty-prompt {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: #a19f9d;                                /* colorNeutralForeground4 */
    font-size: 14px;
    text-align: center;
  }

  .empty-prompt p {
    margin: 0;
    max-width: 280px;
    line-height: 1.5;
  }

  .loading {
    padding: 32px;
    color: #605e5c;                                /* colorNeutralForeground2 */
    font-size: 13px;
    text-align: center;
  }

  .load-error {
    margin: 12px 0;
    padding: 10px 14px;
    background-color: #fdf3f4;                     /* colorPaletteRedBackground1 */
    border: 1px solid #eeacb2;                     /* colorPaletteRedBorder1 */
    border-radius: 4px;                            /* borderRadiusMedium */
    color: #a4262c;                                /* colorPaletteRedForeground1 */
    font-size: 13px;
  }

  /* ── Segment heading ── */
  .segment-heading {
    margin: 0 0 10px;
    font-size: 18px;
    font-weight: 600;
    color: #242424;                                /* colorNeutralForeground1 */
    padding-bottom: 8px;
    border-bottom: 2px solid #0078d4;              /* branded accent */
  }

  .segment-description {
    margin: 0 0 8px;
    font-size: 13px;
    color: #605e5c;                                /* colorNeutralForeground2 */
    line-height: 1.5;
    white-space: pre-wrap;                         /* preserve newlines from CDI descriptions */
  }

  .segment-leaf {
    margin-bottom: 2px;
  }

  /* ── Top-level group section ── */
  .group-section {
    margin-bottom: 14px;
    padding: 8px 14px 10px;
    background: #f5f5f4;                           /* subtle card-like grouping */
    border-radius: 6px;
  }

  /* Subtle divider line above non-first top-level groups */
  .group-section + .group-section {
    border-top: 1px solid #e1dfdd;                 /* colorNeutralStroke2 */
    padding-top: 14px;
    margin-top: 0;
  }

  .group-header {
    margin-bottom: 6px;
  }

  .group-name {
    display: block;
    font-size: 14px;
    font-weight: 600;
    color: #323130;                                /* colorNeutralForeground1 */
  }

  .group-description {
    margin: 4px 0 0;
    font-size: 12px;
    color: #605e5c;                                /* colorNeutralForeground2 */
    line-height: 1.5;
    white-space: pre-wrap;                         /* preserve newlines from CDI descriptions */
  }

  /* Remove the top border on the very first group after the heading —
     it sits right below the blue accent line and looks redundant */
  .segment-content > :global(.pill-section:first-of-type),
  .segment-content > :global(.inline-section:first-of-type) {
    border-top: none;
    padding-top: 0;
  }
</style>
