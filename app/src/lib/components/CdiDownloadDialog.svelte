<script lang="ts">
  /**
   * CdiDownloadDialog — Prompts the user to download CDI for nodes whose
   * configuration definition is not yet in the local cache.
   *
   * dialog-shell-refactor (Slice 6): wraps the Fluent `Dialog` shell.
   * `closable={!downloading}` — Esc / overlay / × dismiss only while the
   * download has not started. Once in flight, the dialog locks.
   */
  import { nodeIdToDisplayHex } from '$lib/utils/nodeId';
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

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

  const nodeLabel = $derived(nodes.length === 1 ? '1 node' : `${nodes.length} nodes`);
</script>

<Dialog
  open
  width="sm"
  role="alertdialog"
  closable={!downloading}
  initialFocus="last"
  onCancel={onCancel}
>
  {#snippet title()}
    <DialogTitle glyph="info">Missing Configuration Definition</DialogTitle>
  {/snippet}

  <p class="cd-body">
    <strong>{nodeLabel}</strong> {nodes.length === 1 ? 'does' : 'do'} not have a
    Configuration Definition (CDI) file in the local cache. Settings cannot be
    read without it. Would you like to download the CDI from {nodes.length === 1 ? 'this node' : 'these nodes'} now?
  </p>

  <ul class="cd-node-list" aria-label="Nodes missing CDI">
    {#each nodes as node}
      <li class="cd-node-item">
        <div class="cd-node-info">
          <span class="cd-node-name">{node.nodeName}</span>
          <span class="cd-node-id">{nodeIdToDisplayHex(node.nodeId)}</span>
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

  {#if downloading}
    <p class="cd-status" role="status">
      Downloading CDI… ({downloadedCount} of {nodes.length})
    </p>
  {/if}

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel} disabled={downloading}>
        Cancel
      </Button>
      <Button appearance="primary" onclick={onDownload} disabled={downloading}>
        {downloading ? 'Downloading…' : 'Download'}
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .cd-body {
    margin: 0 0 8px 0;
    color: var(--fluent-neutralForeground1);
    line-height: 1.5;
  }

  /* ── Node list ── */
  .cd-node-list {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--fluent-neutralStroke1);
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
    border-bottom: 1px solid var(--fluent-neutralBackground3);
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
    color: var(--fluent-neutralForeground1);
  }
  .cd-node-id {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground3);
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
    border: 2px solid var(--fluent-neutralStroke1);
    border-top-color: var(--fluent-brandBackground);
    border-radius: 50%;
    animation: cd-spin 0.7s linear infinite;
  }
  @keyframes cd-spin {
    to { transform: rotate(360deg); }
  }

  .cd-status {
    margin: 8px 0 0 0;
    color: var(--fluent-neutralForeground2);
    font-style: italic;
    font-size: var(--fluent-fontSizeBase200);
  }
</style>
