// S1 integration test for facility CRUD across the draft layer + save round-trip.
// Mirrors `channels.svelte.test.ts` style: mocked IPC + flushSync-free Svelte 5 runes.
// Covers spec 018 S1 acceptance criteria AC1, AC4, AC5 and SC-002 at the store seam.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { FacilitiesListing, Facility, NodeLogicPlacement, PlacedLogicGroup } from '$lib/api/facilities';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { LayoutEditDelta } from '$lib/types/bowtie';

const EMPTY_LISTING: FacilitiesListing = { facilities: [], logicPlacement: [] };
const listing = (facilities: Facility[], logicPlacement: NodeLogicPlacement[] = []): FacilitiesListing => ({ facilities, logicPlacement });
const listFacilitiesMock = vi.fn<() => Promise<FacilitiesListing>>(async () => EMPTY_LISTING);
const listBehaviorTemplatesMock = vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []);

vi.mock('$lib/api/facilities', () => ({
  listFacilities: listFacilitiesMock,
}));
vi.mock('$lib/api/behaviorTemplates', () => ({
  listBehaviorTemplates: listBehaviorTemplatesMock,
}));

// Must import after mocks are set up.
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');

const BLOCK_INDICATOR: BehaviorTemplate = {
  templateId: 'block-indicator',
  displayName: 'Block Indicator',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'output', displayLabel: 'indicator', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
  ],
  mapping: [
    { producerState: 'occupied', consumerCommand: 'lit' },
    { producerState: 'clear', consumerCommand: 'unlit' },
  ],
};

beforeEach(() => {
  facilitiesStore.reset();
  behaviorTemplatesStore.reset();
  listFacilitiesMock.mockReset();
  listFacilitiesMock.mockResolvedValue(EMPTY_LISTING);
  listBehaviorTemplatesMock.mockReset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR]);
});

describe('S1: Facility CRUD with empty slots — end-to-end round-trip', () => {
  it('add → save → reopen → rename → save → reopen → delete → save → reopen', async () => {
    // ── Initial load: templates available, no facilities yet ────────────────
    await behaviorTemplatesStore.loadBehaviorTemplates();
    await facilitiesStore.loadFacilities();

    expect(behaviorTemplatesStore.templates).toEqual([BLOCK_INDICATOR]);
    expect(facilitiesStore.facilities).toEqual([]);
    expect(facilitiesStore.isDirty).toBe(false);

    // ── User adds a facility "Block 5" via the Block Indicator template ─────
    const added = facilitiesStore.addFacility(BLOCK_INDICATOR, 'Block 5');

    expect(facilitiesStore.facilities).toHaveLength(1);
    expect(facilitiesStore.facilities[0]).toEqual({
      facilityId: added.facilityId,
      templateId: 'block-indicator',
      name: 'Block 5',
      slotBindings: { input: [], output: [] },
    });
    expect(facilitiesStore.isDirty).toBe(true);
    expect(facilitiesStore.editCount).toBe(1);

    // ── Save: deltas reflect the addition; hydrate the new baseline ─────────
    const addDeltas: LayoutEditDelta[] = facilitiesStore.collectDeltas();
    expect(addDeltas).toEqual([
      { type: 'addFacility', facility: facilitiesStore.facilities[0] },
    ]);
    facilitiesStore.hydrateBaseline([facilitiesStore.facilities[0]]);
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.collectDeltas()).toEqual([]);

    // ── Close + reopen: reset, then reload from the persisted backend ───────
    const persistedAfterAdd: Facility = {
      facilityId: added.facilityId,
      templateId: 'block-indicator',
      name: 'Block 5',
      slotBindings: { input: [], output: [] },
    };
    facilitiesStore.reset();
    expect(facilitiesStore.facilities).toEqual([]);

    listFacilitiesMock.mockResolvedValue(listing([persistedAfterAdd]));
    await facilitiesStore.loadFacilities();
    expect(facilitiesStore.facilities).toEqual([persistedAfterAdd]); // SC-002: round-trip exact
    expect(facilitiesStore.isDirty).toBe(false);

    // ── User renames the facility to "Block 7" ──────────────────────────────
    const renamed = facilitiesStore.renameFacility(added.facilityId, 'Block 7');
    expect(renamed).toBe(true);
    expect(facilitiesStore.facilities[0].name).toBe('Block 7');
    expect(facilitiesStore.isDirty).toBe(true);

    const renameDeltas = facilitiesStore.collectDeltas();
    expect(renameDeltas).toEqual([
      { type: 'renameFacility', facilityId: added.facilityId, newName: 'Block 7' },
    ]);

    // Save and reopen ─ persistence carries the new name.
    facilitiesStore.hydrateBaseline([{ ...persistedAfterAdd, name: 'Block 7' }]);
    facilitiesStore.reset();
    listFacilitiesMock.mockResolvedValue(listing([{ ...persistedAfterAdd, name: 'Block 7' }]));
    await facilitiesStore.loadFacilities();
    expect(facilitiesStore.facilities[0].name).toBe('Block 7');
    expect(facilitiesStore.isDirty).toBe(false);

    // ── User deletes the facility ───────────────────────────────────────────
    facilitiesStore.deleteFacility(added.facilityId);
    expect(facilitiesStore.facilities).toEqual([]);
    expect(facilitiesStore.isDirty).toBe(true);

    const deleteDeltas = facilitiesStore.collectDeltas();
    expect(deleteDeltas).toEqual([
      { type: 'deleteFacility', facilityId: added.facilityId },
    ]);

    // Save and reopen ─ facility is gone.
    facilitiesStore.hydrateBaseline([]);
    facilitiesStore.reset();
    listFacilitiesMock.mockResolvedValue(EMPTY_LISTING);
    await facilitiesStore.loadFacilities();
    expect(facilitiesStore.facilities).toEqual([]);
    expect(facilitiesStore.isDirty).toBe(false);
  });
});

