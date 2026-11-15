<script lang="ts">
  /**
   * BlocksStep — page 1 of the Automatic Block Signals editor: the name,
   * the movements the signals govern, and the list of detected blocks and
   * hand-thrown turnouts in travel order, with the derived track drawing.
   * The name and drawing stay put; each column below scrolls on its own.
   */
  import type { SelectedDetectedBlock } from '$lib/api/signaling';
  import type { BlockSignalsEditorView } from '$lib/stores/blockSignalsEditor.svelte';
  import type { BlockSignalsEditorActions } from '$lib/orchestration/blockSignalsEditorOrchestrator';
  import TrackDiagram from './TrackDiagram.svelte';
  import EndChipMenu from './EndChipMenu.svelte';
  import SelectBlockPicker from './SelectBlockPicker.svelte';
  import HelpTip from './HelpTip.svelte';
  import RowMenu, { type RowMenuItem } from './RowMenu.svelte';
  import { OCCUPANCY_LABEL, occupancyOf, occupancyStatus, type BlockOccupancyMap } from './occupancy';
  import { endChipLabel } from './aspectText';
  import { DEFAULT_TURNOUT_SHAPE, type TrackSelection, type TurnoutShape } from './trackGeometry';

  let {
    view,
    actions,
    detectedBlocks,
    pickerOpen = $bindable(false),
    occupancy,
  }: {
    view: BlockSignalsEditorView;
    actions: BlockSignalsEditorActions;
    detectedBlocks: SelectedDetectedBlock[];
    pickerOpen?: boolean;
    occupancy?: BlockOccupancyMap;
  } = $props();

  let selection = $state<TrackSelection>(null);
  let menu = $state<{ end: 'start' | 'end'; top: number; left: number } | null>(null);
  let panel = $state<HTMLElement | null>(null);

  const listDirection = $derived(view.direction);
  const both = $derived(view.movements === 'bothDirections');
  const pickerBlocks = $derived(
    detectedBlocks.filter((block) => !view.blocks.some((item) => item.blockId === block.blockId)),
  );
  const selectedTurnout = $derived.by(() => {
    if (selection?.kind !== 'turnout') return null;
    const { turnoutId } = selection;
    for (const block of view.blocks) {
      const turnout = block.turnouts.find((t) => t.turnoutId === turnoutId);
      if (turnout) return turnout;
    }
    return null;
  });
  const selectedShape = $derived(
    selectedTurnout ? view.turnoutShapes[selectedTurnout.turnoutId] ?? DEFAULT_TURNOUT_SHAPE : null,
  );
  const issues = $derived(view.preview?.issues ?? []);

  const SHAPES: Array<TurnoutShape & { path: string }> = [
    { face: 'west', side: 'above', path: 'M1 12 H23 M7 12 L13 5 H23' },
    { face: 'west', side: 'below', path: 'M1 4 H23 M7 4 L13 11 H23' },
    { face: 'east', side: 'above', path: 'M1 12 H23 M17 12 L11 5 H1' },
    { face: 'east', side: 'below', path: 'M1 4 H23 M17 4 L11 11 H1' },
  ];

  function shapeLabel(shape: TurnoutShape): string {
    // face is stored relative to the list; describe it as drawn
    const end = (shape.face === 'west') !== (listDirection === 'westward') ? 'west' : 'east';
    return `Points at the ${end} end · leg ${shape.side} the main`;
  }

  function blockName(blockId: string): string {
    return view.blocks.find((block) => block.blockId === blockId)?.name ?? blockId;
  }

  function protectingSignals(turnoutId: string): string {
    return (view.preview?.signals ?? [])
      .filter((signal) => signal.turnoutIds.includes(turnoutId))
      .map((signal) => signal.name)
      .join(', ');
  }

  /** The block a new turnout goes in: the selected block, the selected turnout's block, or the first block. */
  const turnoutTarget = $derived.by(() => {
    const current = selection;
    if (current?.kind === 'block') return current.blockId;
    if (current?.kind === 'turnout') {
      const owner = view.blocks.find((block) => block.turnouts.some((t) => t.turnoutId === current.turnoutId));
      if (owner) return owner.blockId;
    }
    return view.blocks[0]?.blockId ?? null;
  });

  async function addTurnout(): Promise<void> {
    const blockId = turnoutTarget;
    if (!blockId) return;
    // after the selected turnout, otherwise at the end of the block's turnouts
    const after = selection?.kind === 'turnout' ? selection.turnoutId : undefined;
    const turnoutId = await actions.addTurnout(blockId, after);
    selection = { kind: 'turnout', turnoutId };
  }

  // one signal per direction governed at each boundary
  const signalWord = $derived(both ? 'signals' : 'signal');

  function boundaryItem(blockId: string, aboveName: string, name: string, joined: boolean): RowMenuItem {
    return joined
      ? {
          label: `Add ${both ? 'signals' : 'a signal'} between ${aboveName} and ${name}`,
          onSelect: () => {
            selection = { kind: 'block', blockId };
            void actions.setJoinedToPrevious(blockId, false);
          },
        }
      : {
          label: `Remove the ${signalWord} between ${aboveName} and ${name}`,
          description: `${name} shares signals with ${aboveName}. Use for a short detection section, such as a turnout detected on its own.`,
          onSelect: () => {
            selection = { kind: 'block', blockId };
            void actions.setJoinedToPrevious(blockId, true);
          },
        };
  }

  function removeBlock(blockId: string): void {
    if (selection?.kind === 'block' && selection.blockId === blockId) selection = null;
    void actions.removeBlock(blockId);
  }

  function removeTurnout(turnoutId: string): void {
    const block = view.blocks.find((b) => b.turnouts.some((t) => t.turnoutId === turnoutId));
    selection = block ? { kind: 'block', blockId: block.blockId } : null;
    void actions.removeTurnout(turnoutId);
  }

  function openMenu(end: 'start' | 'end', anchor: Element): void {
    if (menu?.end === end) {
      menu = null;
      return;
    }
    const box = anchor.getBoundingClientRect();
    const host = panel?.getBoundingClientRect();
    const left = host ? Math.max(0, Math.min(box.left - host.left - 120, host.width - 300)) : 0;
    const top = host ? box.bottom - host.top + 6 : 0;
    menu = { end, top, left };
  }

  function chooseBlock(blockId: string): void {
    const block = detectedBlocks.find((candidate) => candidate.blockId === blockId);
    if (block) void actions.addBlock(block);
    pickerOpen = false;
  }
