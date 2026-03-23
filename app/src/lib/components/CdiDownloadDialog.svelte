<script lang="ts">
  /**
   * CdiDownloadDialog — Prompts the user to download CDI for nodes whose
   * configuration definition is not yet in the local cache.
   *
   * Keyboard behaviour:
   *   Escape → Cancel  (safe default — no download started)
   *   Tab    → cycles between Cancel and Download
   */
  import { onMount, onDestroy } from 'svelte';

  export interface MissingCdiNode {
    nodeId: string;
    nodeName: string;
    downloadStatus?: 'waiting' | 'downloading' | 'done' | 'failed';
  }

  interface Props {
    /** Nodes that are missing CDI data in the local cache. */
    nodes: MissingCdiNode[];
    /** True while downloads are in progress — disables buttons, shows status. */
    downloading: boolean;
    /** How many nodes have been downloaded so far (for progress text). */
    downloadedCount: number;
    /** Called when the user clicks Download. */
    onDownload: () => void;
    /** Called when the user cancels the dialog. */
    onCancel: () => void;
  }

  let { nodes, downloading, downloadedCount, onDownload, onCancel }: Props = $props();

  let downloadBtn: HTMLButtonElement | undefined = $state();

  const nodeLabel = $derived(nodes.length === 1 ? '1 node' : `${nodes.length} nodes`);

  // ── Keyboard handling ────────────────────────────────────────────────────

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && !downloading) {
      event.preventDefault();
      onCancel();
    }
  }

  function handleOverlayClick(event: MouseEvent) {
    if (!downloading && event.target === event.currentTarget) {
      onCancel();
    }
  }

  function trapFocus(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const dialog = document.getElementById('cdi-download-dialog');
    if (!dialog) return;
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>('button:not([disabled])')
    );
    if (focusable.length < 2) return;
    const first = focusable[0];
    const last  = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    downloadBtn?.focus();
    window.addEventListener('keydown', handleKeydown);
    window.addEventListener('keydown', trapFocus);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
    window.removeEventListener('keydown', trapFocus);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
<div
  class="cd-overlay"
  role="presentation"
  onclick={handleOverlayClick}
