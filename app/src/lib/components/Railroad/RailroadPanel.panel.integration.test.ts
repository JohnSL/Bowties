/**
 * Slice S1 integration (Spec 021): the user path through + Add facility, Panel, blocks, wiring and Done,
 * through the real RailroadPanel, editor, orchestrators and stores. IPC is the only mock.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { effectiveNodeStore } from '$lib/layout';
import SaveControls from '../ElementCardDeck/SaveControls.svelte';
import { panelEditorStore } from '$lib/stores/panelEditor.svelte';
import { eventStateStore } from '$lib/stores/eventState.svelte';
import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
import { BOARD_NODE, wiringFacts } from '../Panel/panelEditorTestSupport';
import {
  blocks, bodChannel, lampBoardEntry, mockIpc, openPanelEditor, panelIpc, renderRailroadPanel, seedSavedBlockSignals,
  seedSavedPanel, wirePanelBlocks,
} from './railroadPanelTestSupport';

/** Stubs the node roster the way the Lamp-row tests do; the getter is read-only on the store. */
const rosterSpy = () => vi.spyOn(nodeRoster, 'allEntries' as unknown as never, 'get') as unknown as { mockReturnValue(entries: unknown[]): void };

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const PIN_9 = /^#9 H3-G:/;
const PIN_3 = /^#3 H1-R:/;
const PIN_14 = /^#14 H4-Y:/;

async function addBlocks(editor: HTMLElement, names: string[]) {
  for (const name of names) await fireEvent.click(within(editor).getByRole('button', { name: `Add ${name} to panel` }));
}

async function goToWiring(editor: HTMLElement) {
  await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Wiring' }));
  await within(editor).findByRole('listbox', { name: 'Panel blocks' });
}

