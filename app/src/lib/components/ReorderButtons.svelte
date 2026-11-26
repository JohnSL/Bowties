<script lang="ts">
  let { name, canMoveUp, canMoveDown, onMove }: {
    name: string;
    canMoveUp: boolean;
    canMoveDown: boolean;
    onMove: (delta: -1 | 1) => void;
  } = $props();
</script>

<button type="button" class="icon-btn" aria-label={`Move ${name} up`} title={`Move ${name} up`} disabled={!canMoveUp} onclick={() => canMoveUp && onMove(-1)}>
  <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 3 L13 8 M8 3 V13"></path></svg>
</button>
<button type="button" class="icon-btn" aria-label={`Move ${name} down`} title={`Move ${name} down`} disabled={!canMoveDown} onclick={() => canMoveDown && onMove(1)}>
  <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 13 L13 8 M8 3 V13"></path></svg>
</button>

<style>
  .icon-btn { display: grid; place-items: center; width: 28px; height: 28px; padding: 4px; border: 1px solid var(--abs-border-strong, #b8b8b8); border-radius: 4px; background: #fff; color: var(--abs-text-secondary, #424242); cursor: pointer; }
  .icon-btn:hover:not(:disabled) { background: #f5f5f5; }
  .icon-btn:disabled { cursor: default; opacity: .4; }
  .icon-btn svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
</style>
