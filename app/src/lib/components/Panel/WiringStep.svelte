<script lang="ts">
  /**
   * WiringStep — page 2 of the Panel editor: pick a block, then its lamp's
   * board and pin. Pin meaning (free / this block / other block / outside /
   * facility) is mapped here onto the shared PinGrid states; what a click does
   * is decided by the orchestrator.
   */
  import type { PanelEditorView } from './panelEditorView';
  import type { PanelEditorActions } from '$lib/orchestration/panelEditorOrchestrator';
  import type { PanelWiringPin } from '$lib/api/panels';
  import SignalLccPicker from '$lib/components/SignalLcc/SignalLccPicker.svelte';
  import PinGrid, { type PinGridPinView, type PinGridState } from '$lib/components/SignalLcc/PinGrid.svelte';

  let { view, actions }: { view: PanelEditorView; actions: PanelEditorActions } = $props();

  const selected = $derived(view.entries.find((entry) => entry.entryId === view.selectedEntryId));
  const board = $derived(view.boards.find((candidate) => candidate.nodeKey === view.boardNodeKey));
  const wiredCount = $derived(view.entries.filter((entry) => entry.lamp).length);
  const boardName = (nodeKey: string) => view.boards.find((candidate) => candidate.nodeKey === nodeKey)?.name ?? nodeKey;
  const pinLabel = (nodeKey: string, output: number) =>
    view.boards.find((candidate) => candidate.nodeKey === nodeKey)?.pins.find((pin) => pin.output === output)?.label ?? '';

  const PIN_LEGEND: Array<{ state: PinGridState; text: string }> = [
    { state: 'free', text: 'Free' },
    { state: 'selected', text: 'This block' },
    { state: 'sibling', text: 'Used by another block on this panel' },
    { state: 'outside', text: 'Used outside Bowties' },
    { state: 'facility', text: 'Used by Bowties elsewhere' },
  ];

  /** The holder of a pin within this editor session, then the saved owner from the facts. */
  function describe(pin: PanelWiringPin): { state: PinGridState; who: string } {
    const holder = board ? view.entryHolding(board.nodeKey, pin.output) : undefined;
    if (holder) {
      return holder.entryId === selected?.entryId
        ? { state: 'selected', who: 'This block' }
        : { state: 'sibling', who: holder.blockName };
    }
    const { owner } = pin;
    switch (owner.kind) {
      case 'outside': return { state: 'outside', who: `Outside Bowties · “${owner.description}”` };
      case 'facility': return { state: 'facility', who: owner.name };
      case 'panel': return owner.facilityId === view.facilityId ? { state: 'free', who: 'Free' } : { state: 'facility', who: owner.name };
      default: return { state: 'free', who: 'Free' };
    }
  }

  function pinView(output: number): PinGridPinView {
    const pin = board?.pins.find((candidate) => candidate.output === output);
    if (!pin) return { state: 'free', who: '', ariaLabel: `#${output}`, disabled: true };
    const info = describe(pin);
    return {
      ...info,
      ariaLabel: `${pin.label}: ${info.state === 'sibling' ? `Used by ${info.who}` : info.who}`,
      disabled: info.state === 'facility' || info.state === 'sibling',
    };
  }
</script>

<div class="toolbar">
  <span class="field-label" id="lit-when-label">Lamps are lit when the block is</span>
  <div class="segmented" role="radiogroup" aria-labelledby="lit-when-label">
    <label class="segment selected"><input type="radio" name="lit-when" checked disabled />Occupied</label>
    <label class="segment"><input type="radio" name="lit-when" disabled />Clear</label>
  </div>
</div>

<div class="panes">
  <section class="pane" aria-label="Blocks">
    <div class="pane-head">
      <span>Blocks</span>
      <span class="chip">{view.entries.length ? `${wiredCount} of ${view.entries.length} wired` : 'none'}</span>
    </div>
    <div class="pane-body">
      {#if view.entries.length === 0}
        <div class="empty"><strong>No blocks to wire</strong><span>Add blocks on the Blocks step first.</span></div>
      {:else}
        <div class="list" role="listbox" aria-label="Panel blocks">
          {#each view.entries as entry (entry.entryId)}
            <div
              class="list-item selectable"
              class:selected={entry.entryId === view.selectedEntryId}
              role="option"
              aria-selected={entry.entryId === view.selectedEntryId}
              tabindex="0"
              onclick={() => actions.selectEntry(entry.entryId)}
              onkeydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  actions.selectEntry(entry.entryId);
                }
              }}
            >
              <span class="lamp" class:none={!entry.lamp}></span>
              <div class="grow">
                <div class="name">{entry.blockName}</div>
                <div class="meta">
                  {entry.lamp ? `${boardName(entry.lamp.nodeKey)} · ${pinLabel(entry.lamp.nodeKey, entry.lamp.output)}` : 'No lamp yet'}
                </div>
              </div>
              {#if entry.lamp}
                <span class="chip wired">wired</span>
              {:else}
                <span class="chip attn">needs a lamp</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </section>

  <section class="pane" aria-label="Lamp">
    <div class="pane-head"><span>{selected ? `Lamp for ${selected.blockName}` : 'Lamp'}</span></div>
    <div class="pane-body">
      {#if !selected}
        {#if view.entries.length > 0}
          <div class="empty"><strong>Choose a block</strong><span>Pick a block on the left to choose its lamp's board and pin.</span></div>
        {/if}
      {:else}
        <SignalLccPicker
          nodeKeys={view.boards.map((candidate) => candidate.nodeKey)}
          selected={view.boardNodeKey ?? null}
          label="Board"
          onChoose={(nodeKey) => actions.chooseBoard(nodeKey)}
        />
        <div>
          <div class="field-label">Pin</div>
          <PinGrid
            outputs={(board?.pins ?? []).map((pin) => ({ ordinal: pin.output, label: pin.label }))}
            {pinView}
            onPinClick={(output) => board && actions.pinClick(board.nodeKey, output)}
            legend={PIN_LEGEND}
            ariaLabel={`Pins on ${board?.name ?? 'board'}`}
          />
        </div>
        {#if view.message}
          <div class={`note ${view.message.kind}`} role="status">{view.message.text}</div>
        {/if}
        <div>
          <div class="field-label">Lit when</div>
          <div class="segmented" role="radiogroup" aria-label="Lit when">
            <label class="segment selected"><input type="radio" name="lit-when-block" checked disabled />Panel default (occupied)</label>
            <label class="segment"><input type="radio" name="lit-when-block" disabled />Occupied</label>
            <label class="segment"><input type="radio" name="lit-when-block" disabled />Clear</label>
          </div>
        </div>
      {/if}
    </div>
  </section>
</div>