// ── Spec 018 / S4 — Slot Binding (attach / detach) ─────────────────────────

describe('S4: attachChannel / detachChannel on a pending-creation facility', () => {
  it('attach appends to slot Vec; collectDeltas folds bindings into addFacility', () => {
    const f = facilitiesStore.addFacility(BLOCK_INDICATOR, 'Block 5');
    expect(facilitiesStore.attachChannel(f.facilityId, 'input', 'ch-1')).toBe(true);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-1']);

    // Pending-creation: bindings travel inside the addFacility delta; no
    // separate attachChannelToSlot delta is emitted.
    const deltas = facilitiesStore.collectDeltas();
    expect(deltas).toHaveLength(1);
    expect(deltas[0]).toMatchObject({
      type: 'addFacility',
      facility: { facilityId: f.facilityId, slotBindings: { input: ['ch-1'], output: [] } },
    });
  });

  it('attach the same channel twice is a no-op', () => {
    const f = facilitiesStore.addFacility(BLOCK_INDICATOR, 'Block 5');
    expect(facilitiesStore.attachChannel(f.facilityId, 'input', 'ch-1')).toBe(true);
    expect(facilitiesStore.attachChannel(f.facilityId, 'input', 'ch-1')).toBe(false);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-1']);
  });

  it('detach removes the channel; absent channel detach is a no-op', () => {
    const f = facilitiesStore.addFacility(BLOCK_INDICATOR, 'Block 5');
    facilitiesStore.attachChannel(f.facilityId, 'input', 'ch-1');
    expect(facilitiesStore.detachChannel(f.facilityId, 'input', 'ch-1')).toBe(true);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    expect(facilitiesStore.detachChannel(f.facilityId, 'input', 'ch-1')).toBe(false);
  });
});

