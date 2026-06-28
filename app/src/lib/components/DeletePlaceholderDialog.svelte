<script lang="ts">
  /**
   * DeletePlaceholderDialog — Confirmation modal for deleting a placeholder
   * board from the active offline layout.
   *
   * Spec 014 / S8.5 / T11; extracted from `+page.svelte` inline markup during
   * the dialog-shell-refactor (Slice 3). The Delete button keeps the
   * `data-testid="confirm-delete-placeholder"` selector used by integration
   * tests.
   */
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    /** Called on confirm. */
    onConfirm: () => void;
    /** Called on cancel (Esc, overlay click, ×, Cancel button). */
    onCancel: () => void;
  }

  let { onConfirm, onCancel }: Props = $props();
</script>

<Dialog
  open
  width="sm"
  role="alertdialog"
  {onCancel}
>
  {#snippet title()}
    <DialogTitle glyph="warning">Delete placeholder board?</DialogTitle>
  {/snippet}

  <p class="dpd-body">
    This will remove the placeholder board and any unsaved configuration
    changes for it. This cannot be undone.
  </p>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button
        appearance="primary"
        intent="danger"
        dataTestid="confirm-delete-placeholder"
        onclick={onConfirm}
      >Delete</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .dpd-body {
    margin: 0;
    color: var(--fluent-neutralForeground1);
  }
</style>
