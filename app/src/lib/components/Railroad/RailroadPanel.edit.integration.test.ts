/**
 * Spec 020 / E2a-T1 — edit a saved Automatic Block Signals facility end to end
 * through the Facilities card: real stores and orchestrators, IPC mocked only.
 * The layout starts as loaded from disk (seeded baseline), not built through the editor.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { cancelBlockSignalsEditor } from '$lib/orchestration/blockSignalsEditorOrchestrator';
import { effectiveNodeStore } from '$lib/layout';
import type { ComposeBlockSignalsRequest } from '$lib/api/signaling';
import SaveControls from '../ElementCardDeck/SaveControls.svelte';
import {
  addBlock,
  chooseWiringNode,
  mockIpc,
  openEditor,
  releaseOutsideIpc,
  removalIpc,
  renderRailroadPanel,
  SAVED_FACILITY_ID,
  SAVED_FACILITY_NAME,
  SAVED_SIGNAL_CHANNEL_ID,
  seedSavedBlockSignals,
  seedSignalNodeTree,
  SIGNAL_NODE,
  signalIpc,
} from './railroadPanelTestSupport';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

describe('Edit a saved Automatic Block Signals facility (E2a-T1)', () => {
  let editValue = 3;

  beforeEach(async () => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    configChangesStore.clearAllDrafts();
    cancelBlockSignalsEditor();
    invoke.mockReset();
    editValue = 3;
    seedSavedBlockSignals();
    mockIpc(invoke, signalIpc(() => [], () => editValue));
    await renderRailroadPanel();
    render(SaveControls, { props: { toolbar: true } });
  });

  const signalRow = () => within(screen.getByRole('table')).getByText('Cedar Hill East').closest('tr')!;
  const editorDialog = () => screen.queryByRole('dialog', { name: /edit automatic block signals/i });

  it('reopens on the saved choices, stages new pins on the same channel, and Discard restores them', async () => {
    expect(effectiveNodeStore.isDirty).toBe(false);
    expect(signalRow()).toHaveTextContent('#3 / #1');

    await fireEvent.click(within(screen.getByTestId('facility-card')).getByRole('button', { name: 'Edit facility' }));
    const editor = await screen.findByRole('dialog', { name: /edit automatic block signals/i });
    expect(within(editor).getByLabelText('Name')).toHaveValue(SAVED_FACILITY_NAME);
    expect(within(within(editor).getByRole('table')).getByText('Cedar Hill')).toBeInTheDocument();

    await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
    await fireEvent.click(await within(editor).findByRole('button', { name: 'Next: Appearance' }));
    await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Wiring' }));

    // the saved wiring is shown: node, red and green pins
    expect(await within(editor).findByRole('button', { name: /Runs on.*02\.01\.57\.00\.00\.01/ })).toBeInTheDocument();
    const grid = await within(editor).findByRole('group', { name: 'Pins on 02.01.57.00.00.01' });
    expect(within(grid).getByRole('button', { name: 'H1-R: This signal' })).toBeInTheDocument();
    expect(within(grid).getByRole('button', { name: '#1 H1-G: This signal' })).toBeInTheDocument();

    // change red then green
    editValue = 5;
    await fireEvent.click(within(grid).getByRole('button', { name: '#2 H2-R: Free' }));
    await fireEvent.click(within(editor).getByRole('button', { name: /Green lamp/ }));
    await fireEvent.click(within(grid).getByRole('button', { name: '#4 H2-G: Free' }));
    const done = within(editor).getByRole('button', { name: 'Done' });
    await waitFor(() => expect(done).toBeEnabled());
    await fireEvent.click(done);
    await waitFor(() => expect(editorDialog()).not.toBeInTheDocument());

    // compose carried the facility id; the same channel is rebound and still used by the facility
    const composes = invoke.mock.calls.filter(([command]) => command === 'compose_block_signals');
    expect((composes[composes.length - 1][1] as { request: ComposeBlockSignalsRequest }).request.facilityId).toBe(SAVED_FACILITY_ID);
    expect(channelsStore.channels.filter((c) => c.name === 'Cedar Hill East').map((c) => c.id)).toEqual([SAVED_SIGNAL_CHANNEL_ID]);
    expect(signalRow()).toHaveTextContent('#2 / #4');
    expect(signalRow()).toHaveTextContent(`${SAVED_FACILITY_NAME} / signals`);

    // the change shows up as unsaved work
    const toolbar = screen.getByRole('group', { name: 'Configuration save controls' });
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ channels: 1, config: 1, configNodes: 1 });

    // Discard restores the saved pins
    await fireEvent.click(within(toolbar).getByRole('button', { name: 'Discard pending changes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Revert' }));
    await waitFor(() => expect(signalRow()).toHaveTextContent('#3 / #1'));
    expect(signalRow()).toHaveTextContent(`${SAVED_FACILITY_NAME} / signals`);
    expect(effectiveNodeStore.isDirty).toBe(false);
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ channels: 0, config: 0 });
  });

  it('renaming the facility alone stages no configuration edits', async () => {
    await fireEvent.click(within(screen.getByTestId('facility-card')).getByRole('button', { name: 'Rename facility' }));
    const input = screen.getByLabelText('Edit facility name');
    await fireEvent.input(input, { target: { value: 'Cedar Hill Main' } });
    await fireEvent.keyDown(input, { key: 'Enter' });

    await screen.findByRole('group', { name: 'Configuration save controls' });
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ facilities: 1, config: 0, configNodes: 0, channels: 0 });
  });
});

describe('Remove a saved Automatic Block Signals facility (E2b-T1)', () => {
  beforeEach(async () => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    configChangesStore.clearAllDrafts();
    cancelBlockSignalsEditor();
    invoke.mockReset();
    seedSavedBlockSignals();
  });

  async function setup(ipc: ReturnType<typeof removalIpc>) {
    seedSignalNodeTree();
    mockIpc(invoke, ipc);
    await renderRailroadPanel();
    render(SaveControls, { props: { toolbar: true } });
  }

  const card = () => screen.queryByTestId('facility-card');
  const signalRow = () => within(screen.getByRole('table')).queryByText('Cedar Hill East');
  const removeDialog = () => screen.queryByRole('alertdialog', { name: `Remove ${SAVED_FACILITY_NAME}?` });
  const openRemove = async () => {
    await fireEvent.click(within(screen.getByTestId('facility-card')).getByRole('button', { name: 'Delete facility' }));
    return screen.findByRole('alertdialog', { name: `Remove ${SAVED_FACILITY_NAME}?` });
  };

  it('confirms first, stages the release edits with the removal, and Discard restores everything', { timeout: 20000 }, async () => {
    await setup(removalIpc());
    expect(effectiveNodeStore.isDirty).toBe(false);

    const dialog = await openRemove();
    expect(within(dialog).getByText(/02\.01\.57\.00\.00\.01/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Mast 1 and Logic 1/)).toBeInTheDocument();

    // Cancel changes nothing
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(removeDialog()).not.toBeInTheDocument());
    expect(card()).toBeInTheDocument();
    expect(effectiveNodeStore.isDirty).toBe(false);

    // Remove stages the release edits and removes the facility and its channel
    await openRemove();
    await fireEvent.click(within(removeDialog()!).getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(card()).not.toBeInTheDocument());
    expect(removeDialog()).not.toBeInTheDocument();
    expect(signalRow()).not.toBeInTheDocument();
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ config: 7, configNodes: 1, facilities: 1, channels: 1 });
    const toolbar = await screen.findByRole('group', { name: 'Configuration save controls' });

    // Discard restores the facility card and its channel
    await fireEvent.click(within(toolbar).getByRole('button', { name: 'Discard pending changes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Revert' }));
    await waitFor(() => expect(card()).toBeInTheDocument());
    expect(signalRow()).toBeInTheDocument();
    expect(effectiveNodeStore.isDirty).toBe(false);
  });

  it('when the board cannot be reset, shows why and removes without resetting it', { timeout: 20000 }, async () => {
    const issue = {
      code: 'hardwareUnavailable',
      signalId: null,
      nodeKey: SIGNAL_NODE,
      detail: 'unavailable',
    };
    await setup(removalIpc(() => [issue]));

    await openRemove();
    await fireEvent.click(within(removeDialog()!).getByRole('button', { name: 'Remove' }));
    const refused = await screen.findByRole('alert');
    expect(refused).toHaveTextContent('The Signal LCC settings could not be read');
    expect(card()).toBeInTheDocument();
    expect(effectiveNodeStore.isDirty).toBe(false);

    await fireEvent.click(within(removeDialog()!).getByRole('button', { name: 'Remove without resetting the board' }));
    await waitFor(() => expect(card()).not.toBeInTheDocument());
    expect(signalRow()).not.toBeInTheDocument();
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ config: 0, facilities: 1, channels: 1 });
  });
});

describe('Release a pin used outside Bowties (E2c-T1)', () => {
  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    configChangesStore.clearAllDrafts();
    cancelBlockSignalsEditor();
    invoke.mockReset();
  });

  /** Open the editor on Wiring for a new facility, click the pin Mast 2 holds and confirm its release. */
  async function releaseHeldPin() {
    seedSignalNodeTree();
    const editor = await openEditor(invoke, 'Cedar Hill — Eastward', undefined, releaseOutsideIpc());
    render(SaveControls, { props: { toolbar: true } });
    await addBlock('Cedar Hill');
    await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
    await fireEvent.click(await within(editor).findByRole('button', { name: 'Next: Appearance' }));
    await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Wiring' }));
    await chooseWiringNode(editor);
    const grid = await within(editor).findByRole('group', { name: 'Pins on 02.01.57.00.00.01' });
    await fireEvent.click(within(grid).getByRole('button', { name: /#3 H3-R/ }));
    const card = within(editor).getByRole('region', { name: 'Mast 2, not managed by Bowties' });
    expect(card).toHaveTextContent('Aspects set by Logic 1');
    await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
    expect(card).toHaveTextContent('Release Mast 2? All 2 of its pins (#3 H3-R, #4 H3-G) become free, and Mast 2, Logic 1 are reset when you save.');
    await fireEvent.click(within(card).getByRole('button', { name: 'Release and put Red on #3 H3-R' }));
    return { editor, grid };
  }

  const requestsTo = (command: string) =>
    invoke.mock.calls
      .filter(([name]) => name === command)
      .map(([, args]) => (args as { request: { release?: Array<{ owner: string }> } }).request);

  it('releases the owner with the facility, stages its resets with fresh IDs, and Discard restores them', { timeout: 20000 }, async () => {
    const { editor, grid } = await releaseHeldPin();

    await waitFor(() => expect(within(grid).getByRole('button', { name: '#3 H3-R: This signal' })).toBeInTheDocument());
    expect(within(grid).getByRole('button', { name: '#4 H3-G: Free' })).toBeInTheDocument();
    expect(within(editor).queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();
    const previews = requestsTo('preview_block_signals');
    expect(previews[previews.length - 1].release).toEqual([{ nodeKey: SIGNAL_NODE, owner: 'Mast 2' }]);

    await fireEvent.click(within(grid).getByRole('button', { name: /#1 H1-G/ }));
    const done = within(editor).getByRole('button', { name: 'Done' });
    await waitFor(() => expect(done).toBeEnabled());
    await fireEvent.click(done);
    await waitFor(() => expect(screen.queryByRole('dialog', { name: /automatic block signals/i })).not.toBeInTheDocument());

    const composes = requestsTo('compose_block_signals');
    expect(composes[composes.length - 1].release).toEqual([{ nodeKey: SIGNAL_NODE, owner: 'Mast 2' }]);

    // the mast edit, Mast 2's function reset and two fresh event IDs are unsaved changes
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ config: 4, configNodes: 1 });
    const toolbar = await screen.findByRole('group', { name: 'Configuration save controls' });

    await fireEvent.click(within(toolbar).getByRole('button', { name: 'Discard pending changes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Revert' }));
    await waitFor(() => expect(effectiveNodeStore.isDirty).toBe(false));
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ config: 0 });
  });

  it('Cancel after releasing stages nothing', { timeout: 20000 }, async () => {
    const { editor } = await releaseHeldPin();
    await fireEvent.click(within(editor).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: /automatic block signals/i })).not.toBeInTheDocument());
    expect(effectiveNodeStore.isDirty).toBe(false);
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ config: 0, configNodes: 0 });
  });
});
