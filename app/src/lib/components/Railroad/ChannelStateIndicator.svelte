<script lang="ts" module>
  export type ChannelStateIndicatorState = 'occupied' | 'clear' | 'lit' | 'unlit' | 'unknown' | 'no-config';
</script>

<script lang="ts">
  /**
   * ChannelStateIndicator — the live-state dot for a channel: occupied / clear for a
   * block, lit / unlit for a lamp, plus unknown and no-config. One look everywhere a
   * channel's state is drawn (Channels panel rows, panel card tiles). Occupancy uses the
   * shared `--occupancy-*` colours from app.css, the same as the track diagram. `size="tile"`
   * draws every state at the larger tile size; the default keeps the row sizes.
   */

  let {
    state,
    label,
    size = 'row',
  }: {
    state: ChannelStateIndicatorState;
    /** Accessible name and tooltip. */
    label: string;
    size?: 'row' | 'tile';
  } = $props();
</script>

<span
  class="occupancy-indicator {state}"
  class:tile={size === 'tile'}
  title={label}
  aria-label={label}
  role="img"
  data-testid="occupancy-indicator"
></span>

<style>
  .occupancy-indicator {
    display: inline-block;
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--occupancy-unknown-stroke, #8a929c);
    background: var(--occupancy-unknown-fill, #fff);
    box-sizing: border-box;
  }
  .occupancy-indicator.occupied {
    width: 10px;
    height: 10px;
    border-color: var(--occupancy-occupied-stroke, #8a0b16);
    background: var(--occupancy-occupied-fill, #c50f1f);
  }
  .occupancy-indicator.clear {
    width: 10px;
    height: 10px;
    border-color: var(--occupancy-clear-stroke, #107c10);
    background: var(--occupancy-clear-fill, #dff6dd);
  }
  .occupancy-indicator.lit {
    width: 10px;
    height: 10px;
    border: none;
    background: #e6c200;
    box-shadow: 0 0 4px rgba(230, 194, 0, 0.6);
  }
  .occupancy-indicator.unlit {
    border: none;
    background: #555;
  }
  .occupancy-indicator.no-config {
    border-style: dashed;
    border-color: var(--text-muted, #999);
    opacity: 0.6;
  }
  .occupancy-indicator.tile {
    width: 0.75rem;
    height: 0.75rem;
  }
</style>
