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
  import { createEventDispatcher } from 'svelte';
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { configReadNodesStore } from '$lib/stores/configReadStatus';
  import ConnectorSlotSelector from '$lib/components/ConfigSidebar/ConnectorSlotSelector.svelte';
  import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { nodeInfoStore } from '$lib/stores/nodeInfo';
  import type { SegmentNode, TreeConfigValue } from '$lib/types/nodeTree';
  import { groupReplicatedChildren } from '$lib/types/nodeTree';
  import TreeGroupAccordion from './TreeGroupAccordion.svelte';
  import TreeLeafRow from './TreeLeafRow.svelte';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { evaluateConnectorConstraintsForPath } from '$lib/utils/connectorConstraints';
  import { buildSegmentConnectorSlotSelectors } from '$lib/utils/connectorSlotSelectors';

  interface Props {
    onchangeConnectorSelection?: (event: CustomEvent<{
      nodeId: string;
      slotId: string;
      selectedDaughterboardId: string | null;
    }>) => void;
  }

  let { onchangeConnectorSelection }: Props = $props();

  const dispatch = createEventDispatcher<{
    changeConnectorSelection: { nodeId: string; slotId: string; selectedDaughterboardId: string | null };
  }>();

  let selectedSegment = $derived($configSidebarStore.selectedSegment);
  let configReadNodes = $derived($configReadNodesStore);

  let nodeSlotMap = $derived(bowtieCatalogStore.effectiveNodeSlotMap);
  let trees = $derived(nodeTreeStore.trees);
  let connectorRevision = $derived(connectorSelectionsStore.revision);

  let segment = $derived(deriveSegment(selectedSegment, trees));
  let isLoading = $derived(selectedSegment ? nodeTreeStore.isNodeLoading(selectedSegment.nodeId) : false);
  let loadError = $derived(selectedSegment ? nodeTreeStore.getError(selectedSegment.nodeId) ?? null : null);

  let selectedTree = $derived(selectedSegment ? nodeTreeStore.getTree(selectedSegment.nodeId) : null);
  let connectorProfile = $derived.by(() => {
    connectorRevision;
    if (!selectedSegment) {
      return null;
    }

    return connectorSelectionsStore.getProfile(selectedSegment.nodeId) ?? selectedTree?.connectorProfile ?? null;
  });
  let connectorDocument = $derived.by(() => {
    connectorRevision;
    if (!selectedSegment) {
      return null;
    }

    return connectorSelectionsStore.getDocument(selectedSegment.nodeId);
  });
  let connectorError = $derived.by(() => {
    connectorRevision;
    if (!selectedSegment) {
      return null;
    }

    return connectorSelectionsStore.getError(selectedSegment.nodeId);
  });
  let connectorSelectors = $derived(
    segment
      ? buildSegmentConnectorSlotSelectors(connectorProfile, connectorDocument, segment.name)
      : [],
  );
  let connectorControlsEnabled = $derived(
    selectedSegment
      ? layoutStore.hasLayoutFile || configReadNodes.has(selectedSegment.nodeId)
      : false,
  );

  let isNodeOffline = $derived(
    selectedSegment
      ? ($nodeInfoStore.get(selectedSegment.nodeId)?.connection_status === 'NotResponding')
      : false,
  );

  function deriveSegment(
    sel: { nodeId: string; segmentId: string } | null,
    _trees: Map<string, any>,
  ): SegmentNode | null {
    if (!sel) return null;
    const tree = nodeTreeStore.getTree(sel.nodeId);
    if (!tree) return null;

    const match = sel.segmentId.match(/^seg:(\d+)$/);
    if (!match) return null;
    const idx = parseInt(match[1], 10);
    return tree.segments[idx] ?? null;
  }

  function formatTreeValue(v: TreeConfigValue | null): string {
    if (v === null) return '—';
    switch (v.type) {
      case 'int':
        return String(v.value);
      case 'string':
        return v.value || '(empty)';
      case 'float':
        return v.value.toFixed(4);
      case 'eventId':
        return v.bytes.every((b: number) => b === 0)
          ? '(free)'
          : v.bytes.map((b: number) => b.toString(16).padStart(2, '0')).join('.');
    }
  }

  function getUsedIn(nodeId: string, leaf: { path: string[] }) {
    return nodeSlotMap.get(`${nodeId}:${leaf.path.join('/')}`);
  }

  function emitConnectorSelection(detail: {
    nodeId: string;
    slotId: string;
    selectedDaughterboardId: string | null;
  }): void {
    const event = new CustomEvent('changeConnectorSelection', { detail });
    onchangeConnectorSelection?.(event);
    dispatch('changeConnectorSelection', detail);
  }

  function connectorConstraintForPath(path: string[]) {
    return evaluateConnectorConstraintsForPath(connectorProfile, connectorDocument, path);
  }
</script>

