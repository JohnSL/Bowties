<script lang="ts">
  /**
   * Destructive confirmation for removing any facility that claims board slots. Renders the
   * per-board list and emits intent only; FacilitiesSection sequences the removal.
   */
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';

  let {
    facilityName,
    groups,
    reason,
    error,
    busy = false,
    onConfirm,
    onCancel,
  }: {
    facilityName: string;
    /** Each board the facility occupies, with what it holds there. */
    groups: readonly { board: string; items: readonly string[] }[];
    /** Why the board could not be reset; offers removal without resetting it. */
    reason?: string;
    /** A short message when the removal itself failed. */
    error?: string;
    busy?: boolean;
    /** `leaveBoard` is true only from the refusal state. */
    onConfirm: (leaveBoard: boolean) => void;
    onCancel: () => void;
  } = $props();

  function joinAnd(items: readonly string[]): string {
    return items.length > 1 ? `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}` : items[0] ?? '';
  }
</script>

<Dialog open={true} width="sm" role="alertdialog" ariaLabel="Remove {facilityName}?" {onCancel}>
  {#snippet title()}
    <DialogTitle glyph="warning">Remove {facilityName}?</DialogTitle>
  {/snippet}

  {#if reason}
    <p role="alert">{reason}</p>
    <p>You can still remove {facilityName} without resetting the board. Its slots keep their current settings.</p>
  {:else}
    <p>This resets these so they can be used again. Any hand edits to those slots on the board are lost.</p>
    <ul>
      {#each groups as group (group.board)}
        <li>On {group.board}: resets {joinAnd(group.items)}</li>
      {/each}
    </ul>
    <p>Nothing changes on the board until you Save, and Discard restores everything.</p>
    {#if error}
      <p role="alert">{error}</p>
    {/if}
  {/if}

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      {#if reason}
        <Button appearance="primary" intent="danger" disabled={busy} onclick={() => onConfirm(true)}>Remove without resetting the board</Button>
      {:else}
        <Button appearance="primary" intent="danger" disabled={busy} onclick={() => onConfirm(false)}>Remove</Button>
      {/if}
    </DialogActions>
  {/snippet}
</Dialog>
