<script lang="ts">
  import { syncPanelStore, type ConflictResolution } from '$lib/stores/syncPanel.svelte';
  import type { SyncRow } from '$lib/api/sync';

  let { row }: { row: SyncRow } = $props();

  let resolution = $derived(syncPanelStore.getResolution(row.changeId));

  function choose(choice: ConflictResolution) {
    syncPanelStore.resolveConflict(row.changeId, choice);
  }
</script>

<div
  class="cr-card"
  class:cr-resolved={resolution !== undefined}
>
  <div class="cr-header">
    <div class="cr-heading">
      {#if row.fieldLabel}
        <span class="cr-field">{row.fieldLabel}</span>
      {/if}
      {#if row.nodeName || row.nodeId}
        <span class="cr-node-line">
          {#if row.nodeName}
            <span class="cr-node-name">{row.nodeName}</span>
          {/if}
          {#if row.nodeId}
            <span class="cr-node">{row.nodeId}</span>
          {/if}
        </span>
      {/if}
    </div>
    {#if resolution}
      <span class="cr-badge" class:cr-badge-apply={resolution === 'apply'} class:cr-badge-skip={resolution === 'skip'}>
        {resolution === 'apply' ? 'Will apply' : 'Skipped'}
      </span>
    {/if}
  </div>

  <div class="cr-values">
    <div class="cr-col">
      <span class="cr-label">Baseline</span>
      <span class="cr-value">{row.baselineValue}</span>
    </div>
    <div class="cr-col">
      <span class="cr-label">Planned</span>
      <span class="cr-value cr-value-planned">{row.plannedValue}</span>
    </div>
    <div class="cr-col">
      <span class="cr-label">Bus (current)</span>
      <span class="cr-value cr-value-bus">{row.busValue ?? '—'}</span>
    </div>
  </div>

  <div class="cr-actions">
    <button
      class="cr-btn cr-btn-apply"
      class:cr-btn-active={resolution === 'apply'}
      onclick={() => choose('apply')}
    >
      Apply offline value
    </button>
    <button
      class="cr-btn cr-btn-skip"
      class:cr-btn-active={resolution === 'skip'}
      onclick={() => choose('skip')}
    >
      Keep bus value
    </button>
  </div>
</div>

<style>
  .cr-card {
    border: 1px solid #fca5a5;
    border-radius: 8px;
    padding: 12px 14px;
    background: #fef2f2;
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition: border-color 0.15s, background-color 0.15s;
  }

  .cr-resolved {
    border-color: #d1d5db;
    background: #f9fafb;
  }

  .cr-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }

  .cr-heading {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .cr-field {
    font-size: 13px;
    font-weight: 600;
    color: #1f2937;
    word-break: break-word;
  }

  .cr-node-line {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: baseline;
  }

  .cr-node-name {
    font-size: 12px;
    color: #4b5563;
  }

  .cr-node {
    font-size: 12px;
    font-family: monospace;
    color: #6b7280;
  }

  .cr-badge {
    font-size: 11px;
    font-weight: 500;
    padding: 1px 6px;
    border-radius: 9999px;
  }

  .cr-badge-apply {
    background: #dbeafe;
    color: #1e40af;
  }

  .cr-badge-skip {
    background: #f3f4f6;
    color: #6b7280;
  }

  /* ─── Three-column value comparison ───────────────────── */
  .cr-values {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 8px;
  }

  .cr-col {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .cr-label {
    font-size: 11px;
    font-weight: 500;
    color: #9ca3af;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .cr-value {
    font-size: 13px;
    color: #374151;
    word-break: break-all;
    font-family: monospace;
    background: #fff;
    padding: 4px 6px;
    border-radius: 4px;
    border: 1px solid #e5e7eb;
  }

  .cr-value-planned {
    border-color: #93c5fd;
    background: #eff6ff;
  }

  .cr-value-bus {
    border-color: #fca5a5;
    background: #fff1f2;
  }

  /* ─── Action buttons ──────────────────────────────────── */
  .cr-actions {
    display: flex;
    gap: 8px;
  }

  .cr-btn {
    flex: 1;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 500;
    border-radius: 5px;
    cursor: pointer;
    transition: background-color 0.15s, border-color 0.15s;
  }

  .cr-btn-apply {
    background: #fff;
    color: #1e40af;
    border: 1px solid #93c5fd;
  }

  .cr-btn-apply:hover {
    background: #dbeafe;
  }

  .cr-btn-apply.cr-btn-active {
    background: #2563eb;
    color: #fff;
    border-color: #2563eb;
  }

  .cr-btn-skip {
    background: #fff;
    color: #6b7280;
    border: 1px solid #d1d5db;
  }

  .cr-btn-skip:hover {
    background: #f3f4f6;
  }

  .cr-btn-skip.cr-btn-active {
    background: #6b7280;
    color: #fff;
    border-color: #6b7280;
  }
</style>
