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
    boards,
    reason,
    error,
    onConfirm,
    onConfirmLeaveBoard,
    onCancel,
  }: {
    facilityName: string;
    /** Each board the facility occupies, with the masts, Logic and Track Circuits it holds there. */
    boards: readonly { name: string; slots: readonly BoardSlot[] }[];
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

  function joinAnd(items: readonly string[], lastJoiner: string): string {
    return items.length > 1 ? `${items.slice(0, -1).join(', ')}${lastJoiner}${items[items.length - 1]}` : items[0] ?? '';
  }

  function slotsText(slots: readonly BoardSlot[]): string {
    const labels = slots
      .map((slot) => ({ order: SLOT_ORDER.indexOf(slot.kind), index: slot.index, text: `${SLOT_LABELS[slot.kind]} ${slot.index}` }))
      .sort((a, b) => a.order - b.order || a.index - b.index)
      .map((slot) => slot.text);
    return joinAnd([...new Set(labels)], ' and ');
  }

  const resetText = $derived(joinAnd(boards.map((board) => `${slotsText(board.slots)} on ${board.name}`), ', and '));
</script>

<Dialog open={true} width="sm" role="alertdialog" ariaLabel="Remove {facilityName}?" {onCancel}>
  {#snippet title()}
    <DialogTitle glyph="warning">Remove {facilityName}?</DialogTitle>
  {/snippet}

  {#if reason}
    <p role="alert">{reason}</p>
    <p>
      You can still remove {facilityName} without resetting {resetText}. They keep their current settings.
    </p>
  {:else}
    <p>
      This resets {resetText} so they can be used again. Any hand edits to those slots on the board are lost.
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
