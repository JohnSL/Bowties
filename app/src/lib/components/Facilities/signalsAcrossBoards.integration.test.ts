/**
 * Spec 020 / E5-T1 - Signals across two boards (integration).
 *
 * A two-block Automatic Block Signals facility whose signals sit on two Signal
 * LCC boards is staged through `facilityOrchestrator`, saved through the
 * Pending Changes pipeline (rows carry `origin.facilityId`), and then one board
 * fails to write. Only the IPC boundary is mocked; the stores, orchestrators and
 * `FacilitiesSection` / `FacilityCard` / `BlockSignalsRemoveDialog` are real.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { PendingChangeRow } from '$lib/api/sync';

const listBehaviorTemplatesMock = vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []);
vi.mock('$lib/api/behaviorTemplates', () => ({ listBehaviorTemplates: listBehaviorTemplatesMock }));
vi.mock('$lib/api/facilities', () => ({ listFacilities: async () => [] as Facility[] }));
vi.mock('$lib/api/channels', () => ({ listChannels: async () => [] }));
vi.mock('$lib/api/facilityBowties', () => ({ composeFacilityBowties: vi.fn(async () => []) }));
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: vi.fn(async () => undefined),
  clearLayoutDrafts: vi.fn(async () => undefined),
}));
vi.mock('$lib/api/signaling', () => ({
  composeBlockSignals: vi.fn(),
  composeBlockSignalsRemoval: vi.fn(),
}));
vi.mock('$lib/api/sync', () => ({
  listPendingChanges: vi.fn(async () => []),
  revertPendingChange: vi.fn(),
  setPendingChange: vi.fn(async () => 'row-id'),
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
const { pendingChangesStore } = await import('$lib/stores/pendingChanges.svelte');
const { composeBlockSignals } = await import('$lib/api/signaling');
const { listPendingChanges, setPendingChange } = await import('$lib/api/sync');
const orch = await import('$lib/orchestration/facilityOrchestrator');
const { stageConfigDraftsAsPendingChanges } = await import('$lib/orchestration/configDraftOrchestrator');
const FacilitiesSection = (await import('./FacilitiesSection.svelte')).default;

const ABS_TEMPLATE: BehaviorTemplate = {
  templateId: 'automatic-block-signals',
  displayName: 'Automatic Block Signals',
  slots: [
    { label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 0, maxChannels: null },
  ],
  mapping: [],
};

const WEST = '050101018800';
const EAST = '050101018801';
const WEST_DISPLAY = '05.01.01.01.88.00';
const EAST_DISPLAY = '05.01.01.01.88.01';

const COMPOSED = {
  edits: [
    { nodeKey: WEST, space: 253, address: 10, path: 'a', value: { type: 'int', value: 1 } },
    { nodeKey: WEST, space: 253, address: 20, path: 'b', value: { type: 'int', value: 2 } },
    { nodeKey: EAST, space: 253, address: 30, path: 'c', value: { type: 'int', value: 3 } },
  ],
  signalResources: [
    { signalId: 's1', resources: { nodeKey: WEST, mast: 1, trackCircuits: [1] } },
    { signalId: 's2', resources: { nodeKey: EAST, mast: 2, trackCircuits: [] } },
  ],
  logicPlacement: [
    { nodeKey: WEST, groups: [{ facilityId: 'abs-1', signalId: 's1', first: 1, length: 2 }] },
    { nodeKey: EAST, groups: [{ facilityId: 'abs-1', signalId: 's2', first: 4, length: 1 }] },
  ],
  issues: [],
};

async function stageTwoBoardFacility() {
  const result = await orch.createBlockSignalsFacility({
    facilityId: 'abs-1',
    name: 'Cedar Hill',
    line: { facilityId: 'abs-1', facilityName: 'Cedar Hill' } as never,
    wiring: [
      { signalId: 's1', nodeKey: WEST, redOutput: 1, greenOutput: 2 },
      { signalId: 's2', nodeKey: EAST, redOutput: 1, greenOutput: 2 },
    ],
    signals: [
      { signalId: 's1', name: 'Signal West', protectedBlock: 'b1' },
      { signalId: 's2', name: 'Signal East', protectedBlock: 'b2' },
    ],
  });
  expect(result).toEqual({ ok: true });
}

/** Save as the app does, and answer the reload with each row's result by board. */
async function saveWithEastFailing(): Promise<PendingChangeRow[]> {
  stageConfigDraftsAsPendingChanges();
  const calls: Array<Parameters<typeof setPendingChange>[0]> = [];
  vi.mocked(setPendingChange).mockImplementation(async (row) => {
    calls.push(row);
    return `row-${calls.length}`;
  });
  vi.mocked(listPendingChanges).mockImplementation(async () =>
    calls.map((row, i) => {
      const onEast = row.nodeId?.replace(/\./g, '').toUpperCase() === EAST;
      return {
        changeId: `row-${i + 1}`,
        ...row,
        status: onEast ? 'failed' : 'applied',
        ...(onEast ? { error: 'No reply from the board' } : {}),
      } as PendingChangeRow;
    }),
  );
  await pendingChangesStore.recordDraftsToBackend();
  return [...pendingChangesStore.persistedRows];
}

