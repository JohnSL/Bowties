<script lang="ts">
  /**
   * SegmentView — renders the configuration tree for a selected segment.
   *
   * Phase 4 migration (Spec 007): reads from the unified nodeTreeStore
   * instead of calling `get_segment_elements`. Values are embedded in
   * leaf nodes, so no separate configValues lookup is needed.
   */
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import type { SegmentNode, ConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
  import { isGroup, isLeaf } from '$lib/types/nodeTree';
  import TreeGroupAccordion from './TreeGroupAccordion.svelte';
  import TreeLeafRow from './TreeLeafRow.svelte';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

  $: selectedSegment = $configSidebarStore.selectedSegment;

  // Cross-reference lookup for event ID → bowtie card
  $: nodeSlotMap = bowtieCatalogStore.nodeSlotMap;

  // Access trees reactively so segment derivation re-runs when tree data changes
  // (e.g. after node-tree-updated event merges config values)
  $: trees = nodeTreeStore.trees;

  /**
   * Derive the SegmentNode from the tree store whenever the selection or tree changes.
   * The segment index is encoded in segmentPath as "seg:N".
   */
  $: segment = deriveSegment(selectedSegment, trees);

  /** Whether the tree for the selected node is still loading */
  $: isLoading = selectedSegment ? nodeTreeStore.isNodeLoading(selectedSegment.nodeId) : false;

  /** Error from tree loading */
  $: loadError = selectedSegment ? nodeTreeStore.getError(selectedSegment.nodeId) ?? null : null;

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

  {:else if isLoading}
    <div class="loading" role="status" aria-label="Loading segment">
      <span aria-hidden="true">⋯</span> Loading…
    </div>

  {:else if loadError}
    <div class="load-error" role="alert">{loadError}</div>

  {:else if segment}
    {@const nodeId = selectedSegment.nodeId}
    <div class="segment-content">

      <h2 class="segment-heading">{segment.name}</h2>

      {#if segment.description}
        <p class="segment-description">{segment.description}</p>
      {/if}

      {#each segment.children as child (child.kind === 'group' ? child.path.join('/') : child.path.join('/'))}
        {#if isLeaf(child)}
          <!-- Direct leaf field at segment level (e.g. User Info fields) -->
          <div class="segment-leaf">
            <TreeLeafRow leaf={child} usedIn={getUsedIn(nodeId, child)} />
          </div>
        {:else if isGroup(child)}
          {#if child.replicationCount > 1}
            <!-- Replicated group instance → collapsible accordion -->
            <TreeGroupAccordion group={child} {nodeId} depth={0} collapsible={true} />
          {:else}
            <!-- Non-replicated group → section header with children -->
            <section class="group-section">
              <div class="group-header">
                <span class="group-name">{child.instanceLabel}</span>
                {#if child.description}
                  <p class="group-description">{child.description}</p>
                {/if}
              </div>

              {#each child.children as grandchild (grandchild.kind === 'group' ? grandchild.path.join('/') : grandchild.path.join('/'))}
                {#if isLeaf(grandchild)}
                  <TreeLeafRow leaf={grandchild} usedIn={getUsedIn(nodeId, grandchild)} />
                {:else if isGroup(grandchild)}
                  <TreeGroupAccordion
                    group={grandchild}
                    {nodeId}
                    depth={1}
                    collapsible={grandchild.replicationCount > 1}
                  />
                {/if}
              {/each}
            </section>
          {/if}
        {/if}
      {/each}

    </div>

  {:else}
    <!-- Tree loaded but segment not found — unusual edge case -->
    <div class="empty-prompt">
      <p>Segment data not available</p>
    </div>
  {/if}
</div>

<style>
  .segment-view {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    background-color: var(--main-bg, #f8f9fa);
    min-height: 0;
  }

  /* ── Empty / loading / error states ── */
  .empty-prompt {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-secondary, #999);
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
    color: var(--text-secondary, #666);
    font-size: 13px;
    text-align: center;
  }

  .load-error {
    margin: 12px 0;
    padding: 10px 14px;
    background-color: var(--error-bg, #fdf2f2);
    border: 1px solid var(--error-border, #f5c6c6);
    border-radius: 6px;
    color: var(--error-color, #c62828);
    font-size: 13px;
  }

  /* ── Segment heading ── */
  .segment-heading {
    margin: 0 0 16px;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary, #333);
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-color, #ddd);
  }

  .segment-description {
    margin: 0 0 12px;
    font-size: 12px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
  }

  .segment-leaf {
    margin-bottom: 2px;
  }

  /* ── Top-level group section ── */
  .group-section {
    margin-bottom: 20px;
  }

  .group-header {
    margin-bottom: 6px;
  }

  .group-name {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary, #222);
  }

  .group-description {
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
  }
</style>
