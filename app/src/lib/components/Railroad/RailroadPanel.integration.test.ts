import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import RailroadPanel from './RailroadPanel.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import type { InformationChannel } from '$lib/api/channels';
import type { BlockSignalsPreview, BlockSignalsPreviewRequest, ListedBlock, TravelDirection } from '$lib/api/signaling';
import { cancelBlockSignalsEditor } from '$lib/orchestration/blockSignalsEditorOrchestrator';
import { eventStateStore } from '$lib/stores/eventState.svelte';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

function bodChannel(input: number): InformationChannel {
  return {
    id: `ch-bod-${input}`,
    name: `TowerLCC-1 BOD A${input}`,
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: {
      kind: 'connectorInput',
      nodeKey: '05010101FF000001',
      connector: 'connector-a',
      input,
    },
  };
}

async function startBlockSignals(name = 'New Automatic Block Signals') {
  await fireEvent.click(screen.getByRole('radio', { name: /^Automatic Block Signals/ }));
  await fireEvent.input(screen.getByLabelText('Name'), { target: { value: name } });
  await fireEvent.click(screen.getByRole('button', { name: 'Add facility' }));
}

describe('RailroadPanel (Spec 018 / S3 — hardware-organised Channels panel)', () => {
  const stubNodeName = (key: string) =>
    key === '05010101FF000001' ? 'TowerLCC-1' : `Node(${key})`;
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

    const blocks = [
      { ...bodChannel(1), id: 'block:cedar', name: 'Cedar Hill' },
      { ...bodChannel(2), id: 'block:mill', name: 'Mill curve' },
      { ...bodChannel(3), id: 'block:river', name: 'River bridge' },
    ];

    /** A small stand-in for the backend: one signal per signal block, in list order. */
    function fakePreview(request: BlockSignalsPreviewRequest): BlockSignalsPreview {
      const groups: ListedBlock[][] = [];
      request.blocks.forEach((block, index) => {
        if (index > 0 && block.joinedToPrevious) groups[groups.length - 1].push(block);
        else groups.push([block]);
      });
      const chains: Array<{ run: 'withList' | 'againstList'; governs: TravelDirection; groups: ListedBlock[][] }> = [
        { run: 'withList', governs: request.direction, groups },
      ];
      if (request.movements === 'bothDirections') {
        chains.push({
          run: 'againstList',
          governs: request.direction === 'eastward' ? 'westward' : 'eastward',
          groups: [...groups].reverse().map((group) => [...group].reverse()),
        });
      }
      const signals = chains.flatMap(({ run, governs, groups: chain }) => chain.map((group, index) => {
        const signalId = `signal:${request.facilityId}:${group[0].blockId}:${governs}`;
        const last = index === chain.length - 1;
        const turnoutIds = group.flatMap((block) => block.turnouts.map((t) => t.turnoutId));
        const treatAsStop = last && run === 'withList' && request.exitEnd.kind === 'treatAsStop';
        return {
          signalId,
          name: request.signalNames[signalId] ?? `${group[0].name} ${governs === 'eastward' ? 'East' : 'West'}`,
          governs,
          run,
          entranceBlockId: group[0].blockId,
          protectedBlockIds: group.map((block) => block.blockId),
          turnoutIds,
          settings: { aspects: request.settings.aspects, approachLighting: request.settings.approachLighting },
          overridden: false,
          next: treatAsStop
            ? { kind: 'treatedAsStop' as const }
            : last ? { kind: 'nothingBeyond' as const } : { kind: 'signal' as const, signalId: 'next' },
          rules: [
            { condition: { kind: 'blockOccupied' as const }, indication: 'stop' as const },
            ...turnoutIds.map((turnoutId) => ({
              condition: { kind: 'turnoutNotLined' as const, turnoutId },
              indication: request.settings.turnoutNotLined === 'restricting' ? 'restricting' as const : 'stop' as const,
            })),
            treatAsStop
              ? { condition: { kind: 'clearNothingBeyondTreatedAsStop' as const }, indication: 'approach' as const }
              : { condition: { kind: 'blockClear' as const }, indication: 'clear' as const },
          ],
          observation: 'unavailable' as const,
        };
      }));
      return {
        facilityName: request.facilityName,
        direction: request.direction,
        movements: request.movements,
        blocks: request.blocks,
        entranceEnd: request.entranceEnd,
        exitEnd: request.exitEnd,
        signals,
        capability: { kind: 'incompletePreview', stage: 'unavailable', save: 'unavailable', apply: 'unavailable' },
        issues: request.blocks[0]?.blockId === 'block:river'
          ? [{ kind: 'missingDetectionFacts', blockId: 'block:river', missingStates: ['occupied', 'clear'] }]
          : [],
      };
    }

    function lastRequest(): BlockSignalsPreviewRequest {
      const calls = invoke.mock.calls.filter(([command]) => command === 'preview_block_signals');
      return (calls[calls.length - 1][1] as { request: BlockSignalsPreviewRequest }).request;
    }

    async function addBlock(name: string) {
      await fireEvent.click(screen.getByRole('button', { name: 'Add block…' }));
      const picker = await screen.findByRole('dialog', { name: /add block/i });
      await fireEvent.click(within(picker).getByRole('radio', { name }));
      await fireEvent.click(within(picker).getByRole('button', { name: 'Confirm' }));
      await waitFor(() => {
        expect(screen.queryByRole('dialog', { name: /add block/i })).not.toBeInTheDocument();
      });
    }

    async function openEditor(
      name = 'Cedar Hill — Eastward',
      resolvedEventIds?: ReadonlyMap<string, Record<string, string>>,
    ) {
      channelsStore.hydrateBaseline(blocks);
      invoke.mockImplementation((command: string, args?: { request: BlockSignalsPreviewRequest }) => {
        if (command === 'list_behavior_templates') {
          return Promise.resolve([{ templateId: 'block-indicator', displayName: 'Block Indicator', slots: [], mapping: [] }]);
        }
        if (command === 'preview_block_signals') return Promise.resolve(fakePreview(args!.request));
        throw new Error(`unexpected IPC command: ${command}`);
      });
      await behaviorTemplatesStore.loadBehaviorTemplates();
      render(RailroadPanel, { props: { nodeName: stubNodeName, resolvedEventIds } });
      await fireEvent.click(screen.getByTestId('add-facility-button'));
      await startBlockSignals(name);
      return screen.getByRole('dialog', { name: /new automatic block signals/i });
    }

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
      invoke.mockResolvedValue([{ templateId: 'block-indicator', displayName: 'Block Indicator', slots: [], mapping: [] }]);
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

    it('builds a travel-order list with a joined block and a turnout, then shows its rules on Aspects', async () => {
      const editor = await openEditor();
      expect(within(editor).getByLabelText('Name')).toHaveValue('Cedar Hill — Eastward');

      await addBlock('Cedar Hill');
      await addBlock('Mill curve');
      await addBlock('River bridge');
      // joining is a less common choice, so it lives in the row's "More" menu
      expect(within(editor).queryByRole('button', { name: 'More for Cedar Hill' })).not.toBeInTheDocument();
      await fireEvent.click(within(editor).getByRole('button', { name: 'More for Mill curve' }));
      await fireEvent.click(within(editor).getByRole('menuitem', { name: /remove the signal between cedar hill and mill curve/i }));
      expect(within(editor).getByText('No signal between it and Cedar Hill')).toBeInTheDocument();
      expect(within(editor).queryByRole('menu')).not.toBeInTheDocument();
      await fireEvent.click(within(within(editor).getByRole('table')).getByText('Mill curve'));
      await fireEvent.click(within(editor).getByRole('button', { name: 'Add turnout in block' }));
      const turnoutName = await within(editor).findByLabelText('Turnout name');
      await fireEvent.change(turnoutName, { target: { value: 'Mill siding' } });

      await waitFor(() => expect(lastRequest().blocks).toEqual([
        { blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] },
        { blockId: 'block:mill', name: 'Mill curve', joinedToPrevious: true, turnouts: [{ turnoutId: expect.stringMatching(/^turnout:/), name: 'Mill siding' }] },
        { blockId: 'block:river', name: 'River bridge', joinedToPrevious: false, turnouts: [] },
      ]));
      expect(within(editor).getByRole('button', { name: /signals between cedar hill and mill curve are off/i })).toBeInTheDocument();

      await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
      const first = await within(editor).findByRole('button', { name: /configure cedar hill east; protects cedar hill, mill curve/i });
      expect(first).toHaveAttribute('aria-pressed', 'true');
      expect(within(editor).getByLabelText('Selected signal name')).toHaveValue('Cedar Hill East');
      expect(within(editor).getByRole('table', { name: 'Rules for Cedar Hill East' })).toHaveTextContent(
        'A protected block occupiedStopMill siding not lined for the main, or position unknownStopProtected blocks clearClear',
      );

      await fireEvent.click(within(editor).getByRole('radio', { name: 'Restricting' }));
      await waitFor(() => expect(within(editor).getByRole('table', { name: 'Rules for Cedar Hill East' }))
        .toHaveTextContent('Mill siding not lined for the main, or position unknownRestricting'));

      expect(within(editor).getByRole('button', { name: 'Next: Physical signals' })).toBeDisabled();
      expect(within(editor).getByRole('button', { name: /physical signals: available in a later release/i })).toBeDisabled();
      expect(within(editor).queryByRole('button', { name: /save|apply|confirm/i })).not.toBeInTheDocument();
      expect(facilitiesStore.isEmpty).toBe(true);
    });

    it('sets what lies beyond the last block from its end chip, with Plants not yet available', async () => {
      const editor = await openEditor();
      await addBlock('Cedar Hill');
      await addBlock('Mill curve');

      await fireEvent.click(within(editor).getByRole('button', { name: /nothing connected beyond mill curve/i }));
      const menu = within(editor).getByRole('dialog', { name: 'What lies beyond Mill curve' });
      expect(within(menu).getByRole('radio', { name: /^Nothing beyond The last signal/ })).toBeChecked();
      expect(within(menu).getByRole('button', { name: 'Choose a Plant…' })).toBeDisabled();
      await fireEvent.click(within(menu).getByRole('radio', { name: /treat as stop/i }));

      await waitFor(() => expect(lastRequest().exitEnd).toEqual({ kind: 'treatAsStop' }));
      expect(within(editor).getByRole('button', { name: /beyond mill curve: end: stop/i })).toBeInTheDocument();

      await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
      await fireEvent.click(await within(editor).findByRole('button', { name: /configure mill curve east/i }));
      expect(within(editor).getByRole('table', { name: 'Rules for Mill curve East' }))
        .toHaveTextContent('Protected block clear; nothing beyond, treated as StopApproach');
      expect(within(editor).getByText(/shows Approach at best/)).toBeInTheDocument();
    });

    it('draws both directions and keeps the list order when the direction changes', async () => {
      const editor = await openEditor();
      await addBlock('Cedar Hill');
      await addBlock('Mill curve');

      await fireEvent.click(within(editor).getByRole('radio', { name: /both directions/i }));
      expect(within(editor).getByText('List order')).toBeInTheDocument();
      expect(within(editor).getByRole('radio', { name: /coordinate traffic through the single-track section/i })).toBeDisabled();
      await fireEvent.click(within(editor).getByRole('radio', { name: 'Westward' }));

      await waitFor(() => expect(lastRequest()).toMatchObject({ movements: 'bothDirections', direction: 'westward' }));
      expect(lastRequest().blocks.map((block) => block.blockId)).toEqual(['block:cedar', 'block:mill']);
      expect(within(editor).getByText('Track, in westward order')).toBeInTheDocument();

      await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
      expect(await within(editor).findByRole('button', { name: /configure cedar hill west/i })).toBeInTheDocument();
      expect(within(editor).getByRole('button', { name: /configure mill curve east/i })).toBeInTheDocument();
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
        throw new Error(`unexpected IPC command: ${command}`);
      });
      await addBlock('Cedar Hill');
      await fireEvent.click(within(editor).getByRole('button', { name: 'Cancel' }));
      release(undefined as never);

      await waitFor(() => {
        expect(screen.queryByRole('dialog', { name: /new automatic block signals/i })).not.toBeInTheDocument();
      });
      expect(facilitiesStore.isEmpty).toBe(true);
    });
  });
});