</script>

<div class="step-body">
  <div class="step-top">
    <label class="form-row">
      <span>Name</span>
      <input class="input" aria-label="Name" value={view.facilityName} oninput={(event) => actions.setFacilityName(event.currentTarget.value)} />
    </label>

    {#each issues as issue}
      {#if issue.kind === 'missingDetectionFacts'}
        <p class="notice" role="alert">{blockName(issue.blockId)} is missing detected {issue.missingStates.join(' and ')} facts.</p>
      {:else if issue.kind === 'duplicateBlock'}
        <p class="notice" role="alert">{blockName(issue.blockId)} is listed more than once.</p>
      {/if}
    {/each}

    <div class="diagram-panel" bind:this={panel}>
      <div class="diagram-head">
        <span class="field-label">Track diagram · derived from the list</span>
        <div class="turnout-shape">
          {#if selectedTurnout && selectedShape}
            <strong>{selectedTurnout.name}</strong>
            <fieldset class="shape-options">
              <legend class="visually-hidden">Drawing shape for {selectedTurnout.name}</legend>
              {#each SHAPES as shape}
                {@const checked = selectedShape.face === shape.face && selectedShape.side === shape.side}
                <label class:selected={checked} title={shapeLabel(shape)}>
                  <input
                    type="radio"
                    name="turnout-shape"
                    {checked}
                    aria-label={shapeLabel(shape)}
                    onchange={() => actions.setTurnoutShape(selectedTurnout.turnoutId, { face: shape.face, side: shape.side })}
                  />
                  <svg viewBox="0 0 24 16" aria-hidden="true" style:transform={listDirection === 'westward' ? 'scaleX(-1)' : undefined}><path d={shape.path}></path></svg>
                </label>
              {/each}
            </fieldset>
            <span>Drawing only</span>
          {:else if view.blocks.length > 0}
            <span>Select a turnout to set how it is drawn</span>
          {/if}
        </div>
        <span class="muted small">{occupancyStatus(occupancy, view.blocks.map((block) => block.blockId))}</span>
      </div>

      {#if view.blocks.length === 0}
        <p class="empty">No blocks yet. Add an existing detected block to begin.</p>
      {:else}
        <TrackDiagram
          input={{
            mode: 'blocks',
            blocks: view.blocks,
            direction: view.direction,
            movements: view.movements,
            signals: view.preview?.signals ?? [],
            shapes: view.turnoutShapes,
            selection,
            endLabels: {
              start: endChipLabel('start', view.entranceEnd, view.exitEnd),
              end: endChipLabel('end', view.entranceEnd, view.exitEnd),
            },
          }}
          {occupancy}
          onSelectTurnout={(turnoutId) => (selection = { kind: 'turnout', turnoutId })}
          onRestoreBoundary={(blockId) => {
            selection = { kind: 'block', blockId };
            void actions.setJoinedToPrevious(blockId, false);
          }}
          onEndChip={openMenu}
        />
      {/if}

      {#if menu}
        <EndChipMenu
          end={menu.end}
          edgeName={menu.end === 'start' ? view.blocks[0]?.name ?? '' : view.blocks[view.blocks.length - 1]?.name ?? ''}
          entranceEnd={view.entranceEnd}
          exitEnd={view.exitEnd}
          top={menu.top}
          left={menu.left}
          onEntranceChange={(value) => actions.setEntranceEnd(value)}
          onExitChange={(value) => actions.setExitEnd(value)}
          onClose={() => (menu = null)}
        />
      {/if}

      <div class="diagram-key" aria-label="Diagram key">
        <span><i></i>Derived signal · head points in the direction of travel</span>
        <span><b class="sel-block"></b>Selected signal block · its signals protect every turnout in it</span>
        <span><i class="ghost"></i>Signal turned off</span>
        <span><span class="dot occupied" aria-hidden="true"></span>Occupied</span>
        <span><span class="dot clear" aria-hidden="true"></span>Clear</span>
        <span><span class="dot unavailable" aria-hidden="true"></span>Occupancy unavailable</span>
      </div>
    </div>
  </div>

  <div class="columns blocks-columns">
    <section class="panel subtle" aria-label="Movements and direction">
      <fieldset class="choices">
        <legend class="field-label">Which movements should these signals govern?</legend>
        <label class="radio-choice" class:selected={!both}>
          <input type="radio" name="abs-movements" checked={!both} onchange={() => actions.setMovements('oneDirection')} />
          <span>
            <span class="radio-title">One direction</span>
            <span class="radio-desc">Automatic block signaling (<strong>ABS</strong>) for the selected travel direction. Signals respond to block occupancy and the next signal's restriction.</span>
          </span>
        </label>
        <label class="radio-choice" class:selected={both}>
          <input type="radio" name="abs-movements" checked={both} onchange={() => actions.setMovements('bothDirections')} />
          <span>
            <span class="radio-title">Both directions</span>
            <span class="radio-desc">Signals governing movements in either direction over the same blocks. This alone does not coordinate opposing trains.</span>
          </span>
        </label>
      </fieldset>

      <fieldset class="choices">
        <legend class="field-label">{both ? 'List order' : 'Direction of travel'}</legend>
        <div class="segmented">
          <label class="segment" class:selected={view.direction === 'eastward'} title="Eastward · west to east">
            <input type="radio" name="abs-direction" checked={view.direction === 'eastward'} onchange={() => actions.setDirection('eastward')} />Eastward
          </label>
          <label class="segment" class:selected={view.direction === 'westward'} title="Westward · east to west">
            <input type="radio" name="abs-direction" checked={view.direction === 'westward'} onchange={() => actions.setDirection('westward')} />Westward
          </label>
        </div>
        {#if both}
          <p class="muted small hint">The order the list is written in. Trains the other way pass the blocks in reverse.</p>
        {/if}
      </fieldset>

      {#if both}
        <fieldset class="choices">
          <legend class="field-label">How should opposing movements be coordinated?</legend>
          <label class="radio-choice selected">
            <input type="radio" name="abs-coordination" checked />
            <span>
              <span class="radio-title">No shared direction coordination</span>
              <span class="radio-desc">Each direction's signals respond to occupancy and the next signal (<strong>ABS</strong>).</span>
            </span>
          </label>
          <label class="radio-choice unavailable">
            <input type="radio" name="abs-coordination" disabled />
            <span>
              <span class="radio-title">Coordinate traffic through the single-track section</span>
              <span class="radio-desc">Absolute Permissive Block (<strong>APB</strong>). Coming in a later release.</span>
            </span>
          </label>
        </fieldset>
      {/if}
    </section>

    <section class="panel list-panel" aria-label="Track list">
      <div class="list-scroll">
      <h3 class="legend-row">
        <span class="field-label">Track, in {listDirection} order</span>
        <HelpTip label="About the track list">
          Removing a row removes it from these signals, not from the layout. Turnout position inputs are not connected yet,
          so every turnout is treated as not lined for the main. A short detection section, such as a turnout detected on
          its own, can share signals with the block above: choose ⋯ on its row.
        </HelpTip>
      </h3>
      <p class="muted help">
        List detected blocks in the order {listDirection === 'westward' ? 'a westward' : 'an eastward'} train passes them{both ? '; trains the other way pass them in reverse' : ''}.
        Under each block, in the same order, list its hand-thrown turnouts on the main track: the block that detects a car standing on the points.
        When a turnout is not lined for the main, the signals protecting that block show {view.settings.turnoutNotLined === 'restricting' ? 'Restricting' : 'Stop'}.
      </p>
      {#if view.blocks.length > 0}
          <table class="abs-table track-list">
            <colgroup><col /><col style="width:96px" /><col style="width:140px" /></colgroup>
            <thead><tr><th scope="col">Track</th><th scope="col">Observed</th><th scope="col"><span class="visually-hidden">Actions</span></th></tr></thead>
            <tbody>
              {#each view.blocks as block, index (block.blockId)}
                {@const state = occupancyOf(occupancy, block.blockId)}
                <tr
                  class:selected={selection?.kind === 'block' && selection.blockId === block.blockId}
                  onclick={(event) => {
                    if (!(event.target as Element).closest('input, label, button')) selection = { kind: 'block', blockId: block.blockId };
                  }}
                >
                  <td>
                    <div class="row-name">
                      <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 8 H14 M2 5 V11 M14 5 V11"></path></svg>
                      <span>
                        <strong>{block.name}</strong>
                        {#if index > 0 && block.joinedToPrevious}
                          <span class="row-sub joined">No {signalWord} between it and {view.blocks[index - 1].name}</span>
                        {/if}
                      </span>
                    </div>
                  </td>
                  <td>
                    <span class="observed">
                      <span class="dot {state}" role="img" aria-label={`${block.name}: ${OCCUPANCY_LABEL[state].toLowerCase()}`} title={OCCUPANCY_LABEL[state]}></span>
                      {#if state !== 'unavailable'}<span class="small" aria-hidden="true">{OCCUPANCY_LABEL[state]}</span>{/if}
                    </span>
                  </td>
                  <td>
                    <div class="row-actions">
                      <button type="button" class="icon-btn" aria-label={`Move ${block.name} up`} title={`Move ${block.name} up`} disabled={index === 0} onclick={() => actions.moveBlock(block.blockId, -1)}>
                        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 3 L13 8 M8 3 V13"></path></svg>
                      </button>
                      <button type="button" class="icon-btn" aria-label={`Move ${block.name} down`} title={`Move ${block.name} down`} disabled={index === view.blocks.length - 1} onclick={() => actions.moveBlock(block.blockId, 1)}>
                        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 13 L13 8 M8 3 V13"></path></svg>
                      </button>
                      <button type="button" class="icon-btn" aria-label={`Remove ${block.name} from these signals`} title={`Remove ${block.name} from these signals`} onclick={() => removeBlock(block.blockId)}>
                        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4 L12 12 M12 4 L4 12"></path></svg>
                      </button>
                      {#if index > 0}
                        <RowMenu label={`More for ${block.name}`} items={[boundaryItem(block.blockId, view.blocks[index - 1].name, block.name, block.joinedToPrevious)]} />
                      {:else}
                        <span class="icon-spacer" aria-hidden="true"></span>
                      {/if}
                    </div>
                  </td>
                </tr>
                {#each block.turnouts as turnout (turnout.turnoutId)}
                  {@const isSelected = selection?.kind === 'turnout' && selection.turnoutId === turnout.turnoutId}
                  <tr class="turnout-row" class:selected={isSelected} onclick={() => (selection = { kind: 'turnout', turnoutId: turnout.turnoutId })}>
                    <td>
                      <div class="row-name">
                        <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M1 11 H15 M5 11 L10 5 H15"></path></svg>
                        <span>
                          {#if isSelected}
                            <input
                              class="input turnout-name"
                              aria-label="Turnout name"
                              value={turnout.name}
                              onchange={(event) => actions.renameTurnout(turnout.turnoutId, event.currentTarget.value)}
                            />
                          {:else}
                            <strong>{turnout.name}</strong>
                          {/if}
                          <span class="row-sub">Main-track turnout in {block.name}</span>
                          {#if isSelected && protectingSignals(turnout.turnoutId)}
                            <span class="row-sub">Protected by {protectingSignals(turnout.turnoutId)}</span>
                          {/if}
                        </span>
                      </div>
                    </td>
                    <td class="muted small" title="Position input not yet connected; treated as not lined for the main">Not connected</td>
                    <td>
                      <div class="row-actions">
                        <button type="button" class="icon-btn" aria-label={`Move ${turnout.name} up`} title={`Move ${turnout.name} up`} disabled={!view.canMoveTurnout(turnout.turnoutId, -1)} onclick={() => actions.moveTurnout(turnout.turnoutId, -1)}>
                          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 3 L13 8 M8 3 V13"></path></svg>
                        </button>
                        <button type="button" class="icon-btn" aria-label={`Move ${turnout.name} down`} title={`Move ${turnout.name} down`} disabled={!view.canMoveTurnout(turnout.turnoutId, 1)} onclick={() => actions.moveTurnout(turnout.turnoutId, 1)}>
                          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8 L8 13 L13 8 M8 3 V13"></path></svg>
                        </button>
                        <button type="button" class="icon-btn" aria-label={`Remove ${turnout.name} from these signals`} title={`Remove ${turnout.name} from these signals`} onclick={() => removeTurnout(turnout.turnoutId)}>
                          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4 L12 12 M12 4 L4 12"></path></svg>
                        </button>
                        <span class="icon-spacer" aria-hidden="true"></span>
                      </div>
                    </td>
                  </tr>
                {/each}
              {/each}
            </tbody>
          </table>
      {/if}
      </div>
      {#if pickerOpen}
        <SelectBlockPicker candidateBlocks={pickerBlocks} onConfirm={chooseBlock} onCancel={() => (pickerOpen = false)} />
      {/if}
      <div class="list-actions">
        <button type="button" class="btn" onclick={() => (pickerOpen = true)} disabled={pickerBlocks.length === 0}>Add block…</button>
        <button
          type="button"
          class="btn"
          onclick={addTurnout}
          disabled={!turnoutTarget}
          title={turnoutTarget ? `Add a turnout in ${blockName(turnoutTarget)}` : undefined}
        >Add turnout in block</button>
      </div>
    </section>
  </div>
</div>

<style>
  .help { margin: 0 0 10px; }
  .hint { margin-top: 6px; }
  .notice { margin: 8px 0 0; padding: 6px 10px; border-radius: 4px; background: #fff7e6; color: #8a4b08; font-size: 12px; }
  .empty { margin: 8px 0 12px; color: var(--abs-muted); font-size: 12px; }
  .diagram-panel { position: relative; margin-top: 12px; }
  .diagram-head { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; align-items: center; margin-bottom: 4px; }
  .diagram-head .field-label { margin: 0; }
  .turnout-shape { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-height: 30px; color: var(--abs-muted); font-size: 12px; }
  .turnout-shape strong { color: var(--abs-text); font-weight: 600; }
  .shape-options { display: inline-flex; margin: 0; padding: 0; border: 1px solid var(--abs-border); border-radius: 6px; overflow: hidden; }
  .shape-options label { position: relative; display: grid; place-items: center; width: 34px; height: 28px; cursor: pointer; background: #fff; color: #56606d; }
  .shape-options label + label { border-left: 1px solid var(--abs-border); }
  .shape-options label.selected { background: var(--abs-selected); color: var(--abs-brand); }
  .shape-options label:focus-within { outline: 2px solid var(--abs-brand); outline-offset: -2px; }
  .shape-options input { position: absolute; opacity: 0; pointer-events: none; }
  .shape-options svg { width: 24px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .diagram-key { display: flex; flex-wrap: wrap; gap: 14px; color: var(--abs-muted); font-size: 11px; }
  .diagram-key > span { display: inline-flex; align-items: center; gap: 5px; }
  .diagram-key i { display: inline-block; width: 11px; height: 11px; border-radius: 50%; background: #fff; border: 1.5px solid #20242a; }
  .diagram-key i.ghost { background: #fff; border: 1.5px dashed #8a929c; }
  .diagram-key b.sel-block { display: inline-block; width: 18px; height: 11px; border-radius: 3px; background: #ebf3fc; border: 1px solid #b9d4f0; }
  .dot { display: inline-block; width: 13px; height: 13px; border: 2px dashed #8a929c; border-radius: 50%; background: #fff; vertical-align: middle; box-sizing: border-box; }
  .dot.occupied { border: 2px solid #8a0b16; background: #c50f1f; }
  .dot.clear { border: 2px solid #107c10; background: #dff6dd; }
  .observed { display: inline-flex; align-items: center; gap: 5px; }
  .diagram-key .dot { width: 11px; height: 11px; }
  .blocks-columns { grid-template-columns: minmax(260px, .75fr) minmax(0, 1.25fr); }
  /* the list scrolls; the add buttons stay below it */
  section.panel.list-panel { display: flex; flex-direction: column; padding: 0; overflow: hidden; }
  .list-scroll { flex: 1 1 auto; min-height: 0; padding: 14px 15px 10px; overflow-y: auto; overscroll-behavior: contain; scrollbar-gutter: stable; }
  .list-panel h3 { margin: 0 0 6px; }
  .track-list { table-layout: fixed; }
  .track-list tbody tr { cursor: pointer; }
  .track-list tbody tr.selected { background: var(--abs-selected); }
  .row-name { display: flex; align-items: flex-start; gap: 6px; }
  .row-name > span { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .turnout-row .row-name { padding-left: 26px; }
  .row-icon { width: 16px; height: 16px; flex: 0 0 16px; margin-top: 1px; fill: none; stroke: var(--abs-muted); stroke-width: 1.6; stroke-linecap: round; }
  .row-sub { display: block; color: var(--abs-muted); font-size: 11px; }
  .row-sub.joined { color: #8a4b08; }
  .icon-spacer { width: 28px; flex: 0 0 28px; }
  .turnout-name { max-width: 240px; min-height: 26px; padding: 2px 6px; }
  .row-actions { display: flex; gap: 4px; }
  .icon-btn { display: grid; place-items: center; width: 28px; height: 28px; padding: 4px; border: 1px solid var(--abs-border-strong); border-radius: 4px; background: #fff; color: var(--abs-text-secondary); cursor: pointer; }
  .icon-btn:hover:not(:disabled) { background: #f5f5f5; }
  .icon-btn:disabled { cursor: default; opacity: .4; }
  .icon-btn svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
  .list-actions { display: flex; flex: 0 0 auto; flex-wrap: wrap; gap: 7px; padding: 10px 15px; border-top: 1px solid var(--abs-border); }
  @media (max-width: 760px), (max-height: 640px) {
    section.panel.list-panel { overflow: visible; }
    .list-scroll { overflow: visible; }
  }
</style>
