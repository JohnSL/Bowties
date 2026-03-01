<script lang="ts">
  /**
   * DiscardConfirmDialog — Confirmation modal for discarding unsaved changes.
   *
   * Keyboard behaviour:
   *   Enter  → Revert  (confirms the user's explicit request)
   *   Escape → Cancel  (safe default — no changes lost)
   *   Tab    → cycles between Cancel and Revert
   *
   * The Revert button receives focus on open so that Enter immediately
   * confirms the action the user already initiated by clicking Discard.
   * Escape always cancels regardless of focus position.
   *
   * Reusable: also used by navigation-guard prompts (FR-026).
   */
  import { onMount, onDestroy } from 'svelte';

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

  let revertBtn: HTMLButtonElement | undefined = $state();

  const fieldLabel = $derived(fieldCount === 1 ? '1 unsaved change' : `${fieldCount} unsaved changes`);
  const nodeLabel  = $derived(nodeCount  === 1 ? '1 node'           : `${nodeCount} nodes`);

  // ── Keyboard handling ────────────────────────────────────────────────────

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
    }
    // Enter is handled naturally by focus on the Revert button.
    // Tab focus-trap handled below.
  }

  function handleOverlayClick(event: MouseEvent) {
    // Clicking the dimmed backdrop cancels — same as Escape.
    if (event.target === event.currentTarget) {
      onCancel();
    }
  }

  function trapFocus(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const dialog = document.getElementById('discard-confirm-dialog');
    if (!dialog) return;
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>('button:not([disabled])')
    );
    if (focusable.length < 2) return;
    const first = focusable[0];
    const last  = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    revertBtn?.focus();
    window.addEventListener('keydown', handleKeydown);
    window.addEventListener('keydown', trapFocus);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
    window.removeEventListener('keydown', trapFocus);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
<div
  class="dc-overlay"
  role="presentation"
  onclick={handleOverlayClick}
>
  <div
    id="discard-confirm-dialog"
    class="dc-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="dc-title"
    aria-describedby="dc-body"
  >
    <!-- Header -->
    <div class="dc-header">
      <span class="dc-warning-icon" aria-hidden="true">⚠</span>
      <h2 id="dc-title" class="dc-title">Discard unsaved changes</h2>
    </div>

    <!-- Body -->
    <p id="dc-body" class="dc-body">
      This will revert <strong>{fieldLabel}</strong> across
      <strong>{nodeLabel}</strong> to their last saved values.
      This cannot be undone.
    </p>

    <!-- Actions: Cancel left (safe), Revert right (destructive) -->
    <div class="dc-actions">
      <button class="dc-btn dc-btn--cancel" onclick={onCancel}>
        Cancel
      </button>
      <button
        class="dc-btn dc-btn--revert"
        bind:this={revertBtn}
        onclick={onConfirm}
      >
        Revert
      </button>
    </div>
  </div>
</div>

<style>
  .dc-overlay {
    position: fixed;
    inset: 0;
    z-index: 1100;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    animation: dc-fade-in 0.15s ease-out;
  }

  @keyframes dc-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .dc-dialog {
    background: #ffffff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
    width: 360px;
    max-width: 90vw;
    padding: 20px 24px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    animation: dc-slide-in 0.18s ease-out;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    font-size: 13px;
  }

  @keyframes dc-slide-in {
    from { transform: translateY(-12px); opacity: 0; }
    to   { transform: translateY(0);     opacity: 1; }
  }

  /* ── Header ── */

  .dc-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dc-warning-icon {
    font-size: 16px;
    color: #ca5010;                               /* colorPaletteOrangeForeground1 */
    flex-shrink: 0;
  }

  .dc-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #201f1e;                               /* colorNeutralForeground1 */
    line-height: 1.3;
  }

  /* ── Body ── */

  .dc-body {
    margin: 0;
    color: #323130;                               /* colorNeutralForeground2 */
    line-height: 1.5;
  }

  /* ── Actions ── */

  .dc-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }

  .dc-btn {
    padding: 5px 16px;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 0.1s ease, border-color 0.1s ease;
    white-space: nowrap;
  }

  /* Cancel — neutral secondary */
  .dc-btn--cancel {
    background: #ffffff;
    color: #323130;
    border: 1px solid #c8c6c4;
  }

  .dc-btn--cancel:hover {
    background: #f3f2f1;
    border-color: #a19f9d;
  }

  .dc-btn--cancel:active {
    background: #edebe9;
  }

  /* Revert — orange warning (reflects the "unsaved changes" accent) */
  .dc-btn--revert {
    background: #ca5010;                          /* colorPaletteOrangeForeground1 */
    color: #ffffff;
    border: 1px solid transparent;
  }

  .dc-btn--revert:hover {
    background: #a33d0a;
  }

  .dc-btn--revert:active {
    background: #862f06;
  }

  .dc-btn--revert:focus-visible {
    outline: 2px solid #ca5010;
    outline-offset: 2px;
  }
</style>
