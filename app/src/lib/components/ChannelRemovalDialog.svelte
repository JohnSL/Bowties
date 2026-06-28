<script lang="ts">
  /**
   * ChannelRemovalDialog — Confirmation modal shown when changing a daughter
   * board's selection will discard channels already wired to its slot.
   *
   * Spec 015 / S5; extracted from `+page.svelte` inline markup during the
   * dialog-shell-refactor (Slice 3).
   */
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    /** Number of channels that will be removed by confirming. */
    channelCount: number;
    /** Called on confirm. */
    onConfirm: () => void;
    /** Called on cancel (Esc, overlay click, ×, Cancel button). */
    onCancel: () => void;
  }

  let { channelCount, onConfirm, onCancel }: Props = $props();

  const countLabel = $derived(
    channelCount === 1 ? '1 channel' : `${channelCount} channels`,
  );
</script>

<Dialog
  open
  width="sm"
  role="alertdialog"
  ariaLabel="Channel removal confirmation"
  {onCancel}
>
  {#snippet title()}
    <DialogTitle glyph="warning">Remove Channels</DialogTitle>
  {/snippet}

  <p class="crd-body">
    Changing the daughter board will remove <strong>{countLabel}</strong>. Continue?
  </p>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" intent="danger" onclick={onConfirm}>Remove</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .crd-body {
    margin: 0;
    color: var(--fluent-neutralForeground1);
  }
</style>
