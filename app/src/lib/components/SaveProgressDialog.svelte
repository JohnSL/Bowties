<script lang="ts">
  /**
   * SaveProgressDialog — modal feedback during the three-phase save flow.
   *
   * Spec 013 / S3. Reads phase + per-field counters from `saveProgressStore`
   * and displays one of:
   *   - "Saving layout…"             (phase `saving-layout`)
   *   - "Writing N of M: <label>…"   (phase `writing-config`)
   *   - "Updating layout…"           (phase `reconciling`)
   *   - "Save failed"                (phase `error`)
   *
   * The dialog is modal — while visible no other save can be initiated
   * (see usage in `+page.svelte`). On `complete` the dialog dismisses
   * immediately (the save just succeeded — lingering "✓ Save complete"
   * makes the app feel slower than it is). On `error` the dialog stays
   * visible until the user clicks Dismiss so the failure message can be
   * read.
   *
   * dialog-shell-refactor (Slice 6): wraps the Fluent `Dialog` shell.
   * `closable={phase === 'error'}` — only the failure state can be
   * dismissed via Esc / overlay / ×. In-flight phases lock the shell.
   */

  import { saveProgressStore } from '$lib/stores/saveProgress.svelte';
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  let visible = $derived(saveProgressStore.isVisible && saveProgressStore.phase !== 'complete');
  let phase = $derived(saveProgressStore.phase);
  let current = $derived(saveProgressStore.busWriteCurrent);
  let total = $derived(saveProgressStore.busWriteTotal);
  let label = $derived(saveProgressStore.currentLabel);
  let errorMessage = $derived(saveProgressStore.errorMessage);

  let inError = $derived(phase === 'error');

  $effect(() => {
    if (phase === 'complete') {
      // Dismiss immediately: the save finished successfully and any
      // lingering "Save complete" panel just makes the app feel slow.
      saveProgressStore.reset();
    }
  });

  function dismiss(): void {
    saveProgressStore.reset();
  }
</script>

{#snippet errorActions()}
  <DialogActions>
    <Button appearance="primary" onclick={dismiss}>Dismiss</Button>
  </DialogActions>
{/snippet}

<Dialog
  open={visible}
  width="sm"
  role="alertdialog"
  closable={inError}
  initialFocus={inError ? 'first' : 'none'}
  actions={inError ? errorActions : undefined}
  onCancel={dismiss}
>
  {#snippet title()}
    <DialogTitle glyph={inError ? 'warning' : null}>
      {#if phase === 'saving-layout'}
        Saving layout…
      {:else if phase === 'writing-config'}
        Writing configuration to bus
      {:else if phase === 'reconciling'}
        Updating layout…
      {:else if phase === 'error'}
        Save failed
      {/if}
    </DialogTitle>
  {/snippet}

  <div class="sp-body" aria-live="polite" aria-busy={!inError}>
    {#if phase === 'writing-config'}
      <p class="sp-line">
        {#if total > 0}
          Writing {current} of {total}{label ? `: ${label}` : ''}…
        {:else}
          Preparing bus writes…
        {/if}
      </p>
      {#if total > 0}
        <div class="sp-bar-track" aria-hidden="true">
          <div
            class="sp-bar-fill"
            style="width: {Math.min(100, Math.round((current / total) * 100))}%"
          ></div>
        </div>
      {:else}
        <div class="sp-bar-track" aria-hidden="true">
          <div class="sp-bar-fill sp-bar-indeterminate"></div>
        </div>
      {/if}
    {:else if phase === 'saving-layout' || phase === 'reconciling'}
      <div class="sp-bar-track" aria-hidden="true">
        <div class="sp-bar-fill sp-bar-indeterminate"></div>
      </div>
    {:else if phase === 'error'}
      <p class="sp-line sp-error-message">
        {errorMessage ?? 'The save did not finish. Please try again.'}
      </p>
    {/if}
  </div>
</Dialog>

<style>
  .sp-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sp-line {
    margin: 0;
    color: var(--fluent-neutralForeground1);
    font-variant-numeric: tabular-nums;
  }
  .sp-bar-track {
    width: 100%;
    height: 6px;
    background: var(--fluent-neutralBackground3);
    border-radius: 3px;
    overflow: hidden;
  }
  .sp-bar-fill {
    height: 100%;
    background: var(--fluent-brandBackground);
    transition: width 150ms ease-out;
  }
  .sp-bar-indeterminate {
    width: 35% !important;
    animation: sp-indeterminate 1.2s ease-in-out infinite;
  }
  @keyframes sp-indeterminate {
    0%   { margin-left: -35%; }
    100% { margin-left: 100%; }
  }
  .sp-error-message {
    white-space: pre-wrap;
    color: var(--fluent-dangerBackground);
  }
</style>
