/**
 * Spec 018 / S6 — Integration test for the "Facility becomes Wired" journey
 * (headline US3, end-to-end draft-layer composition + teardown).
 *
 * Drives the full consumer-surface stack: render `RailroadPanel` with mocked
 * IPC + seeded stores, then walk the user journey through the Add-channel
 * flow → `facilityOrchestrator.composeBowtiesIfWired` → `compose_facility_bowties`
 * IPC → `configEditor.applyEdit` + `bowtieMetadataStore.createBowtie`
 * → `effectiveLayoutStore.facilityStatus` + `bowtiesForFacility` + the
 * `configChangesStore` draft. Also exercises the teardown path on
 * `removeFromSlot` and the save-close-reopen round-trip.
 *
 * Acceptance contract (mapped to S6 T5 (a)–(g)):
 *   (a) FacilityCard status pill reads Incomplete (driven by
 *       `effectiveLayoutStore.facilityStatus`); no composed bowties for
 *       the facility exist.
 *   (b) invoking `addChannelForSlot` on the empty output slot fills the
 *       slot; the compose-on-Wired hook then runs.
 *   (c) `facilityStatus` flips to `'Wired'`.
 *   (d) `bowtieMetadataStore.bowtiesForFacility(id)` returns exactly two
 *       event-id hex strings; the composed bowtie names come from the
 *       state mapping ("Block 5 — lit", "Block 5 — unlit") and both
 *       carry `createdByFacility === 'f-block-5'`.
 *   (e) `configChangesStore.draftEntries()` contains draft edits for the
 *       consumer's Lamp On / Lamp Off leaves, with the producer BOD
 *       channel's `occupied` / `clear` event IDs written verbatim (D6 —
 *       adopted, not fresh).
 *   (f) invoking `removeFromSlot` on the input slot tears the bowties
 *       down BEFORE detaching: the two consumer leaves show fresh
 *       (regenerated) event IDs in `configChangesStore` drafts, the
 *       metadata rows disappear, `facilityStatus` flips back to
 *       `'Incomplete'`, and the BOD channel returns to unbound.
 *   (g) after simulating a save-close-reopen the two bowties persist in
 *       the reloaded layout with `createdByFacility` intact and the
 *       facility renders Wired again.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';
import type { CompositionOp } from '$lib/api/facilityBowties';

// ── IPC / API mocks ─────────────────────────────────────────────────────

const {
  listBehaviorTemplatesMock,
  listFacilitiesMock,
  listChannelsMock,
  resolveChannelEventIdsMock,
  composeFacilityBowtiesMock,
  eligibleLampRowsMock,
  syncLayoutDraftsMock,
  clearLayoutDraftsMock,
} = vi.hoisted(() => ({
  listBehaviorTemplatesMock: vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []),
  listFacilitiesMock: vi.fn<() => Promise<Facility[]>>(async () => []),
  listChannelsMock: vi.fn<() => Promise<InformationChannel[]>>(async () => []),
  resolveChannelEventIdsMock: vi.fn(
    async (
      _channels: InformationChannel[],
    ): Promise<ReadonlyMap<string, Record<string, string>>> => new Map(),
  ),
  composeFacilityBowtiesMock: vi.fn(async (_facilityId: string): Promise<CompositionOp[]> => []),
  eligibleLampRowsMock: vi.fn(
    () =>
      [] as Array<{
        nodeKey: string;
        nodeName: string;
        nodeParts: { name: string; model: string | null; manufacturer: string | null; isUserNamed: boolean };
        rows: Array<{
          nodeKey: string;
          nodeName: string;
          rowOrdinal: number;
          rowLabel: string;
        }>;
      }>,
  ),
  // Spec 018 / S6 bugfix — the orchestrator now mirrors frontend drafts
  // into `LayoutState.drafts` before every compose IPC. See ADR-0015.
  syncLayoutDraftsMock: vi.fn(async (_deltas: unknown): Promise<void> => undefined),
  clearLayoutDraftsMock: vi.fn(async (): Promise<void> => undefined),
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
vi.mock('$lib/api/facilityBowties', () => ({
  composeFacilityBowties: composeFacilityBowtiesMock,
}));
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: syncLayoutDraftsMock,
  clearLayoutDrafts: clearLayoutDraftsMock,
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
const { bowtieMetadataStore } = await import('$lib/stores/bowtieMetadata.svelte');
const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');
const orch = await import('$lib/orchestration/facilityOrchestrator');

// The consumer leaves that composition writes live under the Signal-LCC's
// Direct Lamp Control segment. The test provides a synthetic CDI tree via
// the composition IPC mock — the S6 composition seam produces the exact
// edit-key + event-id pair the orchestrator dispatches, so this test does
// not need to stand up a real CDI tree to verify the frontend contract.
const SIGNAL_NODE_KEY = '05010101FF000002';
const TOWER_NODE_KEY = '05010101FF000001';

const BOD_A1_OCCUPIED_HEX = '02010101FF010001';
const BOD_A1_CLEAR_HEX = '02010101FF010101';

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

function slotByLabel(label: string): HTMLElement {
  const slots = screen.getAllByTestId('facility-slot');
  const match = slots.find((el) => el.getAttribute('data-slot-label') === label);
  if (!match) throw new Error(`No facility-slot with label "${label}"`);
  return match;
}

/**
 * Compose the two `CompositionOp` records for a Wired Block Indicator. The
 * composition seam's real logic lives in Rust (T7 property tests) — this
 * test mocks the IPC and returns the exact ops the seam produces for the
 * Block 5 fixture wired to BOD A1 (producer) + Signal-LCC Row N (consumer).
 */
