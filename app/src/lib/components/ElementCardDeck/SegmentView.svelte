<script lang="ts">
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { millerColumnsStore } from '$lib/stores/millerColumns';
  import { invoke } from '@tauri-apps/api/core';
  import type { CardField, CardSubGroup } from '$lib/stores/configSidebar';
  import type { ConfigValueWithMetadata, ConfigValue } from '$lib/api/types';
  import SubGroupAccordion from './SubGroupAccordion.svelte';

  // Shape returned by the get_segment_elements Tauri command.
  // Mirrors the Rust SegmentTree struct (camelCase via serde).
  interface SegmentTree {
    segmentName: string;
    /** Direct leaf fields — for leaf-only segments like User Info */
    fields: CardField[];
    /** Top-level CDI groups — rendered as flat section headers */
    groups: CardSubGroup[];
  }

  let segmentTree: SegmentTree | null = null;
  let isLoading = false;
  let loadError: string | null = null;

  // Track the last loaded segment key so we reload only when the selection changes.
  let lastLoadedKey: string | null = null;

  $: selectedSegment = $configSidebarStore.selectedSegment;
  $: configValues = $millerColumnsStore.configValues;

  $: {
    const sel = selectedSegment;
    const key = sel ? `${sel.nodeId}:${sel.segmentId}` : null;
    if (key !== lastLoadedKey) {
      lastLoadedKey = key;
      if (sel) {
        loadSegment(sel.nodeId, sel.segmentPath);
      } else {
        segmentTree = null;
        isLoading = false;
        loadError = null;
      }
    }
  }

  async function loadSegment(nodeId: string, segmentPath: string) {
    isLoading = true;
    loadError = null;
    segmentTree = null;
    try {
      console.log('[SegmentView] invoking get_segment_elements', { nodeId, segmentPath });
      const result = await invoke<SegmentTree>('get_segment_elements', {
        nodeId,
        segmentPath,
      });
      console.log('[SegmentView] got segment tree:', result);
      console.log('[SegmentView] configValues size:', configValues.size,
        'sample keys:', [...configValues.keys()].slice(0, 3));
      segmentTree = result;
    } catch (err) {
      console.error('[SegmentView] get_segment_elements error:', err);
      loadError = String(err);
    } finally {
      isLoading = false;
    }
  }

  function getValue(
    nodeId: string,
    field: CardField,
    values: Map<string, ConfigValueWithMetadata>,
  ): ConfigValueWithMetadata | null {
    const key = `${nodeId}:${field.elementPath.join('/')}`;
    const hit = values.get(key) ?? null;
    if (!hit) {
      console.warn('[SegmentView] cache miss for key:', key);
    }
    return hit;
  }

  function formatValue(meta: ConfigValueWithMetadata | null): string {
    if (!meta) return '—';
    const v: ConfigValue = meta.value;
    switch (v.type) {
      case 'Int':     return String(v.value);
      case 'String':  return v.value || '(empty)';
      case 'Float':   return v.value.toFixed(4);
      case 'EventId': return v.value.map((b: number) => b.toString(16).padStart(2, '0')).join('.');
      case 'Invalid': return `(error: ${v.error})`;
      default:        return '—';
    }
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

  {:else if segmentTree}
    {@const nodeId = selectedSegment.nodeId}
    <div class="segment-content">

      <h2 class="segment-heading">{segmentTree.segmentName}</h2>

      <!-- Direct leaf fields (leaf-only segments, e.g. User Info) -->
      {#if segmentTree.fields.length > 0}
        <div class="fields-list" role="list">
          {#each segmentTree.fields as field (field.elementPath.join('/'))}
            <div class="field-row" role="listitem">
              <span class="field-name">{field.name}</span>
              {#if field.description}
                <p class="field-description">{field.description}</p>
              {/if}
              <span class="field-value">{formatValue(getValue(nodeId, field, configValues))}</span>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Top-level groups as non-collapsible section headers -->
      {#each segmentTree.groups as group (group.groupPath.join('/'))}
        <section class="group-section">
          <div class="group-header">
            <span class="group-name">{group.name}</span>
            {#if group.description}
              <p class="group-description">{group.description}</p>
            {/if}
          </div>

          <!-- Fields directly in this group -->
          {#if group.fields.length > 0}
            <div class="fields-list" role="list">
              {#each group.fields as field (field.elementPath.join('/'))}
                <div class="field-row" role="listitem">
                  <span class="field-name">{field.name}</span>
                  {#if field.description}
                    <p class="field-description">{field.description}</p>
                  {/if}
                  <span class="field-value">{formatValue(getValue(nodeId, field, configValues))}</span>
                </div>
              {/each}
            </div>
          {/if}

          <!-- Nested sub-groups: accordion only for replicated groups -->
          {#each group.subGroups as subGroup (subGroup.groupPath.join('/'))}
            <SubGroupAccordion
              {subGroup}
              {nodeId}
              {configValues}
              collapsible={subGroup.replication > 1}
            />
          {/each}
        </section>
      {/each}

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

  /* ── Field rows (used at both segment-root and group level) ── */
  .fields-list {
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    background: var(--card-bg, #fff);
    overflow: hidden;
    margin-bottom: 6px;
  }

  .field-row {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-light, #f0f0f0);
  }

  .field-row:last-child {
    border-bottom: none;
  }

  .field-name {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #333);
  }

  .field-description {
    margin: 2px 0 4px;
    font-size: 11px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
  }

  .field-value {
    display: block;
    font-size: 12px;
    font-family: monospace;
    color: var(--text-secondary, #555);
    margin-top: 2px;
  }
</style>