describe('RailroadPanel Panel facility (Spec 021 / S1)', () => {
  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    panelEditorStore.reset();
    configChangesStore.clearAllDrafts();
    eventStateStore.clear();
    invoke.mockReset();
    // the wiring board is loaded, so its lamps are not reported as a missing board
    rosterSpy().mockReturnValue([lampBoardEntry(BOARD_NODE)]);
  });

  it('creates a Wired panel with three lamp channels and unsaved Lamp Selection edits, then shows its card', async () => {
    const editor = await openPanelEditor(invoke, panelIpc(wiringFacts()));
    render(SaveControls, { props: { toolbar: true } });
    expect(screen.queryByRole('group', { name: 'Configuration save controls' })).not.toBeInTheDocument();

    await addBlocks(editor, ['Cedar Hill', 'Mill curve', 'River bridge']);
    await goToWiring(editor);
    await wirePanelBlocks(editor, [['Cedar Hill', PIN_9], ['Mill curve', PIN_3], ['River bridge', PIN_14]]);
    await fireEvent.click(within(editor).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument());

    expect(facilitiesStore.facilities).toHaveLength(1);
    expect(facilitiesStore.facilities[0]).toMatchObject({ templateId: 'panel', name: 'Eagle Creek fascia' });

    const lamps = channelsStore.channels.filter((c) => c.role === 'lamp-indicator');
    expect(lamps.map((c) => c.name)).toEqual(['Cedar Hill', 'Mill curve', 'River bridge']);
    const table = screen.getByRole('table');
    for (const name of ['Cedar Hill', 'Mill curve', 'River bridge']) {
      expect(within(table).getAllByText(name).length).toBeGreaterThanOrEqual(2);
    }

    const drafts = configChangesStore.draftEntries();
    expect(drafts).toHaveLength(3);
    expect(drafts.map((d) => (d.value as { value: number }).value).sort((a, b) => a - b)).toEqual([3, 9, 14]);

    expect(await screen.findByRole('group', { name: 'Configuration save controls' })).toBeInTheDocument();
    expect(effectiveNodeStore.isDirty).toBe(true);

    const card = await screen.findByTestId('panel-card');
    expect(card).toHaveTextContent('Eagle Creek fascia');
    expect(within(card).getByText('Panel')).toBeInTheDocument();
    expect(within(card).getByText('Wired')).toBeInTheDocument();
    expect(within(card).getByText(/^3 blocks/)).toBeInTheDocument();
    expect(within(card).getAllByRole('button').map((b) => b.textContent?.trim())).toEqual(['Edit', 'Remove']);
  }, 15000); // walks the whole editor; near the 5 s default under full-suite load

  it('clears a block pin when it is clicked again, and disables a pin owned by Automatic Block Signals', async () => {
    const facts = wiringFacts({ 14: { kind: 'facility', facilityId: 'abs-1', name: 'Mainline Eastbound' } });
    const editor = await openPanelEditor(invoke, panelIpc(facts));
    await addBlocks(editor, ['Cedar Hill']);
    await goToWiring(editor);

    const blocks = within(editor).getByRole('listbox', { name: 'Panel blocks' });
    await wirePanelBlocks(editor, [['Cedar Hill', PIN_9]]);
    expect(within(blocks).getByText('wired')).toBeInTheDocument();

    await fireEvent.click(within(editor).getByRole('button', { name: /^#9 H3-G: This block/ }));
    expect(within(blocks).getByText('needs a lamp')).toBeInTheDocument();
    expect(within(editor).getByRole('button', { name: /^#9 H3-G: Free/ })).toBeInTheDocument();

    expect(within(editor).getByRole('button', { name: /^#14 H4-Y: Mainline Eastbound/ })).toBeDisabled();
  });

  it('Cancel from Wiring leaves no facility, channel or edit', async () => {
    const editor = await openPanelEditor(invoke, panelIpc(wiringFacts()));
    await addBlocks(editor, ['Cedar Hill', 'Mill curve']);
    await goToWiring(editor);
    await wirePanelBlocks(editor, [['Cedar Hill', PIN_9]]);
    await fireEvent.click(within(editor).getByRole('button', { name: 'Cancel' }));

    expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument();
    expect(facilitiesStore.isEmpty).toBe(true);
    expect(channelsStore.channels.filter((c) => c.role === 'lamp-indicator')).toEqual([]);
    expect(configChangesStore.draftEntries()).toEqual([]);
    expect(screen.queryByTestId('panel-card')).not.toBeInTheDocument();
  });

  it('Done with one unwired block creates the panel as Incomplete', async () => {
    const editor = await openPanelEditor(invoke, panelIpc(wiringFacts()));
    await addBlocks(editor, ['Cedar Hill', 'Mill curve']);
    await goToWiring(editor);
    await wirePanelBlocks(editor, [['Cedar Hill', PIN_9]]);
    await fireEvent.click(within(editor).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument());

    const card = await screen.findByTestId('panel-card');
    expect(within(card).getByText('Incomplete · 1 needs a lamp')).toBeInTheDocument();
    expect(within(card).getByText(/^2 blocks/)).toBeInTheDocument();
  });
});

describe('RailroadPanel Panel card and channel usage (Spec 021 / S2)', () => {
  const BOARD = '050101010201';

  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    panelEditorStore.reset();
    configChangesStore.clearAllDrafts();
    eventStateStore.clear();
    invoke.mockReset();
    rosterSpy().mockReturnValue([lampBoardEntry(BOARD, { 1: 9, 2: 3 })]);
    channelsStore.hydrateBaseline(blocks);
  });

  /** Cedar and Mill have lamps on rows 1 and 2 (Lamp Selection 9 and 3); River needs a lamp. */
  function seedPanel() {
    facilitiesStore.hydrateBaseline([seedSavedPanel({
      facilityId: 'p-1', name: 'Eagle Creek fascia', nodeKey: BOARD,
      entries: [{ blockId: 'block:mill', row: 2 }, { blockId: 'block:cedar', row: 1 }, { blockId: 'block:river' }],
    })]);
  }

  it('shows a saved panel with its tiles in panel order, pins from Lamp Selection, not row numbers', async () => {
    seedPanel();
    mockIpc(invoke, panelIpc(wiringFacts()));
    await renderRailroadPanel();

    const card = await screen.findByTestId('panel-card');
    expect(within(card).getByText('Incomplete · 1 needs a lamp')).toBeInTheDocument();
    expect(within(card).getByText('3 blocks · lamps on Board 01')).toBeInTheDocument();
    const tiles = within(card).getAllByTestId('panel-tile');
    expect(tiles.map((tile) => tile.textContent?.replace(/\s+/g, ' ').trim())).toEqual([
      'Mill curve #3 H1-R · no reading',
      'Cedar Hill #9 H3-G · no reading',
      'River bridge needs a lamp · no reading',
    ]);
  });

  it('colours a tile from its block occupancy once an occupancy event is seen', async () => {
    seedPanel();
    mockIpc(invoke, panelIpc(wiringFacts()));
    const resolved = new Map([['block:cedar', { occupied: 'CEDAR_OCC', clear: 'CEDAR_CLR' }]]);
    eventStateStore.record('CEDAR_OCC', 10);
    await renderRailroadPanel(resolved);

    const tiles = await screen.findAllByTestId('panel-tile');
    expect(within(tiles[1]).getByLabelText('Block occupied')).toHaveClass('occupied');
    expect(within(tiles[0]).getByLabelText('No reading')).toBeInTheDocument();
  });

  it('names both the panel and the Automatic Block Signals facility in Used by for a shared block', async () => {
    seedSavedBlockSignals();
    facilitiesStore.hydrateBaseline([
      ...facilitiesStore.facilities,
      seedSavedPanel({ facilityId: 'p-1', name: 'Eagle Creek fascia', nodeKey: BOARD, entries: [{ blockId: 'block:cedar', row: 1 }] }),
    ]);
    mockIpc(invoke, panelIpc(wiringFacts()));
    await renderRailroadPanel();

    const row = within(screen.getByRole('table')).getAllByText('Cedar Hill')[0].closest('tr')!;
    const usedBy = row.querySelector('.used-by-cell')!;
    expect(usedBy).toHaveTextContent('Cedar Hill — Eastward / blocks');
    expect(usedBy).toHaveTextContent('Eagle Creek fascia / blocks');
  });

  it('shows two cards for a layout with 24 lamps on two panels', async () => {
    const BOARD_B = '050101010202';
    const many = Array.from({ length: 24 }, (_, i) => ({ ...bodChannel(1), id: `block:b${i}`, name: `Block ${i}` }));
    channelsStore.hydrateBaseline(many);
    rosterSpy().mockReturnValue([
      lampBoardEntry(BOARD, Object.fromEntries(Array.from({ length: 12 }, (_, i) => [i + 1, i + 1]))),
      lampBoardEntry(BOARD_B, Object.fromEntries(Array.from({ length: 12 }, (_, i) => [i + 1, i + 1]))),
    ]);
    const panelOf = (id: string, nodeKey: string, from: number) => seedSavedPanel({
      facilityId: id, name: `Panel ${id}`, nodeKey,
      entries: Array.from({ length: 12 }, (_, i) => ({ blockId: `block:b${from + i}`, row: i + 1 })),
    });
    facilitiesStore.hydrateBaseline([panelOf('pa', BOARD, 0), panelOf('pb', BOARD_B, 12)]);
    mockIpc(invoke, panelIpc(wiringFacts()));
    await renderRailroadPanel();

    expect(await screen.findAllByTestId('panel-card')).toHaveLength(2);
    expect(screen.getAllByTestId('panel-tile')).toHaveLength(24);
    expect(channelsStore.channels.filter((c) => c.role === 'lamp-indicator')).toHaveLength(24);
  });
});

describe('RailroadPanel Panel removal (Spec 021 / S4)', () => {
  const BOARD = '050101010201';
  const claim = (index: number, lampSelection: number | null) => ({ nodeKey: BOARD, kind: 'directLamp', index, lampSelection });
  const removalStubs = () => ({
    ...panelIpc(wiringFacts()),
    compose_facility_removal: () => ({
      edits: [1, 2].map((row) => ({
        nodeKey: BOARD, space: 253, address: 1000 + row, path: `lamp#${row}/selection`, value: { type: 'int', value: 0 },
      })),
      logicPlacement: [],
      issues: [],
      claims: [claim(1, 9), claim(2, 3)],
    }),
  });

  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    panelEditorStore.reset();
    configChangesStore.clearAllDrafts();
    eventStateStore.clear();
    invoke.mockReset();
    rosterSpy().mockReturnValue([lampBoardEntry(BOARD, { 1: 9, 2: 3 })]);
    channelsStore.hydrateBaseline(blocks);
  });

  it('Remove names the panel pins per board, then resets its lamp rows, drops its lamp channels and keeps the blocks', async () => {
    seedSavedBlockSignals();
    facilitiesStore.hydrateBaseline([
      ...facilitiesStore.facilities,
      seedSavedPanel({
        facilityId: 'p-1', name: 'Eagle Creek fascia', nodeKey: BOARD,
        entries: [{ blockId: 'block:cedar', row: 1 }, { blockId: 'block:mill', row: 2 }],
      }),
    ]);
    const absBefore = JSON.stringify(facilitiesStore.facilities.find((f) => f.templateId === 'automatic-block-signals'));
    mockIpc(invoke, removalStubs());
    await renderRailroadPanel();
    render(SaveControls, { props: { toolbar: true } });
    expect(effectiveNodeStore.isDirty).toBe(false);

    await fireEvent.click(within(await screen.findByTestId('panel-card')).getByRole('button', { name: 'Remove facility' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remove Eagle Creek fascia?' });
    expect(within(dialog).getByText('On Board 01: resets #9 H3-G and #3 H1-R')).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(screen.queryByTestId('panel-card')).not.toBeInTheDocument());
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();

    const drafts = configChangesStore.draftEntries();
    expect(drafts).toHaveLength(2);
    expect(drafts.every((d) => (d.value as { value: number }).value === 0)).toBe(true);
    expect(channelsStore.channels.filter((c) => c.role === 'lamp-indicator')).toEqual([]);
    const table = screen.getByRole('table');
    expect(within(table).queryByText('Lamp 1')).not.toBeInTheDocument();
    expect(channelsStore.channels.filter((c) => c.role === 'block-occupancy').length).toBe(blocks.length);
    expect(JSON.stringify(facilitiesStore.facilities.find((f) => f.templateId === 'automatic-block-signals'))).toBe(absBefore);
    expect(effectiveNodeStore.isDirty).toBe(true);
    expect(await screen.findByRole('group', { name: 'Configuration save controls' })).toBeInTheDocument();
  }, 15000);
});

describe('RailroadPanel Panel edit (Spec 021 / S4)', () => {
  const BOARD = '050101010201';
  const PIN_12 = /^#12 H3-L:/;
  const cardText = () => screen.getByTestId('panel-card').textContent;

  /** The wiring fixture on the saved panel's board, with the panel's own pins owned by it. */
  const editFacts = () => {
    const facts = wiringFacts({
      9: { kind: 'panel', facilityId: 'p-1', name: 'Eagle Creek fascia', row: 1 },
      3: { kind: 'panel', facilityId: 'p-1', name: 'Eagle Creek fascia', row: 2 },
    });
    return { ...facts, boards: facts.boards.map((b) => ({ ...b, nodeKey: BOARD })) };
  };

  /** compose_panel as the backend would answer: the request's lamps stay on their rows, plus the given edits. */
  const composeStub = (edits: unknown[], keptRows: Record<string, number>, seen: unknown[] = []) => ({
    ...panelIpc(editFacts()),
    compose_panel: (args: unknown) => {
      const request = (args as { request: { lamps: Array<{ entryId: string; name: string }> } }).request;
      seen.push(request);
      return {
        edits,
        assignments: request.lamps.flatMap((l) => (l.name in keptRows ? [{ entryId: l.entryId, nodeKey: BOARD, row: keptRows[l.name] }] : [])),
        issues: [],
      };
    },
  });
  const lampEdit = (row: number, value: number) => ({
    nodeKey: BOARD, space: 253, address: 1000 + row, path: `lamp#${row}/selection`, value: { type: 'int', value },
  });

  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    panelEditorStore.reset();
    configChangesStore.clearAllDrafts();
    eventStateStore.clear();
    invoke.mockReset();
    rosterSpy().mockReturnValue([lampBoardEntry(BOARD, { 1: 9, 2: 3 })]);
    channelsStore.hydrateBaseline(blocks);
    seedSavedBlockSignals();
    facilitiesStore.hydrateBaseline([
      ...facilitiesStore.facilities,
      seedSavedPanel({
        facilityId: 'p-1', name: 'Eagle Creek fascia', nodeKey: BOARD,
        entries: [{ blockId: 'block:cedar', row: 1 }, { blockId: 'block:mill', row: 2 }],
      }),
    ]);
  });

  async function openEdit(ipc: Record<string, (args?: unknown) => unknown>) {
    mockIpc(invoke, ipc);
    await renderRailroadPanel();
    await fireEvent.click(within(await screen.findByTestId('panel-card')).getByRole('button', { name: 'Edit facility' }));
    return screen.findByRole('dialog', { name: 'Eagle Creek fascia' });
  }

  it('Edit opens the editor with the panel blocks listed and, on Wiring, each block on its pin', async () => {
    const editor = await openEdit(composeStub([], {}));
    const onPanel = within(editor).getByRole('region', { name: 'Blocks on this panel' });
    expect(within(onPanel).getByText('Cedar Hill')).toBeInTheDocument();
    expect(within(onPanel).getByText('Mill curve')).toBeInTheDocument();

    await goToWiring(editor);
    const list = within(editor).getByRole('listbox', { name: 'Panel blocks' });
    await fireEvent.click(within(list).getByText('Cedar Hill'));
    expect(within(editor).getByRole('button', { name: /^#9 H3-G: This block/ })).toBeInTheDocument();
    expect(within(editor).getByRole('button', { name: /^#3 H1-R: Used by Mill curve/ })).toBeDisabled();
    await fireEvent.click(within(list).getByText('Mill curve'));
    expect(within(editor).getByRole('button', { name: /^#3 H1-R: This block/ })).toBeInTheDocument();
  }, 15000);

  it('moving a lamp to another pin stages exactly what compose returns, keeping the lamp channel', async () => {
    const seen: unknown[] = [];
    const editor = await openEdit(composeStub([lampEdit(1, 12)], { 'Cedar Hill': 1, 'Mill curve': 2 }, seen));
    await goToWiring(editor);
    await wirePanelBlocks(editor, [['Cedar Hill', PIN_12]]);
    await fireEvent.click(within(editor).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument());

    const lamps = (seen[0] as { lamps: Array<{ name: string; output: number }> }).lamps;
    expect(lamps.map((l) => l.output).sort((a, b) => a - b)).toEqual([3, 12]);
    const drafts = configChangesStore.draftEntries();
    expect(drafts).toHaveLength(1);
    expect(drafts[0].value).toMatchObject({ value: 12 });
    const lampChannels = channelsStore.channels.filter((c) => c.role === 'lamp-indicator');
    expect(lampChannels.map((c) => c.id)).toEqual(['lamp:p-1:1', 'lamp:p-1:2']);
  }, 15000);

  it('removing a block stages the returned reset of its row, drops its lamp channel and leaves the block and Automatic Block Signals alone', async () => {
    const absBefore = JSON.stringify(facilitiesStore.facilities.find((f) => f.templateId === 'automatic-block-signals'));
    const editor = await openEdit(composeStub([lampEdit(2, 0)], { 'Cedar Hill': 1 }));
    await fireEvent.click(within(editor).getByRole('button', { name: 'Remove Mill curve from panel' }));
    await goToWiring(editor);
    await fireEvent.click(within(editor).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument());

    const drafts = configChangesStore.draftEntries();
    expect(drafts).toHaveLength(1);
    expect(drafts[0].value).toMatchObject({ value: 0 });
    expect(channelsStore.channels.filter((c) => c.role === 'lamp-indicator').map((c) => c.id)).toEqual(['lamp:p-1:1']);
    expect(channelsStore.channels.filter((c) => c.role === 'block-occupancy').length).toBe(blocks.length);
    expect(JSON.stringify(facilitiesStore.facilities.find((f) => f.templateId === 'automatic-block-signals'))).toBe(absBefore);
  }, 15000);

  it('releases an outside pin: the card shows its Lamp Description, confirming seats the lamp, and Done stages the compose edits including the reset', async () => {
    const seen: Array<{ release?: unknown }> = [];
    const factsRequests: unknown[] = [];
    const outside = { 12: { kind: 'outside', row: 5, description: 'Aisle light' } } as const;
    const withOutside = (released: boolean) => {
      const facts = wiringFacts(released ? {} : { ...outside });
      return { ...facts, boards: facts.boards.map((b) => ({ ...b, nodeKey: BOARD })) };
    };
    const stubs = composeStub([lampEdit(1, 12), lampEdit(5, 0)], { 'Cedar Hill': 1, 'Mill curve': 2 }, seen);
    const editor = await openEdit({
      ...stubs,
      panel_wiring_facts: (args: unknown) => {
        factsRequests.push(args);
        return withOutside(!!(args as { request: { release?: unknown[] } }).request.release?.length);
      },
    });
    await goToWiring(editor);
    await fireEvent.click(within(within(editor).getByRole('listbox', { name: 'Panel blocks' })).getByText('Cedar Hill'));
    await fireEvent.click(within(editor).getByRole('button', { name: /^#12 H3-L: Outside Bowties/ }));

    const card = within(editor).getByRole('region', { name: 'Direct lamp 5, not managed by Bowties' });
    expect(card).toHaveTextContent('Description: "Aisle light"');
    await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
    await fireEvent.click(within(card).getByRole('button', { name: /^Release Direct lamp 5 and use #12 H3-L/ }));

    expect(await within(editor).findByRole('button', { name: /^#12 H3-L: This block/ })).toBeInTheDocument();
    expect(factsRequests.at(-1)).toMatchObject({ request: { release: [{ nodeKey: BOARD, owner: 'Direct lamp 5' }] } });

    await fireEvent.click(within(editor).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument());
    expect(seen[0].release).toEqual([{ nodeKey: BOARD, owner: 'Direct lamp 5' }]);
    const drafts = configChangesStore.draftEntries();
    expect(drafts.map((d) => (d.value as { value: number }).value).sort((a, b) => a - b)).toEqual([0, 12]);
  }, 15000);

  it('Cancel after changing the panel leaves no drafts and the card unchanged', async () => {
    const editor = await openEdit(composeStub([lampEdit(1, 12)], {}));
    const before = cardText();
    await goToWiring(editor);
    await wirePanelBlocks(editor, [['Cedar Hill', PIN_12]]);
    await fireEvent.click(within(editor).getByRole('button', { name: 'Cancel' }));

    expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument();
    expect(configChangesStore.draftEntries()).toEqual([]);
    expect(cardText()).toBe(before);
  }, 15000);
});


describe('RailroadPanel Blocks and lit when (Spec 021 / S3)', () => {
  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    panelEditorStore.reset();
    configChangesStore.clearAllDrafts();
    eventStateStore.clear();
    invoke.mockReset();
    rosterSpy().mockReturnValue([lampBoardEntry(BOARD_NODE)]);
  });

  it('adds an ABS snapshot without duplicates, reorders it, and saves and reopens lit-when settings', async () => {
    seedSavedBlockSignals();
    const abs = facilitiesStore.facilities[0];
    facilitiesStore.hydrateBaseline([{
      ...abs,
      name: 'Mainline Eastbound',
      blockSignals: {
        ...abs.blockSignals!,
        blocks: [blocks[2], blocks[0], blocks[1]].map((block) => ({
          blockId: block.id, name: block.name, joinedToPrevious: false, turnouts: [],
        })),
      },
    }]);
    const editor = await openPanelEditor(invoke, panelIpc(wiringFacts()));
    await addBlocks(editor, ['Mill curve']);
    const source = within(editor).getByLabelText('Add all blocks from');
    expect(source).toBeEnabled();
    await fireEvent.change(source, { target: { value: within(source).getByRole('option', { name: 'Mainline Eastbound · Automatic Block Signals' }).getAttribute('value') } });
    await fireEvent.click(within(within(editor).getByRole('region', { name: 'Detected blocks' })).getByRole('button', { name: 'Add all →' }));
    const onPanel = within(editor).getByRole('region', { name: 'Blocks on this panel' });
    expect(within(onPanel).getAllByRole('row').slice(1).map((item) => item.textContent)).toEqual([
      expect.stringContaining('Mill curve'), expect.stringContaining('River bridge'), expect.stringContaining('Cedar Hill'),
    ]);
    await fireEvent.click(within(onPanel).getByRole('button', { name: 'Move Mill curve down' }));
    expect(within(onPanel).getAllByRole('row').slice(1).map((item) => item.textContent)).toEqual([
      expect.stringContaining('River bridge'), expect.stringContaining('Mill curve'), expect.stringContaining('Cedar Hill'),
    ]);
    await goToWiring(editor);
    const list = within(editor).getByRole('listbox', { name: 'Panel blocks' });
    expect(within(list).getAllByRole('option').map((item) => item.textContent)).toEqual([
      expect.stringContaining('River bridge'), expect.stringContaining('Mill curve'), expect.stringContaining('Cedar Hill'),
    ]);
    await fireEvent.click(within(within(editor).getByRole('radiogroup', { name: 'Lamps are lit when the block is' })).getByRole('radio', { name: 'Clear' }));
    await fireEvent.click(within(list).getByRole('option', { name: /Mill curve/ }));
    await fireEvent.click(within(within(editor).getByRole('radiogroup', { name: 'Lit when' })).getByRole('radio', { name: 'Occupied' }));
    expect(within(list).getByRole('option', { name: /Mill curve/ })).toHaveTextContent('lit when occupied');
    await fireEvent.click(within(editor).getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument());
    const request = invoke.mock.calls.filter(([command]) => command === 'compose_panel').at(-1)?.[1] as { request: { record: { litWhen: string; entries: Array<{ blockId: string; litWhen?: string }> } } };
    expect(request.request.record).toMatchObject({ litWhen: 'clear', entries: [
      { blockId: 'block:river' }, { blockId: 'block:mill', litWhen: 'occupied' }, { blockId: 'block:cedar' },
    ] });
    const card = await screen.findByTestId('panel-card');
    expect(within(card).getAllByTestId('panel-tile').map((tile) => tile.textContent)).toEqual([
      expect.stringContaining('River bridge'), expect.stringContaining('Mill curve'), expect.stringContaining('Cedar Hill'),
    ]);
    expect(card).not.toHaveTextContent('lit when');
    await fireEvent.click(within(card).getByRole('button', { name: 'Edit facility' }));
    const reopened = await screen.findByRole('dialog', { name: 'Eagle Creek fascia' });
    await goToWiring(reopened);
    expect(within(within(reopened).getByRole('radiogroup', { name: 'Lamps are lit when the block is' })).getByRole('radio', { name: 'Clear' })).toBeChecked();
    await fireEvent.click(within(within(reopened).getByRole('listbox', { name: 'Panel blocks' })).getByRole('option', { name: /Mill curve/ }));
    expect(within(within(reopened).getByRole('radiogroup', { name: 'Lit when' })).getByRole('radio', { name: 'Occupied' })).toBeChecked();
    await fireEvent.click(within(reopened).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('dialog', { name: 'Eagle Creek fascia' })).not.toBeInTheDocument();
  }, 15000);
});
