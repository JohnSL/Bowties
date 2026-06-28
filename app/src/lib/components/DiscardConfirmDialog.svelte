<script lang="ts">
  /**
   * DiscardConfirmDialog — Confirmation modal for discarding unsaved changes.
   *
   * dialog-shell-refactor (Slice 3): wraps the Fluent `Dialog` shell. The
   * "Revert" verb is preserved as the button label because `SaveControls.test.ts`
   * (and the call-site UX) names the action that way; the dialog title remains
   * "Discard unsaved changes" for clarity.
   *
   * Reusable: also used by navigation-guard prompts (FR-026).
   */
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    /** Number of individual field edits that will be reverted. */
    fieldCount: number;
    /** Number of distinct nodes that have pending edits. */
    nodeCount: number;
    /** Called when the user confirms the discard. */
    onConfirm: () => void;
    /** Called when the user cancels (dialog closes without discarding). */
    onCancel: () => void;
  }

  let { fieldCount, nodeCount, onConfirm, onCancel }: Props = $props();

  const fieldLabel = $derived(fieldCount === 1 ? '1 unsaved change' : `${fieldCount} unsaved changes`);
  const nodeLabel  = $derived(nodeCount  === 1 ? '1 node'           : `${nodeCount} nodes`);
</script>

<Dialog
  open
  width="sm"
  role="alertdialog"
  {onCancel}
>
  {#snippet title()}
    <DialogTitle glyph="warning">Discard unsaved changes</DialogTitle>
  {/snippet}

  <p class="dc-body">
    This will revert <strong>{fieldLabel}</strong> across
    <strong>{nodeLabel}</strong> to their last saved values.
    This cannot be undone.
  </p>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" intent="danger" onclick={onConfirm}>Revert</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .dc-body {
    margin: 0;
    color: var(--fluent-neutralForeground1);
  }
</style>
