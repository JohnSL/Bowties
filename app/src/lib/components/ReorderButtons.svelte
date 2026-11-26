<script lang="ts" module>
  /** After a move, keep focus on the arrow just used, or the other arrow when that one is now disabled. */
  export function focusMovedArrow(row: Element | null | undefined, name: string, delta: -1 | 1): void {
    const arrows = Array.from(row?.querySelectorAll<HTMLButtonElement>('button') ?? []);
    const find = (dir: 'up' | 'down') => arrows.find((b) => b.getAttribute('aria-label') === `Move ${name} ${dir}`);
    const used = find(delta < 0 ? 'up' : 'down');
    (used && !used.disabled ? used : find(delta < 0 ? 'down' : 'up'))?.focus();
  }
</script>

<script lang="ts">
  /**
   * ReorderButtons — the up / down (and optional remove) icon buttons of an
   * ordered list row. Shared by the Panel and signals Blocks steps so both
   * lists look and behave the same.
   */
  let { name, canMoveUp, canMoveDown, onMove, removeLabel, onRemove }: {
    name: string;
    canMoveUp: boolean;
    canMoveDown: boolean;
    onMove: (delta: -1 | 1) => void;
    removeLabel?: string;
    onRemove?: () => void;
  } = $props();
</script>

<button type="button" class="icon-btn" aria-label={`Move ${name} up`} title={`Move ${name} up`} disabled={!canMoveUp} onclick={() => canMoveUp && onMove(-1)}>
  <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 3 L13 8 M8 3 V13"></path></svg>
</button>
<button type="button" class="icon-btn" aria-label={`Move ${name} down`} title={`Move ${name} down`} disabled={!canMoveDown} onclick={() => canMoveDown && onMove(1)}>
  <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 13 L13 8 M8 3 V13"></path></svg>
</button>
{#if onRemove}
  <button type="button" class="icon-btn" aria-label={removeLabel} title={removeLabel} onclick={onRemove}>
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4 L12 12 M12 4 L4 12"></path></svg>
  </button>
{/if}

<style>
  .icon-btn { display: grid; place-items: center; width: 28px; height: 28px; padding: 4px; border: 1px solid #b8b8b8; border-radius: 4px; background: #fff; color: #424242; cursor: pointer; }
  .icon-btn:hover:not(:disabled) { background: #f5f5f5; }
  .icon-btn:disabled { cursor: default; opacity: .4; }
  .icon-btn svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
</style>
