<script lang="ts">
  /**
   * BlocksStep — page 1 of the Panel editor: the blocks on this panel, and
   * the detected blocks that can be added, grouped by detector board.
   * Renders and emits intent only.
   */
  import { tick } from 'svelte';
  import ReorderButtons, { focusMovedArrow } from '$lib/components/ReorderButtons.svelte';
  import DragHandle from '$lib/components/DragHandle.svelte';
  import { DragReorder } from '$lib/components/dragReorder.svelte';
  import type { PanelEditorView, PanelDetectedBlock } from './panelEditorView';
  import type { PanelEditorActions } from '$lib/orchestration/panelEditorOrchestrator';

  let {
    view,
    actions,
    detectedBlocks,
    blockSources,
  }: {
    view: PanelEditorView;
    actions: PanelEditorActions;
    detectedBlocks: PanelDetectedBlock[];
    blockSources: Array<{ id: string; label: string; blocks: Array<{ blockId: string; name: string }> }>;
  } = $props();

  let search = $state('');
  let sourceId = $state('');
  const selectedSource = $derived(blockSources.find((source) => source.id === sourceId) ?? blockSources[0]);

  const OTHER_BOARD = 'Other detected blocks';
  const countText = $derived(`${view.entries.length} block${view.entries.length === 1 ? '' : 's'}`);
  const onPanel = (blockId: string) => view.entries.some((entry) => entry.blockId === blockId);
  const metaFor = (blockId: string) => {
    const block = detectedBlocks.find((candidate) => candidate.blockId === blockId);
    return [block?.board, block?.facility].filter(Boolean).join(' · ');
  };

  /** The block the user last added, moved or clicked; display only, so a move stays visible. */
  let picked = $state<string | null>(null);
  let orderBody = $state<HTMLElement | null>(null);
  const drag = new DragReorder(() => orderBody, (entryId, toIndex) => actions.moveEntryTo(entryId, toIndex));

  function addOne(block: { blockId: string; name: string }): void {
    actions.addBlock(block);
    picked = block.blockId;
  }

  function addAll(blocks: Array<{ blockId: string; name: string }>): void {
    const added = blocks.filter((block) => !onPanel(block.blockId));
    actions.addBlocks(blocks);
    if (added.length) picked = added[added.length - 1].blockId;
  }

  async function move(entryId: string, blockId: string, name: string, delta: -1 | 1): Promise<void> {
    picked = blockId;
    actions.moveEntry(entryId, delta);
    await tick();
    focusMovedArrow(orderBody?.querySelector(`[data-reorder-id="${CSS.escape(entryId)}"]`), name, delta);
  }

  function remove(entryId: string, blockId: string, index: number): void {
    if (picked === blockId) {
      const next = view.entries[index + 1] ?? view.entries[index - 1];
      picked = next?.blockId ?? null;
    }
    actions.removeEntry(entryId);
  }

  const groups = $derived.by(() => {
    const needle = search.trim().toLowerCase();
    const byBoard = new Map<string, PanelDetectedBlock[]>();
    for (const block of detectedBlocks) {
      if (needle && !block.name.toLowerCase().includes(needle)) continue;
      const board = block.board ?? OTHER_BOARD;
      byBoard.set(board, [...(byBoard.get(board) ?? []), block]);
    }
    return [...byBoard.entries()];
  });
</script>