function composeOpsFor(rowOrdinal: number): CompositionOp[] {
  const litLeafAddress = 100 + (rowOrdinal - 1) * 16;
  const unlitLeafAddress = litLeafAddress + 8;
  return [
    {
      consumerNodeKey: SIGNAL_NODE_KEY,
      consumerLeafPath: ['Direct Lamp Control', `Lamp #${rowOrdinal}`, 'Lamp On'],
      consumerLeafSpace: 253,
      consumerLeafAddress: litLeafAddress,
      eventIdBytes: parseHex(BOD_A1_OCCUPIED_HEX),
      bowtieName: 'Block 5 — lit',
      createdByFacility: 'f-block-5',
    },
    {
      consumerNodeKey: SIGNAL_NODE_KEY,
      consumerLeafPath: ['Direct Lamp Control', `Lamp #${rowOrdinal}`, 'Lamp Off'],
      consumerLeafSpace: 253,
      consumerLeafAddress: unlitLeafAddress,
      eventIdBytes: parseHex(BOD_A1_CLEAR_HEX),
      bowtieName: 'Block 5 — unlit',
      createdByFacility: 'f-block-5',
    },
  ];
}

function parseHex(hex: string): number[] {
  const bytes: number[] = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substring(i, i + 2), 16));
  }
  return bytes;
}

/**
 * A minimal Signal-LCC tree with a Direct Lamp Control segment containing
 * a single Lamp #N group with two consumer EventId leaves (`Lamp On`,
 * `Lamp Off`). Used to satisfy `findLeafByPath` + `generateFreshEventIdForNode`
 * during the teardown path.
 */
