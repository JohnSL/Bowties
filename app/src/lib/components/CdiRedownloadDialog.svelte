<script lang="ts">
  /**
   * CdiRedownloadDialog — Compact dialog shown while re-downloading CDI for a
   * single node via the "Re-download CDI" menu item.
   *
   * Auto-starts the download on mount. Closes automatically on success.
   * A Cancel button stops the download mid-flight.
   *
   * Keyboard behaviour:
   *   Escape → Cancel (only when downloading — after success it auto-closes)
   */
  import { onMount, onDestroy } from 'svelte';
  import { cancelCdiDownload, downloadCdi } from '$lib/api/cdi';

  interface Props {
    nodeId: string;
    nodeName: string;
    onClose: () => void;
  }

  let { nodeId, nodeName, onClose }: Props = $props();

  type Status = 'downloading' | 'done' | 'failed';
  let status = $state<Status>('downloading');
  let errorMessage = $state<string | null>(null);
  let cancelling = $state(false);

  async function startDownload() {
    status = 'downloading';
    errorMessage = null;
    try {
      await downloadCdi(nodeId);
      status = 'done';
      // Auto-close after a brief moment so the user sees the success indicator.
      setTimeout(() => onClose(), 800);
    } catch (e: any) {
      const msg = String(e);
      if (msg.includes('cancelled') || msg.includes('Cancelled')) {
        // Cancelled by user — just close.
        onClose();
      } else {
        status = 'failed';
        errorMessage = msg;
      }
    }
  }

  async function handleCancel() {
    if (cancelling || status === 'done') return;
    cancelling = true;
    try {
      await cancelCdiDownload();
    } catch {
      // Ignore — the download will surface as a cancelled error in startDownload.
    }
    // onClose() is called from startDownload once the cancellation error propagates.
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && status === 'downloading' && !cancelling) {
      event.preventDefault();
      handleCancel();
    }
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget && status === 'downloading' && !cancelling) {
      handleCancel();
    }
  }

  onMount(() => {
    startDownload();
    window.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
<div
  class="cd-overlay"
  role="presentation"
  onclick={handleOverlayClick}
>
  <div
    id="cdi-redownload-dialog"
    class="cd-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="cdr-title"
    aria-describedby="cdr-body"
  >
    <!-- Header -->
    <div class="cd-header">
      <span class="cd-info-icon" aria-hidden="true">i</span>
      <h2 id="cdr-title" class="cd-title">Re-download Configuration Definition</h2>
    </div>

    <!-- Body -->
    <p id="cdr-body" class="cd-body">
      {#if status === 'downloading'}
        Downloading CDI from <strong>{nodeName}</strong>…
      {:else if status === 'done'}
        CDI downloaded successfully for <strong>{nodeName}</strong>.
      {:else}
        Failed to download CDI for <strong>{nodeName}</strong>.
      {/if}
    </p>

    <!-- Node row with status indicator -->
    <ul class="cd-node-list" aria-label="Download status">
      <li class="cd-node-item">
        <div class="cd-node-info">
          <span class="cd-node-name">{nodeName}</span>
          <span class="cd-node-id">{nodeId}</span>
        </div>
        {#if status === 'downloading'}
          <span class="cd-node-status cd-node-status--downloading" aria-label="Downloading">
            <span class="cd-spinner" aria-hidden="true"></span>
          </span>
        {:else if status === 'done'}
          <span class="cd-node-status cd-node-status--done" aria-label="Downloaded">✓</span>
        {:else if status === 'failed'}
          <span class="cd-node-status cd-node-status--failed" aria-label="Failed">✗</span>
        {/if}
      </li>
    </ul>

    {#if status === 'failed' && errorMessage}
      <p class="cd-error" role="alert">{errorMessage}</p>
    {/if}

    <!-- Actions -->
    <div class="cd-actions">
      {#if status === 'downloading'}
        <button
          class="cd-btn cd-btn--cancel"
          onclick={handleCancel}
          disabled={cancelling}
        >
          {cancelling ? 'Cancelling…' : 'Cancel'}
        </button>
      {:else if status === 'failed'}
        <button class="cd-btn cd-btn--cancel" onclick={onClose}>Close</button>
        <button class="cd-btn cd-btn--download" onclick={startDownload}>Retry</button>
      {/if}
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

  .cd-body {
    margin: 0;
    color: #323130;
    line-height: 1.5;
  }

  .cd-node-list {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    overflow-y: auto;
  }

  .cd-node-item {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    gap: 8px;
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

  .cd-error {
    margin: 0;
    color: #721c24;
    font-size: 12px;
    background: #f8d7da;
    border: 1px solid #f5c2c7;
    border-radius: 4px;
    padding: 6px 10px;
  }

  .cd-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
    min-height: 30px;
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

  .cd-btn--download {
    background: #0078d4;
    color: #ffffff;
    border: 1px solid transparent;
  }

  .cd-btn--download:hover:not(:disabled) {
    background: #006cbe;
  }

  .cd-btn--download:active:not(:disabled) {
    background: #005ba1;
  }
</style>
