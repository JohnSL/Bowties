<script lang="ts">
  /**
   * CdiRedownloadDialog — Compact dialog shown while re-downloading CDI for a
   * single node via the "Re-download CDI" menu item.
   *
   * Auto-starts the download on mount. Closes automatically on success.
   * A Cancel button stops the download mid-flight.
   *
   * dialog-shell-refactor (Slice 6): wraps the Fluent `Dialog` shell.
   * `closable={status === 'downloading' && !cancelling}` — Esc / overlay /
   * × triggers Cancel only while a download is in-flight (and not already
   * cancelling). In the `done` state the dialog auto-closes; in the `failed`
   * state the explicit Close button calls `onClose`.
   */
  import { onMount } from 'svelte';
  import { cancelCdiDownload, downloadCdi } from '$lib/api/cdi';
  import { getCdiErrorMessage } from '$lib/types/cdi';
  import { nodeIdToDisplayHex } from '$lib/utils/nodeId';
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    nodeId: string;
    nodeName: string;
    onClose: () => void;
  }

  let { nodeId, nodeName, onClose }: Props = $props();

  const displayNodeId = $derived(nodeIdToDisplayHex(nodeId));

  type Status = 'downloading' | 'done' | 'failed';
  let status = $state<Status>('downloading');
  let errorMessage = $state<string | null>(null);
  let cancelling = $state(false);

  const canDismiss = $derived(status === 'downloading' && !cancelling);

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
        errorMessage = getCdiErrorMessage(e);
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

  // Shell `onCancel` hook: route to whatever cancel/close semantics apply.
  function shellCancel() {
    if (status === 'downloading') void handleCancel();
    else if (status === 'failed') onClose();
  }

  onMount(() => {
    startDownload();
  });
</script>

{#snippet downloadingActions()}
  <DialogActions>
    <Button appearance="secondary" onclick={handleCancel} disabled={cancelling}>
      {cancelling ? 'Cancelling…' : 'Cancel'}
    </Button>
  </DialogActions>
{/snippet}

{#snippet failedActions()}
  <DialogActions>
    <Button appearance="secondary" onclick={onClose}>Close</Button>
    <Button appearance="primary" onclick={startDownload}>Retry</Button>
  </DialogActions>
{/snippet}

<Dialog
  open
  width="sm"
  role="alertdialog"
  closable={canDismiss}
  initialFocus="none"
  actions={status === 'downloading' ? downloadingActions
           : status === 'failed'   ? failedActions
           : undefined}
  onCancel={shellCancel}
>
  {#snippet title()}
    <DialogTitle glyph="info">Re-download Configuration Definition</DialogTitle>
  {/snippet}

  <p class="cdr-body">
    {#if status === 'downloading'}
      Downloading CDI from <strong>{nodeName}</strong>…
    {:else if status === 'done'}
      CDI downloaded successfully for <strong>{nodeName}</strong>.
    {:else}
      Failed to download CDI for <strong>{nodeName}</strong>.
    {/if}
  </p>

  <ul class="cdr-node-list" aria-label="Download status">
    <li class="cdr-node-item">
      <div class="cdr-node-info">
        <span class="cdr-node-name">{nodeName}</span>
        <span class="cdr-node-id">{displayNodeId}</span>
      </div>
      {#if status === 'downloading'}
        <span class="cdr-node-status cdr-node-status--downloading" aria-label="Downloading">
          <span class="cdr-spinner" aria-hidden="true"></span>
        </span>
      {:else if status === 'done'}
        <span class="cdr-node-status cdr-node-status--done" aria-label="Downloaded">✓</span>
      {:else if status === 'failed'}
        <span class="cdr-node-status cdr-node-status--failed" aria-label="Failed">✗</span>
      {/if}
    </li>
  </ul>

  {#if status === 'failed' && errorMessage}
    <p class="cdr-error" role="alert">{errorMessage}</p>
  {/if}
</Dialog>

<style>
  .cdr-body {
    margin: 0 0 8px 0;
    color: var(--fluent-neutralForeground1);
    line-height: 1.5;
  }
  .cdr-node-list {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    overflow: hidden;
  }
  .cdr-node-item {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    gap: 8px;
  }
  .cdr-node-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .cdr-node-name {
    font-weight: 500;
    color: var(--fluent-neutralForeground1);
  }
  .cdr-node-id {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground3);
    font-family: 'Consolas', 'Courier New', monospace;
  }
  .cdr-node-status {
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
  .cdr-node-status--done {
    background: #d4edda;
    color: #155724;
  }
  .cdr-node-status--failed {
    background: #f8d7da;
    color: #721c24;
  }
  .cdr-node-status--downloading {
    background: transparent;
  }
  .cdr-spinner {
    display: inline-block;
    width: 14px;
    height: 14px;
    border: 2px solid var(--fluent-neutralStroke1);
    border-top-color: var(--fluent-brandBackground);
    border-radius: 50%;
    animation: cdr-spin 0.7s linear infinite;
  }
  @keyframes cdr-spin {
    to { transform: rotate(360deg); }
  }
  .cdr-error {
    margin: 8px 0 0 0;
    color: var(--fluent-dangerBackground);
    font-size: var(--fluent-fontSizeBase200);
  }
</style>
