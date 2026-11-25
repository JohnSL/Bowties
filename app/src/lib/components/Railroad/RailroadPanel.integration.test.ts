import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import RailroadPanel from './RailroadPanel.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import type { BlockSignalsPreview, BlockSignalsPreviewRequest } from '$lib/api/signaling';
import { cancelBlockSignalsEditor } from '$lib/orchestration/blockSignalsEditorOrchestrator';
import { eventStateStore } from '$lib/stores/eventState.svelte';
import { effectiveLayoutStore, effectiveNodeStore } from '$lib/layout';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import SaveControls from '../ElementCardDeck/SaveControls.svelte';
import {
  addBlock,
  bodChannel,
  fakePreview,
  lastRequest as lastRequestOf,
  openEditor as openEditorWith,
  SIGNAL_NODE,
  signalIpc,
  stubNodeName,
  wireFirstSignal,
  type IpcStubs,
} from './railroadPanelTestSupport';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

describe('RailroadPanel (Spec 018 / S3 — hardware-organised Channels panel)', () => {
  const stubDaughterboardName = (_nodeKey: string, _connector: string) => 'BOD-8';

  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    invoke.mockReset();
  });

  it('composes a Facilities section above a Channels section', () => {
    render(RailroadPanel, { props: { nodeName: stubNodeName } });
    expect(screen.getByTestId('facilities-section')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /^channels$/i })).toBeInTheDocument();
  });

  describe('with 8 BOD-8 channels on TowerLCC-1 connector A', () => {
    beforeEach(() => {
      const channels = Array.from({ length: 8 }, (_, i) => bodChannel(i + 1));
      channelsStore.hydrateBaseline(channels);
    });

    it('renders the 6-column header row including "Used by"', () => {
      render(RailroadPanel, {
        props: { nodeName: stubNodeName, daughterboardName: stubDaughterboardName },
      });
      const table = screen.getByRole('table');
      const headers = within(table).getAllByRole('columnheader');
      // 6 columns: state-dot (header text may be empty), Name, Role / Style,
      // Location, State, Used by.
      expect(headers.length).toBe(6);
      const labels = headers.map((h) => h.textContent?.trim() ?? '');
      expect(labels).toEqual(
        expect.arrayContaining(['Name', 'Role / Style', 'Location', 'State', 'Used by']),
      );
    });

    it('groups channels under a node + connector + daughter-board header row', () => {
      render(RailroadPanel, {
        props: { nodeName: stubNodeName, daughterboardName: stubDaughterboardName },
      });
      // Group-header row text mentions the node, the connector, and the
      // daughter-board id (mockup 4/8: "TowerLCC-1 · Connector A · BOD-8").
      const groupHeader = screen.getByText(
        (content) =>
          content.includes('TowerLCC-1') &&
          /connector\s*a/i.test(content) &&
          /bod-?8/i.test(content),
      );
      expect(groupHeader).toBeInTheDocument();
      // The group header should span all 6 columns (single <td colspan="6">).
      const groupRow = groupHeader.closest('tr');
      expect(groupRow).not.toBeNull();
      const groupCells = groupRow!.querySelectorAll('td');
      expect(groupCells.length).toBe(1);
      expect(groupCells[0].getAttribute('colspan')).toBe('6');
    });

    it('renders 8 channel rows with name + HW badge + role/style + location + "—" Used-by', () => {
      render(RailroadPanel, {
        props: { nodeName: stubNodeName, daughterboardName: stubDaughterboardName },
      });

      // All 8 channels appear by their default names.
      for (let i = 1; i <= 8; i++) {
        expect(screen.getByText(`TowerLCC-1 BOD A${i}`)).toBeInTheDocument();
      }

      // Inspect the first row in detail.
      const row1Name = screen.getByText('TowerLCC-1 BOD A1');
      const row1 = row1Name.closest('tr');
      expect(row1).not.toBeNull();
      const row1Cells = within(row1!).getAllByRole('cell');
      expect(row1Cells.length).toBe(6);

      // Ownership badge: HW for hardware-owned channels.
      expect(within(row1!).getByText(/^HW$/)).toBeInTheDocument();
      // Role label (human-readable) and style id present.
      expect(within(row1!).getByText(/block occupancy/i)).toBeInTheDocument();
      expect(within(row1!).getByText('bod-block-detector-input')).toBeInTheDocument();
      // Location renders connector + input ordinal.
      expect(
        within(row1!).getByText((content) =>
          /connector\s*a/i.test(content) && /input\s*1\b/i.test(content),
        ),
      ).toBeInTheDocument();
      // Used-by column shows em-dash for unbound channels (S3: no facilities yet).
      const usedByCell = row1Cells[row1Cells.length - 1];
      expect(usedByCell.textContent?.trim()).toBe('—');
    });
  });

  describe('Automatic Block Signals editor (S2R)', () => {
    // the editor store outlives a render; close it so each test starts clean
    beforeEach(() => cancelBlockSignalsEditor());

    const openEditor = (
      name?: string,
      resolvedEventIds?: ReadonlyMap<string, Record<string, string>>,
      ipc: IpcStubs = {},
    ) => openEditorWith(invoke, name, resolvedEventIds, ipc);
    const lastRequest = () => lastRequestOf(invoke);

    it('shows live occupancy for blocks with readings and unavailable for the rest', async () => {
      eventStateStore.clear();
      const editor = await openEditor('Cedar Hill — Eastward', new Map([
        ['block:cedar', { occupied: '0501010100000001', clear: '0501010100000002' }],
        ['block:mill', { occupied: '0501010100000003', clear: '0501010100000004' }],
      ]));
      await addBlock('Cedar Hill');
      await addBlock('Mill curve');
      await addBlock('River bridge');
      expect(within(editor).getByRole('img', { name: 'Cedar Hill: occupancy unavailable' })).toBeInTheDocument();
      expect(within(editor).getByText('Occupancy unavailable', { selector: '.diagram-head span' })).toBeInTheDocument();

      eventStateStore.record('0501010100000001', 100);
      eventStateStore.record('0501010100000004', 100);

      expect(await within(editor).findByRole('img', { name: 'Cedar Hill: occupied' })).toBeInTheDocument();
      expect(within(editor).getByRole('img', { name: 'Mill curve: clear' })).toBeInTheDocument();
      // no events resolved for River bridge, so it stays unavailable rather than looking live
      expect(within(editor).getByRole('img', { name: 'River bridge: occupancy unavailable' })).toBeInTheDocument();
      expect(within(editor).getByText('Live occupancy')).toBeInTheDocument();

      eventStateStore.record('0501010100000002', 200);
      expect(await within(editor).findByRole('img', { name: 'Cedar Hill: clear' })).toBeInTheDocument();
      eventStateStore.clear();
    });

    it('offers Interlocking Plant disabled beside Automatic Block Signals in Add facility', async () => {
      invoke.mockResolvedValue([{ templateId: 'block-indicator', displayName: 'Block Indicator', slots: [], mapping: [] }, { templateId: 'automatic-block-signals', displayName: 'Automatic Block Signals', slots: [], mapping: [] }]);
      await behaviorTemplatesStore.loadBehaviorTemplates();
      render(RailroadPanel, { props: { nodeName: stubNodeName } });
      await fireEvent.click(screen.getByTestId('add-facility-button'));

      const plant = screen.getByRole('radio', { name: /^Interlocking Plant/ });
      const abs = screen.getByRole('radio', { name: /^Automatic Block Signals/ });
      expect(plant).toBeDisabled();
      expect(screen.getByText('Coming in a later release')).toBeInTheDocument();
      expect(abs).toBeEnabled();
      expect(screen.getByText('Signals that respond to occupancy, the next signal, and hand-thrown turnouts.')).toBeInTheDocument();
      // Automatic Block Signals is listed before Interlocking Plant
      expect(abs.compareDocumentPosition(plant) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    /** Join Mill curve to Cedar Hill and add a "Mill siding" turnout to Mill curve (blocks already added). */
    async function joinMillAndAddTurnout(editor: HTMLElement) {
      await fireEvent.click(within(editor).getByRole('button', { name: 'More for Mill curve' }));
      await fireEvent.click(within(editor).getByRole('menuitem', { name: /remove the signal between cedar hill and mill curve/i }));
      await fireEvent.click(within(within(editor).getByRole('table')).getByText('Mill curve'));
      await fireEvent.click(within(editor).getByRole('button', { name: 'Add turnout in block' }));
      await fireEvent.change(await within(editor).findByLabelText('Turnout name'), { target: { value: 'Mill siding' } });
    }

    it('builds a travel-order list with a joined block and a turnout', async () => {
      const editor = await openEditor();

      await addBlock('Cedar Hill');
      await addBlock('Mill curve');
      // joining is a less common choice, so it lives in the row's "More" menu
      expect(within(editor).queryByRole('button', { name: 'More for Cedar Hill' })).not.toBeInTheDocument();
      await joinMillAndAddTurnout(editor);
      expect(within(editor).getByText('No signal between it and Cedar Hill')).toBeInTheDocument();
      expect(within(editor).queryByRole('menu')).not.toBeInTheDocument();

      await waitFor(() => expect(lastRequest().blocks).toEqual([
        { blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] },
        { blockId: 'block:mill', name: 'Mill curve', joinedToPrevious: true, turnouts: [{ turnoutId: expect.stringMatching(/^turnout:/), name: 'Mill siding' }] },
      ]));
      expect(within(editor).getByRole('button', { name: /signals between cedar hill and mill curve are off/i })).toBeInTheDocument();
      expect(facilitiesStore.isEmpty).toBe(true);
    });

    it('shows the rules of a joined block with a turnout on Aspects', async () => {
      const editor = await openEditor();
      await addBlock('Cedar Hill');
      await addBlock('Mill curve');
      await joinMillAndAddTurnout(editor);

      await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
      const first = await within(editor).findByRole('button', { name: /configure cedar hill east; protects cedar hill, mill curve/i });
      expect(first).toHaveAttribute('aria-pressed', 'true');
      expect(within(editor).getByLabelText('Selected signal name')).toHaveValue('Cedar Hill East');
      expect(within(editor).getByRole('table', { name: 'Rules for Cedar Hill East' })).toHaveTextContent(
        'A protected block occupiedStopMill siding not lined for the main, or position unknownStopProtected blocks clearClear',
      );

      // Restricting is outside this release's scope: offered, but not selectable
      expect(within(editor).getByRole('radio', { name: 'Restricting' })).toBeDisabled();
      expect(within(editor).getByRole('button', { name: 'Next: Appearance' })).toBeEnabled();
      expect(within(editor).queryByRole('button', { name: /^\d?\s*Review/ })).not.toBeInTheDocument();
      expect(within(editor).queryByRole('button', { name: /save|apply|confirm/i })).not.toBeInTheDocument();
    });

    it('sets what lies beyond the last block from its end chip, with Plants not yet available', async () => {
      const editor = await openEditor();
      await addBlock('Cedar Hill');
      await addBlock('Mill curve');

      await fireEvent.click(within(editor).getByRole('button', { name: /nothing connected beyond mill curve/i }));
      const menu = within(editor).getByRole('dialog', { name: 'What lies beyond Mill curve' });
      expect(within(menu).getByRole('radio', { name: /^Nothing beyond The last signal/ })).toBeChecked();
      expect(within(menu).getByRole('button', { name: 'Choose a Plant…' })).toBeDisabled();
      expect(menu.textContent).not.toMatch(/later release/i);
      await fireEvent.click(within(menu).getByRole('radio', { name: /treat as stop/i }));

      await waitFor(() => expect(lastRequest().exitEnd).toEqual({ kind: 'treatAsStop' }));
      expect(within(editor).getByRole('button', { name: /beyond mill curve: end: stop/i })).toBeInTheDocument();

      await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
      await fireEvent.click(await within(editor).findByRole('button', { name: /configure mill curve east/i }));
      expect(within(editor).getByRole('table', { name: 'Rules for Mill curve East' }))
        .toHaveTextContent('Protected block clear; nothing beyond, treated as StopApproach');
      expect(within(editor).getByText(/shows Approach at best/)).toBeInTheDocument();
    });

    it('shows Both directions disabled and keeps the list order when the direction changes', async () => {
      const editor = await openEditor();
      await addBlock('Cedar Hill');
      await addBlock('Mill curve');

      expect(within(editor).getByRole('radio', { name: /both directions/i })).toBeDisabled();
      expect(within(editor).getByRole('radio', { name: /one direction/i })).toBeEnabled();
      await fireEvent.click(within(editor).getByRole('radio', { name: 'Westward' }));

      await waitFor(() => expect(lastRequest()).toMatchObject({ movements: 'oneDirection', direction: 'westward' }));
      expect(lastRequest().blocks.map((block) => block.blockId)).toEqual(['block:cedar', 'block:mill']);
      expect(within(editor).getByText('Track, in westward order')).toBeInTheDocument();

      await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
      expect(await within(editor).findByRole('button', { name: /configure cedar hill west/i })).toBeInTheDocument();
      expect(within(editor).getByRole('button', { name: /configure mill curve west/i })).toBeInTheDocument();
    });

    describe('Done to Discard (E1a-T1)', () => {
      let composeIssues: unknown[] = [];
      const ipc = signalIpc(() => composeIssues);

      beforeEach(() => {
        composeIssues = [];
        configChangesStore.clearAllDrafts();
      });

      it('adds the facility card, channel row and configuration edits on Done, and Discard removes all three', async () => {
        const editor = await openEditor('Cedar Hill — Eastward', undefined, ipc);
        render(SaveControls, { props: { toolbar: true } });
        expect(screen.queryByRole('group', { name: 'Configuration save controls' })).not.toBeInTheDocument();

        await wireFirstSignal(editor);
        const done = within(editor).getByRole('button', { name: 'Done' });
        await waitFor(() => expect(done).toBeEnabled());
        await fireEvent.click(done);

        await waitFor(() => expect(screen.queryByRole('dialog', { name: /new automatic block signals/i })).not.toBeInTheDocument());

        // the facility card lists the signal read-only
        const card = await screen.findByTestId('facility-card');
        expect(card).toHaveTextContent('Cedar Hill — Eastward');
        expect(within(card).getByText('Cedar Hill East')).toBeInTheDocument();
        expect(within(card).queryByTestId('add-channel-button')).not.toBeInTheDocument();

        // the channel appears in its own Signal LCC group, used by the facility
        const row = within(screen.getByRole('table')).getByText('Cedar Hill East').closest('tr')!;
        expect(within(row).getByText('Signal')).toBeInTheDocument();
        expect(row).toHaveTextContent('Cedar Hill — Eastward / signals');
        expect(within(screen.getByRole('table')).getByText(/Signal LCC/)).toBeInTheDocument();

        // the Save toolbar lights up and the breakdown counts all three kinds of edit
        const toolbar = await screen.findByRole('group', { name: 'Configuration save controls' });
        expect(within(toolbar).getByRole('button', { name: 'Discard pending changes' })).toBeEnabled();
        expect(effectiveNodeStore.isDirty).toBe(true);
        expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ facilities: 1, channels: 1, config: 1, configNodes: 1 });

        // Discard clears the card, the channel row and the configuration edits together
        await fireEvent.click(within(toolbar).getByRole('button', { name: 'Discard pending changes' }));
        await fireEvent.click(await screen.findByRole('button', { name: 'Revert' }));

        await waitFor(() => expect(screen.queryByTestId('facility-card')).not.toBeInTheDocument());
        expect(within(screen.getByRole('table')).queryByText('Cedar Hill East')).not.toBeInTheDocument();
        expect(effectiveNodeStore.isDirty).toBe(false);
        expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ facilities: 0, channels: 0, config: 0 });
        expect(screen.queryByRole('group', { name: 'Configuration save controls' })).not.toBeInTheDocument();
      });

      it('shows a compose issue on Wiring and keeps Done disabled', async () => {
        composeIssues = [{ code: 'outputInUse', signalId: null, nodeKey: SIGNAL_NODE, detail: 'H1-R is used' }];
        const editor = await openEditor('Cedar Hill — Eastward', undefined, ipc);

        await wireFirstSignal(editor);

        expect(await within(editor).findByText('A chosen pin is already in use by another signal. Choose a free pin.')).toBeInTheDocument();
        expect(within(editor).getByRole('button', { name: 'Done' })).toBeDisabled();
        expect(facilitiesStore.isEmpty).toBe(true);
      });

      it('names the Logic on Wiring when the board has Logic Bowties cannot read, keeps Done disabled and stages nothing', async () => {
        composeIssues = [{
          code: 'logicUndefinedFunction',
          signalId: null,
          nodeKey: SIGNAL_NODE,
          detail: 'Logic 3 has a Function value the CDI does not define.',
          logic: [3],
        }];
        const editor = await openEditor('Cedar Hill — Eastward', undefined, ipc);

        await wireFirstSignal(editor);

        expect(await within(editor).findByText(
          'Logic 3 has a Function value this board does not define. Fix it on the board, or release it, before Bowties places signals here.',
        )).toBeInTheDocument();
        expect(within(editor).getByRole('button', { name: 'Done' })).toBeDisabled();
        expect(facilitiesStore.isEmpty).toBe(true);
        expect(facilitiesStore.collectDeltas()).toEqual([]);
        expect(configChangesStore.draftEntries()).toEqual([]);
      });

      it('stages each signal resources and the board placement on Done, and Discard removes both', async () => {
        const editor = await openEditor('Cedar Hill — Eastward', undefined, ipc);
        render(SaveControls, { props: { toolbar: true } });

        await wireFirstSignal(editor);
        const done = within(editor).getByRole('button', { name: 'Done' });
        await waitFor(() => expect(done).toBeEnabled());
        await fireEvent.click(done);
        await waitFor(() => expect(screen.queryByRole('dialog', { name: /new automatic block signals/i })).not.toBeInTheDocument());

        const facility = facilitiesStore.facilities[0];
        const signal = facility.blockSignals!.signals[0];
        expect(signal.resources).toEqual({ nodeKey: SIGNAL_NODE, mast: 1, trackCircuits: [] });
        const deltas = facilitiesStore.collectDeltas();
        expect(deltas.find((d) => d.type === 'setLogicPlacement')).toEqual({
          type: 'setLogicPlacement',
          nodeKey: SIGNAL_NODE,
          groups: [{ facilityId: facility.facilityId, signalId: signal.signalId, first: 1, length: 1 }],
        });
        expect(await screen.findByRole('group', { name: 'Configuration save controls' })).toBeInTheDocument();
        expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ facilities: 1 });

        await fireEvent.click(screen.getByRole('button', { name: 'Discard pending changes' }));
        await fireEvent.click(await screen.findByRole('button', { name: 'Revert' }));
        await waitFor(() => expect(facilitiesStore.isEmpty).toBe(true));
        expect(facilitiesStore.collectDeltas()).toEqual([]);
        expect(facilitiesStore.logicPlacement).toEqual([]);
      });
    });

    it('shows missing detection facts and explains Restricting in a help bubble that closes on an outside click', async () => {
      const editor = await openEditor();
      await addBlock('River bridge');
      expect(await within(editor).findByText('River bridge is missing detected occupied and clear facts.')).toBeInTheDocument();

      await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
      await fireEvent.click(await within(editor).findByRole('button', { name: 'About Stop and Restricting' }));
      expect(within(editor).getByRole('note')).toHaveTextContent(/Restricting is its own aspect/);
      await fireEvent.pointerDown(within(editor).getByText('All signals'));
      expect(within(editor).queryByRole('note')).not.toBeInTheDocument();
    });

    it('cancels without a pending preview reopening the editor or creating a facility', async () => {
      const editor = await openEditor();
      let release!: (value: BlockSignalsPreview) => void;
      invoke.mockImplementation((command: string, args?: { request: BlockSignalsPreviewRequest }) => {
        if (command === 'preview_block_signals') {
          return new Promise((resolve) => { release = () => resolve(fakePreview(args!.request)); });
        }
        if (command === 'sync_layout_drafts') return Promise.resolve(undefined);
        throw new Error(`unexpected IPC command: ${command}`);
      });
      await addBlock('Cedar Hill');
      await waitFor(() => expect(release).toBeDefined());
      await fireEvent.click(within(editor).getByRole('button', { name: 'Cancel' }));
      release(undefined as never);

      await waitFor(() => {
        expect(screen.queryByRole('dialog', { name: /new automatic block signals/i })).not.toBeInTheDocument();
      });
      expect(facilitiesStore.isEmpty).toBe(true);
    });
  });
});
