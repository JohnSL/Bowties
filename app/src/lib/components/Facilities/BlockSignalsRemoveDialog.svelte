<script lang="ts">
  /**
   * Destructive confirmation for removing an Automatic Block Signals facility.
   * Renders and emits intent only; FacilitiesSection sequences the removal.
   */
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';
  import type { BoardSlot } from '$lib/utils/facilityBoardSlots';

  let {
    facilityName,
    boardName,
    slots,
    reason,
    error,
    onConfirm,
    onConfirmLeaveBoard,
    onCancel,
  }: {
    facilityName: string;
    boardName: string;
    /** The masts, Logic and Track Circuits the facility occupies on the board. */
    slots: readonly BoardSlot[];
    /** Why the board could not be reset; offers removal without resetting it. */
    reason?: string;
    /** A short message when the removal itself failed. */
    error?: string;
    onConfirm: () => void;
    onConfirmLeaveBoard: () => void;
    onCancel: () => void;
  } = $props();

  const SLOT_LABELS = { mast: 'Mast', logic: 'Logic', trackCircuit: 'Track Circuit' } as const;
  const SLOT_ORDER = ['mast', 'logic', 'trackCircuit'] as const;

  const slotsText = $derived.by(() => {
    const labels = slots
      .map((slot) => ({ order: SLOT_ORDER.indexOf(slot.kind), index: slot.index, text: `${SLOT_LABELS[slot.kind]} ${slot.index}` }))
      .sort((a, b) => a.order - b.order || a.index - b.index)
      .map((slot) => slot.text);
    const unique = [...new Set(labels)];
    return unique.length > 1 ? `${unique.slice(0, -1).join(', ')} and ${unique[unique.length - 1]}` : unique[0] ?? '';
  });
</script>

<Dialog open={true} width="sm" role="alertdialog" ariaLabel="Remove {facilityName}?" {onCancel}>
  {#snippet title()}
    <DialogTitle glyph="warning">Remove {facilityName}?</DialogTitle>
  {/snippet}

  {#if reason}
    <p role="alert">{reason}</p>
    <p>
      You can still remove {facilityName} without resetting {slotsText} on {boardName}. They keep their current settings.
    </p>
  {:else}
    <p>
      This resets {slotsText} on {boardName} so they can be used again. Any hand edits to those slots on the board are lost.
    </p>
    <p>Nothing changes on the board until you Save, and Discard restores everything.</p>
    {#if error}
      <p role="alert">{error}</p>
    {/if}
  {/if}

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      {#if reason}
        <Button appearance="primary" intent="danger" onclick={onConfirmLeaveBoard}>Remove without resetting the board</Button>
      {:else}
        <Button appearance="primary" intent="danger" onclick={onConfirm}>Remove</Button>
      {/if}
    </DialogActions>
  {/snippet}
</Dialog>