<div class="panes blocks-panes">
  <section class="pane catalog" aria-label="Detected blocks">
    <div class="pane-head"><span>Detected blocks</span></div>
    <div class="pane-body">
      <div>
        <label class="field-label" for="panel-add-all">Add all blocks from</label>
        <div class="shortcut-row">
          <select id="panel-add-all" class="input" value={selectedSource?.id ?? ''} onchange={(event) => sourceId = event.currentTarget.value} disabled={blockSources.length === 0}>
            {#if blockSources.length === 0}<option value="">No sources available</option>{/if}
            {#each blockSources as source (source.id)}
              <option value={source.id}>{source.label}</option>
            {/each}
          </select>
          <button type="button" class="btn" disabled={!selectedSource} onclick={() => selectedSource && addAll(selectedSource.blocks)}>Add all →</button>
        </div>
      </div>
      <input class="input" type="search" aria-label="Search blocks" placeholder="Search blocks" bind:value={search} />
      <div class="list">
        {#each groups as [board, blocks] (board)}
          <div class="list-group">{board}</div>
          {#each blocks as block (block.blockId)}
            <div class="list-item" class:on-panel={onPanel(block.blockId)}>
              <div class="grow">
                <div>{block.name}</div>
                <div class="meta">{block.facility ? `In ${block.facility}` : ' '}</div>
              </div>
              {#if onPanel(block.blockId)}
                <span class="chip">on panel</span>
              {:else}
                <button
                  type="button"
                  class="btn btn-small"
                  aria-label={`Add ${block.name} to panel`}
                  onclick={() => addOne({ blockId: block.blockId, name: block.name })}
                >Add →</button>
              {/if}
            </div>
          {/each}
        {:else}
          <div class="list-item meta">No blocks match.</div>
        {/each}
      </div>
    </div>
  </section>

  <section class="pane panel-workspace" aria-label="Blocks on this panel">
    <div class="pane-head"><span>Blocks on this panel</span><span class="chip">{countText}</span></div>
    <div class="pane-body">
      {#if view.entries.length === 0}
        <div class="empty">
          <strong>No blocks on this panel yet</strong>
          <span>Choose a detected block and click Add →. Its position here sets the panel order.</span>
        </div>
      {:else}
        <table class="order-table">
          <thead><tr><th scope="col">Block</th><th scope="col"><span class="visually-hidden">Actions</span></th></tr></thead>
          <tbody bind:this={orderBody}>
            {#each view.entries as entry, index (entry.entryId)}
              <tr
                class:selected={picked === entry.blockId}
                data-reorder-id={entry.entryId}
                data-drop={drag.sideFor(entry.entryId)}
                onclick={(event) => {
                  if (!(event.target as Element).closest('button')) picked = entry.blockId;
                }}
              >
                <td>
                  <div class="row-name">
                    <DragHandle {drag} id={entry.entryId} onStart={() => (picked = entry.blockId)} />
                    <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 8 H14 M2 5 V11 M14 5 V11"></path></svg>
                    <span>
                      <strong>{entry.blockName}</strong>
                      <span class="row-sub">{metaFor(entry.blockId)}</span>
                    </span>
                  </div>
                </td>
                <td>
                  <div class="row-actions">
                    <ReorderButtons
                      name={entry.blockName}
                      canMoveUp={index > 0}
                      canMoveDown={index < view.entries.length - 1}
                      onMove={(delta) => void move(entry.entryId, entry.blockId, entry.blockName, delta)}
                      removeLabel={`Remove ${entry.blockName} from panel`}
                      onRemove={() => remove(entry.entryId, entry.blockId, index)}
                    />
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </section>

</div>

<style>
  .panes .pane.catalog { background: var(--pe-subtle); }
  .panes .catalog .list { background: #fff; }
  .panes .pane.panel-workspace { border-color: var(--pe-border-strong); }
  .panes .panel-workspace .empty { border: 1px dashed var(--pe-border-strong); border-radius: 6px; }
  .panes .panel-workspace .pane-head { padding: 12px; background: #edf2f7; font-size: 14px; }

  /* Same row anatomy as the signals track list (BlockSignals/BlocksStep) */
  .order-table { width: 100%; border-collapse: collapse; table-layout: fixed; font-size: 12px; }
  .order-table th {
    padding: 7px 9px; border-bottom: 1px solid var(--pe-border); background: #f4f5f7; color: var(--pe-muted);
    font-size: 10px; font-weight: 600; letter-spacing: .04em; text-align: left; text-transform: uppercase;
  }
  .order-table th:last-child { width: 104px; }
  .order-table td { padding: 8px 9px; border-bottom: 1px solid var(--pe-border-soft); vertical-align: top; background: #fff; }
  .order-table tbody tr { cursor: pointer; }
  .order-table tbody tr:last-child td { border-bottom: 0; }
  .order-table tbody tr.selected td { background: var(--pe-selected); }
  .order-table tbody tr.selected > td:first-child { box-shadow: inset 3px 0 0 var(--pe-brand); }
  .order-table tbody tr[data-drop='before'] > td { box-shadow: inset 0 3px 0 var(--pe-brand); }
  .order-table tbody tr[data-drop='after'] > td { box-shadow: inset 0 -3px 0 var(--pe-brand); }
  .row-name { display: flex; align-items: flex-start; gap: 6px; }
  .row-name > span { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .row-icon { width: 16px; height: 16px; flex: 0 0 16px; margin-top: 1px; fill: none; stroke: var(--pe-muted); stroke-width: 1.6; stroke-linecap: round; }
  .row-sub { display: block; color: var(--pe-muted); font-size: 11px; }
  .row-actions { display: flex; gap: 4px; }
  .visually-hidden { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
  .shortcut-row .btn { flex: none; white-space: nowrap; }</style>
