<script lang="ts">
  import { SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH } from '$lib/stores/sidebarWidth';

  let { currentWidth, onresize }: { currentWidth: number; onresize: (width: number) => void } = $props();

  let isDragging = $state(false);
  let startX = 0;
  let startWidth = 0;

  function onPointerDown(e: PointerEvent) {
    isDragging = true;
    startX = e.clientX;
    startWidth = currentWidth;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function onPointerMove(e: PointerEvent) {
    if (!isDragging) return;
    const delta = e.clientX - startX;
    const newWidth = Math.max(SIDEBAR_MIN_WIDTH, Math.min(SIDEBAR_MAX_WIDTH, startWidth + delta));
    onresize(newWidth);
  }

  function onPointerUp(e: PointerEvent) {
    if (!isDragging) return;
    isDragging = false;
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }
</script>

<!--
  A focusable separator is the W3C-recommended window-splitter pattern
  (https://www.w3.org/WAI/ARIA/apg/patterns/windowsplitter/). Svelte's a11y
  linter does not recognise this role exception, so the warnings are suppressed.
-->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="resize-handle"
  class:dragging={isDragging}
  role="separator"
  aria-orientation="vertical"
  aria-valuenow={currentWidth}
  aria-valuemin={SIDEBAR_MIN_WIDTH}
  aria-valuemax={SIDEBAR_MAX_WIDTH}
  tabindex="0"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onkeydown={(e) => {
    if (e.key === 'ArrowLeft') {
      onresize(Math.max(SIDEBAR_MIN_WIDTH, currentWidth - 10));
    } else if (e.key === 'ArrowRight') {
      onresize(Math.min(SIDEBAR_MAX_WIDTH, currentWidth + 10));
    }
  }}
></div>

<style>
  .resize-handle {
    flex: 0 0 9px;
    width: 9px;
    align-self: stretch;
    cursor: col-resize;
    background-color: var(--sidebar-bg, #fafafa);
    position: relative;
    z-index: 10;
  }

  /* Visible divider line (centered in hit area, always shown) */
  .resize-handle::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 1px;
    transform: translateX(-50%);
    background-color: var(--border-color, #ddd);
    transition: width 0.15s ease, background-color 0.15s ease;
  }

  .resize-handle:hover::after {
    width: 3px;
    background-color: var(--accent-color, #0078d4);
  }

  .resize-handle.dragging::after {
    width: 3px;
    background-color: var(--accent-color, #0078d4);
  }
</style>