describe('S4: attachChannel / detachChannel on a baseline facility', () => {
  const baseline = (id: string): Facility => ({
    facilityId: id,
    templateId: 'block-indicator',
    name: 'Block 5',
    slotBindings: { input: [], output: [] },
  });

  it('attach emits an attachChannelToSlot delta; isDirty + editCount flip', () => {
    facilitiesStore.hydrateBaseline([baseline('f-1')]);
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.attachChannel('f-1', 'input', 'ch-1')).toBe(true);
    expect(facilitiesStore.isDirty).toBe(true);
    expect(facilitiesStore.editCount).toBe(1);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-1']);
    expect(facilitiesStore.collectDeltas()).toEqual([
      { type: 'attachChannelToSlot', facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-1' },
    ]);
  });

  it('detach emits a detachChannelToSlot delta', () => {
    facilitiesStore.hydrateBaseline([
      { ...baseline('f-1'), slotBindings: { input: ['ch-1'], output: [] } },
    ]);
    expect(facilitiesStore.detachChannel('f-1', 'input', 'ch-1')).toBe(true);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    expect(facilitiesStore.collectDeltas()).toEqual([
      { type: 'detachChannelFromSlot', facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-1' },
    ]);
  });

  it('attach-then-detach that returns to the baseline is a no-op (collapses to empty deltas)', () => {
    facilitiesStore.hydrateBaseline([baseline('f-1')]);
    facilitiesStore.attachChannel('f-1', 'input', 'ch-1');
    facilitiesStore.detachChannel('f-1', 'input', 'ch-1');
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
  });

  it('rebind (detach previous + attach new) emits one detach + one attach delta', () => {
    facilitiesStore.hydrateBaseline([
      { ...baseline('f-1'), slotBindings: { input: ['ch-old'], output: [] } },
    ]);
    facilitiesStore.detachChannel('f-1', 'input', 'ch-old');
    facilitiesStore.attachChannel('f-1', 'input', 'ch-new');
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-new']);
    const deltas = facilitiesStore.collectDeltas();
    expect(deltas).toEqual([
      { type: 'detachChannelFromSlot', facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-old' },
      { type: 'attachChannelToSlot', facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-new' },
    ]);
  });

  it('discard clears slot-binding edits', () => {
    facilitiesStore.hydrateBaseline([baseline('f-1')]);
    facilitiesStore.attachChannel('f-1', 'input', 'ch-1');
    facilitiesStore.discard();
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    expect(facilitiesStore.isDirty).toBe(false);
  });

  it('hydrateBaseline clears slot-binding edits', () => {
    facilitiesStore.hydrateBaseline([baseline('f-1')]);
    facilitiesStore.attachChannel('f-1', 'input', 'ch-1');
    facilitiesStore.hydrateBaseline([
      { ...baseline('f-1'), slotBindings: { input: ['ch-1'], output: [] } },
    ]);
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
  });
});

// ── Spec 020 / E1a: block-signals record ────────────────────────────────────

import type { BlockSignalsRecord } from '$lib/api/facilities';

const SIGNALS_TEMPLATE: BehaviorTemplate = {
  templateId: 'automatic-block-signals',
  displayName: 'Automatic Block Signals',
  slots: [
    { label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 0, maxChannels: 64 },
  ],
  mapping: [],
};

const BOARD = '020157000001';

function record(overrides: Partial<BlockSignalsRecord> = {}): BlockSignalsRecord {
  return {
    direction: 'forward',
    movements: 'oneWay',
    entranceEnd: 'a',
    exitEnd: 'b',
    blocks: [],
    settings: {},
    signals: [{
      signalId: 's-1', name: 'Mast 1', protectedBlock: 'b-1', channelId: 'ch-sig',
      resources: { nodeKey: BOARD, mast: 1, trackCircuits: [] },
    }],
    ...overrides,
  } as unknown as BlockSignalsRecord;
}

describe('Spec 020 / E1a: facility block-signals record', () => {
  it('addFacility keeps a caller-supplied id and emits the record inside one addFacility delta with its slot binding', () => {
    const rec = record();
    const added = facilitiesStore.addFacility(SIGNALS_TEMPLATE, 'ABS 1', 'fac-fixed', rec);
    facilitiesStore.attachChannel('fac-fixed', 'signals', 'ch-sig');

    expect(added.facilityId).toBe('fac-fixed');
    expect(facilitiesStore.facilities[0].blockSignals).toEqual(rec);
    const deltas = facilitiesStore.collectDeltas();
    expect(deltas).toHaveLength(1);
    expect(deltas[0]).toMatchObject({
      type: 'addFacility',
      facility: { facilityId: 'fac-fixed', blockSignals: rec, slotBindings: { signals: ['ch-sig'] } },
    });
  });

  it('setBlockSignalsRecord on a saved facility replaces wholesale, suppresses no-ops and drops the entry when reverted', () => {
    const saved = record();
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'automatic-block-signals', name: 'ABS 1', slotBindings: { signals: ['ch-sig'] }, blockSignals: saved },
    ]);

    expect(facilitiesStore.setBlockSignalsRecord('f-1', record())).toBe(false);
    expect(facilitiesStore.isDirty).toBe(false);

    const changed = record({ settings: { changed: true } } as unknown as Partial<BlockSignalsRecord>);
    expect(facilitiesStore.setBlockSignalsRecord('f-1', changed)).toBe(true);
    expect(facilitiesStore.facilities[0].blockSignals).toEqual(changed);
    expect(facilitiesStore.editCount).toBe(1);
    expect(facilitiesStore.collectDeltas()).toEqual([
      { type: 'setBlockSignalsRecord', facilityId: 'f-1', record: changed },
    ]);

    expect(facilitiesStore.setBlockSignalsRecord('f-1', changed)).toBe(false);
    expect(facilitiesStore.setBlockSignalsRecord('f-1', saved)).toBe(true);
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
  });
});

