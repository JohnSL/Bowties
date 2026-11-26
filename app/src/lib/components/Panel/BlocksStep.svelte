<script lang="ts">
  /**
   * BlocksStep — page 1 of the Panel editor: the blocks on this panel, and
   * the detected blocks that can be added, grouped by detector board.
   * Renders and emits intent only.
   */
  import ReorderButtons from '$lib/components/ReorderButtons.svelte';
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
          <span>Add the blocks whose lamps are on this panel. You can add all of a signals facility’s or a detector board’s blocks at once.</span>
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
              <ReorderButtons
                name={entry.blockName}
                canMoveUp={index > 0}
                canMoveDown={index < view.entries.length - 1}
                onMove={(delta) => actions.moveEntry(entry.entryId, delta)}
              />
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
          <select id="panel-add-all" class="input" value={selectedSource?.id ?? ''} onchange={(event) => sourceId = event.currentTarget.value} disabled={blockSources.length === 0}>
            {#if blockSources.length === 0}<option value="">No sources available</option>{/if}
            {#each blockSources as source (source.id)}
              <option value={source.id}>{source.label}</option>
            {/each}
          </select>
          <button type="button" class="btn" disabled={!selectedSource} onclick={() => selectedSource && actions.addBlocks(selectedSource.blocks)}>Add</button>
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
