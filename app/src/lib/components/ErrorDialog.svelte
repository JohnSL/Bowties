<script lang="ts">
  /**
   * ErrorDialog — Simple error modal for displaying error messages.
   *
   * Keyboard behaviour:
   *   Escape → Close
   *
   * Enter is intentionally ignored so selecting/copying text does not
   * accidentally dismiss the dialog.
   */
  import { onMount, onDestroy } from 'svelte';

  interface Props {
    /** Title of the error dialog */
    title: string;
    /** Error message to display */
    message: string;
    /** Called when the user closes the dialog */
    onClose: () => void;
  }

  let { title, message, onClose }: Props = $props();

  let okBtn: HTMLButtonElement | undefined = $state();
  let copyStatus = $state<'idle' | 'copied' | 'failed'>('idle');

  // ── Keyboard handling ────────────────────────────────────────────────────

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }

  async function copyMessage(): Promise<void> {
    try {
      await navigator.clipboard.writeText(message);
      copyStatus = 'copied';
    } catch {
      copyStatus = 'failed';
    }
  }

  onMount(() => {
    okBtn?.focus();
    window.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

<div
  class="ed-overlay"
  role="presentation"
>
  <div
    id="error-dialog"
    class="ed-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="ed-title"
    aria-describedby="ed-body"
  >
    <!-- Header -->
    <div class="ed-header">
      <span class="ed-error-icon" aria-hidden="true">❌</span>
      <h2 id="ed-title" class="ed-title">{title}</h2>
    </div>

    <!-- Body -->
    <p id="ed-body" class="ed-body">{message}</p>

    <!-- Actions -->
    <div class="ed-actions">
      <button
        class="ed-btn ed-btn--copy"
        onclick={copyMessage}
      >
        Copy Error
      </button>
      <button
        class="ed-btn ed-btn--ok"
        bind:this={okBtn}
        onclick={onClose}
      >
        Close
      </button>
    </div>
    {#if copyStatus === 'copied'}
      <p class="ed-copy-status" role="status">Copied to clipboard.</p>
    {:else if copyStatus === 'failed'}
      <p class="ed-copy-status ed-copy-status--failed" role="status">Could not copy. Please select and copy manually.</p>
    {/if}
  </div>
</div>

<style>
  .ed-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .ed-dialog {
    background: white;
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
    max-width: 500px;
    min-width: 300px;
    padding: 24px;
    animation: slideIn 0.2s ease-out;
  }

  @keyframes slideIn {
    from {
      transform: translateY(-20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .ed-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }

  .ed-error-icon {
    font-size: 24px;
  }

  .ed-title {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: #333;
  }

  .ed-body {
    margin: 0 0 24px 0;
    font-size: 14px;
    color: #666;
    line-height: 1.5;
    word-break: break-word;
    white-space: pre-wrap;
  }

  .ed-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .ed-btn {
    padding: 8px 16px;
    border: none;
    border-radius: 4px;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.2s ease;
    font-weight: 500;
  }

  .ed-btn--ok {
    background: #0066cc;
    color: white;
  }

  .ed-btn--copy {
    background: #f2f4f7;
    color: #1f2937;
    border: 1px solid #d0d7de;
  }

  .ed-btn--copy:hover {
    background: #e7ecf2;
  }

  .ed-btn--ok:hover {
    background: #0052a3;
  }

  .ed-btn--ok:active {
    background: #003d7a;
  }

  .ed-btn--ok:focus {
    outline: 2px solid #0066cc;
    outline-offset: 2px;
  }

  .ed-copy-status {
    margin: 10px 0 0 0;
    font-size: 12px;
    color: #256029;
  }

  .ed-copy-status--failed {
    color: #9a3412;
  }
</style>
