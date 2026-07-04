/**
 * Spec 018 / S6 (D3) — Hardware Channel Cascade orchestrator tests.
 *
 * Drives `reconcile()` directly instead of exercising the `$effect.root`
 * path (which requires a mounted Svelte component). The seen-set diff
 * logic is the interesting behaviour; the effect boilerplate is trusted.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';

const listBehaviorTemplatesMock = vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []);
vi.mock('$lib/api/behaviorTemplates', () => ({
  listBehaviorTemplates: listBehaviorTemplatesMock,
}));
vi.mock('$lib/api/facilities', () => ({
  listFacilities: async () => [] as Facility[],
}));
vi.mock('$lib/api/channels', () => ({
  listChannels: async () => [] as InformationChannel[],
}));
// The teardown call inside the cascade dispatches a compose IPC when
// facilities were Wired; stub it so the test observes cascade behaviour
// rather than the IPC internals.
vi.mock('$lib/api/facilityBowties', () => ({
  composeFacilityBowties: vi.fn(async () => []),
}));
// Spec 018 / S6 bugfix — teardown now syncs frontend drafts to
// LayoutState before invoking compose; stub the two draft-sync IPCs.
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: vi.fn(async () => undefined),
  clearLayoutDrafts: vi.fn(async () => undefined),
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const orchModule = await import('$lib/orchestration/facilityOrchestrator');
const { facilityCascadeOrchestrator } = await import(
  '$lib/orchestration/facilityCascadeOrchestrator.svelte'
);

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

function bod(input: number): InformationChannel {
  return {
    id: `ch-bod-${input}`,
    name: `BOD A${input}`,
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input },
  };
}

function lamp(rowOrdinal: number): InformationChannel {
  return {
    id: `ch-lamp-${rowOrdinal}`,
    name: `Lamp ${rowOrdinal}`,
    role: 'lamp-indicator',
    style: 'single-led-direct-lamp',
    ownership: 'user-owned',
    binding: { kind: 'lampRow', nodeKey: 'N2', rowOrdinal },
  };
}

beforeEach(async () => {
  facilitiesStore.reset();
  channelsStore.reset();
  behaviorTemplatesStore.reset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
  // Reset the cascade orchestrator's seen-set by stop + fresh seed.
  facilityCascadeOrchestrator.stopCascade();
});

describe('facilityCascadeOrchestrator (Spec 018 / S6 — D3)', () => {
  it('no-op when no channels have been lost', () => {
    channelsStore.hydrateBaseline([bod(1), bod(2)]);
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-1', 'ch-bod-2']));
    // Prime the seen-set.
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-1', 'ch-bod-2']));
    // No detaches — facilities untouched.
    expect(facilitiesStore.facilities.length).toBe(0);
  });

  it('detaches every slot referencing a lost hardware channel', () => {
    channelsStore.hydrateBaseline([bod(1), bod(2)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [] } },
    ]);
    // Seed baseline seen-set.
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-1', 'ch-bod-2']));
    // ch-bod-1 disappears (BOD daughter-board cleared).
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-2']));

    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
  });

  it('tears down bowties for a facility that transitions Wired → Incomplete', async () => {
    const tearDownSpy = vi.spyOn(orchModule, 'tearDownFacilityBowties');
    tearDownSpy.mockResolvedValue(undefined);

    channelsStore.hydrateBaseline([bod(1), lamp(2)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] } },
    ]);
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-1', 'ch-lamp-2']));
    // Lose the hardware channel.
    facilityCascadeOrchestrator.reconcile(new Set(['ch-lamp-2']));

    expect(tearDownSpy).toHaveBeenCalledWith('f-1');
    tearDownSpy.mockRestore();
  });

  it('does NOT tear down when the facility was already Incomplete before the loss', () => {
    const tearDownSpy = vi.spyOn(orchModule, 'tearDownFacilityBowties');
    tearDownSpy.mockResolvedValue(undefined);

    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [] } }, // output empty → Incomplete
    ]);
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-1']));
    facilityCascadeOrchestrator.reconcile(new Set([]));

    // Detach happened…
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    // …but no teardown because the facility never left Wired.
    expect(tearDownSpy).not.toHaveBeenCalled();
    tearDownSpy.mockRestore();
  });

  it('cascades across multiple facilities in one pass', () => {
    channelsStore.hydrateBaseline([bod(1), bod(2)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-a', templateId: 'block-indicator', name: 'A',
        slotBindings: { input: ['ch-bod-1'], output: [] } },
      { facilityId: 'f-b', templateId: 'block-indicator', name: 'B',
        slotBindings: { input: ['ch-bod-2'], output: [] } },
    ]);
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-1', 'ch-bod-2']));
    // Both hardware channels lost simultaneously.
    facilityCascadeOrchestrator.reconcile(new Set([]));

    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    expect(facilitiesStore.facilities[1].slotBindings.input).toEqual([]);
  });

  it('user-owned channel losses via removeFromSlot do not re-trigger the cascade', async () => {
    channelsStore.hydrateBaseline([bod(1), lamp(2)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] } },
    ]);
    facilityCascadeOrchestrator.reconcile(new Set(['ch-bod-1', 'ch-lamp-2']));

    // Remove the user-owned lamp channel via the orchestrator (S5 flow).
    // This deletes ch-lamp-2 from the channels store as a side effect.
    await orchModule.removeFromSlot({
      facilityId: 'f-1',
      slotLabel: 'output',
      channelId: 'ch-lamp-2',
    });

    // Now the cascade runs on the post-removal snapshot. The output slot
    // was already detached by removeFromSlot; the cascade should not
    // re-detach or re-teardown anything.
    const beforeBindings = { ...facilitiesStore.facilities[0].slotBindings };
    facilityCascadeOrchestrator.reconcile(
      new Set(channelsStore.channels.map((c) => c.id)),
    );
    expect(facilitiesStore.facilities[0].slotBindings).toEqual(beforeBindings);
  });
});

// ── 2026-07-03 load-time repair — dangling channel refs staged as drafts ──
//
// Regression: on layout open, `read_layout_capture` normalises the in-memory
// facilities doc against `channels.yaml` and surfaces a toast, but the
// separate `list_facilities` IPC that hydrates the frontend baseline does
// NOT normalise. Result: the frontend baseline retained the ghost channel
// id, so the slot appeared at its cap (blocking Add channel), the effective
// facility still looked Wired (routing Delete through the composer, which
// then errored with "has no consumer channel"), and no dirty flag was set
// so the user had nothing to save. This ADR-0012-shaped fix stages the
// cleanup as a normal `detachChannelFromSlot` draft at load time.
describe('reconcileDanglingChannelRefsOnLoad (2026-07-03 load-time repair)', () => {
  it('stages a detach draft for each slot binding pointing at an unknown channel', () => {
    channelsStore.hydrateBaseline([bod(1)]); // ch-bod-1 only — no lamp
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ghost-uuid'] } },
    ]);
    expect(facilitiesStore.isDirty).toBe(false);

    facilityCascadeOrchestrator.reconcileDanglingChannelRefsOnLoad();

    // Effective view: dangling ref removed, valid ref preserved.
    expect(facilitiesStore.facilities[0].slotBindings.output).toEqual([]);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-bod-1']);

    // Layout is now dirty via a normal detach delta — matches the user's
    // stated mental model ("stage a change to delete that output channel
    // so we can save the layout and eliminate this error").
    expect(facilitiesStore.isDirty).toBe(true);
    expect(facilitiesStore.collectDeltas()).toContainEqual({
      type: 'detachChannelFromSlot',
      facilityId: 'f-1',
      slotLabel: 'output',
      channelId: 'ghost-uuid',
    });
  });

  it('no-op when every slot binding resolves to a known channel', () => {
    channelsStore.hydrateBaseline([bod(1), lamp(2)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] } },
    ]);

    facilityCascadeOrchestrator.reconcileDanglingChannelRefsOnLoad();

    expect(facilitiesStore.isDirty).toBe(false);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
  });

  it('handles multiple facilities and multiple dangling refs in one pass', () => {
    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-a', templateId: 'block-indicator', name: 'A',
        slotBindings: { input: ['ghost-1'], output: ['ghost-2'] } },
      { facilityId: 'f-b', templateId: 'block-indicator', name: 'B',
        slotBindings: { input: ['ch-bod-1'], output: ['ghost-3'] } },
    ]);

    facilityCascadeOrchestrator.reconcileDanglingChannelRefsOnLoad();

    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    expect(facilitiesStore.facilities[0].slotBindings.output).toEqual([]);
    expect(facilitiesStore.facilities[1].slotBindings.input).toEqual(['ch-bod-1']);
    expect(facilitiesStore.facilities[1].slotBindings.output).toEqual([]);
    expect(facilitiesStore.collectDeltas().length).toBe(3);
  });

  it('tears down bowties for a facility that transitions Wired → Incomplete on load repair', async () => {
    const tearDownSpy = vi.spyOn(orchModule, 'tearDownFacilityBowties');
    tearDownSpy.mockResolvedValue(undefined);

    // ch-bod-1 exists; ghost-lamp does not. Facility looks Wired against
    // the uncleaned baseline (both slots non-empty) but the consumer slot
    // is dangling.
    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ghost-lamp'] } },
    ]);

    facilityCascadeOrchestrator.reconcileDanglingChannelRefsOnLoad();

    expect(tearDownSpy).toHaveBeenCalledWith('f-1');
    tearDownSpy.mockRestore();
  });
});