<div class="segment-view">
  {#if !selectedSegment}
    <div class="empty-prompt">
      <p>Select a segment from the sidebar to view its configuration</p>
    </div>

  {:else if segment}
    {@const nodeId = selectedSegment.nodeId}
    {#key `${nodeId}:${connectorRevision}`}
      {@const groupedChildren = groupReplicatedChildren(segment.children)}
      <div class="segment-content">
        <h2 class="segment-heading">{segment.name}</h2>
        {#if segment.description}
          <p class="segment-description">{segment.description}</p>
        {/if}
        {#if connectorError}
          <div class="load-error" role="alert">{connectorError}</div>
        {:else if connectorSelectors.length > 0}
          <section class="connector-section" aria-label="Connector daughterboards for {segment.name}">
            <h3 class="connector-heading">Connector daughterboards</h3>
            <div class="connector-selector-list" role="group" aria-label="Connector daughterboards for {segment.name}">
              {#each connectorSelectors as selector (selector.slotId)}
                <ConnectorSlotSelector
                  {selector}
                  disabled={!connectorControlsEnabled || isNodeOffline}
                  on:change={(event) => emitConnectorSelection({
                    nodeId,
                    slotId: event.detail.slotId,
                    selectedDaughterboardId: event.detail.selectedDaughterboardId,
                  })}
                />
              {/each}
            </div>
            {#if !connectorControlsEnabled}
              <p class="connector-hint">Read this node configuration online or open a layout to edit connector selections.</p>
            {/if}
          </section>
        {/if}
        {#each groupedChildren as item, idx (idx)}
          {#if item.type === 'leaf'}
            {@const leafConstraint = connectorConstraintForPath(item.node.path)}
            {#if !leafConstraint.hidden}
              <div class="segment-leaf">
                <TreeLeafRow leaf={item.node} usedIn={getUsedIn(nodeId, item.node)} depth={0} {nodeId} segmentOrigin={segment.origin} segmentName={segment.name} {isNodeOffline} connectorConstraintState={leafConstraint} />
              </div>
            {/if}
          {:else if item.type === 'replicatedSet'}
            {@const replicatedConstraint = connectorConstraintForPath(item.instances[0].path)}
            {#if !replicatedConstraint.hidden}
              <TreeGroupAccordion
                group={item.instances[0]}
                {nodeId}
                depth={0}
                siblings={item.instances}
                segmentOrigin={segment.origin}
                segmentName={segment.name}
                {isNodeOffline}
                {connectorProfile}
                {connectorDocument}
              />
            {/if}
          {:else if item.type === 'group'}
            {@const groupConstraint = connectorConstraintForPath(item.node.path)}
            {#if !groupConstraint.hidden && item.node.replicationCount > 1}
              <TreeGroupAccordion group={item.node} {nodeId} depth={0} segmentOrigin={segment.origin} segmentName={segment.name} {isNodeOffline} {connectorProfile} {connectorDocument} />
            {:else if !groupConstraint.hidden}
              {@const innerGrouped = groupReplicatedChildren(item.node.children)}
              {@const groupEffectiveOffline = isNodeOffline || !!item.node.readOnly}
              <section class="group-section">
                {#if item.node.hasName !== false}
                  <div class="group-header">
                    <span class="group-name">{item.node.instanceLabel}</span>
                    {#if item.node.description}
                      <p class="group-description">{item.node.description}</p>
                    {/if}
                  </div>
                {/if}

                {#each innerGrouped as inner, innerIdx (innerIdx)}
                  {#if inner.type === 'leaf'}
                    {@const innerLeafConstraint = connectorConstraintForPath(inner.node.path)}
                    {#if !innerLeafConstraint.hidden}
                      <TreeLeafRow leaf={inner.node} usedIn={getUsedIn(nodeId, inner.node)} depth={1} {nodeId} segmentOrigin={segment.origin} segmentName={segment.name} isNodeOffline={groupEffectiveOffline} connectorConstraintState={innerLeafConstraint} />
                    {/if}
                  {:else if inner.type === 'replicatedSet'}
                    {@const innerReplicatedConstraint = connectorConstraintForPath(inner.instances[0].path)}
                    {#if !innerReplicatedConstraint.hidden}
                      <TreeGroupAccordion
                        group={inner.instances[0]}
                        {nodeId}
                        depth={1}
                        siblings={inner.instances}
                        segmentOrigin={segment.origin}
                        segmentName={segment.name}
                        isNodeOffline={groupEffectiveOffline}
                        {connectorProfile}
                        {connectorDocument}
                      />
                    {/if}
                  {:else if inner.type === 'group'}
                    {@const innerGroupConstraint = connectorConstraintForPath(inner.node.path)}
                    {#if !innerGroupConstraint.hidden}
                      <TreeGroupAccordion
                        group={inner.node}
                        {nodeId}
                        depth={1}
                        segmentOrigin={segment.origin}
                        segmentName={segment.name}
                        isNodeOffline={groupEffectiveOffline}
                        {connectorProfile}
                        {connectorDocument}
                      />
                    {/if}
                  {/if}
                {/each}
              </section>
            {/if}
          {/if}
        {/each}
      </div>
    {/key}

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

  .connector-section {
    margin: 0 0 16px;
    padding: 12px 14px;
    background: #f5f5f4;
    border-radius: 6px;
  }

  .connector-heading {
    margin: 0 0 10px;
    font-size: 13px;
    font-weight: 600;
    color: #323130;
  }

  .connector-selector-list {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 8px;
    align-items: start;
  }

  .connector-selector-list :global(.connector-slot-selector) {
    padding: 0;
  }

  .connector-hint {
    margin: 10px 0 0;
    font-size: 12px;
    color: #605e5c;
    line-height: 1.4;
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
