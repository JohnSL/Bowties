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
    { label: 'input', kind: 'producer', requiredRole: 'block-occupancy' },
    { label: 'output', kind: 'consumer', requiredRole: 'lamp-indicator' },
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
      slotBindings: { input: null, output: null },
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
      slotBindings: { input: null, output: null },
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