>
  <div
    id="cdi-download-dialog"
    class="cd-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="cd-title"
    aria-describedby="cd-body"
  >
    <!-- Header -->
    <div class="cd-header">
      <span class="cd-info-icon" aria-hidden="true">i</span>
      <h2 id="cd-title" class="cd-title">Missing Configuration Definition</h2>
    </div>

    <!-- Body -->
    <p id="cd-body" class="cd-body">
      <strong>{nodeLabel}</strong> {nodes.length === 1 ? 'does' : 'do'} not have a
      Configuration Definition (CDI) file in the local cache. Settings cannot be
      read without it. Would you like to download the CDI from {nodes.length === 1 ? 'this node' : 'these nodes'} now?
    </p>

    <!-- Node list -->
    <ul class="cd-node-list" aria-label="Nodes missing CDI">
      {#each nodes as node}
        <li class="cd-node-item">
          <div class="cd-node-info">
            <span class="cd-node-name">{node.nodeName}</span>
            <span class="cd-node-id">{node.nodeId}</span>
          </div>
          {#if node.downloadStatus === 'downloading'}
            <span class="cd-node-status cd-node-status--downloading" aria-label="Downloading">
              <span class="cd-spinner" aria-hidden="true"></span>
            </span>
          {:else if node.downloadStatus === 'done'}
            <span class="cd-node-status cd-node-status--done" aria-label="Downloaded">✓</span>
          {:else if node.downloadStatus === 'failed'}
            <span class="cd-node-status cd-node-status--failed" aria-label="Failed">✗</span>
          {/if}
        </li>
      {/each}
    </ul>

    <!-- Download status -->
    {#if downloading}
      <p class="cd-status" role="status">
        Downloading CDI… ({downloadedCount} of {nodes.length})
      </p>
    {/if}

    <!-- Actions: Cancel left (safe), Download right (primary) -->
    <div class="cd-actions">
      <button
        class="cd-btn cd-btn--cancel"
        onclick={onCancel}
        disabled={downloading}
      >
        Cancel
      </button>
      <button
        class="cd-btn cd-btn--download"
        bind:this={downloadBtn}
        onclick={onDownload}
        disabled={downloading}
      >
        {downloading ? 'Downloading…' : 'Download'}
      </button>
    </div>
  </div>
</div>

<style>
  .cd-overlay {
    position: fixed;
    inset: 0;
    z-index: 1100;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    animation: cd-fade-in 0.15s ease-out;
  }

  @keyframes cd-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .cd-dialog {
    background: #ffffff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
    width: 420px;
    max-width: 90vw;
    max-height: 80vh;
    padding: 20px 24px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    animation: cd-slide-in 0.18s ease-out;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    font-size: 13px;
  }

  @keyframes cd-slide-in {
    from { transform: translateY(-12px); opacity: 0; }
    to   { transform: translateY(0);     opacity: 1; }
  }

  /* ── Header ── */

  .cd-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .cd-info-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #0078d4;
    color: #ffffff;
    font-size: 11px;
    font-weight: 700;
    font-style: italic;
    flex-shrink: 0;
  }

  .cd-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #201f1e;
    line-height: 1.3;
  }

  /* ── Body ── */

  .cd-body {
    margin: 0;
    color: #323130;
    line-height: 1.5;
  }

  /* ── Node list ── */

  .cd-node-list {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    overflow-y: auto;
    max-height: 180px;
  }

  .cd-node-item {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-bottom: 1px solid #f0f0f0;
    gap: 8px;
  }

  .cd-node-item:last-child {
    border-bottom: none;
  }

  .cd-node-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .cd-node-name {
    font-weight: 500;
    color: #201f1e;
  }

  .cd-node-id {
    font-size: 11px;
    color: #605e5c;
    font-family: 'Consolas', 'Courier New', monospace;
  }

  /* ── Per-node download status indicators ── */

  .cd-node-status {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    font-size: 12px;
    font-weight: 700;
    flex-shrink: 0;
  }

  .cd-node-status--done {
    background: #d4edda;
    color: #155724;
  }

  .cd-node-status--failed {
    background: #f8d7da;
    color: #721c24;
  }

  .cd-node-status--downloading {
    background: transparent;
  }

  .cd-spinner {
    display: inline-block;
    width: 14px;
    height: 14px;
    border: 2px solid #c8c6c4;
    border-top-color: #0078d4;
    border-radius: 50%;
    animation: cd-spin 0.7s linear infinite;
  }

  @keyframes cd-spin {
    to { transform: rotate(360deg); }
  }

  /* ── Status ── */

  .cd-status {
    margin: 0;
    color: #323130;
    font-style: italic;
    font-size: 12px;
  }

  /* ── Actions ── */

  .cd-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }

  .cd-btn {
    padding: 5px 16px;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 0.1s ease, border-color 0.1s ease;
    white-space: nowrap;
  }

  .cd-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  /* Cancel — neutral secondary */
  .cd-btn--cancel {
    background: #ffffff;
    color: #323130;
    border: 1px solid #c8c6c4;
  }

  .cd-btn--cancel:hover:not(:disabled) {
    background: #f3f2f1;
    border-color: #a19f9d;
  }

  .cd-btn--cancel:active:not(:disabled) {
    background: #edebe9;
  }

  /* Download — blue primary */
  .cd-btn--download {
    background: #0078d4;
    color: #ffffff;
    border: 1px solid transparent;
  }

  .cd-btn--download:hover:not(:disabled) {
    background: #006cbe;
  }

  .cd-btn--download:active:not(:disabled) {
    background: #005fa3;
  }

  .cd-btn--download:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 2px;
  }
</style>
