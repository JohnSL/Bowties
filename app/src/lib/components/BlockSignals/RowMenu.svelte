<script lang="ts" module>
  export type RowMenuItem = { label: string; description?: string; onSelect: () => void };
</script>

<script lang="ts">
  /**
   * RowMenu — a "⋯" button that opens a short menu of less common actions
   * for one list row. Closes on choosing an item, a click outside, or Escape.
   */
  import { tick } from 'svelte';
  import { shouldOpenAbove } from './popover';

  let { label, items }: { label: string; items: RowMenuItem[] } = $props();

  let open = $state(false);
  let above = $state(false);
  let root = $state<HTMLElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);
  let menu = $state<HTMLElement | null>(null);

  async function toggle(): Promise<void> {
    if (open) {
      open = false;
      return;
    }
    above = false;
    open = true;
    await tick();
    if (!menu || !button) return;
    above = shouldOpenAbove(button, menu);
    await tick();
    // focus without scrolling, so the list stays where the user left it
    menu?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus({ preventScroll: true });
  }

  function choose(item: RowMenuItem): void {
    open = false;
    button?.focus();
    item.onSelect();
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

<span class="row-menu" class:open bind:this={root}>
  <button
    bind:this={button}
    type="button"
    class="icon-btn"
    aria-label={label}
    title={label}
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={toggle}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="3.5" cy="8" r="1.2"></circle><circle cx="8" cy="8" r="1.2"></circle><circle cx="12.5" cy="8" r="1.2"></circle></svg>
  </button>
  {#if open}
    <div bind:this={menu} class="menu" class:above role="menu" aria-label={label}>
      {#each items as item}
        <button type="button" role="menuitem" onclick={() => choose(item)}>
          <span class="item-label">{item.label}</span>
          {#if item.description}<span class="item-desc">{item.description}</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
</span>

<style>
  .row-menu { position: relative; display: inline-flex; }
  .row-menu.open { z-index: 30; }
  .icon-btn { display: grid; place-items: center; width: 28px; height: 28px; padding: 4px; border: 1px solid var(--abs-border-strong); border-radius: 4px; background: #fff; color: var(--abs-text-secondary); cursor: pointer; }
  .icon-btn:hover, .row-menu.open .icon-btn { background: #f5f5f5; }
  .icon-btn svg { width: 16px; height: 16px; fill: currentColor; }
  .menu {
    position: absolute;
    top: 32px;
    right: 0;
    width: 270px;
    padding: 4px;
    border: 1px solid var(--abs-border-strong);
    border-radius: 6px;
    background: #fff;
    box-shadow: 0 4px 14px rgba(0, 0, 0, .16);
  }
  .menu.above { top: auto; bottom: 32px; }
  .menu button { display: flex; flex-direction: column; gap: 2px; width: 100%; padding: 7px 9px; border: 0; border-radius: 4px; background: transparent; color: var(--abs-text); font: inherit; font-size: 12px; text-align: left; cursor: pointer; }
  .menu button:hover, .menu button:focus-visible { background: var(--abs-selected); outline: none; }
  .item-desc { color: var(--abs-muted); font-size: 11px; }
</style>
