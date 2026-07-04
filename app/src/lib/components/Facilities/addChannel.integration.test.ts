/**
 * Spec 018 / S5 — Integration test for the Add-channel user journey
 * (consumer half of US3).
 *
 * Drives the full consumer-surface stack: render `RailroadPanel` with
 * mocked IPC + seeded BOD-8 + Signal-LCC DLC rows, then exercise the
 * user journey through the AddChannelPicker → facilityOrchestrator →
 * channelsStore + facilitiesStore → derivation → DOM. Reaches the
 * Channels-panel row (new lamp-indicator channel + lit/unlit state),
 * the filled output-slot display, and the orchestrator's atomic
 * removeFromSlot behavior (detach + user-owned delete).
 *
 * Acceptance contract (mapped to slice T1):
 *   (a) output slot empty → Add channel button visible; Select hidden
 *   (b) Add channel → AddChannelPicker opens with unclaimed DLC rows
 *   (c) confirm → atomic: user-owned lamp-indicator channel created +
 *       slot attached; channel appears in Channels panel with USER badge,
 *       'Lamp indicator' role, single-led-direct-lamp style, location
 *       'Row N', Used by 'Block 5 / output', initial state Unknown
 *   (d) dirtyBreakdown reports new channel + facility edit
 *   (e) PCER 'lit' event arriving → ChannelRow renders 'Lit'; PCER 'unlit'
 *       → renders 'Unlit'
 *   (f) collectDeltas emits both createChannel and
 *       AttachChannelToSlot (D2 atomic-save contract)
 *   (g) Remove-from-slot → detach + delete user-owned channel; row
 *       disappears from Channels panel; lamp row re-eligible
 *   (h) save → close → reopen round-trips the user-owned channel + binding
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';

// ── IPC mocks ────────────────────────────────────────────────────────────

const {
  listBehaviorTemplatesMock,
  listFacilitiesMock,
  listChannelsMock,
  resolveChannelEventIdsMock,
  eligibleLampRowsMock,
} = vi.hoisted(() => ({
  listBehaviorTemplatesMock: vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []),
  listFacilitiesMock: vi.fn<() => Promise<Facility[]>>(async () => []),
  listChannelsMock: vi.fn<() => Promise<InformationChannel[]>>(async () => []),
  resolveChannelEventIdsMock: vi.fn(
    async (
      _channels: InformationChannel[],
    ): Promise<ReadonlyMap<string, Record<string, string>>> => new Map(),
  ),
  eligibleLampRowsMock: vi.fn(
    () =>
      [] as Array<{
        nodeKey: string;
        nodeName: string;
        rows: Array<{
          nodeKey: string;
          nodeName: string;
          rowOrdinal: number;
          rowLabel: string;
        }>;
      }>,
  ),
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
// S6 compose-on-Wired hook triggers this IPC when Add-channel completes
// the facility. Stub it out so the S5 tests only observe the S5 seams.
vi.mock('$lib/api/facilityBowties', () => ({
  composeFacilityBowties: async () => [],
}));
// Spec 018 / S6 bugfix — orchestrator now mirrors drafts to LayoutState
// before compose IPC. Stub the two draft-sync IPCs.
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: async () => undefined,
  clearLayoutDrafts: async () => undefined,
}));
vi.mock('$lib/orchestration/eventStateOrchestrator', async () => {
  const actual =
    await vi.importActual<typeof import('$lib/orchestration/eventStateOrchestrator')>(
      '$lib/orchestration/eventStateOrchestrator',
    );
  return {
    ...actual,
    resolveChannelEventIds: resolveChannelEventIdsMock,
  };
});

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { effectiveLayoutStore } = await import('$lib/layout/effectiveLayoutStore.svelte');
const { effectiveNodeStore } = await import('$lib/layout/effectiveNodeStore.svelte');
const { eventStateStore } = await import('$lib/stores/eventState.svelte');
const orch = await import('$lib/orchestration/facilityOrchestrator');

// The eligibleLampRowsForStyle derivation depends on nodeRegistryStore +
// per-node CDI; for this integration test we stub it via a spy on the
// effectiveLayoutStore method. Its own unit test (T14) covers the
// derivation; here we want to drive the user journey through it.
const SIGNAL_NODE_KEY = '05010101FF000002';

const TOWER_NODE_KEY = '05010101FF000001';

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
    binding: { kind: 'connectorInput', nodeKey: TOWER_NODE_KEY, connector: 'connector-a', input },
  };
}

const stubNodeName = (key: string) =>
  key === TOWER_NODE_KEY
    ? 'TowerLCC-1'
    : key === SIGNAL_NODE_KEY
      ? 'Signal-LCC-1'
      : `Node(${key})`;

function usedByCell(channelName: string): HTMLTableCellElement {
  const table = within(screen.getByTestId('channels-panel')).getByRole('table');
  const nameEl = within(table).getByText(channelName);
  const row = nameEl.closest('tr');
  if (!row) throw new Error(`No row for channel "${channelName}"`);
  const cells = within(row).getAllByRole('cell') as HTMLTableCellElement[];
  return cells[cells.length - 1];
}

function stateCellFor(channelName: string): HTMLTableCellElement {
  const table = within(screen.getByTestId('channels-panel')).getByRole('table');
  const nameEl = within(table).getByText(channelName);
  const row = nameEl.closest('tr');
  if (!row) throw new Error(`No row for channel "${channelName}"`);
  const cells = within(row).getAllByRole('cell') as HTMLTableCellElement[];
  // Layout: state-dot, name, role/style, location, state, used-by (6 cells).
  return cells[cells.length - 2];
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
  eventStateStore.clear();

  listBehaviorTemplatesMock.mockReset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR]);
  listFacilitiesMock.mockReset();
  listFacilitiesMock.mockResolvedValue([]);
  listChannelsMock.mockReset();
  listChannelsMock.mockResolvedValue([]);

  // Default: 4 DLC rows on Signal-LCC-1, all unclaimed.
  eligibleLampRowsMock.mockReset();
  eligibleLampRowsMock.mockImplementation(() => [
    {
      nodeKey: SIGNAL_NODE_KEY,
      nodeName: 'Signal-LCC-1',
      rows: [
        { nodeKey: SIGNAL_NODE_KEY, nodeName: 'Signal-LCC-1', rowOrdinal: 1, rowLabel: 'Lamp 1' },
        { nodeKey: SIGNAL_NODE_KEY, nodeName: 'Signal-LCC-1', rowOrdinal: 2, rowLabel: 'Lamp 2' },
        { nodeKey: SIGNAL_NODE_KEY, nodeName: 'Signal-LCC-1', rowOrdinal: 3, rowLabel: 'Lamp 3' },
        { nodeKey: SIGNAL_NODE_KEY, nodeName: 'Signal-LCC-1', rowOrdinal: 4, rowLabel: 'Lamp 4' },
      ],
    },
  ]);
  // Stub the derivation. The unit test for the real derivation lives in T14.
  vi.spyOn(effectiveLayoutStore, 'eligibleLampRowsForStyle').mockImplementation((styleId) =>
    styleId === 'single-led-direct-lamp' ? eligibleLampRowsMock() : [],
  );

  // Deterministic event-id resolution for lamp-indicator channels: every
  // lamp-row channel gets a unique pair derived from its rowOrdinal so we
  // can drive lit/unlit state by recording into eventStateStore.
  resolveChannelEventIdsMock.mockReset();
  resolveChannelEventIdsMock.mockImplementation(async (channels) => {
    const map = new Map<string, Record<string, string>>();
    for (const ch of channels) {
      if (ch.role === 'lamp-indicator' && ch.binding.kind === 'lampRow') {
        const n = ch.binding.rowOrdinal;
        map.set(ch.id, {
          lit: `02010101FF02000${n.toString(16).toUpperCase()}`.padStart(16, '0').slice(-16),
          unlit: `02010101FF02001${n.toString(16).toUpperCase()}`.padStart(16, '0').slice(-16),
        });
      } else if (ch.role === 'block-occupancy' && ch.binding.kind === 'connectorInput') {
        const n = ch.binding.input;
        map.set(ch.id, {
          occupied: `02010101FF01000${n.toString(16).toUpperCase()}`.padStart(16, '0').slice(-16),
          clear: `02010101FF01001${n.toString(16).toUpperCase()}`.padStart(16, '0').slice(-16),
        });
      }
    }
    return map;
  });

  await behaviorTemplatesStore.loadBehaviorTemplates();

  // Seed 8 BOD-8 channels (all hardware-owned, unbound) + one Block 5 facility
  // with input slot already filled by S4 mechanism (so this slice's focus is
  // purely on the output side).
  const channels = Array.from({ length: 8 }, (_, i) => bod(i + 1));
  channelsStore.hydrateBaseline(channels);
  facilitiesStore.hydrateBaseline([
    {
      facilityId: 'f-block-5',
      templateId: 'block-indicator',
      name: 'Block 5',
      slotBindings: { input: ['ch-bod-1'], output: [] },
    },
  ]);
});

describe('Spec 018 / S5 — Add-channel user journey (integration)', () => {
  function mountPanel(opts: {
    onSelectChannel?: (fId: string, slot: string) => void;
    onAddChannel?: (fId: string, slot: string) => void;
    onRemoveFromSlot?: (fId: string, slot: string, cur: string) => void;
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
  } = {}) {
    return render(RailroadPanel, {
      props: {
        nodeName: stubNodeName,
        resolvedEventIds: opts.resolvedEventIds,
        usedBy: (channelId: string) => effectiveLayoutStore.channelUsageMap.get(channelId) ?? [],
        onSelectChannel: opts.onSelectChannel,
        onAddChannel: opts.onAddChannel,
        onRemoveFromSlot: opts.onRemoveFromSlot,
      },
    });
  }

  it('AC(a): output slot empty → Add channel button visible; Select channel hidden', async () => {
    mountPanel();
    const outputSlot = slotByLabel('output');
    expect(within(outputSlot).getByTestId('add-channel-button')).toBeInTheDocument();
    expect(within(outputSlot).queryByTestId('select-channel-button')).toBeNull();

    // Input slot keeps S4 behaviour: it's already filled, so neither button shows.
    const inputSlot = slotByLabel('input');
    expect(within(inputSlot).queryByTestId('add-channel-button')).toBeNull();
  });

  it('AC(b)–(d) + (f): Add channel → orchestrator atomic create+attach lights up the Channels panel + dirtyBreakdown + collectDeltas', async () => {
    const onAddChannel = vi.fn();
    mountPanel({ onAddChannel });

    // (b) Click Add channel on output → component emits intent up to the route.
    const outputSlot = slotByLabel('output');
    const addBtn = within(outputSlot).getByTestId('add-channel-button');
    await fireEvent.click(addBtn);
    expect(onAddChannel).toHaveBeenCalledWith('f-block-5', 'output');

    // Route would open AddChannelPicker; confirm dispatches the orchestrator.
    const { channelId: newChannelId } = await orch.addChannelForSlot({
      facilityId: 'f-block-5',
      slotLabel: 'output',
      lampRowNodeKey: SIGNAL_NODE_KEY,
      rowOrdinal: 2,
    });
    await tick();

    // (c) New user-owned lamp-indicator channel appears in the store + UI.
    const created = channelsStore.channels.find((c) => c.id === newChannelId)!;
    expect(created).toBeDefined();
    expect(created.role).toBe('lamp-indicator');
    expect(created.style).toBe('single-led-direct-lamp');
    expect(created.ownership).toBe('user-owned');
    expect(created.binding).toEqual({ kind: 'lampRow', nodeKey: SIGNAL_NODE_KEY, rowOrdinal: 2 });
    // Default name: `{facility.name} {slotLabel}` per T16.
    expect(created.name).toBe('Block 5 output');

    // The Channels panel renders the new row under the Signal-LCC-1
    // Direct Lamp Control group with USER ownership badge and the
    // facility-slot Used-by cell populated.
    const row = within(screen.getByTestId('channels-panel'))
      .getByText('Block 5 output')
      .closest('tr')!;
    expect(within(row).getByText('USER')).toBeInTheDocument();
    expect(within(row).getByText(/Lamp indicator/i)).toBeInTheDocument();
    expect(within(row).getByText('single-led-direct-lamp')).toBeInTheDocument();
    expect(within(row).getByText(/Row\s*2/i)).toBeInTheDocument();
    expect(usedByCell('Block 5 output').textContent?.trim()).toBe('Block 5 / output');

    // Initial state is Unknown (no PCER seen).
    expect(stateCellFor('Block 5 output').textContent?.trim()).toMatch(/Unknown/i);

    // (d) Dirty breakdown reflects the new channel edit + facility attach edit.
    expect(effectiveNodeStore.dirtyBreakdown.channels).toBeGreaterThanOrEqual(1);
    expect(effectiveNodeStore.dirtyBreakdown.facilities).toBeGreaterThanOrEqual(1);

    // (f) collectDeltas emits both createChannel AND attachChannelToSlot.
    const channelDeltas = channelsStore.collectDeltas();
    const facilityDeltas = facilitiesStore.collectDeltas();
    expect(channelDeltas).toContainEqual({
      type: 'createChannel',
      channel: created,
    });
    expect(facilityDeltas).toContainEqual({
      type: 'attachChannelToSlot',
      facilityId: 'f-block-5',
      slotLabel: 'output',
      channelId: newChannelId,
    });

    // The filled output slot displays the channel name.
    const outputSlotAfter = slotByLabel('output');
    expect(within(outputSlotAfter).getByTestId('slot-channel-name').textContent).toBe(
      'Block 5 output',
    );
  });

  it('AC(e): PCER lit / unlit drives ChannelRow state through the new ChannelState discriminated union', async () => {
    // Create the channel first so we can build the resolution map ahead of
    // the render. The real route does this via a $derived effect after the
    // orchestrator confirms; here we simulate the same flow inline.
    const { channelId } = await orch.addChannelForSlot({
      facilityId: 'f-block-5',
      slotLabel: 'output',
      lampRowNodeKey: SIGNAL_NODE_KEY,
      rowOrdinal: 3,
    });

    // Build the resolution map for every channel currently in the store —
    // this mirrors what the route's `resolveChannelEventIds(channels)` call
    // produces.
    const resolved = await resolveChannelEventIdsMock(channelsStore.channels);
    const ids = resolved.get(channelId)!;
    expect(ids.lit).toBeTruthy();
    expect(ids.unlit).toBeTruthy();

    mountPanel({ resolvedEventIds: resolved });

    // Push PCER lit → state should become 'Lit'.
    eventStateStore.record(ids.lit, 1000);
    await tick();
    expect(stateCellFor('Block 5 output').textContent?.trim()).toMatch(/Lit/);

    // Push PCER unlit (later timestamp) → state should become 'Unlit'.
    eventStateStore.record(ids.unlit, 2000);
    await tick();
    expect(stateCellFor('Block 5 output').textContent?.trim()).toMatch(/Unlit/);

    // Block-occupancy channel state still works (D3 preservation).
    const bodIds = resolved.get('ch-bod-1')!;
    eventStateStore.record(bodIds.occupied, 3000);
    await tick();
    expect(stateCellFor('TowerLCC-1 BOD A1').textContent?.trim()).toMatch(/Occupied/);
  });

  it('AC(g): Remove-from-slot deletes the user-owned channel + frees the lamp row', async () => {
    // Pre-bind: simulate a session where Add-channel already ran and saved.
    const lampChannel: InformationChannel = {
      id: 'ch-lamp-2',
      name: 'Block 5 output',
      role: 'lamp-indicator',
      style: 'single-led-direct-lamp',
      ownership: 'user-owned',
      binding: { kind: 'lampRow', nodeKey: SIGNAL_NODE_KEY, rowOrdinal: 2 },
    };
    channelsStore.hydrateBaseline([
      ...Array.from({ length: 8 }, (_, i) => bod(i + 1)),
      lampChannel,
    ]);
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-block-5',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] },
      },
    ]);

    const onRemoveFromSlot = vi.fn((fId: string, slot: string, cur: string) => {
      orch.removeFromSlot({ facilityId: fId, slotLabel: slot, channelId: cur });
    });
    mountPanel({ onRemoveFromSlot });

    // Lamp channel visible in Channels panel; slot filled.
    expect(
      within(screen.getByTestId('channels-panel')).queryByText('Block 5 output'),
    ).toBeInTheDocument();

    const outputSlot = slotByLabel('output');
    const removeBtn = within(outputSlot).getByTestId('remove-from-slot-button');
    await fireEvent.click(removeBtn);
    await tick();

    expect(onRemoveFromSlot).toHaveBeenCalledWith('f-block-5', 'output', 'ch-lamp-2');

    // Orchestrator should have detached + deleted the user-owned channel.
    expect(facilitiesStore.facilities[0].slotBindings.output).toEqual([]);
    expect(channelsStore.channels.find((c) => c.id === 'ch-lamp-2')).toBeUndefined();

    // Channels-panel row gone.
    expect(
      within(screen.getByTestId('channels-panel')).queryByText('Block 5 output'),
    ).toBeNull();

    // Output slot returns to empty + Add channel button reappears.
    expect(within(slotByLabel('output')).getByTestId('add-channel-button')).toBeInTheDocument();
  });

  it('AC(h): save → close → reopen round-trips the user-owned channel + binding via the new delta path', async () => {
    const { channelId } = await orch.addChannelForSlot({
      facilityId: 'f-block-5',
      slotLabel: 'output',
      lampRowNodeKey: SIGNAL_NODE_KEY,
      rowOrdinal: 1,
    });

    // Save flow: backend would apply both deltas atomically. Simulate by
    // hydrating the post-save state: the new channel is now in the
    // baseline, and the facility's output slot binding is persisted.
    const persistedChannel = channelsStore.channels.find((c) => c.id === channelId)!;
    channelsStore.hydrateBaseline([
      ...channelsStore.channels.filter((c) => c.ownership === 'hardware-owned'),
      persistedChannel,
    ]);
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-block-5',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [channelId] },
      },
    ]);
    expect(channelsStore.isDirty).toBe(false);
    expect(facilitiesStore.isDirty).toBe(false);

    // Reopen: reset + reload.
    channelsStore.reset();
    facilitiesStore.reset();
    listChannelsMock.mockResolvedValue([persistedChannel]);
    listFacilitiesMock.mockResolvedValue([
      {
        facilityId: 'f-block-5',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [channelId] },
      },
    ]);
    await channelsStore.loadChannels();
    await facilitiesStore.loadFacilities();

    const reloaded = channelsStore.channels.find((c) => c.id === channelId);
    expect(reloaded).toBeDefined();
    expect(reloaded!.role).toBe('lamp-indicator');
    expect(reloaded!.style).toBe('single-led-direct-lamp');
    expect(reloaded!.ownership).toBe('user-owned');
    expect(reloaded!.binding).toEqual({
      kind: 'lampRow',
      nodeKey: SIGNAL_NODE_KEY,
      rowOrdinal: 1,
    });
    expect(facilitiesStore.facilities[0].slotBindings.output).toEqual([channelId]);
  });
});
