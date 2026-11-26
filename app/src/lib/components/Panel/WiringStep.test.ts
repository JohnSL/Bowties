import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import type { PanelWiringFacts, PinOwner } from '$lib/api/panels';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import type { DiscoveredNode } from '$lib/api/tauri';
import { BOARD_NODE, OTHER_BOARD_NODE, wiringFacts } from './panelEditorTestSupport';

const panelWiringFactsMock = vi.fn<(nodeKeys?: string[], release?: unknown[]) => Promise<PanelWiringFacts>>();
vi.mock('$lib/api/panels', () => ({ panelWiringFacts: panelWiringFactsMock }));
const createPanelFacilityMock = vi.fn();
vi.mock('$lib/orchestration/facilityOrchestrator', () => ({
  createPanelFacility: createPanelFacilityMock,
  syncDraftsForComposition: vi.fn(async () => undefined),
}));

const { default: PanelEditor } = await import('./PanelEditor.svelte');
const { panelEditorStore } = await import('$lib/stores/panelEditor.svelte');
const { panelEditorActions } = await import('$lib/orchestration/panelEditorOrchestrator');

const DETECTED = [
  { blockId: 'block:a', name: 'Main 1' },
  { blockId: 'block:b', name: 'Main 2' },
];

async function openOnWiring(owners: Record<number, PinOwner> = {}, blocks = DETECTED) {
  panelWiringFactsMock.mockResolvedValue(wiringFacts(owners));
  panelEditorActions.startPanelEditor('Fascia');
  for (const block of blocks) panelEditorActions.addBlock(block);
  render(PanelEditor, { props: { view: panelEditorStore, actions: panelEditorActions, detectedBlocks: DETECTED } });
  await fireEvent.click(screen.getByRole('button', { name: 'Next: Wiring' }));
  await waitFor(() => expect(panelEditorStore.factsLoaded).toBe(true));
}

const snipNode = (userName: string): DiscoveredNode =>
  ({ snip_data: { user_name: userName, model: 'Signal-LCC', manufacturer: 'RR-CirKits, Inc.' } }) as unknown as DiscoveredNode;

const row = (name: string) => screen.getByRole('option', { name: new RegExp(name) });
const pin = (label: RegExp) => screen.getByRole('button', { name: label });

