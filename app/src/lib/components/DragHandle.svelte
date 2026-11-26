<script lang="ts">
  /**
   * DragHandle — the six-dot grip that drags a row to a new position. One
   * look everywhere; callers place it but do not restyle it. Pointer only;
   * the row's up/down buttons are the keyboard path.
   */
  import type { DragReorder } from './dragReorder.svelte';

  let { drag, id, onStart }: {
    drag: DragReorder;
    id: string;
    onStart?: () => void;
  } = $props();
</script>

<span
  class="grip"
  class:dragging={drag.draggingId === id}
  title="Drag to reorder"
  aria-hidden="true"
  onpointerdown={(event) => {
    onStart?.();
    drag.start(id, event);
  }}
  onpointermove={(event) => drag.move(event)}
  onpointerup={(event) => drag.finish(event)}
  onpointercancel={() => drag.cancel()}
></span>

<style>
  .grip {
    display: inline-grid; place-items: center; flex: 0 0 14px; width: 14px; height: 20px;
    border-radius: 3px; color: #858585; cursor: grab; user-select: none; touch-action: none;
  }
  .grip::before {
    content: ""; width: 8px; height: 12px;
    background: radial-gradient(circle, currentColor 1px, transparent 1.25px) 0 0 / 4px 4px;
  }
  .grip:hover { color: #424242; background: #f0f0f0; }
  .grip.dragging { cursor: grabbing; color: #0f6cbd; }
</style>
