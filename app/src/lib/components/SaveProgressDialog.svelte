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
   */

  import { saveProgressStore } from '$lib/stores/saveProgress.svelte';

  let visible = $derived(saveProgressStore.isVisible && saveProgressStore.phase !== 'complete');
  let phase = $derived(saveProgressStore.phase);
  let current = $derived(saveProgressStore.busWriteCurrent);
  let total = $derived(saveProgressStore.busWriteTotal);
  let label = $derived(saveProgressStore.currentLabel);
  let errorMessage = $derived(saveProgressStore.errorMessage);

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

{#if visible}
  <div class="sp-overlay" role="presentation">
    <div
      class="sp-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="sp-title"
      aria-live="polite"
      aria-busy={phase !== 'error'}
    >
      <h2 id="sp-title" class="sp-title">
        {#if phase === 'saving-layout'}
          Saving layout…
        {:else if phase === 'writing-config'}
          Writing configuration to bus
        {:else if phase === 'reconciling'}
          Updating layout…
        {:else if phase === 'error'}
          ⚠ Save failed
        {/if}
      </h2>

      <div class="sp-body">
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
          <div class="sp-actions">
            <button type="button" class="sp-dismiss" onclick={dismiss}>Dismiss</button>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .sp-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1500;
  }

  .sp-dialog {
    background: #ffffff;
    border-radius: 6px;
    padding: 20px 24px;
    min-width: 320px;
    max-width: 480px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  .sp-title {
    margin: 0 0 12px 0;
    font-size: 16px;
    font-weight: 600;
    color: #201f1e;
  }

  .sp-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .sp-line {
    margin: 0;
    font-size: 13px;
    color: #323130;
    font-variant-numeric: tabular-nums;
  }

  .sp-bar-track {
    width: 100%;
    height: 6px;
    background: #edebe9;
    border-radius: 3px;
    overflow: hidden;
  }

  .sp-bar-fill {
    height: 100%;
    background: #0078d4;
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
    color: #a4262c;
  }

  .sp-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 6px;
  }

  .sp-dismiss {
    padding: 6px 16px;
    font-size: 13px;
    background: #0078d4;
    color: #ffffff;
    border: 1px solid #0078d4;
    border-radius: 3px;
    cursor: pointer;
  }

  .sp-dismiss:hover {
    background: #106ebe;
    border-color: #106ebe;
  }

  .sp-dismiss:focus-visible {
    outline: 2px solid #2b88d8;
    outline-offset: 2px;
  }
</style>
