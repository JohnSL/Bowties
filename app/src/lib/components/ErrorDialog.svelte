<script lang="ts">
  /**
   * ErrorDialog — Simple error modal for displaying error messages.
   *
   * dialog-shell-refactor (Slice 3): wraps the Fluent `Dialog` shell.
   * Uses `zIndex=2000` so an error raised while another dialog is open
   * still stacks on top (preserves the prior behavior).
   *
   * Keyboard:
   *   Esc / overlay click / × → Close (provided by shell)
   *   Enter is intentionally NOT bound to Close so the user can select
   *   and copy text without accidentally dismissing the dialog.
   */
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    /** Title of the error dialog */
    title: string;
    /** Error message to display */
    message: string;
    /** Called when the user closes the dialog */
    onClose: () => void;
  }

  let { title: titleText, message, onClose }: Props = $props();

  let copyStatus = $state<'idle' | 'copied' | 'failed'>('idle');

  async function copyMessage(): Promise<void> {
    try {
      await navigator.clipboard.writeText(message);
      copyStatus = 'copied';
    } catch {
      copyStatus = 'failed';
    }
  }
</script>

<Dialog
  open
  width="md"
  role="alertdialog"
  zIndex={2000}
  onCancel={onClose}
>
  {#snippet title()}
    <DialogTitle glyph="error">{titleText}</DialogTitle>
  {/snippet}

  <p class="ed-body">{message}</p>
  {#if copyStatus === 'copied'}
    <p class="ed-copy-status" role="status">Copied to clipboard.</p>
  {:else if copyStatus === 'failed'}
    <p class="ed-copy-status ed-copy-status--failed" role="status">
      Could not copy. Please select and copy manually.
    </p>
  {/if}

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={copyMessage}>Copy Error</Button>
      <Button appearance="primary" onclick={onClose}>Close</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .ed-body {
    margin: 0;
    color: var(--fluent-neutralForeground1);
    line-height: 1.5;
    word-break: break-word;
    white-space: pre-wrap;
  }
  .ed-copy-status {
    margin: 10px 0 0 0;
    font-size: var(--fluent-fontSizeBase200);
    color: #256029;
  }
  .ed-copy-status--failed {
    color: #9a3412;
  }
</style>