describe('Spec 020 / E3a: board Logic placement', () => {
  const group = (facilityId: string, signalId: string, first: number, length = 1): PlacedLogicGroup => ({
    facilityId, signalId, first, length,
  });
  const absFacility = (facilityId: string): Facility => ({
    facilityId, templateId: 'automatic-block-signals', name: facilityId, slotBindings: { signals: [] }, blockSignals: record(),
  });
  const saved: NodeLogicPlacement[] = [{ nodeKey: BOARD, groups: [group('f-1', 's-1', 1), group('f-2', 's-2', 2)] }];

  it('loadFacilities hydrates the placement from the same read as the facilities, clean', async () => {
    listFacilitiesMock.mockResolvedValue(listing([absFacility('f-1'), absFacility('f-2')], saved));
    await facilitiesStore.loadFacilities();

    expect(facilitiesStore.logicPlacement).toEqual(saved);
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
  });

  it('setLogicPlacement replaces a board wholesale, suppresses no-ops, emits one delta and discards back to the saved placement', () => {
    facilitiesStore.hydrateBaseline([absFacility('f-1'), absFacility('f-2')], saved);

    expect(facilitiesStore.setLogicPlacement(BOARD, saved[0].groups)).toBe(false);
    expect(facilitiesStore.isDirty).toBe(false);

    const moved = [group('f-1', 's-1', 3), group('f-2', 's-2', 2)];
    expect(facilitiesStore.setLogicPlacement(BOARD, moved)).toBe(true);
    expect(facilitiesStore.logicPlacement).toEqual([{ nodeKey: BOARD, groups: moved }]);
    expect(facilitiesStore.isDirty).toBe(true);
    expect(facilitiesStore.collectDeltas()).toEqual([{ type: 'setLogicPlacement', nodeKey: BOARD, groups: moved }]);

    expect(facilitiesStore.setLogicPlacement(BOARD, moved)).toBe(false);
    facilitiesStore.discard();
    expect(facilitiesStore.logicPlacement).toEqual(saved);
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.collectDeltas()).toEqual([]);

    expect(facilitiesStore.setLogicPlacement(BOARD, moved)).toBe(true);
    expect(facilitiesStore.setLogicPlacement(BOARD, saved[0].groups)).toBe(true);
    expect(facilitiesStore.isDirty).toBe(false);
  });

  it('a board with no groups left has no entry, and a new board is added with its groups', () => {
    facilitiesStore.hydrateBaseline([absFacility('f-1')], [{ nodeKey: BOARD, groups: [group('f-1', 's-1', 1)] }]);

    expect(facilitiesStore.setLogicPlacement(BOARD, [])).toBe(true);
    expect(facilitiesStore.logicPlacement).toEqual([]);
    expect(facilitiesStore.collectDeltas()).toEqual([{ type: 'setLogicPlacement', nodeKey: BOARD, groups: [] }]);

    expect(facilitiesStore.setLogicPlacement('020157000002', [group('f-1', 's-1', 4)])).toBe(true);
    expect(facilitiesStore.logicPlacement).toEqual([{ nodeKey: '020157000002', groups: [group('f-1', 's-1', 4)] }]);
  });

  it('deleting a facility prunes its groups from the effective placement, and from staged placement, as the backend does', () => {
    facilitiesStore.hydrateBaseline([absFacility('f-1'), absFacility('f-2')], saved);
    facilitiesStore.deleteFacility('f-1');

    expect(facilitiesStore.logicPlacement).toEqual([{ nodeKey: BOARD, groups: [group('f-2', 's-2', 2)] }]);
    expect(facilitiesStore.collectDeltas()).toEqual([{ type: 'deleteFacility', facilityId: 'f-1' }]);

    facilitiesStore.setLogicPlacement('020157000002', [group('new', 's-9', 1)]);
    facilitiesStore.addFacility(SIGNALS_TEMPLATE, 'New', 'new', record());
    facilitiesStore.deleteFacility('new');
    expect(facilitiesStore.logicPlacement).toEqual([{ nodeKey: BOARD, groups: [group('f-2', 's-2', 2)] }]);
    expect(facilitiesStore.collectDeltas()).toEqual([{ type: 'deleteFacility', facilityId: 'f-1' }]);
  });

  it('counts a placement-only change as one facility edit, and not on top of the facility edit it comes with', () => {
    facilitiesStore.setLogicPlacement(BOARD, [group('f-1', 's-1', 1)]);
    expect(facilitiesStore.editCount).toBe(1);

    facilitiesStore.addFacility(SIGNALS_TEMPLATE, 'ABS', 'f-1', record());
    expect(facilitiesStore.editCount).toBe(1);
  });
});

