<script lang="ts">
  /**
   * BlocksStep — page 1 of the Panel editor: the blocks on this panel, and
   * the detected blocks that can be added, grouped by detector board.
   * Renders and emits intent only; reordering and add-all arrive in S3.
   */
  import type { PanelEditorView, PanelDetectedBlock } from './panelEditorView';
  import type { PanelEditorActions } from '$lib/orchestration/panelEditorOrchestrator';

  let {
    view,
    actions,
    detectedBlocks,
  }: {
    view: PanelEditorView;
    actions: PanelEditorActions;
    detectedBlocks: PanelDetectedBlock[];
  } = $props();

  let search = $state('');

  const OTHER_BOARD = 'Other detected blocks';
  const countText = $derived(`${view.entries.length} block${view.entries.length === 1 ? '' : 's'}`);
  const onPanel = (blockId: string) => view.entries.some((entry) => entry.blockId === blockId);
  const metaFor = (blockId: string) => {
    const block = detectedBlocks.find((candidate) => candidate.blockId === blockId);
    return [block?.board, block?.facility].filter(Boolean).join(' · ');
  };

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

<div class="panes">
  <section class="pane" aria-label="On this panel">
    <div class="pane-head"><span>On this panel</span><span class="chip">{countText}</span></div>
    <div class="pane-body">
      {#if view.entries.length === 0}
        <div class="empty">
          <strong>No blocks on this panel yet</strong>
          <span>Add the blocks whose lamps are on this panel. Use the list on the right to add them one at a time.</span>
        </div>
      {:else}
        <ol class="list">
          {#each view.entries as entry, index (entry.entryId)}
            <li class="list-item">
              <span class="meta index">{index + 1}</span>
              <div class="grow">
                <div class="name">{entry.blockName}</div>
                <div class="meta">{metaFor(entry.blockId)}</div>
              </div>
              <button type="button" class="btn btn-icon" disabled title="Move up" aria-label={`Move ${entry.blockName} up`}>↑</button>
              <button type="button" class="btn btn-icon" disabled title="Move down" aria-label={`Move ${entry.blockName} down`}>↓</button>
              <button
                type="button"
                class="btn btn-icon"
                title="Remove from panel"
                aria-label={`Remove ${entry.blockName} from panel`}
                onclick={() => actions.removeEntry(entry.entryId)}
              >×</button>
            </li>
          {/each}
        </ol>
      {/if}
    </div>
  </section>

  <section class="pane" aria-label="Detected blocks">
    <div class="pane-head"><span>Detected blocks</span></div>
    <div class="pane-body">
      <div>
        <label class="field-label" for="panel-add-all">Add all blocks from</label>
        <div class="shortcut-row">
          <select id="panel-add-all" class="input" disabled>
            <option>Not available yet</option>
          </select>
          <button type="button" class="btn" disabled>Add</button>
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
                  aria-label={`Add ${block.name}`}
                  onclick={() => actions.addBlock({ blockId: block.blockId, name: block.name })}
                >Add</button>
              {/if}
            </div>
          {/each}
        {:else}
          <div class="list-item meta">No blocks match.</div>
        {/each}
      </div>
    </div>
  </section>
</div>
