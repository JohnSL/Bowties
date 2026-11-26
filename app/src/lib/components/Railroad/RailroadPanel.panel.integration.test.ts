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
  for (const name of names) await fireEvent.click(within(editor).getByRole('button', { name: `Add ${name}` }));
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
    expect(within(card).queryByRole('button')).not.toBeInTheDocument();
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
