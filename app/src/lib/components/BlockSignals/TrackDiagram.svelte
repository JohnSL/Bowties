<script lang="ts">
  /**
   * TrackDiagram — the shared Automatic Block Signals track drawing.
   *
   * Blocks mode: turnouts are selectable, ghost signals turn a boundary's
   * signals back on, and the end chips open the end menu. Signals mode: each
   * derived signal is selectable. Layout comes from `trackGeometry.ts`; this
   * component only draws it and emits intent.
   */
  import {
    buildTrackModel,
    mirrorRect,
    mirrorX,
    type DrawnSignal,
    type TrackInput,
  } from './trackGeometry';
  import { OCCUPANCY_LABEL, occupancyOf, type BlockOccupancyMap } from './occupancy';

  let {
    input,
    occupancy,
    onSelectTurnout,
    onRestoreBoundary,
    onEndChip,
    onSelectSignal,
  }: {
    input: TrackInput;
    /** Live occupancy by block id; drawn on each block's dot. */
    occupancy?: BlockOccupancyMap;
    onSelectTurnout?: (turnoutId: string) => void;
    onRestoreBoundary?: (blockId: string) => void;
    onEndChip?: (end: 'start' | 'end', anchor: Element) => void;
    onSelectSignal?: (signalId: string) => void;
  } = $props();

  const model = $derived(buildTrackModel(input));
  const blocksMode = $derived(input.mode === 'blocks');
  const Y = $derived(model.railY);
  const mx = (x: number) => mirrorX(model, x);

  function activate(event: KeyboardEvent, action: () => void): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      action();
    }
  }

  function signalParts(signal: DrawnSignal) {
    const x0 = signal.jx - 22 * signal.k;
    const y = Y + signal.sy * 28;
    return {
      x0,
      y,
      headX: x0 + signal.k * 16,
      armX: x0 + signal.k * 10,
      labelX: x0 + signal.k * 8,
      labelY: Y + signal.sy * 46 + (signal.sy < 0 ? 4 : 0),
    };
  }

  function chipWidth(label: string): number {
    return Math.max(56, label.length * 6 + 18);
  }

  function pickWidth(name: string): number {
    return Math.max(60, name.length * 5.6 + 16);
  }

  const startName = $derived(input.blocks[0]?.name ?? '');
  const endName = $derived(input.blocks[input.blocks.length - 1]?.name ?? '');
</script>