function synthConsumerTree(rowOrdinal: number) {
  const litAddr = 100 + (rowOrdinal - 1) * 16;
  const unlitAddr = litAddr + 8;
  return {
    nodeId: SIGNAL_NODE_KEY,
    identity: null,
    connectorProfile: null,
    connectorProfileWarning: null,
    unknownVariants: [],
    profileApplied: true,
    segments: [
      {
        name: 'Direct Lamp Control',
        description: null,
        origin: 0,
        space: 253,
        children: [
          {
            kind: 'group' as const,
            name: `Lamp #${rowOrdinal}`,
            hasName: true,
            description: null,
            instance: rowOrdinal,
            instanceLabel: `Lamp #${rowOrdinal}`,
            replicationOf: 'Lamp',
            replicationCount: 4,
            path: ['Direct Lamp Control', `Lamp #${rowOrdinal}`],
            displayName: `Lamp #${rowOrdinal}`,
            hideable: false,
            hiddenByDefault: false,
            readOnly: false,
            children: [
              {
                kind: 'leaf' as const,
                name: 'Lamp On',
                description: null,
                elementType: 'eventId' as const,
                address: litAddr,
                size: 8,
                space: 253,
                path: ['Direct Lamp Control', `Lamp #${rowOrdinal}`, 'Lamp On'],
                value: {
                  type: 'eventId' as const,
                  bytes: [0, 0, 0, 0, 0, 0, 0, 0],
                  hex: '0000000000000000',
                },
                eventRole: 'Consumer' as const,
                constraints: null,
                readOnly: false,
                modifiedValue: null,
              },
              {
                kind: 'leaf' as const,
                name: 'Lamp Off',
                description: null,
                elementType: 'eventId' as const,
                address: unlitAddr,
                size: 8,
                space: 253,
                path: ['Direct Lamp Control', `Lamp #${rowOrdinal}`, 'Lamp Off'],
                value: {
                  type: 'eventId' as const,
                  bytes: [0, 0, 0, 0, 0, 0, 0, 0],
                  hex: '0000000000000000',
                },
                eventRole: 'Consumer' as const,
                constraints: null,
                readOnly: false,
                modifiedValue: null,
              },
            ],
          },
        ],
      },
    ],
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any;
}

import RailroadPanel from '$lib/components/Railroad/RailroadPanel.svelte';

beforeEach(async () => {
  facilitiesStore.reset();
  channelsStore.reset();
  behaviorTemplatesStore.reset();
  bowtieMetadataStore.clearAll();
  configChangesStore.clearAllDrafts();

  listBehaviorTemplatesMock.mockReset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR]);
  listFacilitiesMock.mockReset();
  listFacilitiesMock.mockResolvedValue([]);
  listChannelsMock.mockReset();
  listChannelsMock.mockResolvedValue([]);
  composeFacilityBowtiesMock.mockReset();
  composeFacilityBowtiesMock.mockResolvedValue([]);

  eligibleLampRowsMock.mockReset();
  eligibleLampRowsMock.mockImplementation(() => [
    {
      nodeKey: SIGNAL_NODE_KEY,
      nodeName: 'Signal-LCC-1',
      nodeParts: { name: 'Signal-LCC-1', model: 'Signal-LCC', manufacturer: 'RR-CirKits', isUserNamed: true },
      rows: [
        { nodeKey: SIGNAL_NODE_KEY, nodeName: 'Signal-LCC-1', rowOrdinal: 1, rowLabel: 'Lamp 1' },
        { nodeKey: SIGNAL_NODE_KEY, nodeName: 'Signal-LCC-1', rowOrdinal: 2, rowLabel: 'Lamp 2' },
      ],
    },
  ]);
  vi.spyOn(effectiveLayoutStore, 'eligibleLampRowsForStyle').mockImplementation((styleId) =>
    styleId === 'single-led-direct-lamp' ? eligibleLampRowsMock() : [],
  );

  resolveChannelEventIdsMock.mockReset();
  resolveChannelEventIdsMock.mockImplementation(async (channels) => {
    const map = new Map<string, Record<string, string>>();
    for (const ch of channels) {
      if (ch.role === 'block-occupancy' && ch.binding.kind === 'connectorInput') {
        const n = ch.binding.input;
        if (n === 1) {
          map.set(ch.id, { occupied: BOD_A1_OCCUPIED_HEX, clear: BOD_A1_CLEAR_HEX });
        }
      }
    }
    return map;
  });

  await behaviorTemplatesStore.loadBehaviorTemplates();

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

describe('Spec 018 / S6 — Wired facility end-to-end (integration)', () => {
  function mountPanel() {
    return render(RailroadPanel, {
      props: {
        nodeName: stubNodeName,
        usedBy: (channelId: string) => effectiveLayoutStore.channelUsageMap.get(channelId) ?? [],
      },
    });
  }

  it('AC(a): facility with one empty slot renders Incomplete via the facade; no composed bowties', () => {
    mountPanel();

    // AC(a) — facade drives the pill.
    expect(effectiveLayoutStore.facilityStatus('f-block-5')).toBe('Incomplete');

    const outputSlot = slotByLabel('output');
    expect(within(outputSlot).getByTestId('add-channel-button')).toBeInTheDocument();

    // No composed bowties for the facility yet.
    expect(bowtieMetadataStore.bowtiesForFacility('f-block-5')).toEqual([]);
  });

  it('AC(b)–(e): filling the last slot atomically composes the two draft bowties + writes consumer leaves with the producer\'s event IDs', async () => {
    composeFacilityBowtiesMock.mockImplementation(async (facilityId) => {
      expect(facilityId).toBe('f-block-5');
      return composeOpsFor(2);
    });

    mountPanel();

    // AC(b) — attach the last empty slot; orchestrator hook composes on wired.
    await orch.addChannelForSlot({
      facilityId: 'f-block-5',
      slotLabel: 'output',
      lampRowNodeKey: SIGNAL_NODE_KEY,
      rowOrdinal: 2,
    });
    await tick();

    // AC(c) — status flips to Wired driven by the facade.
    expect(effectiveLayoutStore.facilityStatus('f-block-5')).toBe('Wired');

    // AC(d) — two draft bowties tagged with createdByFacility === 'f-block-5'.
    const composed = bowtieMetadataStore.bowtiesForFacility('f-block-5');
    expect(composed).toHaveLength(2);
    // Both composed event-id hex strings must be the producer's event IDs
    // (D6 — adopt, do not regenerate). Canonical form is 16-char uppercase
    // contiguous hex (no dots).
    expect(new Set(composed)).toEqual(
      new Set([BOD_A1_OCCUPIED_HEX, BOD_A1_CLEAR_HEX]),
    );

    // AC(d) — names come from the state mapping.
    const litMeta = bowtieMetadataStore.getMetadata(BOD_A1_OCCUPIED_HEX);
    expect(litMeta?.name).toBe('Block 5 — lit');
    expect(litMeta?.createdByFacility).toBe('f-block-5');
    const unlitMeta = bowtieMetadataStore.getMetadata(BOD_A1_CLEAR_HEX);
    expect(unlitMeta?.name).toBe('Block 5 — unlit');
    expect(unlitMeta?.createdByFacility).toBe('f-block-5');

    // AC(e) — draft config edits target the consumer's Lamp On / Lamp Off
    // leaves with the producer's event IDs (D6, verbatim adoption).
    const drafts = configChangesStore.draftEntries();
    const consumerEdits = drafts.filter((d) => d.key.startsWith(SIGNAL_NODE_KEY + ':'));
    expect(consumerEdits).toHaveLength(2);
    const editByHex = new Map<string, (typeof consumerEdits)[number]>();
    for (const edit of consumerEdits) {
      if (edit.value.type !== 'eventId') throw new Error('expected eventId edit');
      editByHex.set(edit.value.hex, edit);
    }
    expect(editByHex.has(BOD_A1_OCCUPIED_HEX)).toBe(true);
    expect(editByHex.has(BOD_A1_CLEAR_HEX)).toBe(true);
  });

  it('AC(f): remove-from-slot on the input tears bowties down BEFORE detach; fresh event IDs land on consumer leaves', async () => {
    // Pre-Wire the facility.
    channelsStore.hydrateBaseline([
      ...Array.from({ length: 8 }, (_, i) => bod(i + 1)),
      {
        id: 'ch-lamp-2',
        name: 'Block 5 output',
        role: 'lamp-indicator',
        style: 'single-led-direct-lamp',
        ownership: 'user-owned',
        binding: { kind: 'lampRow', nodeKey: SIGNAL_NODE_KEY, rowOrdinal: 2 },
      },
    ]);
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-block-5',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] },
      },
    ]);
    // Simulate the composed state as if a prior session already Wired.
    bowtieMetadataStore.createBowtie(BOD_A1_OCCUPIED_HEX, 'Block 5 — lit', {
      createdByFacility: 'f-block-5',
    });
    bowtieMetadataStore.createBowtie(BOD_A1_CLEAR_HEX, 'Block 5 — unlit', {
      createdByFacility: 'f-block-5',
    });
    // The teardown resolves the consumer leaves via `nodeTreeStore.getTree`
    // + `findLeafByPath`. Provide a synthetic Signal-LCC tree whose leaf
    // paths match the composition ops.
    vi.spyOn(nodeTreeStore, 'getTree').mockImplementation((k) => {
      if (k !== SIGNAL_NODE_KEY) return undefined;
      return synthConsumerTree(2);
    });
    // Teardown must know which leaves to overwrite; the orchestrator calls
    // `composeFacilityBowties` on the still-Wired shape and uses the
    // returned op leaves. This is the T13 (ii) architecture choice.
    composeFacilityBowtiesMock.mockImplementation(async (facilityId) => {
      expect(facilityId).toBe('f-block-5');
      return composeOpsFor(2);
    });

    expect(effectiveLayoutStore.facilityStatus('f-block-5')).toBe('Wired');

    await orch.removeFromSlot({ facilityId: 'f-block-5', slotLabel: 'input', channelId: 'ch-bod-1' });
    await tick();

    // AC(f) — metadata rows disappear (pending delete).
    expect(bowtieMetadataStore.bowtiesForFacility('f-block-5')).toEqual([]);
    expect(bowtieMetadataStore.hasPendingDeletion(BOD_A1_OCCUPIED_HEX)).toBe(true);
    expect(bowtieMetadataStore.hasPendingDeletion(BOD_A1_CLEAR_HEX)).toBe(true);

    // Fresh event IDs land on the consumer leaves — the producer's event IDs
    // stop flowing to the LED.
    const drafts = configChangesStore.draftEntries();
    const consumerEdits = drafts.filter((d) => d.key.startsWith(SIGNAL_NODE_KEY + ':'));
    expect(consumerEdits).toHaveLength(2);
    for (const edit of consumerEdits) {
      if (edit.value.type !== 'eventId') throw new Error('expected eventId edit');
      expect(edit.value.hex).not.toBe(BOD_A1_OCCUPIED_HEX);
      expect(edit.value.hex).not.toBe(BOD_A1_CLEAR_HEX);
    }

    // Facility falls back to Incomplete.
    expect(effectiveLayoutStore.facilityStatus('f-block-5')).toBe('Incomplete');
    // BOD channel is back to unbound.
    expect(effectiveLayoutStore.channelUsageMap.has('ch-bod-1')).toBe(false);
  });

  it('AC(g): save → close → reopen round-trips the composed bowties + createdByFacility', async () => {
    composeFacilityBowtiesMock.mockImplementation(async () => composeOpsFor(1));
    mountPanel();

    await orch.addChannelForSlot({
      facilityId: 'f-block-5',
      slotLabel: 'output',
      lampRowNodeKey: SIGNAL_NODE_KEY,
      rowOrdinal: 1,
    });
    await tick();

    expect(effectiveLayoutStore.facilityStatus('f-block-5')).toBe('Wired');
    const composed = bowtieMetadataStore.bowtiesForFacility('f-block-5');
    expect(composed).toHaveLength(2);

    // Simulate save: emit deltas + verify the createBowtie deltas carry the
    // createdByFacility back-reference.
    const deltas = bowtieMetadataStore.collectDeltas();
    const createDeltas = deltas.filter((d) => d.type === 'createBowtie');
    expect(createDeltas).toHaveLength(2);
    for (const d of createDeltas) {
      if (d.type !== 'createBowtie') continue;
      expect(d.createdByFacility).toBe('f-block-5');
    }

    // Simulate reopen: reset stores + reload layout with the composed
    // bowties persisted in the layout file's `bowties` map.
    bowtieMetadataStore.clearAll();
    configChangesStore.clearAllDrafts();

    // Emulate the reloaded LayoutFile.bowties map (mirrors what `load_layout`
    // returns after backend serde round-trip): each entry carries the
    // `createdByFacility` back-reference.
    const { layoutStore } = await import('$lib/stores/layout.svelte');
    layoutStore.updateLayout({
      schemaVersion: '2.0',
      bowties: {
        [BOD_A1_OCCUPIED_HEX]: {
          name: 'Block 5 — lit',
          tags: [],
          createdByFacility: 'f-block-5',
        },
        [BOD_A1_CLEAR_HEX]: {
          name: 'Block 5 — unlit',
          tags: [],
          createdByFacility: 'f-block-5',
        },
      },
      roleClassifications: {},
    });

    // `bowtiesForFacility` must consult the effective view: baseline layout
    // + pending edits. With no pending edits, it should return the two
    // reloaded ids.
    const reloaded = bowtieMetadataStore.bowtiesForFacility('f-block-5');
    expect(new Set(reloaded)).toEqual(
      new Set([BOD_A1_OCCUPIED_HEX, BOD_A1_CLEAR_HEX]),
    );
    // Facility is Wired again because the slot bindings are still hydrated.
    expect(effectiveLayoutStore.facilityStatus('f-block-5')).toBe('Wired');
  });
});
