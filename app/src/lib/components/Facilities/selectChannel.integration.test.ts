/**
 * Spec 018 / S4 — Integration test for the Select-channel user journey.
 *
 * Drives the full consumer-surface stack: render `RailroadPanel` with
 * mocked IPC + seeded BOD-8 channels, then exercise the user journey
 * through the picker → orchestrator → store → derivation → DOM. The
 * test reaches the Channels-panel "Used by" cell and the filled-slot
 * display, not only `facilitiesStore` internals (per T1's seam-aware
 * requirement).
 *
 * Acceptance contract (mapped to slice T1):
 *   (a) empty slot — Used by `—` for all 8 channels
 *   (b) Select channel opens picker with all 8 unbound role-compatible
 *   (c) confirm fills slot + lights up the Channels-panel cell
 *   (d) `effectiveNodeStore.dirtyBreakdown.facilities === 1`
 *   (e) [retired in S6 D4 — Rebind removed; swap = Remove + Select]
 *   (f) Remove-from-slot empties + clears Used by
 *   (g) Channels-panel rename flows through to slot display
 *   (h) save → close → reopen round-trips the binding
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { fireEvent, render, screen, within, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';

// ── IPC mocks ────────────────────────────────────────────────────────────

const { listBehaviorTemplatesMock, listFacilitiesMock, listChannelsMock } = vi.hoisted(() => ({
  listBehaviorTemplatesMock: vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []),
  listFacilitiesMock: vi.fn<() => Promise<Facility[]>>(async () => []),
  listChannelsMock: vi.fn<() => Promise<InformationChannel[]>>(async () => []),
}));

vi.mock('$lib/api/behaviorTemplates', () => ({
  listBehaviorTemplates: listBehaviorTemplatesMock,
}));
vi.mock('$lib/api/facilities', () => ({
  listFacilities: listFacilitiesMock,
}));
vi.mock('$lib/api/channels', () => ({
  listChannels: listChannelsMock,
}));
// S6 compose-on-Wired hook triggers this IPC; stub it out because the S4
// select-only tests here don't reach Wired, but the compose call still runs
// on the input-slot fill path (it's a cheap Incomplete no-op) so the IPC
// import must resolve to something.
vi.mock('$lib/api/facilityBowties', () => ({
  composeFacilityBowties: async () => [],
}));
// Spec 018 / S6 bugfix — orchestrator syncs drafts to LayoutState before
// each compose (when Wired). Stub for safety even though most select-only
// paths don't cross the Wired guard.
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: async () => undefined,
  clearLayoutDrafts: async () => undefined,
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { effectiveLayoutStore } = await import('$lib/layout/effectiveLayoutStore.svelte');
const { effectiveNodeStore } = await import('$lib/layout/effectiveNodeStore.svelte');
const orch = await import('$lib/orchestration/facilityOrchestrator');

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
    name: `TowerLCC-1 BOD A${input}`,
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: '05010101FF000001', connector: 'connector-a', input },
  };
}

const stubNodeName = (key: string) =>
  key === '05010101FF000001' ? 'TowerLCC-1' : `Node(${key})`;

// Helper: get the Channels-panel row for a given channel name and return its
// last <td> (the "Used by" cell). Scoped to the channels-panel <table> so a
// channel name that also appears in a filled FacilitySlot does not confuse
// the lookup.
function usedByCell(channelName: string): HTMLTableCellElement {
  const table = within(screen.getByTestId('channels-panel')).getByRole('table');
  const nameEl = within(table).getByText(channelName);
  const row = nameEl.closest('tr');
  if (!row) throw new Error(`No row for channel "${channelName}"`);
  const cells = within(row).getAllByRole('cell') as HTMLTableCellElement[];
  return cells[cells.length - 1];
}

function slotByLabel(label: string): HTMLElement {
  const slots = screen.getAllByTestId('facility-slot');
  const match = slots.find((el) => el.getAttribute('data-slot-label') === label);
  if (!match) throw new Error(`No facility-slot with label "${label}"`);
  return match;
}

import RailroadPanel from '$lib/components/Railroad/RailroadPanel.svelte';

beforeEach(async () => {
  facilitiesStore.reset();
  channelsStore.reset();
  behaviorTemplatesStore.reset();
  listBehaviorTemplatesMock.mockReset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR]);
  listFacilitiesMock.mockReset();
  listFacilitiesMock.mockResolvedValue([]);
  listChannelsMock.mockReset();
  listChannelsMock.mockResolvedValue([]);

  await behaviorTemplatesStore.loadBehaviorTemplates();

  // Seed 8 BOD-8 channels + one Block 5 facility (Incomplete, both slots empty).
  const channels = Array.from({ length: 8 }, (_, i) => bod(i + 1));
  channelsStore.hydrateBaseline(channels);
  facilitiesStore.hydrateBaseline([
    {
      facilityId: 'f-block-5',
      templateId: 'block-indicator',
      name: 'Block 5',
      slotBindings: { input: [], output: [] },
    },
  ]);
});

describe('Spec 018 / S4 — Select-channel user journey (integration)', () => {
  function mountPanel(opts: { onSelectChannel?: (fId: string, slot: string) => void; onRemoveFromSlot?: (fId: string, slot: string, cur: string) => void } = {}) {
    return render(RailroadPanel, {
      props: {
        nodeName: stubNodeName,
        usedBy: (channelId: string) => effectiveLayoutStore.channelUsageMap.get(channelId) ?? [],
        onSelectChannel: opts.onSelectChannel,
        onRemoveFromSlot: opts.onRemoveFromSlot,
      },
    });
  }

  it('AC(a): empty slot shows Used by "—" for all 8 channels', async () => {
    mountPanel();
    for (let i = 1; i <= 8; i++) {
      expect(usedByCell(`TowerLCC-1 BOD A${i}`).textContent?.trim()).toBe('—');
    }
  });

  it('AC(b)–(d): Select channel → orchestrator attach lights up the slot + Channels-panel cell + dirtyBreakdown.facilities === 1', async () => {
    const onSelectChannel = vi.fn();
    mountPanel({ onSelectChannel });

    // (b) Click Select channel on the input slot.
    const inputSlot = slotByLabel('input');
    const selectBtn = within(inputSlot).getByTestId('select-channel-button');
    await fireEvent.click(selectBtn);
    expect(onSelectChannel).toHaveBeenCalledWith('f-block-5', 'input');

    // Simulate the route's picker confirm → orchestrator dispatch.
    orch.selectChannelForSlot({
      facilityId: 'f-block-5', slotLabel: 'input', channelId: 'ch-bod-1',
    });
    await tick();

    // (c) The Channels-panel "Used by" cell for the chosen channel reads "Block 5 / input".
    expect(usedByCell('TowerLCC-1 BOD A1').textContent?.trim()).toBe('Block 5 / input');
    for (let i = 2; i <= 8; i++) {
      expect(usedByCell(`TowerLCC-1 BOD A${i}`).textContent?.trim()).toBe('—');
    }

    // The slot itself now shows the bound channel name (filled state).
    const inputSlotAfter = slotByLabel('input');
    expect(within(inputSlotAfter).getByTestId('slot-channel-name').textContent).toBe('TowerLCC-1 BOD A1');

    // (d) dirtyBreakdown.facilities === 1 (one attach edit).
    expect(effectiveNodeStore.dirtyBreakdown.facilities).toBe(1);
  });

  it('AC(e) [retired in S6 D4]: swapping a channel is Remove + Select — no atomic Rebind action', async () => {
    // Start with ch-bod-1 already attached (simulating "saved baseline").
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-block-5',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [] },
      },
    ]);
    mountPanel();
    expect(usedByCell('TowerLCC-1 BOD A1').textContent?.trim()).toBe('Block 5 / input');

    // The filled-state slot no longer offers a Rebind button.
    const filledSlot = slotByLabel('input');
    expect(within(filledSlot).queryByTestId('rebind-channel-button')).toBeNull();

    // Attempting to attach a second channel into the max=1 slot is rejected
    // by the orchestrator's cardinality guard — the user MUST Remove first.
    expect(() =>
      orch.selectChannelForSlot({
        facilityId: 'f-block-5', slotLabel: 'input', channelId: 'ch-bod-2',
      }),
    ).toThrow(orch.SlotAtMaxError);

    // Two-step swap: Remove then Select. Both actions ship in S6 as the
    // sole swap flow (D4).
    await orch.removeFromSlot({ facilityId: 'f-block-5', slotLabel: 'input', channelId: 'ch-bod-1' });
    await orch.selectChannelForSlot({ facilityId: 'f-block-5', slotLabel: 'input', channelId: 'ch-bod-2' });
    await tick();

    expect(usedByCell('TowerLCC-1 BOD A1').textContent?.trim()).toBe('—');
    expect(usedByCell('TowerLCC-1 BOD A2').textContent?.trim()).toBe('Block 5 / input');
    const inputSlot = slotByLabel('input');
    expect(within(inputSlot).getByTestId('slot-channel-name').textContent).toBe('TowerLCC-1 BOD A2');
  });

  it('AC(f): Remove-from-slot empties the slot + clears Used by', async () => {
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-block-5',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [] },
      },
    ]);
    const onRemoveFromSlot = vi.fn((fId: string, slot: string, cur: string) => {
      orch.removeFromSlot({ facilityId: fId, slotLabel: slot, channelId: cur });
    });
    mountPanel({ onRemoveFromSlot });

    const inputSlot = slotByLabel('input');
    const removeBtn = within(inputSlot).getByTestId('remove-from-slot-button');
    await fireEvent.click(removeBtn);
    await tick();

    expect(onRemoveFromSlot).toHaveBeenCalledWith('f-block-5', 'input', 'ch-bod-1');
    expect(usedByCell('TowerLCC-1 BOD A1').textContent?.trim()).toBe('—');
    // Empty slot returns — the Select-channel button reappears on the input slot.
    await waitFor(() =>
      expect(within(slotByLabel('input')).getByTestId('select-channel-button')).toBeInTheDocument(),
    );
  });

  it('AC(g): Renaming the bound channel via the Channels panel updates the slot display', async () => {
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-block-5',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [] },
      },
    ]);
    mountPanel();
    const inputSlotBefore = slotByLabel('input');
    expect(within(inputSlotBefore).getByTestId('slot-channel-name').textContent).toBe('TowerLCC-1 BOD A1');

    // Rename via store (the Channels panel's click-to-rename emits the same call).
    channelsStore.renameChannel('ch-bod-1', 'Block 5 sensor');
    await tick();

    const inputSlotAfter = slotByLabel('input');
    expect(within(inputSlotAfter).getByTestId('slot-channel-name').textContent).toBe('Block 5 sensor');
  });

  it('AC(h): save → close → reopen round-trips the slot binding', async () => {
    // Simulate the post-save hydrate cycle.
    orch.selectChannelForSlot({
      facilityId: 'f-block-5', slotLabel: 'input', channelId: 'ch-bod-1',
    });
    expect(facilitiesStore.collectDeltas()).toEqual([
      { type: 'attachChannelToSlot', facilityId: 'f-block-5', slotLabel: 'input', channelId: 'ch-bod-1' },
    ]);

    // "Save": replay the delta against a fresh document, then hydrate.
    const persistedFacility: Facility = {
      facilityId: 'f-block-5',
      templateId: 'block-indicator',
      name: 'Block 5',
      slotBindings: { input: ['ch-bod-1'], output: [] },
    };
    facilitiesStore.hydrateBaseline([persistedFacility]);
    expect(facilitiesStore.isDirty).toBe(false);

    // "Reopen": reset + reload from the persisted backend.
    facilitiesStore.reset();
    listFacilitiesMock.mockResolvedValue([persistedFacility]);
    await facilitiesStore.loadFacilities();
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-bod-1']);
  });
});
