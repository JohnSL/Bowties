<script lang="ts">
  /**
   * Button — Fluent v9 button styles for Bowties.
   *
   * Variants:
   *   appearance="primary"  + intent="default" → brand blue (default action)
   *   appearance="primary"  + intent="danger"  → danger red  (destructive action)
   *   appearance="secondary"                   → neutral outlined (Cancel, dismiss)
   *   appearance="subtle"                      → text-only, hover fill (inline link-like)
   *   appearance="outline"                     → outlined neutral, transparent fill
   *
   * Sizes:
   *   size="sm" → compact (12px text, 4px×8px padding)
   *   size="md" → default (14px text, 5px×12px padding)
   *
   * See specs/018-block-indicator-facility/dialog-shell-refactor.md.
   */
  import type { Snippet } from 'svelte';

  type Appearance = 'primary' | 'secondary' | 'subtle' | 'outline';
  type Intent = 'default' | 'danger';
  type Size = 'sm' | 'md';

  interface Props {
    appearance?: Appearance;
    intent?: Intent;
    size?: Size;
    disabled?: boolean;
    type?: 'button' | 'submit' | 'reset';
    title?: string;
    ariaLabel?: string;
    /** Forwarded as `data-testid` for test-stable selectors. */
    dataTestid?: string;
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
    /** Extra class names appended after the variant classes. */
    class?: string;
    /** Imperative ref for the underlying button element. */
    ref?: HTMLButtonElement;
  }

  let {
    appearance = 'secondary',
    intent = 'default',
    size = 'md',
    disabled = false,
    type = 'button',
    title,
    ariaLabel,
    dataTestid,
    onclick,
    children,
    class: extraClass = '',
    ref = $bindable(),
  }: Props = $props();
</script>

<button
  bind:this={ref}
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  data-testid={dataTestid}
  class="fluent-btn fluent-btn--{appearance} fluent-btn--{size} {intent === 'danger' ? 'fluent-btn--danger' : ''} {extraClass}"
  onclick={onclick}
>{@render children()}</button>

<style>
  .fluent-btn {
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase300);
    font-weight: var(--fluent-fontWeightSemibold);
    line-height: var(--fluent-lineHeightBase300);
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
    transition: background-color 0.1s ease, border-color 0.1s ease, color 0.1s ease;
    padding: 5px 12px;
    border: 1px solid transparent;
  }

  .fluent-btn:focus-visible {
    outline: 2px solid var(--fluent-strokeFocus2);
    outline-offset: 2px;
  }

  .fluent-btn:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  /* ── Size ─────────────────────────────────────────────────────────── */

  .fluent-btn--sm {
    font-size: var(--fluent-fontSizeBase200);
    line-height: var(--fluent-lineHeightBase200);
    padding: 3px 8px;
  }

  .fluent-btn--md {
    /* base */
  }

  /* ── Appearance: primary (brand) ──────────────────────────────────── */

  .fluent-btn--primary {
    background: var(--fluent-brandBackground);
    color: var(--fluent-neutralForegroundOnBrand);
    border-color: var(--fluent-brandBackground);
  }
  .fluent-btn--primary:hover:not(:disabled) {
    background: var(--fluent-brandBackgroundHover);
    border-color: var(--fluent-brandBackgroundHover);
  }
  .fluent-btn--primary:active:not(:disabled) {
    background: var(--fluent-brandBackgroundPressed);
    border-color: var(--fluent-brandBackgroundPressed);
  }

  /* ── Appearance: primary + intent=danger ──────────────────────────── */

  .fluent-btn--primary.fluent-btn--danger {
    background: var(--fluent-dangerBackground);
    color: var(--fluent-neutralForegroundOnBrand);
    border-color: var(--fluent-dangerBackground);
  }
  .fluent-btn--primary.fluent-btn--danger:hover:not(:disabled) {
    background: var(--fluent-dangerBackgroundHover);
    border-color: var(--fluent-dangerBackgroundHover);
  }
  .fluent-btn--primary.fluent-btn--danger:active:not(:disabled) {
    background: var(--fluent-dangerBackgroundPressed);
    border-color: var(--fluent-dangerBackgroundPressed);
  }

  /* ── Appearance: secondary (neutral outlined) ─────────────────────── */

  .fluent-btn--secondary {
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
    border-color: var(--fluent-neutralStroke1);
  }
  .fluent-btn--secondary:hover:not(:disabled) {
    background: var(--fluent-subtleBackgroundHover);
    border-color: var(--fluent-neutralStroke1Hover);
  }
  .fluent-btn--secondary:active:not(:disabled) {
    background: var(--fluent-subtleBackgroundPressed);
  }

  /* Secondary + danger: red text/border, neutral fill — used rarely (e.g., danger affordances inline). */
  .fluent-btn--secondary.fluent-btn--danger {
    color: var(--fluent-dangerBackground);
    border-color: var(--fluent-dangerBackground);
  }
  .fluent-btn--secondary.fluent-btn--danger:hover:not(:disabled) {
    background: var(--fluent-subtleBackgroundHover);
    color: var(--fluent-dangerBackgroundHover);
    border-color: var(--fluent-dangerBackgroundHover);
  }

  /* ── Appearance: subtle (text-only with hover fill) ───────────────── */

  .fluent-btn--subtle {
    background: transparent;
    color: var(--fluent-neutralForeground2);
    border-color: transparent;
  }
  .fluent-btn--subtle:hover:not(:disabled) {
    background: var(--fluent-subtleBackgroundHover);
    color: var(--fluent-neutralForeground1);
  }
  .fluent-btn--subtle:active:not(:disabled) {
    background: var(--fluent-subtleBackgroundPressed);
  }
  .fluent-btn--subtle.fluent-btn--danger {
    color: var(--fluent-dangerBackground);
  }
  .fluent-btn--subtle.fluent-btn--danger:hover:not(:disabled) {
    color: var(--fluent-dangerBackgroundHover);
  }

  /* ── Appearance: outline (transparent fill, stroked) ──────────────── */

  .fluent-btn--outline {
    background: transparent;
    color: var(--fluent-neutralForeground1);
    border-color: var(--fluent-neutralStroke1);
  }
  .fluent-btn--outline:hover:not(:disabled) {
    background: var(--fluent-subtleBackgroundHover);
    border-color: var(--fluent-neutralStroke1Hover);
  }
</style>