import type { PanelRecord } from '$lib/api/facilities';

const PANEL_TEMPLATE: BehaviorTemplate = {
  templateId: 'panel',
  displayName: 'Panel',
  slots: [
    { label: 'lamps', displayLabel: 'lamps', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 0, maxChannels: null },
  ],
  mapping: [],
};

function panelRecord(blockId = 'b-1'): PanelRecord {
  return { litWhen: 'occupied', entries: [{ entryId: 'e-1', blockId, lampChannelId: 'ch-1' }] };
}

describe('Spec 021: facility panel record', () => {
  it('folds the record into a pending creation so one addFacility delta carries record and lamp bindings', () => {
    facilitiesStore.addFacility(PANEL_TEMPLATE, 'Panel 1', 'p-1');
    expect(facilitiesStore.setPanelRecord('p-1', panelRecord())).toBe(true);
    facilitiesStore.attachChannel('p-1', 'lamps', 'ch-1');

    expect(facilitiesStore.facilities[0].panel).toEqual(panelRecord());
    const deltas = facilitiesStore.collectDeltas();
    expect(deltas).toHaveLength(1);
    expect(deltas[0]).toMatchObject({
      type: 'addFacility',
      facility: { facilityId: 'p-1', panel: panelRecord(), slotBindings: { lamps: ['ch-1'] } },
    });
  });

  it('replaces a saved record wholesale, suppresses no-ops, and clears on revert, discard and hydrate', () => {
    const saved = panelRecord();
    facilitiesStore.hydrateBaseline([
      { facilityId: 'p-2', templateId: 'panel', name: 'Panel 2', slotBindings: { lamps: ['ch-1'] }, panel: saved },
    ]);
    expect(facilitiesStore.setPanelRecord('p-2', panelRecord())).toBe(false);
    expect(facilitiesStore.isDirty).toBe(false);

    const changed = panelRecord('b-9');
    expect(facilitiesStore.setPanelRecord('p-2', changed)).toBe(true);
    expect(facilitiesStore.isDirty).toBe(true);
    expect(facilitiesStore.editCount).toBe(1);
    expect(facilitiesStore.facilities[0].panel).toEqual(changed);
    expect(facilitiesStore.collectDeltas()).toEqual([
      { type: 'setPanelRecord', facilityId: 'p-2', record: changed },
    ]);

    expect(facilitiesStore.setPanelRecord('p-2', saved)).toBe(true);
    expect(facilitiesStore.collectDeltas()).toEqual([]);

    facilitiesStore.setPanelRecord('p-2', changed);
    facilitiesStore.discard();
    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.facilities[0].panel).toEqual(saved);

    facilitiesStore.setPanelRecord('p-2', changed);
    facilitiesStore.hydrateBaseline([]);
    expect(facilitiesStore.isDirty).toBe(false);
  });

  it('drops a deleted facility staged record', () => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'p-3', templateId: 'panel', name: 'Panel 3', slotBindings: { lamps: [] }, panel: panelRecord() },
    ]);
    facilitiesStore.setPanelRecord('p-3', panelRecord('b-2'));
    facilitiesStore.deleteFacility('p-3');
    expect(facilitiesStore.collectDeltas()).toEqual([{ type: 'deleteFacility', facilityId: 'p-3' }]);
  });
});
