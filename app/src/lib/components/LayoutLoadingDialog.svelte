<script lang="ts">
  /**
   * LayoutLoadingDialog — Non-dismissible progress dialog shown while a
   * layout is opening.
   *
   * Extracted from `+page.svelte` inline markup in dialog-shell-refactor
   * (Slice 6). `closable={false}` locks the shell — Esc / overlay / × are
   * all ignored while the open is in flight. `zIndex={2200}` preserves the
   * prior stacking (above `ErrorDialog`'s 2000).
   */
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';

  interface Props {
    /** Current status text (e.g. "Reading layout files…"). */
    statusText: string;
  }

  let { statusText }: Props = $props();
</script>

<Dialog
  open
  width="sm"
  role="alertdialog"
  closable={false}
  initialFocus="none"
  zIndex={2200}
  ariaLabel="Loading layout"
  onCancel={() => {}}
>
  {#snippet title()}
    <DialogTitle>Opening layout</DialogTitle>
  {/snippet}

  <div class="ll-body" aria-live="polite">
    <div class="ll-spinner" aria-hidden="true"></div>
    <p class="ll-status">{statusText}</p>
  </div>
</Dialog>

<style>
  .ll-body {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 10px;
  }
  .ll-spinner {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid var(--fluent-neutralBackground3);
    border-top-color: var(--fluent-brandBackground);
    animation: ll-spin 0.85s linear infinite;
  }
  @keyframes ll-spin {
    to { transform: rotate(360deg); }
  }
  .ll-status {
    margin: 0;
    color: var(--fluent-neutralForeground2);
  }
</style>
