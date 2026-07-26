<script lang="ts">
  /**
   * DeleteFacilityDialog — Confirmation modal shown when deleting a facility
   * that is referenced as a downstream-signal input by other (upstream)
   * facilities. Lists the affected upstream facilities.
   *
   * Spec 020 / S6; mirrors ChannelRemovalDialog.svelte pattern.
   */
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';
  import type { FacilityRecord } from '$lib/api/facilities';

  interface Props {
    /** Name of the facility being deleted. */
    facilityName: string;
    /** Facilities whose downstream-signal input is bound to this facility's output. */
    referrers: FacilityRecord[];
    /** Called on confirm. */
    onConfirm: () => void;
    /** Called on cancel (Esc, overlay click, ×, Cancel button). */
    onCancel: () => void;
  }

  let { facilityName, referrers, onConfirm, onCancel }: Props = $props();

  const facilityLabel = $derived(
    referrers.length === 1 ? '1 facility' : `${referrers.length} facilities`,
  );
</script>

<Dialog
  open
  width="sm"
  role="alertdialog"
  ariaLabel="Facility deletion confirmation"
  {onCancel}
>
  {#snippet title()}
    <DialogTitle glyph="warning">Delete Signal Facility</DialogTitle>
  {/snippet}

  <div class="dfd-body">
    <p>
      Deleting <strong>{facilityName}</strong> will affect <strong>{facilityLabel}</strong>
      downstream. These upstream signals will no longer cascade:
    </p>
    <ul class="dfd-list">
      {#each referrers as facility (facility.facilityId)}
        <li>{facility.name}</li>
      {/each}
    </ul>
    <p class="dfd-warning">This action cannot be undone. Continue?</p>
  </div>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" intent="danger" onclick={onConfirm}>Delete</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .dfd-body {
    margin: 0;
    color: var(--fluent-neutralForeground1);
  }

  .dfd-body p {
    margin: 0.5rem 0;
    line-height: 1.4;
  }

  .dfd-list {
    margin: 0.5rem 0;
    padding-left: 1.5rem;
  }

  .dfd-list li {
    margin: 0.25rem 0;
  }

  .dfd-warning {
    margin-top: 0.75rem;
    font-weight: 500;
  }
</style>
