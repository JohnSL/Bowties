<script lang="ts">
  /**
   * DialogTitle — header content helper used inside `<Dialog>`'s `title` snippet.
   *
   * Renders an optional leading glyph (⚠/❌/ⓘ) followed by the title text.
   * The bare × close affordance is rendered by `<Dialog>` itself, not here.
   */
  import type { Snippet } from 'svelte';

  interface Props {
    /** Leading glyph. 'warning' = orange ⚠ (severeWarning), 'error' = ❌, 'info' = ⓘ. */
    glyph?: 'warning' | 'error' | 'info' | null;
    children: Snippet;
  }

  let { glyph = null, children }: Props = $props();

  const glyphChar: Record<NonNullable<Props['glyph']>, string> = {
    warning: '⚠',
    error: '❌',
    info: 'ⓘ',
  };
</script>

<span class="fluent-dialog-title">
  {#if glyph}
    <span class="fluent-dialog-title__glyph fluent-dialog-title__glyph--{glyph}" aria-hidden="true">
      {glyphChar[glyph]}
    </span>
  {/if}
  <span class="fluent-dialog-title__text">{@render children()}</span>
</span>

<style>
  .fluent-dialog-title {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase400);
    font-weight: var(--fluent-fontWeightSemibold);
    line-height: var(--fluent-lineHeightBase400);
    color: var(--fluent-neutralForeground1);
  }

  .fluent-dialog-title__glyph {
    font-size: var(--fluent-fontSizeBase400);
    line-height: 1;
  }

  .fluent-dialog-title__glyph--warning { color: var(--fluent-warningGlyph); }
  .fluent-dialog-title__glyph--error   { color: var(--fluent-dangerBackground); }
  .fluent-dialog-title__glyph--info    { color: var(--fluent-brandBackground); }
</style>
