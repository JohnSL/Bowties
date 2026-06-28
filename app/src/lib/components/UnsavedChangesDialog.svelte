<script lang="ts">
  /**
   * UnsavedChangesDialog — modal that lists per-bucket counts when the user
   * tries to open/close/disconnect/exit with unsaved edits in the layout.
   *
   * Spec 018 / S1.2 — replaces the inline `<div class="unsaved-dialog">`
   * markup that used to live in `+page.svelte` and the parallel
   * `changeTrackerStore` aggregation path. All counts come from
   * `effectiveNodeStore.dirtyBreakdown` (ADR-0011 extension 2026-06-28).
   *
   * Keyboard:
   *   Enter  → Confirm (the destructive action — focuses the confirm button)
   *   Escape → Cancel (safe default)
   *   Tab    → cycles Cancel ↔ Confirm
   */
  import { onMount, onDestroy } from 'svelte';
  import type { DirtyBreakdown } from '$lib/layout';

  interface Props {
    message: string;
    breakdown: DirtyBreakdown;
    confirmLabel: string;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let { message, breakdown, confirmLabel, onConfirm, onCancel }: Props = $props();

  let confirmBtn: HTMLButtonElement | undefined = $state();

  const lines = $derived(formatBreakdown(breakdown));

  function plural(n: number, singular: string, plural?: string): string {
    return n === 1 ? singular : (plural ?? `${singular}s`);
  }

  function formatBreakdown(b: DirtyBreakdown): string[] {
    const out: string[] = [];
    if (b.config > 0) {
      const fields = `${b.config} ${plural(b.config, 'config edit')}`;
      const across = b.configNodes > 0
        ? ` across ${b.configNodes} ${plural(b.configNodes, 'node')}`
        : '';
      out.push(`${fields}${across}`);
    }
    if (b.metadata > 0) {
      out.push(`${b.metadata} bowtie metadata ${plural(b.metadata, 'edit')}`);
    }
    if (b.facilities > 0) {
      out.push(`${b.facilities} facility ${plural(b.facilities, 'edit')}`);
    }
    if (b.channels > 0) {
      out.push(`${b.channels} channel ${plural(b.channels, 'edit')}`);
    }
    if (b.connectorSelections > 0) {
      out.push(
        `${b.connectorSelections} connector selection ${plural(b.connectorSelections, 'change')}`,
      );
    }
    if (b.offlineDrafts > 0) {
      out.push(`${b.offlineDrafts} offline ${plural(b.offlineDrafts, 'draft')}`);
    }
    if (b.offlineRevertedPersisted > 0) {
      out.push(
        `${b.offlineRevertedPersisted} reverted persisted ${plural(b.offlineRevertedPersisted, 'change')}`,
      );
    }
    if (b.layoutStruct > 0) {
      out.push('layout structure edits');
    }
    if (b.unsavedNewNodes > 0) {
      out.push(
        `${b.unsavedNewNodes} new ${plural(b.unsavedNewNodes, 'node')} not yet added to the layout`,
      );
    }
    if (b.unsavedRemovedNodes > 0) {
      out.push(
        `${b.unsavedRemovedNodes} ${plural(b.unsavedRemovedNodes, 'node')} removed but not yet saved`,
      );
    }
    return out;
  }

  // ── Keyboard handling ────────────────────────────────────────────────────

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
    }
  }

  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onCancel();
    }
  }

  function trapFocus(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const dialog = document.getElementById('unsaved-changes-dialog');
    if (!dialog) return;
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>('button:not([disabled])'),
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
    confirmBtn?.focus();
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
  class="uc-overlay"
  role="presentation"
  onclick={handleOverlayClick}
>
  <div
    id="unsaved-changes-dialog"
    class="uc-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="uc-title"
    aria-describedby="uc-body"
  >
    <div class="uc-header">
      <span class="uc-warning-icon" aria-hidden="true">⚠</span>
      <h2 id="uc-title" class="uc-title">Unsaved Changes</h2>
    </div>

    <div id="uc-body" class="uc-body">
      <p class="uc-message">{message}</p>
      {#if lines.length > 0}
        <ul class="uc-breakdown">
          {#each lines as line}
            <li>{line}</li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="uc-actions">
      <button class="uc-btn uc-btn--cancel" onclick={onCancel}>Cancel</button>
      <button
        class="uc-btn uc-btn--confirm"
        bind:this={confirmBtn}
        onclick={onConfirm}
      >{confirmLabel}</button>
    </div>
  </div>
</div>

<style>
  .uc-overlay {
    position: fixed;
    inset: 0;
    z-index: 1500;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    animation: uc-fade-in 0.15s ease-out;
  }

  @keyframes uc-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .uc-dialog {
    background: #ffffff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
    width: 400px;
    max-width: 90vw;
    padding: 20px 24px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    animation: uc-slide-in 0.18s ease-out;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    font-size: 13px;
  }

  @keyframes uc-slide-in {
    from { transform: translateY(-12px); opacity: 0; }
    to   { transform: translateY(0);     opacity: 1; }
  }

  .uc-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .uc-warning-icon {
    font-size: 16px;
    color: #ca5010;
    flex-shrink: 0;
  }

  .uc-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #201f1e;
    line-height: 1.3;
  }

  .uc-body {
    color: #323130;
    line-height: 1.5;
  }

  .uc-message {
    margin: 0 0 8px 0;
  }

  .uc-breakdown {
    margin: 0;
    padding-left: 20px;
    color: #605e5c;
  }

  .uc-breakdown li {
    margin: 2px 0;
  }

  .uc-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }

  .uc-btn {
    padding: 5px 16px;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 0.1s ease, border-color 0.1s ease;
    white-space: nowrap;
  }

  .uc-btn--cancel {
    background: #ffffff;
    color: #323130;
    border: 1px solid #c8c6c4;
  }

  .uc-btn--cancel:hover {
    background: #f3f2f1;
    border-color: #a19f9d;
  }

  .uc-btn--cancel:active {
    background: #edebe9;
  }

  .uc-btn--confirm {
    background: #ca5010;
    color: #ffffff;
    border: 1px solid transparent;
  }

  .uc-btn--confirm:hover {
    background: #a33d0a;
  }

  .uc-btn--confirm:active {
    background: #862f06;
  }

  .uc-btn--confirm:focus-visible {
    outline: 2px solid #ca5010;
    outline-offset: 2px;
  }
</style>
