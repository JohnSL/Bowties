// S1 integration test for facility CRUD across the draft layer + save round-trip.
// Mirrors `channels.svelte.test.ts` style: mocked IPC + flushSync-free Svelte 5 runes.
// Covers spec 018 S1 acceptance criteria AC1, AC4, AC5 and SC-002 at the store seam.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { Facility } from '$lib/api/facilities';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { LayoutEditDelta } from '$lib/types/bowtie';

const listFacilitiesMock = vi.fn<() => Promise<Facility[]>>(async () => []);
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
    { label: 'input', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'output', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
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
  listFacilitiesMock.mockResolvedValue([]);
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

    listFacilitiesMock.mockResolvedValue([persistedAfterAdd]);
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
    listFacilitiesMock.mockResolvedValue([{ ...persistedAfterAdd, name: 'Block 7' }]);
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
    listFacilitiesMock.mockResolvedValue([]);
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
