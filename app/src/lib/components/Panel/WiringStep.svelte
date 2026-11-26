<script lang="ts">
  /**
   * WiringStep — page 2 of the Panel editor: pick a block, then its lamp's
   * board and pin. Pin meaning (free / this block / other block / outside /
   * facility) is derived here for display; what a click does is decided by
   * the orchestrator.
   */
  import type { PanelEditorView } from './panelEditorView';
  import type { PanelEditorActions } from '$lib/orchestration/panelEditorOrchestrator';
  import type { PanelWiringPin } from '$lib/api/panels';

  let { view, actions }: { view: PanelEditorView; actions: PanelEditorActions } = $props();

  type PinState = 'free' | 'mine' | 'panel' | 'outside' | 'blocked';

  const selected = $derived(view.entries.find((entry) => entry.entryId === view.selectedEntryId));
  const board = $derived(view.boards.find((candidate) => candidate.nodeKey === view.boardNodeKey));
  const wiredCount = $derived(view.entries.filter((entry) => entry.lamp).length);
  const heads = $derived.by(() => {
    const byHead = new Map<string, PanelWiringPin[]>();
    for (const pin of board?.pins ?? []) byHead.set(pin.group, [...(byHead.get(pin.group) ?? []), pin]);
    return [...byHead.entries()];
  });

  const boardName = (nodeKey: string) => view.boards.find((candidate) => candidate.nodeKey === nodeKey)?.name ?? nodeKey;
  const pinLabel = (nodeKey: string, output: number) =>
    view.boards.find((candidate) => candidate.nodeKey === nodeKey)?.pins.find((pin) => pin.output === output)?.label ?? '';

  /** The holder of a pin within this editor session, then the saved owner from the facts. */
  function describe(pin: PanelWiringPin): { state: PinState; who: string } {
    const holder = board ? view.entryHolding(board.nodeKey, pin.output) : undefined;
    if (holder) {
      return holder.entryId === selected?.entryId
        ? { state: 'mine', who: 'this block' }
        : { state: 'panel', who: holder.blockName };
    }
    const { owner } = pin;
    switch (owner.kind) {
      case 'outside': return { state: 'outside', who: `Outside Bowties · “${owner.description}”` };
      case 'facility': return { state: 'blocked', who: owner.name };
      case 'panel': return owner.facilityId === view.facilityId ? { state: 'free', who: 'free' } : { state: 'blocked', who: owner.name };
      default: return { state: 'free', who: 'free' };
    }
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
        <div>
          <label class="field-label" for="panel-board">Board</label>
          <select
            id="panel-board"
            class="input"
            value={view.boardNodeKey}
            onchange={(event) => actions.chooseBoard(event.currentTarget.value)}
          >
            {#each view.boards as candidate (candidate.nodeKey)}
              <option value={candidate.nodeKey}>{candidate.name}</option>
            {/each}
          </select>
        </div>
        <div>
          <div class="field-label">Pin</div>
          {#each heads as [head, pins] (head)}
            <div class="connector" role="group" aria-label={`Head ${head}`}>
              <div class="connector-label">Head {head}</div>
              <div class="pin-grid">
                {#each pins as pin (pin.output)}
                  {@const info = describe(pin)}
                  <button
                    type="button"
                    class={`pin ${info.state}`}
                    disabled={info.state === 'blocked'}
                    aria-label={`${pin.label}: ${info.who}`}
                    onclick={() => board && actions.pinClick(board.nodeKey, pin.output)}
                  >
                    <span class="pin-id">{pin.label}</span>
                    <span class="pin-owner">{info.who}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/each}
        </div>
        <div class="legend" aria-hidden="true">
          <span class="l-free">free</span><span class="l-mine">this block</span><span class="l-panel">another block on this panel</span><span class="l-outside">used outside Bowties</span><span class="l-blocked">used by Bowties elsewhere</span>
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
