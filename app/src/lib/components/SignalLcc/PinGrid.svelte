<script lang="ts" module>
  export type PinGridState = 'selected' | 'mine' | 'sibling' | 'facility' | 'outside' | 'free';

  export interface PinGridPinView {
    state: PinGridState;
    /** The line under the pin label: its lamp, its holder, or Free. */
    who: string;
    ariaLabel: string;
    disabled?: boolean;
    /** A warning mark after the who line (a pin held outside Bowties). */
    mark?: boolean;
    inspected?: boolean;
    /** Exposed as `data-owner` so a caller can return focus to a pin. */
    dataOwner?: string;
  }
</script>

<script lang="ts">
  /**
   * PinGrid — the Signal LCC pin chooser shared by the Automatic Block Signals
   * and Panel Wiring steps: one row per head, one button per pin, with a
   * swatch legend. Purely presentational: the editor decides what each pin
   * means (`pinView`) and what a click does (`onPinClick`).
   */
  import { signalOutputRows } from '$lib/utils/signalOutputLabels';

  let {
    outputs,
    pinView,
    onPinClick,
    legend,
    ariaLabel,
    element = $bindable(null),
  }: {
    outputs: Array<{ ordinal: number; label: string }>;
    pinView: (ordinal: number) => PinGridPinView;
    onPinClick: (ordinal: number) => void;
    legend: Array<{ state: PinGridState; text: string }>;
    ariaLabel: string;
    element?: HTMLElement | null;
  } = $props();

  const rows = $derived(signalOutputRows(outputs));
  const columns = $derived(Math.max(1, ...rows.map((row) => row.cells.length)));
</script>

<div class="pin-grid" role="group" aria-label={ariaLabel} style:--pin-columns={columns} bind:this={element}>
  {#each rows as row (row.header)}
    <span class="pin-header">{row.header}</span>
    {#each row.cells as output (output.ordinal)}
      {@const view = pinView(output.ordinal)}
      <button
        type="button"
        class={`pin ${view.state}`}
        class:inspected={view.inspected}
        disabled={view.disabled}
        aria-label={view.ariaLabel}
        data-owner={view.dataOwner}
        onclick={() => onPinClick(output.ordinal)}
      >
        <span class="pin-label">{output.label}</span>
        <span class="pin-who">{view.who}{#if view.mark} <span class="held-mark" aria-hidden="true">⚠</span>{/if}</span>
      </button>
    {/each}
  {/each}
</div>
<div class="pin-key" aria-hidden="true">
  {#each legend as entry (entry.state)}
    <span><i class={entry.state}></i>{entry.text}</span>
  {/each}
</div>

<style>
  .pin-grid { display: grid; grid-template-columns: auto repeat(var(--pin-columns, 4), minmax(0, 1fr)); gap: 4px; align-items: stretch; margin-top: 8px; font-size: 12px; }
  .pin-header { display: flex; align-items: center; padding: 0 8px; color: var(--abs-brand, var(--pe-brand, #0f6cbd)); font-size: 12px; font-weight: 700; }
  .pin {
    display: flex; flex-direction: column; align-items: center; gap: 1px; min-width: 0; padding: 4px 2px;
    border: 1px solid var(--abs-border, var(--pe-border, #d1d1d1)); border-radius: 4px; background: #fff; color: var(--abs-text, var(--pe-text, #242424));
    font: inherit; font-size: 11px; line-height: 1.2; cursor: pointer;
  }
  .pin .pin-label { font-weight: 650; white-space: nowrap; }
  .pin .pin-who { max-width: 100%; overflow: hidden; color: var(--abs-muted, var(--pe-muted, #616161)); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
  .pin.free:hover { border-color: var(--abs-brand, var(--pe-brand, #0f6cbd)); }
  .pin.mine { border-color: var(--abs-brand, var(--pe-brand, #0f6cbd)); background: var(--abs-selected, var(--pe-selected, #ebf3fc)); }
  .pin.mine .pin-who { color: var(--abs-brand-hover, var(--pe-brand-hover, #115ea3)); font-weight: 650; }
  .pin.selected { border-color: var(--abs-brand, var(--pe-brand, #0f6cbd)); background: var(--abs-brand, var(--pe-brand, #0f6cbd)); color: #fff; box-shadow: 0 0 0 2px var(--abs-selected, var(--pe-selected, #ebf3fc)); }
  .pin.selected .pin-who { color: #fff; }
  .pin.facility, .pin.sibling { background: #eef1f5; color: var(--abs-muted, var(--pe-muted, #616161)); }
  .pin.outside { border-style: dashed; border-color: #b07a00; background: #fff8e6; color: var(--abs-text-secondary, var(--pe-text, #424242)); }
  .pin.outside:hover, .pin.outside.inspected { border-style: solid; box-shadow: 0 0 0 2px #f3dca0; }
  .pin .held-mark { color: #b07a00; font-weight: 700; }
  .pin.selected .held-mark { color: #fff; }
  .pin:disabled { cursor: not-allowed; }
  .pin-key { display: flex; flex-wrap: wrap; gap: 4px 12px; margin-top: 8px; font-size: 11px; color: var(--abs-muted, var(--pe-muted, #616161)); }
  .pin-key span { display: inline-flex; align-items: center; gap: 4px; }
  .pin-key i { display: inline-block; width: 12px; height: 12px; border: 1px solid var(--abs-border, var(--pe-border, #d1d1d1)); border-radius: 3px; background: #fff; }
  .pin-key i.selected { border-color: var(--abs-brand, var(--pe-brand, #0f6cbd)); background: var(--abs-brand, var(--pe-brand, #0f6cbd)); }
  .pin-key i.mine { border-color: var(--abs-brand, var(--pe-brand, #0f6cbd)); background: var(--abs-selected, var(--pe-selected, #ebf3fc)); }
  .pin-key i.facility, .pin-key i.sibling { background: #eef1f5; }
  .pin-key i.outside { border-style: dashed; border-color: #b07a00; background: #fff8e6; }
</style>
