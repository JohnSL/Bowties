<script lang="ts">
  /**
   * AddBoardDialog — modal picker for adding a placeholder board to the
   * active offline layout (Spec 014 / S8 + S8.5).
   *
   * Lists bundled board-model profiles (FR-018a) sorted by manufacturer
   * and model. Implicit-naming pivot (2026-05-25): the dialog no longer
   * prompts for a name — the user just picks a profile and confirms. The
   * sidebar falls back to `{manufacturer} — {model}` until the CDI User
   * Name leaf is edited.
   *
   * Keyboard behaviour:
   *   Enter  → confirm (when a profile is selected)
   *   Escape → cancel
   *   Tab    → focus-trapped inside the dialog
   *
   * Visual style mirrors `DiscardConfirmDialog.svelte` for consistency
   * with other Bowties modals.
   */
  import { onMount, onDestroy } from 'svelte';
  import { listBundledProfiles, type BundledProfileSummary } from '$lib/api/layout';
  import { addPlaceholderBoard } from '$lib/orchestration/placeholderBoardOrchestrator';

  interface Props {
    /** Called when the user cancels (Escape, backdrop click, or Cancel). */
    onCancel: () => void;
    /** Called after a successful add, with the new `placeholder:<uuidv4>` NodeKey. */
    onAdded: (nodeKey: string) => void;
  }

  let { onCancel, onAdded }: Props = $props();

  let profiles = $state<BundledProfileSummary[] | null>(null);
  let loadError = $state<string | null>(null);
  let selectedStem = $state<string | null>(null);
  let submitting = $state(false);
  let submitError = $state<string | null>(null);

  let profileSelect: HTMLSelectElement | undefined = $state();

  const canSubmit = $derived(selectedStem !== null && !submitting);

  // ── Load profiles on mount ──────────────────────────────────────────────

  async function loadProfiles() {
    try {
      const result = await listBundledProfiles();
      profiles = result;
      if (result.length > 0 && selectedStem === null) {
        selectedStem = result[0].stem;
      }
    } catch (err) {
      loadError = String(err);
    }
  }

  // ── Submit ──────────────────────────────────────────────────────────────

  async function handleSubmit() {
    if (!canSubmit || selectedStem === null) return;
    submitting = true;
    submitError = null;
    try {
      const { nodeKey } = await addPlaceholderBoard({ profileStem: selectedStem });
      onAdded(nodeKey);
    } catch (err) {
      submitError = String(err);
    } finally {
      submitting = false;
    }
  }

  // ── Keyboard handling ───────────────────────────────────────────────────

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
    } else if (event.key === 'Enter' && canSubmit) {
      // Only intercept Enter when not inside a multi-line/textarea control.
      const target = event.target as HTMLElement | null;
      if (target?.tagName !== 'TEXTAREA') {
        event.preventDefault();
        void handleSubmit();
      }
    }
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) onCancel();
  }

  function trapFocus(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const dialog = document.getElementById('add-board-dialog');
    if (!dialog) return;
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(
        'input:not([disabled]), select:not([disabled]), button:not([disabled])',
      ),
    );
    if (focusable.length < 2) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    void loadProfiles();
    profileSelect?.focus();
    window.addEventListener('keydown', handleKeydown);
    window.addEventListener('keydown', trapFocus);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
    window.removeEventListener('keydown', trapFocus);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
<div class="abd-overlay" role="presentation" onclick={handleOverlayClick}>
  <div
    id="add-board-dialog"
    class="abd-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="abd-title"
  >
    <div class="abd-header">
      <h2 id="abd-title" class="abd-title">Add placeholder board</h2>
    </div>

    <div class="abd-body">
      <label class="abd-field">
        <span class="abd-label">Board model</span>
        {#if loadError}
          <div class="abd-error">Failed to load profiles: {loadError}</div>
        {:else if profiles === null}
          <div class="abd-loading">Loading…</div>
        {:else if profiles.length === 0}
          <div class="abd-empty">No bundled board profiles are available.</div>
        {:else}
          <select
            bind:this={profileSelect}
            bind:value={selectedStem}
            class="abd-input"
            disabled={submitting}
          >
            {#each profiles as profile (profile.stem)}
              <option value={profile.stem}>
                {profile.manufacturer} — {profile.model}
              </option>
            {/each}
          </select>
        {/if}
      </label>

      {#if submitError}
        <div class="abd-error">{submitError}</div>
      {/if}
    </div>

    <div class="abd-actions">
      <button class="abd-btn abd-btn--cancel" onclick={onCancel} disabled={submitting}>
        Cancel
      </button>
      <button
        class="abd-btn abd-btn--primary"
        onclick={handleSubmit}
        disabled={!canSubmit}
      >
        Add board
      </button>
    </div>
  </div>
</div>

<style>
  .abd-overlay {
    position: fixed;
    inset: 0;
    z-index: 1100;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    animation: abd-fade-in 0.15s ease-out;
  }

  @keyframes abd-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .abd-dialog {
    background: #ffffff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
    width: 420px;
    max-width: 90vw;
    padding: 20px 24px 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: abd-slide-in 0.18s ease-out;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    font-size: 13px;
  }

  @keyframes abd-slide-in {
    from { transform: translateY(-12px); opacity: 0; }
    to   { transform: translateY(0);     opacity: 1; }
  }

  .abd-header { display: flex; align-items: center; }

  .abd-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #201f1e;
    line-height: 1.3;
  }

  .abd-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .abd-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .abd-label {
    font-size: 12px;
    font-weight: 600;
    color: #323130;
  }

  .abd-input {
    padding: 6px 8px;
    font-size: 13px;
    font-family: inherit;
    color: #201f1e;
    background: #ffffff;
    border: 1px solid #c8c6c4;
    border-radius: 4px;
  }

  .abd-input:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 1px;
  }

  .abd-loading,
  .abd-empty {
    color: #605e5c;
    font-style: italic;
    padding: 4px 0;
  }

  .abd-error {
    color: #a4262c;
    background: #fde7e9;
    padding: 6px 8px;
    border-radius: 4px;
    border: 1px solid #f1bbbc;
  }

  .abd-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }

  .abd-btn {
    padding: 5px 16px;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 0.1s ease, border-color 0.1s ease;
    white-space: nowrap;
  }

  .abd-btn:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .abd-btn--cancel {
    background: #ffffff;
    color: #323130;
    border: 1px solid #c8c6c4;
  }

  .abd-btn--cancel:hover:not(:disabled) {
    background: #f3f2f1;
    border-color: #a19f9d;
  }

  .abd-btn--primary {
    background: #0078d4;
    color: #ffffff;
    border: 1px solid transparent;
  }

  .abd-btn--primary:hover:not(:disabled) { background: #106ebe; }
  .abd-btn--primary:active:not(:disabled) { background: #005a9e; }

  .abd-btn--primary:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 2px;
  }
</style>
