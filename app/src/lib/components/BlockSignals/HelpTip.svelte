<script lang="ts">
  /**
   * HelpTip — a small "?" button that shows a longer explanation in a
   * bubble. The bubble closes on a click outside it or Escape.
   *
   * The bubble is attached to its button, so it scrolls with it. Inside a
   * scrolling column it opens upward when there isn't room below, so the
   * column never clips it.
   */
  import { tick, type Snippet } from 'svelte';
  import { shouldOpenAbove } from './popover';

  let { label, children }: { label: string; children: Snippet } = $props();

  let open = $state(false);
  let above = $state(false);
  let root = $state<HTMLElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);
  let bubble = $state<HTMLElement | null>(null);

  async function toggle(): Promise<void> {
    if (open) {
      open = false;
      return;
    }
    above = false;
    open = true;
    await tick();
    if (bubble && button) above = shouldOpenAbove(button, bubble);
  }

  $effect(() => {
    if (!open) return;
    const onPointer = (event: PointerEvent) => {
      if (root && !root.contains(event.target as Node)) open = false;
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        open = false;
        button?.focus();
      }
    };
    document.addEventListener('pointerdown', onPointer, true);
    document.addEventListener('keydown', onKey, true);
    return () => {
      document.removeEventListener('pointerdown', onPointer, true);
      document.removeEventListener('keydown', onKey, true);
    };
  });
</script>

<span class="help-tip" class:open bind:this={root}>
  <button
    bind:this={button}
    type="button"
    class="help-tip-button"
    class:open
    aria-label={label}
    aria-expanded={open}
    onclick={toggle}
  >?</button>
  {#if open}
    <span bind:this={bubble} class="help-tip-bubble" class:above role="note">{@render children()}</span>
  {/if}
</span>

<style>
  .help-tip { position: relative; display: inline-flex; flex: 0 0 auto; }
  /* an open bubble sits above everything after it, including other "?" buttons */
  .help-tip.open { z-index: 30; }
  .help-tip-button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 50%;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-brandBackground);
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
  }
  .help-tip-button.open { background: rgba(15, 108, 189, .12); }
  .help-tip-bubble {
    position: absolute;
    top: 24px;
    right: 0;
    width: 260px;
    box-sizing: border-box;
    padding: 8px 10px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 6px;
    background: var(--fluent-neutralBackground1);
    box-shadow: 0 4px 14px rgba(0, 0, 0, .16);
    color: var(--fluent-neutralForeground2);
    font-size: 11px;
    font-weight: 400;
    line-height: 1.45;
    letter-spacing: normal;
    text-align: left;
    text-transform: none;
  }
  .help-tip-bubble.above { top: auto; bottom: 24px; }
</style>
