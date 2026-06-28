<script lang="ts">
  /**
   * Dialog — Fluent v9 modal shell for Bowties.
   *
   * Owns: overlay, surface, focus trap, Esc / overlay-click / × → onCancel,
   * ARIA wiring, and the visual chrome (header divider + filled footer).
   *
   * Does NOT own: workflow sequencing or business logic. Callers decide what
   * `onCancel` (and any confirm callback they wire to an action button) does.
   * Per `frontend-components.instructions.md`, this component stays declarative.
   *
   * Slots (Svelte 5 snippets):
   *   - `title`   → header content. Omit to render a header-less dialog.
   *   - `children`→ body content. Required.
   *   - `actions` → footer content. Omit to render a footer-less dialog.
   *
   * Variants:
   *   - `closable={true}`  (default) → × close button, Esc, and overlay click
   *                                    all call `onCancel`.
   *   - `closable={false}`           → no ×, Esc/overlay click ignored. Use
   *                                    for in-flight progress dialogs.
   *
   * See specs/018-block-indicator-facility/dialog-shell-refactor.md.
   */
  import { onMount, onDestroy, untrack, type Snippet } from 'svelte';

  type Width = 'sm' | 'md' | 'lg' | number;
  type Role = 'dialog' | 'alertdialog';
  type InitialFocus = 'first' | 'last' | 'none';

  interface Props {
    /** Whether the dialog is open. Caller owns visibility. */
    open: boolean;
    /** `sm` = 400, `md` = 480, `lg` = 600. A number is treated as pixels. */
    width?: Width;
    /** When false, hides ×, ignores Esc and overlay-click. Default true. */
    closable?: boolean;
    /** ARIA role. Use `alertdialog` for destructive confirms. Default `dialog`. */
    role?: Role;
    /** Used as `aria-label` when `title` snippet is non-text. */
    ariaLabel?: string;
    /** Which focusable to focus on mount. Default `first`. */
    initialFocus?: InitialFocus;
    /**
     * Stacking z-index for the overlay. Default 1500. Use a higher value
     * (e.g. 2000 for `ErrorDialog`) when this dialog must stay above other
     * dialogs that might be open at the same time.
     */
    zIndex?: number;
    /** Called for Esc, overlay click, and × — when `closable` is true. */
    onCancel: () => void;
    title?: Snippet;
    children: Snippet;
    actions?: Snippet;
  }

  let {
    open,
    width = 'md',
    closable = true,
    role = 'dialog',
    ariaLabel,
    initialFocus = 'first',
    zIndex = 1500,
    onCancel,
    title,
    children,
    actions,
  }: Props = $props();

  let surfaceEl: HTMLDivElement | undefined = $state();
  let previouslyFocused: HTMLElement | null = null;

  const widthPx = $derived(
    typeof width === 'number'
      ? `${width}px`
      : width === 'sm'
        ? '400px'
        : width === 'lg'
          ? '600px'
          : '480px',
  );

  function focusableElements(): HTMLElement[] {
    if (!surfaceEl) return [];
    const selector = [
      'button:not([disabled])',
      '[href]',
      'input:not([disabled])',
      'select:not([disabled])',
      'textarea:not([disabled])',
      '[tabindex]:not([tabindex="-1"])',
    ].join(',');
    return Array.from(surfaceEl.querySelectorAll<HTMLElement>(selector));
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === 'Escape' && closable) {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key === 'Tab') {
      const focusables = focusableElements();
      if (focusables.length === 0) {
        event.preventDefault();
        return;
      }
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    }
  }

  function handleOverlayClick(event: MouseEvent) {
    if (!closable) return;
    if (event.target === event.currentTarget) onCancel();
  }

  function applyInitialFocus() {
    const focusables = focusableElements();
    if (focusables.length === 0) {
      surfaceEl?.focus();
      return;
    }
    if (initialFocus === 'none') return;
    if (initialFocus === 'last') focusables[focusables.length - 1].focus();
    else focusables[0].focus();
  }

  // React to `open` transitions: remember the previously focused element,
  // move focus into the dialog, and restore focus when the dialog closes.
  $effect(() => {
    if (open) {
      untrack(() => {
        previouslyFocused = document.activeElement as HTMLElement | null;
      });
      // Wait one microtask so the surface is in the DOM before focusing.
      queueMicrotask(applyInitialFocus);
    } else {
      untrack(() => {
        previouslyFocused?.focus?.();
        previouslyFocused = null;
      });
    }
  });

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fluent-dialog-overlay"
    style="z-index: {zIndex}"
    role="presentation"
    onclick={handleOverlayClick}
  >
    <div
      bind:this={surfaceEl}
      class="fluent-dialog-surface"
      style="width: {widthPx}"
      {role}
      aria-modal="true"
      aria-label={ariaLabel}
      tabindex="-1"
    >
      {#if title}
        <div class="fluent-dialog-header">
          <div class="fluent-dialog-header__title">
            {@render title()}
          </div>
          {#if closable}
            <button
              type="button"
              class="fluent-dialog-close"
              aria-label="Close"
              onclick={onCancel}
            >×</button>
          {/if}
        </div>
      {/if}

      <div class="fluent-dialog-body">
        {@render children()}
      </div>

      {#if actions}
        <div class="fluent-dialog-footer">
          {@render actions()}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .fluent-dialog-overlay {
    position: fixed;
    inset: 0;
    background: var(--fluent-dialogOverlay);
    display: flex;
    align-items: center;
    justify-content: center;
    animation: fluent-fade-in 0.15s ease-out;
    font-family: var(--fluent-fontFamily);
  }

  @keyframes fluent-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .fluent-dialog-surface {
    background: var(--fluent-neutralBackground1);
    border-radius: 8px;
    box-shadow: var(--fluent-shadow16);
    max-width: 90vw;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: fluent-slide-in 0.18s ease-out;
  }

  @keyframes fluent-slide-in {
    from { transform: translateY(-12px); opacity: 0; }
    to   { transform: translateY(0);     opacity: 1; }
  }

  .fluent-dialog-surface:focus-visible {
    outline: none;
  }

  .fluent-dialog-header {
    padding: 14px 18px;
    border-bottom: 1px solid var(--fluent-neutralStroke1);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex: 0 0 auto;
  }

  .fluent-dialog-header__title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .fluent-dialog-close {
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--fluent-neutralForeground2);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
  }
  .fluent-dialog-close:hover {
    background: var(--fluent-subtleBackgroundHover);
    color: var(--fluent-neutralForeground1);
  }
  .fluent-dialog-close:focus-visible {
    outline: 2px solid var(--fluent-strokeFocus2);
    outline-offset: 2px;
  }

  .fluent-dialog-body {
    padding: 16px 18px;
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase300);
    line-height: var(--fluent-lineHeightBase300);
    color: var(--fluent-neutralForeground2);
    overflow: auto;
    flex: 1 1 auto;
  }

  .fluent-dialog-footer {
    padding: 12px 18px;
    border-top: 1px solid var(--fluent-neutralStroke1);
    background: var(--fluent-neutralBackground2);
    flex: 0 0 auto;
  }
</style>