function expectWaitingOnEastOnly() {
  const card = screen.getByTestId('facility-card');
  expect(within(card).getByText(/^Waiting$/)).toBeInTheDocument();
  expect(within(card).queryByText(/^Wired$/)).toBeNull();
  const lines = within(screen.getByTestId('facility-write-outcome')).getAllByRole('listitem');
  expect(lines).toHaveLength(1);
  expect(lines[0]).toHaveTextContent(`1 change waiting on ${EAST_DISPLAY}: No reply from the board`);
  expect(screen.queryByText(new RegExp(`waiting on ${WEST_DISPLAY}`))).toBeNull();
  expect(screen.queryByText(new RegExp(`not yet written to ${WEST_DISPLAY}`))).toBeNull();
}

beforeEach(async () => {
  channelsStore.reset();
  facilitiesStore.reset();
  behaviorTemplatesStore.reset();
  configChangesStore.clearAllDrafts();
  pendingChangesStore.clear();
  vi.mocked(composeBlockSignals).mockReset();
  vi.mocked(composeBlockSignals).mockResolvedValue(COMPOSED as never);
  listBehaviorTemplatesMock.mockResolvedValue([ABS_TEMPLATE]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
});

describe('Signals across two boards (E5-T1)', () => {
  it('shows Waiting and names only the failing board after a save, and again after the layout reopens', async () => {
    await stageTwoBoardFacility();
    const rows = await saveWithEastFailing();

    expect(rows.map((r) => r.origin)).toEqual(rows.map(() => ({ facilityId: 'abs-1' })));
    expect(rows).toHaveLength(3);

    render(FacilitiesSection, { props: {} });
    expectWaitingOnEastOnly();

    // Layout reopened: the persisted rows are loaded as they were saved.
    pendingChangesStore.clear();
    pendingChangesStore.setRows(rows);
    await Promise.resolve();
    expectWaitingOnEastOnly();
  });

  it('names both boards in the Remove dialog', async () => {
    await stageTwoBoardFacility();

    render(FacilitiesSection, { props: {} });
    await fireEvent.click(screen.getByRole('button', { name: 'Delete facility' }));

    const dialog = screen.getByRole('alertdialog', { name: /Remove Cedar Hill\?/ });
    expect(within(dialog).getByText(
      new RegExp(`Mast 1, Logic 1, Logic 2 and Track Circuit 1 on ${WEST_DISPLAY}, and Mast 2 and Logic 4 on ${EAST_DISPLAY}`),
    )).toBeInTheDocument();
  });
});
