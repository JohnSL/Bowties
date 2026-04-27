<script lang="ts">
  import { syncPanelStore } from '$lib/stores/syncPanel.svelte';

  let expanded = $state(false);

  function toggleExpanded() {
    expanded = !expanded;
  }

  function handleSelectAll() {
    syncPanelStore.selectAllClean();
  }

  function handleDeselectAll() {
    syncPanelStore.deselectAllClean();
  }

  let allSelected = $derived(syncPanelStore.selectedCleanCount === syncPanelStore.cleanRows.length);
  let noneSelected = $derived(syncPanelStore.selectedCleanCount === 0);
</script>

<section class="cs-section">
  <button class="cs-header" onclick={toggleExpanded} aria-expanded={expanded}>
    <span class="cs-chevron" class:cs-chevron-open={expanded}>▶</span>
    <h3 class="cs-title">
      Clean changes
      <span class="cs-count">
        ({syncPanelStore.selectedCleanCount}/{syncPanelStore.cleanRows.length} selected)
      </span>
    </h3>
  </button>

  {#if expanded}
    <div class="cs-body">
      <div class="cs-toolbar">
        <button class="cs-toggle-btn" onclick={handleSelectAll} disabled={allSelected}>
          Select all
        </button>
        <button class="cs-toggle-btn" onclick={handleDeselectAll} disabled={noneSelected}>
          Deselect all
        </button>
      </div>

      <div class="cs-list">
        {#each syncPanelStore.cleanRows as row (row.changeId)}
          <label class="cs-row">
            <input
              type="checkbox"
              checked={!syncPanelStore.isCleanRowDeselected(row.changeId)}
              onchange={() => syncPanelStore.toggleCleanRow(row.changeId)}
            />
            <span class="cs-row-detail">
              {#if row.fieldLabel}
                <span class="cs-row-field">{row.fieldLabel}</span>
              {/if}
              {#if row.nodeName || row.nodeId}
                <span class="cs-row-node">
                  {#if row.nodeName}
                    <span class="cs-row-node-name">{row.nodeName}</span>
                  {/if}
                  {#if row.nodeId}
                    <span class="cs-row-node-id">{row.nodeId}</span>
                  {/if}
                </span>
              {/if}
              <span class="cs-row-value">{row.baselineValue} → {row.plannedValue}</span>
            </span>
          </label>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .cs-section {
    border: 1px solid #d1d5db;
    border-radius: 8px;
    overflow: hidden;
  }

  .cs-header {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 14px;
    background: #f9fafb;
    border: none;
    cursor: pointer;
    text-align: left;
    transition: background-color 0.15s;
  }

  .cs-header:hover {
    background: #f3f4f6;
  }

  .cs-chevron {
    font-size: 10px;
    color: #6b7280;
    transition: transform 0.15s;
  }

  .cs-chevron-open {
    transform: rotate(90deg);
  }

  .cs-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #1e293b;
  }

  .cs-count {
    font-weight: 400;
    font-size: 13px;
    color: #64748b;
  }

  .cs-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid #e5e7eb;
  }

  .cs-toolbar {
    display: flex;
    gap: 8px;
  }

  .cs-toggle-btn {
    padding: 3px 10px;
    font-size: 12px;
    border: 1px solid #cbd5e1;
    border-radius: 4px;
    background: #fff;
    color: #475569;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .cs-toggle-btn:hover:not(:disabled) {
    background: #f1f5f9;
  }

  .cs-toggle-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .cs-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 240px;
    overflow-y: auto;
  }

  .cs-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    transition: background-color 0.1s;
  }

  .cs-row:hover {
    background: #f8fafc;
  }

  .cs-row input[type="checkbox"] {
    flex-shrink: 0;
  }

  .cs-row-detail {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .cs-row-field {
    font-size: 13px;
    font-weight: 600;
    color: #1f2937;
    word-break: break-word;
  }

  .cs-row-node {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: baseline;
    font-size: 11px;
    color: #9ca3af;
  }

  .cs-row-node-id {
    font-family: monospace;
  }

  .cs-row-value {
    color: #374151;
    font-family: monospace;
    word-break: break-all;
  }
</style>