<div class="track-diagram-wrap">
  <svg
    width={model.width}
    height={model.height}
    viewBox={`0 0 ${model.width} ${model.height}`}
    role="group"
    aria-label={blocksMode
      ? `Track diagram derived from the list; the list runs ${model.flip ? 'right to left' : 'left to right'}`
      : `Signals for ${input.direction} travel; select a signal to configure it`}
  >
    {#if model.shade}
      <rect x={mirrorRect(model, model.shade.x0, model.shade.x1 - model.shade.x0)} y="2" width={model.shade.x1 - model.shade.x0} height={model.height - 4} rx="6" fill="#ebf3fc" />
    {/if}

    <!-- track: approaches at both ends, then one segment per block with gaps at the joints -->
    <line x1={mx(10)} y1={Y} x2={mx(model.joints[0] - model.gap)} y2={Y} stroke="#b4bac2" stroke-width="3" />
    <line x1={mx(model.joints[model.joints.length - 1] + model.gap)} y1={Y} x2={mx(model.width - 10)} y2={Y} stroke="#b4bac2" stroke-width="3" />
    {#each model.blocks as _, index}
      <line x1={mx(model.joints[index] + model.gap)} y1={Y} x2={mx(model.joints[index + 1] - model.gap)} y2={Y} stroke="#56606d" stroke-width="3" />
    {/each}
    {#each model.joints as jx}
      {#each [-model.gap, model.gap] as dx}
        <line x1={mx(jx + dx)} y1={Y - 6} x2={mx(jx + dx)} y2={Y + 6} stroke="#56606d" stroke-width="1.5" />
      {/each}
    {/each}

    {#each model.turnouts as t (t.turnoutId)}
      {@const colour = t.selected ? '#0f6cbd' : '#56606d'}
      {@const strokeWidth = t.selected ? 4 : 3}
      {@const lx = t.x + t.dir * 24}
      {@const ly = Y + t.sy * 18}
      {#if t.sidingToX !== null}
        <line x1={mx(lx)} y1={ly} x2={mx(t.sidingToX - 24)} y2={ly} stroke="#56606d" stroke-width="3" />
      {/if}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <g
        class:selectable={blocksMode}
        class="track-turnout"
        role={blocksMode ? 'button' : undefined}
        tabindex={blocksMode ? 0 : undefined}
        aria-label={blocksMode ? `${t.name}: in ${t.blockName}` : undefined}
        aria-pressed={blocksMode ? t.selected : undefined}
        onclick={() => blocksMode && onSelectTurnout?.(t.turnoutId)}
        onkeydown={(event) => blocksMode && activate(event, () => onSelectTurnout?.(t.turnoutId))}
      >
        <title>{t.name} · in {t.blockName}</title>
        {#if blocksMode}
          <rect class="track-hit" x={mirrorRect(model, t.x - 34, 68)} y={Y - 26} width="68" height="52" rx="6" fill="transparent" stroke="transparent" stroke-width="6" />
        {/if}
        {#if t.selected}
          <line x1={mx(t.x - t.dir * 8)} y1={Y} x2={mx(t.x + t.dir * 32)} y2={Y} stroke={colour} stroke-width={strokeWidth} />
        {/if}
        <line x1={mx(t.x)} y1={Y} x2={mx(lx)} y2={ly} stroke={colour} stroke-width={strokeWidth} />
        {#if !t.paired}
          <line x1={mx(lx)} y1={ly} x2={mx(t.x + t.dir * 52)} y2={ly} stroke={colour} stroke-width={strokeWidth} />
          <line x1={mx(t.x + t.dir * 52)} y1={ly - 5} x2={mx(t.x + t.dir * 52)} y2={ly + 5} stroke={colour} stroke-width="2" />
        {/if}
      </g>
    {/each}

    <!-- block names in a top row and occupancy on the track, both at the centre of each block -->
    {#each model.blocks as block (block.blockId)}
      <text x={mx(block.cx)} y="14" font-size="11" font-weight="700" text-anchor="middle" fill="#20242a">{block.name}</text>
      {@const state = occupancyOf(occupancy, block.blockId)}
      <circle
        class="occupancy {state}"
        cx={mx(block.cx)}
        cy={Y}
        r="6"
        stroke-width="2"
        stroke-dasharray={state === 'unavailable' ? '3 2' : undefined}
      >
        <title>{block.name}: {OCCUPANCY_LABEL[state].toLowerCase()}</title>
      </circle>
    {/each}

    {#snippet signalShape(signal: DrawnSignal, mode: 'normal' | 'ghost' | 'selected')}
      {@const p = signalParts(signal)}
      {@const ghost = mode === 'ghost'}
      <line x1={mx(p.x0)} y1={p.y - 6} x2={mx(p.x0)} y2={p.y + 6} stroke-width="2" stroke={ghost ? '#a7aeb7' : '#737b86'} stroke-dasharray={ghost ? '2 2' : undefined} />
      <line x1={mx(p.x0)} y1={p.y} x2={mx(p.armX)} y2={p.y} stroke-width="2" stroke={ghost ? '#a7aeb7' : '#737b86'} stroke-dasharray={ghost ? '2 2' : undefined} />
      {#if ghost}
        <circle cx={mx(p.headX)} cy={p.y} r="6" fill="#fff" stroke="#8a929c" stroke-width="1.5" stroke-dasharray="2.5 2" />
      {:else}
        <circle cx={mx(p.headX)} cy={p.y} r="6" fill="#fff" stroke="#20242a" stroke-width="1.5" />
      {/if}
      <text
        x={mx(p.labelX)}
        y={p.labelY}
        font-size="10"
        text-anchor="middle"
        font-weight={mode === 'selected' ? 700 : 400}
        font-style={ghost ? 'italic' : 'normal'}
        fill={ghost ? '#8a929c' : mode === 'selected' ? '#0f6cbd' : '#424a55'}
      >{ghost ? `${signal.name} · off` : signal.name}</text>
    {/snippet}

    {#each model.signals as signal (signal.signalId)}
      {#if blocksMode}
        {@render signalShape(signal, 'normal')}
      {:else}
        {@const cx = signal.jx - 14 * signal.k}
        {@const w = pickWidth(signal.name)}
        {@const h = signal.issueCount ? 50 : 40}
        <g
          class="track-pick-signal"
          role="button"
          tabindex="0"
          aria-pressed={signal.selected}
          aria-label={`Configure ${signal.name}; protects ${signal.protects}${signal.issueCount ? `; ${signal.issueCount} to resolve` : ''}`}
          data-signal-id={signal.signalId}
          onclick={() => onSelectSignal?.(signal.signalId)}
          onkeydown={(event) => activate(event, () => onSelectSignal?.(signal.signalId))}
        >
          <title>{signal.name} · protects {signal.protects}</title>
          <rect
            x={mirrorRect(model, cx - w / 2, w)}
            y={signal.sy > 0 ? Y + 16 : Y - 16 - h}
            width={w}
            height={h}
            rx="6"
            fill={signal.selected ? '#ebf3fc' : 'transparent'}
            fill-opacity="0.7"
            stroke={signal.selected ? '#0f6cbd' : 'transparent'}
            stroke-width="1.5"
          />
          {@render signalShape(signal, signal.selected ? 'selected' : 'normal')}
          {#if signal.issueCount}
            <text x={mx(cx)} y={signal.sy > 0 ? Y + 58 : Y - 54} font-size="9" text-anchor="middle" fill="#9d5d00">{signal.issueCount} to resolve</text>
          {/if}
        </g>
      {/if}
    {/each}

    {#each model.ghosts as ghost (ghost.blockId)}
      <g
        class="track-ghost-signal"
        role="button"
        tabindex="0"
        aria-label={`Signals between ${ghost.between} are off. Activate to turn them back on.`}
        onclick={() => onRestoreBoundary?.(ghost.blockId)}
        onkeydown={(event) => activate(event, () => onRestoreBoundary?.(ghost.blockId))}
      >
        <title>No signals between {ghost.between} · click to turn them back on</title>
        {#each ghost.signals as signal}
          {@render signalShape(signal, 'ghost')}
        {/each}
      </g>
    {/each}

    <!-- what lies at each end: an editable chip on Blocks, a plain label on later pages -->
    {#each model.ends as end (end.end)}
      {@const edge = end.end === 'start' ? `before ${startName}` : `beyond ${endName}`}
      {#if !blocksMode}
        {#if end.label}
          <text x={mx(end.cx)} y={Y - 8} font-size="10" font-weight="600" text-anchor="middle" fill="#424a55">{end.label}</text>
        {/if}
      {:else}
        {@const label = end.label ?? '+ Plant'}
        {@const w = chipWidth(label)}
        <g
          class="track-end-chip"
          role="button"
          tabindex="0"
          aria-haspopup="dialog"
          data-end={end.end}
          aria-label={end.label ? `${edge}: ${end.label}. Activate to change.` : `Nothing connected ${edge}. Activate to choose what is there.`}
          onclick={(event) => onEndChip?.(end.end, event.currentTarget as Element)}
          onkeydown={(event) => activate(event, () => onEndChip?.(end.end, event.currentTarget as Element))}
        >
          <title>{end.label ? `${end.label} · ${edge}` : `What lies ${edge}`}</title>
          <rect x={mirrorRect(model, end.cx - w / 2, w)} y={Y - 9} width={w} height="18" rx="9" fill="#fff" stroke={end.label ? '#56606d' : '#0f6cbd'} stroke-width="1.5" stroke-dasharray={end.label ? undefined : '3 2'} />
          <text x={mx(end.cx)} y={Y + 3.5} font-size="10" text-anchor="middle" font-weight={end.label ? 600 : 400} fill={end.label ? '#20242a' : '#0f6cbd'}>{label}</text>
        </g>
      {/if}
    {/each}

    {#if model.arrows.west}
      <text x="10" y="14" font-size="10" text-anchor="start" fill="#737b86">← Westward</text>
    {/if}
    {#if model.arrows.east}
      <text x={model.width - 10} y="14" font-size="10" text-anchor="end" fill="#737b86">Eastward →</text>
    {/if}
  </svg>
</div>

<style>
  .track-diagram-wrap { position: relative; overflow-x: auto; overflow-y: hidden; margin-bottom: 8px; }
  .track-diagram-wrap svg { display: block; font-family: var(--fluent-fontFamily); }
  .track-turnout.selectable, .track-pick-signal, .track-ghost-signal, .track-end-chip { cursor: pointer; }
  .track-turnout:focus, .track-pick-signal:focus, .track-ghost-signal:focus, .track-end-chip:focus { outline: none; }
  .track-turnout:focus-visible .track-hit { stroke: var(--fluent-brandBackground); stroke-opacity: .25; }
  .track-pick-signal:focus-visible rect { stroke: #0f6cbd; stroke-width: 2; }
  .track-end-chip:focus-visible rect { stroke-width: 2.5; }
  .track-ghost-signal:focus-visible text { text-decoration: underline; }
  .occupancy.unavailable { fill: #fff; stroke: #8a929c; }
  .occupancy.occupied { fill: #c50f1f; stroke: #8a0b16; }
  .occupancy.clear { fill: #dff6dd; stroke: #107c10; }
</style>