describe('Panel editor Wiring step', () => {
  beforeEach(() => {
    nodeInfoStore.set(new Map([[BOARD_NODE, snipNode('Eagle Creek Fascia')], [OTHER_BOARD_NODE, snipNode('West Signals')]]));
    panelEditorStore.reset();
    panelWiringFactsMock.mockReset();
    createPanelFacilityMock.mockReset();
  });

  it('shows no-blocks guidance and still allows Done with zero blocks', async () => {
    createPanelFacilityMock.mockResolvedValue({ ok: true });
    await openOnWiring({}, []);
    expect(screen.getByText('No blocks to wire')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(createPanelFacilityMock).toHaveBeenCalledTimes(1);
  });

  it('rows show their status and selecting a block shows its board and the labelled pin grid', async () => {
    await openOnWiring();
    expect(within(row('Main 1')).getByText('needs a lamp')).toBeInTheDocument();
    expect(within(row('Main 1')).getByText('No lamp yet')).toBeInTheDocument();
    expect(screen.getByText('0 of 2 wired')).toBeInTheDocument();
    expect(screen.getByText('2 blocks without a lamp. Done saves the panel as Incomplete.')).toBeInTheDocument();

    await fireEvent.click(row('Main 1'));
    expect(screen.getByText('Lamp for Main 1')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Board/ }).textContent?.replace(/\s+/g, ' ')).toContain(
      'Eagle Creek Fascia Signal-LCC · 05.01.01.01.01.01',
    );
    const grid = screen.getByRole('group', { name: 'Pins on Eagle Creek Fascia' });
    for (const head of ['H1', 'H2', 'H3', 'H4']) expect(grid).toHaveTextContent(head);
    expect(pin(/^#1 H1-G: Free/)).toBeInTheDocument();
    expect(pin(/^#16 H4-L: Free/)).toBeInTheDocument();
  });

  it('chooses another board through the picker dialog and shows its pins', async () => {
    await openOnWiring();
    await fireEvent.click(row('Main 1'));
    await fireEvent.click(screen.getByRole('button', { name: /Board/ }));
    const dialog = screen.getByRole('dialog', { name: 'Choose a Signal LCC' });
    await fireEvent.click(within(dialog).getByRole('radio', { name: /West Signals/ }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Confirm' }));
    expect(panelEditorStore.boardNodeKey).toBe(OTHER_BOARD_NODE);
    expect(screen.getByRole('button', { name: /Board/ })).toHaveTextContent('West Signals');
  });

  it('a free pin wires the block and clicking it again clears it', async () => {
    await openOnWiring();
    await fireEvent.click(row('Main 1'));
    await fireEvent.click(pin(/^#9 H3-G/));
    expect(within(row('Main 1')).getByText('wired')).toBeInTheDocument();
    expect(within(row('Main 1')).getByText('Eagle Creek Fascia · #9 H3-G')).toBeInTheDocument();
    expect(pin(/^#9 H3-G: This block/)).toBeInTheDocument();
    expect(screen.getByText('1 of 2 wired')).toBeInTheDocument();

    await fireEvent.click(pin(/^#9 H3-G/));
    expect(within(row('Main 1')).getByText('needs a lamp')).toBeInTheDocument();
    expect(pin(/^#9 H3-G: Free/)).toBeInTheDocument();
  });

  it("shows another block's pin as used by that block and not choosable", async () => {
    await openOnWiring();
    await fireEvent.click(row('Main 1'));
    await fireEvent.click(pin(/^#9 H3-G/));
    await fireEvent.click(row('Main 2'));
    expect(pin(/^#9 H3-G: Used by Main 1/)).toBeDisabled();
  });

  it('disables pins used by another facility and shows outside pins by their Lamp Description', async () => {
    await openOnWiring({
      1: { kind: 'facility', facilityId: 'abs-1', name: 'Mainline Eastbound' },
      16: { kind: 'outside', row: 3, description: 'Aisle light' },
    });
    await fireEvent.click(row('Main 1'));
    expect(pin(/^#1 H1-G: Mainline Eastbound/)).toBeDisabled();
    expect(pin(/^#16 H4-L: Outside Bowties/)).toBeEnabled();
    expect(screen.getByText('Outside Bowties · “Aisle light”')).toBeInTheDocument();
  });

  it('opens the shared release card for an outside pin, and confirming releases it and seats the lamp', async () => {
    await openOnWiring({ 16: { kind: 'outside', row: 3, description: 'Aisle light' } });
    await fireEvent.click(row('Main 1'));
    await fireEvent.click(pin(/^#16 H4-L: Outside Bowties/));
    expect(screen.queryByText(/cannot be used here/)).not.toBeInTheDocument();
    const card = screen.getByRole('region', { name: 'Direct lamp 3, not managed by Bowties' });
    expect(card).toHaveTextContent('Description: "Aisle light"');
    expect(within(row('Main 1')).getByText('needs a lamp')).toBeInTheDocument();

    panelWiringFactsMock.mockResolvedValue(wiringFacts({}));
    await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
    expect(card).toHaveTextContent('Release Direct lamp 3? Its pin (#16 H4-L) becomes free, and Direct lamp 3 is reset when you save.');
    await fireEvent.click(within(card).getByRole('button', { name: /^Release Direct lamp 3 and use #16 H4-L/ }));

    await waitFor(() => expect(pin(/^#16 H4-L: This block/)).toBeInTheDocument());
    expect(within(row('Main 1')).getByText('Eagle Creek Fascia · #16 H4-L')).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();
    expect(panelWiringFactsMock).toHaveBeenLastCalledWith(undefined, [{ nodeKey: BOARD_NODE, owner: 'Direct lamp 3' }]);
  });

  it('shows what the backend could not release as an error note', async () => {
    await openOnWiring({ 16: { kind: 'outside', row: 3, description: 'Aisle light' } });
    await fireEvent.click(row('Main 1'));
    await fireEvent.click(pin(/^#16 H4-L: Outside Bowties/));
    panelWiringFactsMock.mockResolvedValue({ ...wiringFacts({}), releaseIssues: [{ nodeKey: BOARD_NODE, detail: 'Board has no row 3' }] });
    await fireEvent.click(screen.getByRole('button', { name: 'Release and use…' }));
    await fireEvent.click(screen.getByRole('button', { name: /^Release Direct lamp 3 and use/ }));
    expect(await screen.findByText('Board has no row 3')).toBeInTheDocument();
  });

  it('keeps Lit-when controls disabled with Occupied selected, and Done calls finish', async () => {
    createPanelFacilityMock.mockResolvedValue({ ok: false, error: new Error('Could not save') });
    await openOnWiring();
    expect(screen.getByRole('radio', { name: 'Occupied' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Clear' })).toBeDisabled();

    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(createPanelFacilityMock).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not save');
    expect(screen.getByRole('button', { name: 'Back' })).toBeInTheDocument();
  });
});
