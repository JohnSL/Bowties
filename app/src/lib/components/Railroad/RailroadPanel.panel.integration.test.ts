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
import { wiringFacts } from '../Panel/panelEditorTestSupport';
import { openPanelEditor, panelIpc, wirePanelBlocks } from './railroadPanelTestSupport';

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
    invoke.mockReset();
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
    expect(within(card).getByText('3 blocks')).toBeInTheDocument();
    expect(within(card).queryByRole('button')).not.toBeInTheDocument();
  });

  it('clears a block pin when it is clicked again, and disables a pin owned by Automatic Block Signals', async () => {
    const facts = wiringFacts({ 14: { kind: 'facility', facilityId: 'abs-1', name: 'Mainline Eastbound' } });
    const editor = await openPanelEditor(invoke, panelIpc(facts));
    await addBlocks(editor, ['Cedar Hill']);
    await goToWiring(editor);

    const blocks = within(editor).getByRole('listbox', { name: 'Panel blocks' });
    await wirePanelBlocks(editor, [['Cedar Hill', PIN_9]]);
    expect(within(blocks).getByText('wired')).toBeInTheDocument();

    await fireEvent.click(within(editor).getByRole('button', { name: /^#9 H3-G: this block/ }));
    expect(within(blocks).getByText('needs a lamp')).toBeInTheDocument();
    expect(within(editor).getByRole('button', { name: /^#9 H3-G: free/ })).toBeInTheDocument();

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
    expect(within(card).getByText('Incomplete')).toBeInTheDocument();
    expect(within(card).getByText('2 blocks')).toBeInTheDocument();
    expect(within(card).getByText('1 needs a lamp')).toBeInTheDocument();
  });
});
