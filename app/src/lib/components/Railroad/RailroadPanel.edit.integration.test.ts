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
  mockIpc,
  renderRailroadPanel,
  SAVED_FACILITY_ID,
  SAVED_FACILITY_NAME,
  SAVED_SIGNAL_CHANNEL_ID,
  seedSavedBlockSignals,
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
