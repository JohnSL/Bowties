/**
 * Spec 018 / S4 tests for `facilityOrchestrator`.
 *
 * Drives the real `facilitiesStore` / `channelsStore` / `behaviorTemplatesStore`
 * singletons with mocked IPC: the orchestrator is the only seam under test,
 * so its role-validation + attach contract is verified at the store
 * mutation level. Rebind was retired in S6 (2026-07-01) — see slice 018-S6 D4.
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
vi.mock('$lib/api/channels', async () => ({
  listChannels: async () => [] as InformationChannel[],
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const orch = await import('$lib/orchestration/facilityOrchestrator');

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
    name: `BOD A${input}`,
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input },
  };
}

function lamp(): InformationChannel {
  return {
    id: 'ch-lamp-1',
    name: 'Lamp 1',
    role: 'lamp-indicator',
    style: 'single-led-direct-lamp',
    ownership: 'user-owned',
    binding: { kind: 'lampRow', nodeKey: 'N2', rowOrdinal: 1 },
  };
}

beforeEach(async () => {
  facilitiesStore.reset();
  channelsStore.reset();
  behaviorTemplatesStore.reset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
  channelsStore.hydrateBaseline([bod(1), bod(2), lamp()]);
  facilitiesStore.hydrateBaseline([
    { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5', slotBindings: { input: [], output: [] } },
  ]);
});

describe('selectChannelForSlot', () => {
  it('attaches the channel when the role matches', () => {
    orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-1',
    });
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-bod-1']);
  });

  it('throws RoleMismatchError when the channel role does not match the slot', () => {
    expect(() => orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-lamp-1',
    })).toThrow(orch.RoleMismatchError);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
  });

  it('throws UnknownReferenceError when the channel id is unknown', () => {
    expect(() => orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'nope',
    })).toThrow(orch.UnknownReferenceError);
  });

  it('rejects a second attach into an already-filled max=1 slot (S4 D8 cardinality)', () => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5', slotBindings: { input: ['ch-bod-1'], output: [] } },
    ]);
    // Post-Rebind-retirement: to swap a channel the user must Remove first,
    // then Select. Attempting to attach a second channel into a max=1 slot
    // is rejected by the orchestrator's cardinality guard.
    expect(() => orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-2',
    })).toThrow(orch.SlotAtMaxError);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-bod-1']);
  });
});

describe('removeFromSlot', () => {
  it('detaches the channel; no-op when already absent', async () => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5', slotBindings: { input: ['ch-bod-1'], output: [] } },
    ]);
    await orch.removeFromSlot({ facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-1' });
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    // Second call is a no-op (does not throw).
    await orch.removeFromSlot({ facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-1' });
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
  });
});

// ── Spec 018 / S6 (T13) — composeBowtiesIfWired / tearDownFacilityBowties ──

import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';

// Mock the compose IPC so T13 tests exercise orchestrator dispatch only.
vi.mock('$lib/api/facilityBowties', () => ({
  composeFacilityBowties: vi.fn(),
}));
// Spec 018 / S6 bugfix — orchestrator now mirrors drafts through the
// LayoutState.drafts seam before every compose IPC. Mock the two
// draft-sync IPCs so orchestrator tests stay decoupled from Tauri.
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: vi.fn(async (_deltas: unknown) => undefined),
  clearLayoutDrafts: vi.fn(async () => undefined),
}));
const { composeFacilityBowties } = await import('$lib/api/facilityBowties');
const { syncLayoutDrafts } = await import('$lib/api/layout');

// Stub the node-tree store lookup: tearDownFacilityBowties needs to resolve
// consumer leaves in the tree so it can write fresh event IDs onto them.
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';

const CONSUMER_NODE_KEY = '05010101FF000002';

function stubConsumerTree() {
  vi.spyOn(nodeTreeStore, 'getTree').mockImplementation((k) => {
    if (k !== CONSUMER_NODE_KEY) return undefined;
    return {
      nodeId: CONSUMER_NODE_KEY,
      identity: null,
      connectorProfile: null,
      profileApplied: true,
      unknownVariants: [],
      segments: [
        {
          name: 'Direct Lamp Control',
          description: null,
          origin: 0,
          space: 253,
          children: [
            {
              kind: 'leaf',
              name: 'Lamp On',
              description: null,
              elementType: 'eventId',
              address: 100,
              size: 8,
              space: 253,
              path: ['Direct Lamp Control', 'Lamp #2', 'Lamp On'],
              value: { type: 'eventId', bytes: [0,0,0,0,0,0,0,0], hex: '0000000000000000' },
              eventRole: 'Consumer',
              constraints: null,
              readOnly: false,
            },
            {
              kind: 'leaf',
              name: 'Lamp Off',
              description: null,
              elementType: 'eventId',
              address: 108,
              size: 8,
              space: 253,
              path: ['Direct Lamp Control', 'Lamp #2', 'Lamp Off'],
              value: { type: 'eventId', bytes: [0,0,0,0,0,0,0,0], hex: '0000000000000000' },
              eventRole: 'Consumer',
              constraints: null,
              readOnly: false,
            },
          ],
        },
      ],
    } as unknown as ReturnType<typeof nodeTreeStore.getTree>;
  });
}

describe('composeBowtiesIfWired (Spec 018 / S6 — D2)', () => {
  beforeEach(() => {
    bowtieMetadataStore.clearAll();
    configChangesStore.clearAllDrafts();
    vi.mocked(composeFacilityBowties).mockReset();
    vi.mocked(composeFacilityBowties).mockResolvedValue([]);
    vi.mocked(syncLayoutDrafts).mockClear();
  });

  it('is a no-op when the facility is Incomplete', async () => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [] } },
    ]);
    await orch.composeBowtiesIfWired('f-1');
    expect(composeFacilityBowties).not.toHaveBeenCalled();
  });

  it('applies each op via configEditor + creates matching metadata rows when Wired', async () => {
    // Wire the facility so the guard admits.
    channelsStore.hydrateBaseline([bod(1), {
      id: 'ch-lamp-2', name: 'Lamp 2', role: 'lamp-indicator', style: 'single-led-direct-lamp',
      ownership: 'user-owned',
      binding: { kind: 'lampRow', nodeKey: CONSUMER_NODE_KEY, rowOrdinal: 2 },
    }]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] } },
    ]);
    vi.mocked(composeFacilityBowties).mockResolvedValue([
      {
        consumerNodeKey: CONSUMER_NODE_KEY,
        consumerLeafPath: ['Direct Lamp Control', 'Lamp #2', 'Lamp On'],
        consumerLeafSpace: 253,
        consumerLeafAddress: 100,
        eventIdBytes: [2,1,1,1,0xff,1,0,1],
        bowtieName: 'Block 5 — lit',
        createdByFacility: 'f-1',
      },
      {
        consumerNodeKey: CONSUMER_NODE_KEY,
        consumerLeafPath: ['Direct Lamp Control', 'Lamp #2', 'Lamp Off'],
        consumerLeafSpace: 253,
        consumerLeafAddress: 108,
        eventIdBytes: [2,1,1,1,0xff,1,1,1],
        bowtieName: 'Block 5 — unlit',
        createdByFacility: 'f-1',
      },
    ]);

    await orch.composeBowtiesIfWired('f-1');

    expect(composeFacilityBowties).toHaveBeenCalledWith('f-1');
    // Two consumer leaf drafts appear in configChangesStore.
    const drafts = configChangesStore.draftEntries();
    const consumerDrafts = drafts.filter((d) => d.key.startsWith(CONSUMER_NODE_KEY + ':'));
    expect(consumerDrafts).toHaveLength(2);
    // Two metadata rows appear for the facility.
    expect(bowtieMetadataStore.bowtiesForFacility('f-1')).toHaveLength(2);
  });

  // Spec 018 / S6 bugfix — regression test.
  //
  // The IPC-side symptom that shipped in S6 was that
  // `composeFacilityBowties` read facilities/channels from
  // `LayoutState.saved` (an empty view for a just-created draft
  // facility) and returned `unknown facility`, so no bowties composed.
  // The fix: `composeBowtiesIfWired` mirrors the frontend's current
  // draft set into `LayoutState.drafts` via `syncLayoutDrafts` BEFORE
  // calling the compose IPC, so the backend reads through
  // `effective_facilities()` / `effective_channels()` and sees the
  // pending edits. This test pins that ordering.
  it('syncs frontend drafts into LayoutState before invoking the compose IPC', async () => {
    channelsStore.hydrateBaseline([bod(1), {
      id: 'ch-lamp-2', name: 'Lamp 2', role: 'lamp-indicator', style: 'single-led-direct-lamp',
      ownership: 'user-owned',
      binding: { kind: 'lampRow', nodeKey: CONSUMER_NODE_KEY, rowOrdinal: 2 },
    }]);
    facilitiesStore.hydrateBaseline([]);
    // A fresh draft facility that only lives on the frontend.
    facilitiesStore.addFacility(
      { templateId: 'block-indicator', displayName: 'Block Indicator', slots: [
        { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
        { label: 'output', displayLabel: 'indicator', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
      ], mapping: [] },
      'Block 5',
    );
    const facilityId = facilitiesStore.pendingCreations[0].facilityId;
    // Fill both slots so the facility is Wired.
    facilitiesStore.attachChannel(facilityId, 'input', 'ch-bod-1');
    facilitiesStore.attachChannel(facilityId, 'output', 'ch-lamp-2');

    vi.mocked(composeFacilityBowties).mockResolvedValue([]);

    await orch.composeBowtiesIfWired(facilityId);

    // The IPC must have been called AND syncLayoutDrafts must have run
    // first, with the addFacility + attachChannel drafts included.
    expect(syncLayoutDrafts).toHaveBeenCalledTimes(1);
    expect(composeFacilityBowties).toHaveBeenCalledTimes(1);
    const syncCallOrder = vi.mocked(syncLayoutDrafts).mock.invocationCallOrder[0];
    const composeCallOrder = vi.mocked(composeFacilityBowties).mock.invocationCallOrder[0];
    expect(syncCallOrder).toBeLessThan(composeCallOrder);

    const [syncedDeltas] = vi.mocked(syncLayoutDrafts).mock.calls[0] as [unknown[]];
    // For a pending-creation facility, `facilitiesStore.collectDeltas()`
    // bakes the slot bindings into the `AddFacility` delta (attach/detach
    // deltas only fire against baseline facilities). What matters is that
    // the composition backend sees the fully-bound facility.
    const addFacility = (syncedDeltas as Array<{ type: string; facility?: unknown }>)
      .find((d) => d.type === 'addFacility');
    expect(addFacility).toBeDefined();
    const facility = (addFacility as { facility: { slotBindings: Record<string, string[]> } })
      .facility;
    expect(facility.slotBindings.input).toEqual(['ch-bod-1']);
    expect(facility.slotBindings.output).toEqual(['ch-lamp-2']);
  });
});

describe('tearDownFacilityBowties (Spec 018 / S6 — T13)', () => {
  beforeEach(() => {
    bowtieMetadataStore.clearAll();
    configChangesStore.clearAllDrafts();
    vi.mocked(composeFacilityBowties).mockReset();
    stubConsumerTree();
  });

  it('re-derives ops on the still-Wired facility, writes fresh event IDs, drops metadata rows', async () => {
    channelsStore.hydrateBaseline([bod(1), {
      id: 'ch-lamp-2', name: 'Lamp 2', role: 'lamp-indicator', style: 'single-led-direct-lamp',
      ownership: 'user-owned',
      binding: { kind: 'lampRow', nodeKey: CONSUMER_NODE_KEY, rowOrdinal: 2 },
    }]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] } },
    ]);
    // Seed metadata rows as if compose had already run.
    bowtieMetadataStore.createBowtie('0201010101010101', 'Block 5 — lit', { createdByFacility: 'f-1' });
    bowtieMetadataStore.createBowtie('0201010101010102', 'Block 5 — unlit', { createdByFacility: 'f-1' });

    vi.mocked(composeFacilityBowties).mockResolvedValue([
      {
        consumerNodeKey: CONSUMER_NODE_KEY,
        consumerLeafPath: ['Direct Lamp Control', 'Lamp #2', 'Lamp On'],
        consumerLeafSpace: 253,
        consumerLeafAddress: 100,
        eventIdBytes: [2,1,1,1,0xff,1,0,1],
        bowtieName: 'Block 5 — lit',
        createdByFacility: 'f-1',
      },
      {
        consumerNodeKey: CONSUMER_NODE_KEY,
        consumerLeafPath: ['Direct Lamp Control', 'Lamp #2', 'Lamp Off'],
        consumerLeafSpace: 253,
        consumerLeafAddress: 108,
        eventIdBytes: [2,1,1,1,0xff,1,1,1],
        bowtieName: 'Block 5 — unlit',
        createdByFacility: 'f-1',
      },
    ]);

    await orch.tearDownFacilityBowties('f-1');

    expect(bowtieMetadataStore.bowtiesForFacility('f-1')).toEqual([]);
    const consumerDrafts = configChangesStore.draftEntries()
      .filter((d) => d.key.startsWith(CONSUMER_NODE_KEY + ':'));
    expect(consumerDrafts).toHaveLength(2);
    // Fresh event IDs — must NOT match the producer's adopted event IDs.
    for (const d of consumerDrafts) {
      if (d.value.type !== 'eventId') throw new Error('expected eventId');
      expect(d.value.hex).not.toBe('0201010101010101');
      expect(d.value.hex).not.toBe('0201010101010102');
    }
  });

  it('is a no-op when the facility is Incomplete and has no metadata rows', async () => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: [], output: [] } },
    ]);
    await orch.tearDownFacilityBowties('f-1');
    expect(composeFacilityBowties).not.toHaveBeenCalled();
  });

  // 2026-07-03 — consolidated teardown reversal (ADR-0012 extension).
  //
  // When a facility is already Incomplete at teardown time (either because a
  // cascade detached its channel first, or because a ghost binding was
  // repaired at load), the composer path cannot re-derive consumer leaves.
  // The metadata-driven fallback searches loaded config trees for EventID
  // leaves whose value equals a `createdByFacility` metadata event id and
  // resets each to a fresh id, so save+reopen no longer resurrects the
  // composed bowtie via CDI-scan auto-catalog.
  it('resets consumer leaves matching metadata event IDs when the facility is Incomplete (metadata fallback)', async () => {
    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: [] } }, // output empty → Incomplete
    ]);

    const composedHexA = '0201010101010101';
    const composedHexB = '0201010101010102';
    bowtieMetadataStore.createBowtie(composedHexA, 'Block 5 — lit', { createdByFacility: 'f-1' });
    bowtieMetadataStore.createBowtie(composedHexB, 'Block 5 — unlit', { createdByFacility: 'f-1' });

    // Two consumer leaves still hold the previously-composed event ids.
    nodeTreeStore.setTree(CONSUMER_NODE_KEY, {
      nodeId: CONSUMER_NODE_KEY,
      identity: null,
      connectorProfile: null,
      profileApplied: true,
      unknownVariants: [],
      segments: [{
        name: 'Direct Lamp Control',
        description: null,
        origin: 0,
        space: 253,
        children: [
          {
            kind: 'leaf', name: 'Lamp On', description: null,
            elementType: 'eventId', address: 100, size: 8, space: 253,
            path: ['Direct Lamp Control', 'Lamp #2', 'Lamp On'],
            value: { type: 'eventId', bytes: [0x02,0x01,0x01,0x01,0x01,0x01,0x01,0x01], hex: composedHexA },
            eventRole: 'Consumer', constraints: null, readOnly: false,
          },
          {
            kind: 'leaf', name: 'Lamp Off', description: null,
            elementType: 'eventId', address: 108, size: 8, space: 253,
            path: ['Direct Lamp Control', 'Lamp #2', 'Lamp Off'],
            value: { type: 'eventId', bytes: [0x02,0x01,0x01,0x01,0x01,0x01,0x01,0x02], hex: composedHexB },
            eventRole: 'Consumer', constraints: null, readOnly: false,
          },
        ],
      }],
    } as unknown as Parameters<typeof nodeTreeStore.setTree>[1]);

    await orch.tearDownFacilityBowties('f-1');

    // Composer must NOT be called — facility is Incomplete so the fallback runs.
    expect(composeFacilityBowties).not.toHaveBeenCalled();

    // Both consumer leaves have staged draft edits with fresh (different) event ids.
    const drafts = configChangesStore.draftEntries()
      .filter((d) => d.key.startsWith(CONSUMER_NODE_KEY + ':'));
    expect(drafts).toHaveLength(2);
    for (const d of drafts) {
      if (d.value.type !== 'eventId') throw new Error('expected eventId draft');
      expect(d.value.hex).not.toBe(composedHexA);
      expect(d.value.hex).not.toBe(composedHexB);
    }

    // Metadata rows are pending deletion for the same facility.
    expect(bowtieMetadataStore.bowtiesForFacility('f-1')).toEqual([]);
  });

  it('does not reset leaves whose event ids do not match any metadata for this facility', async () => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: [], output: [] } },
    ]);
    bowtieMetadataStore.createBowtie('0201010101010101', 'x', { createdByFacility: 'f-1' });

    // Tree holds an UNRELATED event id — should NOT be reset.
    nodeTreeStore.setTree(CONSUMER_NODE_KEY, {
      nodeId: CONSUMER_NODE_KEY,
      identity: null,
      connectorProfile: null,
      profileApplied: true,
      unknownVariants: [],
      segments: [{
        name: 'S', description: null, origin: 0, space: 253,
        children: [{
          kind: 'leaf', name: 'Other', description: null,
          elementType: 'eventId', address: 200, size: 8, space: 253,
          path: ['Other'],
          value: { type: 'eventId', bytes: [0xff,0,0,0,0,0,0,0xaa], hex: 'ff000000000000aa' },
          eventRole: 'Consumer', constraints: null, readOnly: false,
        }],
      }],
    } as unknown as Parameters<typeof nodeTreeStore.setTree>[1]);

    await orch.tearDownFacilityBowties('f-1');

    const drafts = configChangesStore.draftEntries()
      .filter((d) => d.key.startsWith(CONSUMER_NODE_KEY + ':'));
    expect(drafts).toEqual([]);
  });
});

describe('removeFromSlot triggers teardown BEFORE detach (Spec 018 / S6 — T13)', () => {
  beforeEach(() => {
    bowtieMetadataStore.clearAll();
    configChangesStore.clearAllDrafts();
    vi.mocked(composeFacilityBowties).mockReset();
    stubConsumerTree();
  });

  it('composeFacilityBowties runs on the still-Wired shape', async () => {
    channelsStore.hydrateBaseline([bod(1), {
      id: 'ch-lamp-2', name: 'Lamp 2', role: 'lamp-indicator', style: 'single-led-direct-lamp',
      ownership: 'user-owned',
      binding: { kind: 'lampRow', nodeKey: CONSUMER_NODE_KEY, rowOrdinal: 2 },
    }]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] } },
    ]);
    vi.mocked(composeFacilityBowties).mockImplementation(async () => {
      // At the moment the composer runs, the facility must still be Wired.
      const bindings = facilitiesStore.facilities.find((f) => f.facilityId === 'f-1')!.slotBindings;
      expect(bindings.input).toEqual(['ch-bod-1']);
      expect(bindings.output).toEqual(['ch-lamp-2']);
      return [];
    });

    await orch.removeFromSlot({ facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-1' });

    // Now the detach has landed.
    const post = facilitiesStore.facilities.find((f) => f.facilityId === 'f-1')!.slotBindings;
    expect(post.input).toEqual([]);
    expect(composeFacilityBowties).toHaveBeenCalledOnce();
  });
});

describe('deleteFacility (Spec 018 / S6 — D2 wrapper)', () => {
  it('tears down bowties then deletes the facility', async () => {
    channelsStore.hydrateBaseline([bod(1), {
      id: 'ch-lamp-2', name: 'Lamp 2', role: 'lamp-indicator', style: 'single-led-direct-lamp',
      ownership: 'user-owned',
      binding: { kind: 'lampRow', nodeKey: CONSUMER_NODE_KEY, rowOrdinal: 2 },
    }]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-2'] } },
    ]);
    vi.mocked(composeFacilityBowties).mockResolvedValue([]);

    await orch.deleteFacility('f-1');

    expect(facilitiesStore.facilities.some((f) => f.facilityId === 'f-1')).toBe(false);
  });

  it('removes user-owned channels bound to facility slots', async () => {
    channelsStore.hydrateBaseline([bod(1), lamp()]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-1'] } },
    ]);
    vi.mocked(composeFacilityBowties).mockResolvedValue([]);
    const removeSpy = vi.spyOn(channelsStore, 'removeUserOwnedChannel');

    await orch.deleteFacility('f-1');

    expect(removeSpy).toHaveBeenCalledWith('ch-lamp-1');
    expect(facilitiesStore.facilities.some((f) => f.facilityId === 'f-1')).toBe(false);
    removeSpy.mockRestore();
  });

  it('does NOT delete hardware-owned channels bound to facility slots', async () => {
    channelsStore.hydrateBaseline([bod(1), lamp()]);
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
        slotBindings: { input: ['ch-bod-1'], output: ['ch-lamp-1'] } },
    ]);
    vi.mocked(composeFacilityBowties).mockResolvedValue([]);
    const removeSpy = vi.spyOn(channelsStore, 'removeUserOwnedChannel');

    await orch.deleteFacility('f-1');

    expect(removeSpy).not.toHaveBeenCalledWith('ch-bod-1');
    expect(facilitiesStore.facilities.some((f) => f.facilityId === 'f-1')).toBe(false);
    removeSpy.mockRestore();
  });
});

// ── Spec 020 / E1a — createBlockSignalsFacility (facilityOrchestrator owns it) ──

import type { ComposeBlockSignalsRequest } from '$lib/api/signaling';
import { configEditor } from '$lib/stores/configEditor.svelte';
import { effectiveNodeStore } from '$lib/layout/effectiveNodeStore.svelte';
import { editKeyForLeaf } from '$lib/utils/editKey';

vi.mock('$lib/api/signaling', () => ({
  composeBlockSignals: vi.fn(),
  composeBlockSignalsRemoval: vi.fn(),
}));
const { composeBlockSignals, composeBlockSignalsRemoval } = await import('$lib/api/signaling');

vi.mock('$lib/api/sync', () => ({
  listPendingChanges: vi.fn(async () => []),
  revertPendingChange: vi.fn(),
  setPendingChange: vi.fn(async () => 'row-id'),
}));
const { setPendingChange } = await import('$lib/api/sync');
const { pendingChangesStore } = await import('$lib/stores/pendingChanges.svelte');
const { stageConfigDraftsAsPendingChanges } = await import('$lib/orchestration/configDraftOrchestrator');

/** The origin each Save would record, by field address (the IPC payload's offset). */
async function savedOrigins(): Promise<Record<string, unknown>> {
  pendingChangesStore.clear();
  vi.mocked(setPendingChange).mockClear();
  stageConfigDraftsAsPendingChanges();
  await pendingChangesStore.recordDraftsToBackend();
  return Object.fromEntries(
    vi.mocked(setPendingChange).mock.calls.map(([row]) => [row.offset, row.origin]),
  );
}

const ABS_TEMPLATE: BehaviorTemplate = {
  templateId: 'automatic-block-signals',
  displayName: 'Automatic Block Signals',
  slots: [
    { label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 0, maxChannels: null },
  ],
  mapping: [],
};

const ABS_NODE = '050101018800';
const OTHER_NODE = '050101018899';

function absArgs(): Parameters<typeof orch.createBlockSignalsFacility>[0] {
  return {
    facilityId: 'abs-1',
    name: 'Main line',
    line: { facilityId: 'abs-1', facilityName: 'Main line' } as never,
    wiring: [{ signalId: 's1', nodeKey: ABS_NODE, redOutput: 1, greenOutput: 2 }],
    signals: [{ signalId: 's1', name: 'Signal A', protectedBlock: 'b1' }],
  };
}

const COMPOSED = {
  edits: [
    { nodeKey: ABS_NODE, space: 253, address: 10, path: 'a', value: { type: 'int', value: 1 } },
    { nodeKey: ABS_NODE, space: 253, address: 20, path: 'b', value: { type: 'int', value: 2 } },
  ],
  signalResources: [{ signalId: 's1', resources: { nodeKey: ABS_NODE, mast: 1, trackCircuits: [] } }],
  logicPlacement: [{ nodeKey: ABS_NODE, groups: [{ facilityId: 'abs-1', signalId: 's1', first: 1, length: 1 }] }],
  issues: [],
};

const SAVED_PLACEMENT = {
  nodeKey: ABS_NODE,
  groups: [{ facilityId: 'abs-1', signalId: 's1', first: 5, length: 1 }],
};

describe('createBlockSignalsFacility (Spec 020 / E1a — facilityOrchestrator)', () => {
  beforeEach(async () => {
    vi.restoreAllMocks();
    bowtieMetadataStore.clearAll();
    configChangesStore.clearAllDrafts();
    facilitiesStore.hydrateBaseline([]);
    channelsStore.hydrateBaseline([]);
    vi.mocked(composeFacilityBowties).mockReset();
    vi.mocked(syncLayoutDrafts).mockClear();
    vi.mocked(composeBlockSignals).mockReset();
    vi.mocked(composeBlockSignals).mockResolvedValue(COMPOSED as never);
    listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR, ABS_TEMPLATE]);
    behaviorTemplatesStore.reset();
    await behaviorTemplatesStore.loadBehaviorTemplates();
  });

  it('stages facility, record, channel, attach and edits, dirtying the Save toolbar facade', async () => {
    expect(effectiveNodeStore.isDirty).toBe(false);
    const result = await orch.createBlockSignalsFacility(absArgs());

    expect(result).toEqual({ ok: true });
    const facility = facilitiesStore.facilities.find((f) => f.facilityId === 'abs-1')!;
    const channel = channelsStore.channels.find((c) => c.name === 'Signal A')!;
    expect(channel).toMatchObject({
      role: 'signal-indication',
      style: 'two-driver-rg-signal',
      ownership: 'user-owned',
      binding: { kind: 'signalLccRgOutputs', nodeKey: ABS_NODE, redOutput: 1, greenOutput: 2 },
    });
    expect(facility.slotBindings.signals).toEqual([channel.id]);
    expect(facility.blockSignals?.signals).toEqual([
      { signalId: 's1', name: 'Signal A', protectedBlock: 'b1', channelId: channel.id, resources: COMPOSED.signalResources[0].resources },
    ]);
    expect(facilitiesStore.logicPlacement).toEqual(COMPOSED.logicPlacement);
    expect(facilitiesStore.collectDeltas()).toContainEqual({ type: 'setLogicPlacement', ...COMPOSED.logicPlacement[0] });
    expect(configChangesStore.draftEntries().map((d) => d.key)).toEqual([
      editKeyForLeaf(ABS_NODE, 253, 10),
      editKeyForLeaf(ABS_NODE, 253, 20),
    ]);
    expect(effectiveNodeStore.isDirty).toBe(true);
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ config: 2, channels: 1, facilities: 1 });
    expect(composeFacilityBowties).not.toHaveBeenCalled();
  });

  it('records the staged edits as Pending Changes that carry the facility that staged them', async () => {
    await orch.createBlockSignalsFacility(absArgs());

    expect(await savedOrigins()).toEqual({
      '0x0000000A': { facilityId: 'abs-1' },
      '0x00000014': { facilityId: 'abs-1' },
    });
  });

  it('records a field the user then edits by hand with no origin', async () => {
    await orch.createBlockSignalsFacility(absArgs());
    configEditor.applyEdit(editKeyForLeaf(ABS_NODE, 253, 10), { type: 'int', value: 5 });

    expect(await savedOrigins()).toEqual({
      '0x0000000A': undefined,
      '0x00000014': { facilityId: 'abs-1' },
    });
  });

  it('syncs every unsaved config edit with the drafts before composing, not in the compose request', async () => {
    configEditor.applyEdit(editKeyForLeaf(ABS_NODE, 253, 99), { type: 'int', value: 7 });
    configEditor.applyEdit(editKeyForLeaf(OTHER_NODE, 253, 98), { type: 'int', value: 8 });

    await orch.createBlockSignalsFacility(absArgs());

    const request = vi.mocked(composeBlockSignals).mock.calls[0][0] as ComposeBlockSignalsRequest;
    expect(request.facilityId).toBe('abs-1');
    expect(request).not.toHaveProperty('pendingEdits');
    const [, configDrafts] = vi.mocked(syncLayoutDrafts).mock.calls[0] as [unknown, unknown];
    expect(configDrafts).toEqual([
      { nodeKey: ABS_NODE, space: 253, address: 99, value: { type: 'int', value: 7 } },
      { nodeKey: OTHER_NODE, space: 253, address: 98, value: { type: 'int', value: 8 } },
    ]);
    expect(vi.mocked(syncLayoutDrafts).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(composeBlockSignals).mock.invocationCallOrder[0]);
  });

  it('stages nothing when compose reports issues', async () => {
    const issues = [{ code: 'outputInUse', signalId: 's1', nodeKey: ABS_NODE, detail: 'busy' }];
    vi.mocked(composeBlockSignals).mockResolvedValue({ edits: [], signalResources: [], logicPlacement: [], issues } as never);

    const result = await orch.createBlockSignalsFacility(absArgs());

    expect(result).toEqual({ ok: false, issues });
    expect(facilitiesStore.pendingCreations).toHaveLength(0);
    expect(channelsStore.channels).toHaveLength(0);
    expect(effectiveNodeStore.isDirty).toBe(false);
  });

  it('rolls everything back when staging an edit fails', async () => {
    configEditor.applyEdit(editKeyForLeaf(ABS_NODE, 253, 10), { type: 'int', value: 5 });
    const realApply = configEditor.applyEdit.bind(configEditor);
    let calls = 0;
    vi.spyOn(configEditor, 'applyEdit').mockImplementation((k, v) => {
      if (++calls === 2) throw new Error('boom');
      realApply(k, v);
    });

    const result = await orch.createBlockSignalsFacility(absArgs());

    expect(result).toMatchObject({ ok: false });
    expect((result as { error: unknown }).error).toBeInstanceOf(Error);
    expect(facilitiesStore.pendingCreations).toHaveLength(0);
    expect(channelsStore.channels).toHaveLength(0);
    // The unrelated pre-existing hand edit is restored, the staged one removed.
    expect(configChangesStore.draftEntries()).toEqual([
      { key: editKeyForLeaf(ABS_NODE, 253, 10), value: { type: 'int', value: 5 } },
    ]);
  });

  it('leaves no placement staged when staging an edit fails', async () => {
    const realApply = configEditor.applyEdit.bind(configEditor);
    let calls = 0;
    vi.spyOn(configEditor, 'applyEdit').mockImplementation((k, v) => {
      if (++calls === 2) throw new Error('boom');
      realApply(k, v);
    });

    const result = await orch.createBlockSignalsFacility(absArgs());

    expect(result).toMatchObject({ ok: false });
    expect(facilitiesStore.logicPlacement).toEqual([]);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
  });

  it('leaves the facade clean after a failed staging with no prior edits', async () => {
    const realApply = configEditor.applyEdit.bind(configEditor);
    let calls = 0;
    vi.spyOn(configEditor, 'applyEdit').mockImplementation((k, v) => {
      if (++calls === 2) throw new Error('boom');
      realApply(k, v);
    });
    await orch.createBlockSignalsFacility(absArgs());
    expect(effectiveNodeStore.isDirty).toBe(false);
  });
});

// ── Spec 020 / E2a — updateBlockSignalsFacility (shared staging pipeline) ──

describe('updateBlockSignalsFacility (Spec 020 / E2a — facilityOrchestrator)', () => {
  const SAVED_LINE = {
    direction: 'bidirectional',
    movements: 'both',
    entranceEnd: 'a',
    exitEnd: 'b',
    blocks: [{ blockId: 'b1' }],
    settings: { mode: 'old' },
  };
  const NEW_LINE = { ...SAVED_LINE, settings: { mode: 'new' }, facilityId: 'abs-1', facilityName: 'Main line' };

  function savedChannel(): InformationChannel {
    return {
      id: 'ch-sig-1',
      name: 'Signal A',
      role: 'signal-indication',
      style: 'two-driver-rg-signal',
      ownership: 'user-owned',
      binding: { kind: 'signalLccRgOutputs', nodeKey: ABS_NODE, redOutput: 1, greenOutput: 2 },
    } as InformationChannel;
  }

  function updateArgs(over: Partial<Parameters<typeof orch.updateBlockSignalsFacility>[0]> = {}) {
    return {
      facilityId: 'abs-1',
      name: 'Main line',
      line: NEW_LINE as never,
      wiring: [{ signalId: 's1', nodeKey: ABS_NODE, redOutput: 3, greenOutput: 4 }],
      signals: [{ signalId: 's1', name: 'Signal A', protectedBlock: 'b1' }],
      ...over,
    };
  }

  beforeEach(async () => {
    vi.restoreAllMocks();
    configChangesStore.clearAllDrafts();
    vi.mocked(composeBlockSignals).mockReset();
    vi.mocked(composeBlockSignals).mockResolvedValue(COMPOSED as never);
    listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR, ABS_TEMPLATE]);
    behaviorTemplatesStore.reset();
    await behaviorTemplatesStore.loadBehaviorTemplates();
    channelsStore.hydrateBaseline([savedChannel()]);
    facilitiesStore.hydrateBaseline([{
      facilityId: 'abs-1',
      templateId: 'automatic-block-signals',
      name: 'Main line',
      slotBindings: { signals: ['ch-sig-1'] },
      blockSignals: {
        ...SAVED_LINE,
        signals: [{
          signalId: 's1', name: 'Signal A', protectedBlock: 'b1', channelId: 'ch-sig-1',
          resources: { nodeKey: ABS_NODE, mast: 9, trackCircuits: [] },
        }],
      },
    } as never], [SAVED_PLACEMENT]);
  });

  it('replaces the record, rebinds the same channel and stages the compiled edits', async () => {
    const result = await orch.updateBlockSignalsFacility(updateArgs());

    expect(result).toEqual({ ok: true });
    const request = vi.mocked(composeBlockSignals).mock.calls[0][0] as ComposeBlockSignalsRequest;
    expect(request.facilityId).toBe('abs-1');
    const facility = facilitiesStore.facilities.find((f) => f.facilityId === 'abs-1')!;
    expect(facility.blockSignals?.settings).toEqual({ mode: 'new' });
    expect(facility.blockSignals?.signals[0].resources).toEqual(COMPOSED.signalResources[0].resources);
    expect(facilitiesStore.logicPlacement).toEqual(COMPOSED.logicPlacement);
    expect(facility.blockSignals?.signals[0].channelId).toBe('ch-sig-1');
    expect(channelsStore.channels).toHaveLength(1);
    expect(channelsStore.channels[0].binding).toMatchObject({ redOutput: 3, greenOutput: 4 });
    expect(configChangesStore.draftEntries().map((d) => d.key)).toEqual([
      editKeyForLeaf(ABS_NODE, 253, 10),
      editKeyForLeaf(ABS_NODE, 253, 20),
    ]);
  });

  it('renames the facility and a signal channel when their names changed', async () => {
    await orch.updateBlockSignalsFacility(updateArgs({
      name: 'Renamed line',
      signals: [{ signalId: 's1', name: 'Signal Z', protectedBlock: 'b1' }],
    }));

    expect(facilitiesStore.facilities[0].name).toBe('Renamed line');
    expect(channelsStore.channels[0].name).toBe('Signal Z');
    expect(facilitiesStore.facilities[0].blockSignals?.signals[0].name).toBe('Signal Z');
  });

  it('stages no configuration edits for a name-only change', async () => {
    vi.mocked(composeBlockSignals).mockResolvedValue({
      edits: [],
      signalResources: [{ signalId: 's1', resources: { nodeKey: ABS_NODE, mast: 9, trackCircuits: [] } }],
      logicPlacement: [SAVED_PLACEMENT],
      issues: [],
    } as never);

    const result = await orch.updateBlockSignalsFacility(updateArgs({
      name: 'Renamed line',
      line: { ...SAVED_LINE, facilityId: 'abs-1' } as never,
      wiring: [{ signalId: 's1', nodeKey: ABS_NODE, redOutput: 1, greenOutput: 2 }],
    }));

    expect(result).toEqual({ ok: true });
    expect(configChangesStore.draftEntries()).toHaveLength(0);
    expect(facilitiesStore.facilities[0].name).toBe('Renamed line');
    expect(facilitiesStore.collectDeltas().some((d) => d.type === 'setLogicPlacement')).toBe(false);
  });

  it('stages nothing when compose reports issues', async () => {
    const issues = [{ code: 'outputInUse', signalId: 's1', nodeKey: ABS_NODE, detail: 'busy' }];
    vi.mocked(composeBlockSignals).mockResolvedValue({ edits: [], signalResources: [], logicPlacement: [], issues } as never);

    const result = await orch.updateBlockSignalsFacility(updateArgs({ name: 'Renamed line' }));

    expect(result).toEqual({ ok: false, issues });
    expect(effectiveNodeStore.isDirty).toBe(false);
  });

  describe('a signal the edit turns off', () => {
    function hydrateTwoSignals(): void {
      const second = { ...savedChannel(), id: 'ch-sig-2', name: 'Signal B' } as InformationChannel;
      channelsStore.hydrateBaseline([savedChannel(), second]);
      facilitiesStore.hydrateBaseline([{
        facilityId: 'abs-1',
        templateId: 'automatic-block-signals',
        name: 'Main line',
        slotBindings: { signals: ['ch-sig-1', 'ch-sig-2'] },
        blockSignals: {
          ...SAVED_LINE,
          signals: [
            { signalId: 's1', name: 'Signal A', protectedBlock: 'b1', channelId: 'ch-sig-1',
              resources: { nodeKey: ABS_NODE, mast: 9, trackCircuits: [] } },
            { signalId: 's2', name: 'Signal B', protectedBlock: 'b2', channelId: 'ch-sig-2',
              resources: { nodeKey: ABS_NODE, mast: 10, trackCircuits: [] } },
          ],
        },
      } as never], [SAVED_PLACEMENT]);
    }

    it('detaches and removes its channel, so the record and the slot agree', async () => {
      hydrateTwoSignals();

      const result = await orch.updateBlockSignalsFacility(updateArgs());

      expect(result).toEqual({ ok: true });
      const facility = facilitiesStore.facilities[0];
      expect(facility.blockSignals?.signals.map((s) => s.channelId)).toEqual(['ch-sig-1']);
      expect(facility.slotBindings.signals).toEqual(['ch-sig-1']);
      expect(channelsStore.channels.map((c) => c.id)).toEqual(['ch-sig-1']);
    });

    it('is restored by Discard through the draft layer', async () => {
      hydrateTwoSignals();
      await orch.updateBlockSignalsFacility(updateArgs());

      facilitiesStore.discard();
      channelsStore.discard();

      expect(facilitiesStore.facilities[0].slotBindings.signals).toEqual(['ch-sig-1', 'ch-sig-2']);
      expect(facilitiesStore.facilities[0].blockSignals?.signals).toHaveLength(2);
      expect(channelsStore.channels.map((c) => c.id)).toEqual(['ch-sig-1', 'ch-sig-2']);
    });

    it('keeps its channel and attachment when staging fails', async () => {
      hydrateTwoSignals();
      vi.spyOn(configEditor, 'applyEdit').mockImplementation(() => {
        throw new Error('boom');
      });

      const result = await orch.updateBlockSignalsFacility(updateArgs());

      expect(result).toMatchObject({ ok: false });
      expect(facilitiesStore.facilities[0].slotBindings.signals).toEqual(['ch-sig-1', 'ch-sig-2']);
      expect(facilitiesStore.facilities[0].blockSignals?.signals).toHaveLength(2);
      expect(channelsStore.channels.map((c) => c.id)).toEqual(['ch-sig-1', 'ch-sig-2']);
      expect(facilitiesStore.collectDeltas()).toEqual([]);
      expect(channelsStore.collectDeltas()).toEqual([]);
    });
  });

  it('rolls the record, names, binding and drafts back when staging fails', async () => {
    configEditor.applyEdit(editKeyForLeaf(ABS_NODE, 253, 10), { type: 'int', value: 5 });
    const realApply = configEditor.applyEdit.bind(configEditor);
    let calls = 0;
    vi.spyOn(configEditor, 'applyEdit').mockImplementation((k, v) => {
      if (++calls === 2) throw new Error('boom');
      realApply(k, v);
    });

    const result = await orch.updateBlockSignalsFacility(updateArgs({
      name: 'Renamed line',
      signals: [{ signalId: 's1', name: 'Signal Z', protectedBlock: 'b1' }],
    }));

    expect(result).toMatchObject({ ok: false });
    const facility = facilitiesStore.facilities[0];
    expect(facility.name).toBe('Main line');
    expect(facility.blockSignals?.settings).toEqual({ mode: 'old' });
    expect(channelsStore.channels[0].name).toBe('Signal A');
    expect(channelsStore.channels[0].binding).toMatchObject({ redOutput: 1, greenOutput: 2 });
    expect(configChangesStore.draftEntries()).toEqual([
      { key: editKeyForLeaf(ABS_NODE, 253, 10), value: { type: 'int', value: 5 } },
    ]);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
    expect(channelsStore.collectDeltas()).toEqual([]);
  });
});

// ── Spec 020 / E2b — removeBlockSignalsFacility ──

describe('removeBlockSignalsFacility (Spec 020 / E2b — facilityOrchestrator)', () => {
  const RELEASE = {
    edits: [
      { nodeKey: ABS_NODE, space: 253, address: 10, path: 'a', value: { type: 'int', value: 0 } },
      { nodeKey: ABS_NODE, space: 253, address: 20, path: 'b', value: { type: 'string', value: '' } },
    ],
    signalResources: [],
    logicPlacement: [{ nodeKey: ABS_NODE, groups: [] }],
    issues: [],
  };

  function savedChannel(): InformationChannel {
    return {
      id: 'ch-sig-1', name: 'Signal A', role: 'signal-indication', style: 'two-driver-rg-signal',
      ownership: 'user-owned',
      binding: { kind: 'signalLccRgOutputs', nodeKey: ABS_NODE, redOutput: 1, greenOutput: 2 },
    } as InformationChannel;
  }

  beforeEach(async () => {
    vi.restoreAllMocks();
    bowtieMetadataStore.clearAll();
    configChangesStore.clearAllDrafts();
    vi.mocked(syncLayoutDrafts).mockClear();
    vi.mocked(composeBlockSignalsRemoval).mockReset();
    vi.mocked(composeBlockSignalsRemoval).mockResolvedValue(RELEASE as never);
    listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR, ABS_TEMPLATE]);
    behaviorTemplatesStore.reset();
    await behaviorTemplatesStore.loadBehaviorTemplates();
    channelsStore.hydrateBaseline([savedChannel()]);
    facilitiesStore.hydrateBaseline([{
      facilityId: 'abs-1',
      templateId: 'automatic-block-signals',
      name: 'Main line',
      slotBindings: { signals: ['ch-sig-1'] },
      blockSignals: {
        direction: 'bidirectional', movements: 'both', entranceEnd: 'a', exitEnd: 'b',
        blocks: [{ blockId: 'b1' }], settings: {},
        signals: [{
          signalId: 's1', name: 'Signal A', protectedBlock: 'b1', channelId: 'ch-sig-1',
          resources: { nodeKey: ABS_NODE, mast: 9, trackCircuits: [] },
        }],
      },
    } as never], [SAVED_PLACEMENT]);
  });

  it('records the release edits as Pending Changes that carry the facility being removed', async () => {
    await orch.removeBlockSignalsFacility({ facilityId: 'abs-1' });

    // The release's second edit leaves its field unchanged, so only the first is recorded.
    expect(await savedOrigins()).toEqual({ '0x0000000A': { facilityId: 'abs-1' } });
  });

  it('stages the release edits and deletes the facility and its signal channels', async () => {
    const result = await orch.removeBlockSignalsFacility({ facilityId: 'abs-1' });

    expect(result).toEqual({ ok: true });
    expect(vi.mocked(syncLayoutDrafts).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(composeBlockSignalsRemoval).mock.invocationCallOrder[0]);
    expect(facilitiesStore.facilities).toHaveLength(0);
    expect(channelsStore.channels).toHaveLength(0);
    expect(configChangesStore.draftEntries().map((d) => d.key)).toEqual([
      editKeyForLeaf(ABS_NODE, 253, 10),
      editKeyForLeaf(ABS_NODE, 253, 20),
    ]);
    expect(facilitiesStore.collectDeltas()).toEqual([{ type: 'deleteFacility', facilityId: 'abs-1' }]);
    expect(facilitiesStore.logicPlacement).toEqual([]);
  });

  it('keeps the groups of other facilities the release leaves on the board', async () => {
    const other = { facilityId: 'other', signalId: 'x', first: 1, length: 1 };
    facilitiesStore.hydrateBaseline(facilitiesStore.facilities, [
      { nodeKey: ABS_NODE, groups: [other, ...SAVED_PLACEMENT.groups] },
    ]);
    vi.mocked(composeBlockSignalsRemoval).mockResolvedValue(
      { ...RELEASE, logicPlacement: [{ nodeKey: ABS_NODE, groups: [other] }] } as never,
    );

    await orch.removeBlockSignalsFacility({ facilityId: 'abs-1' });

    expect(facilitiesStore.logicPlacement).toEqual([{ nodeKey: ABS_NODE, groups: [other] }]);
    expect(facilitiesStore.collectDeltas()).toEqual([{ type: 'deleteFacility', facilityId: 'abs-1' }]);
  });

  it('stages nothing when the release reports issues', async () => {
    const issues = [{ code: 'claimedSlotUnavailable', nodeKey: ABS_NODE, detail: 'not loaded' }];
    vi.mocked(composeBlockSignalsRemoval).mockResolvedValue({ edits: [], signalResources: [], logicPlacement: [], issues } as never);

    const result = await orch.removeBlockSignalsFacility({ facilityId: 'abs-1' });

    expect(result).toEqual({ ok: false, issues });
    expect(facilitiesStore.facilities).toHaveLength(1);
    expect(channelsStore.channels).toHaveLength(1);
    expect(configChangesStore.draftEntries()).toHaveLength(0);
  });

  it('leaveBoard deletes the facility and channels without composing or staging config edits', async () => {
    const result = await orch.removeBlockSignalsFacility({ facilityId: 'abs-1', leaveBoard: true });

    expect(result).toEqual({ ok: true });
    expect(composeBlockSignalsRemoval).not.toHaveBeenCalled();
    expect(facilitiesStore.facilities).toHaveLength(0);
    expect(channelsStore.channels).toHaveLength(0);
    expect(configChangesStore.draftEntries()).toHaveLength(0);
  });

  it('restores drafts and keeps the facility and channels when staging fails', async () => {
    configEditor.applyEdit(editKeyForLeaf(ABS_NODE, 253, 10), { type: 'int', value: 5 });
    const realApply = configEditor.applyEdit.bind(configEditor);
    let calls = 0;
    vi.spyOn(configEditor, 'applyEdit').mockImplementation((k, v) => {
      if (++calls === 2) throw new Error('boom');
      realApply(k, v);
    });

    const result = await orch.removeBlockSignalsFacility({ facilityId: 'abs-1' });

    expect(result).toMatchObject({ ok: false });
    expect((result as { error: unknown }).error).toBeInstanceOf(Error);
    expect(facilitiesStore.facilities).toHaveLength(1);
    expect(channelsStore.channels).toHaveLength(1);
    expect(configChangesStore.draftEntries()).toEqual([
      { key: editKeyForLeaf(ABS_NODE, 253, 10), value: { type: 'int', value: 5 } },
    ]);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
    expect(channelsStore.collectDeltas()).toEqual([]);
  });

  describe('fresh event IDs', () => {
    const FRESH_RELEASE = {
      edits: [
        { nodeKey: ABS_NODE, space: 253, address: 10, path: 'a', value: { type: 'int', value: 0 } },
        { nodeKey: ABS_NODE, space: 253, address: 30, path: 'e1', value: { type: 'freshEventId' } },
        { nodeKey: ABS_NODE, space: 253, address: 38, path: 'e2', value: { type: 'freshEventId' } },
      ],
      signalResources: [],
      logicPlacement: [],
      issues: [],
    };

    beforeEach(() => {
      vi.mocked(composeBlockSignalsRemoval).mockResolvedValue(FRESH_RELEASE as never);
    });

    it('stages each fresh event ID leaf as a concrete event ID, distinct per leaf', async () => {
      nodeTreeStore.setTree(ABS_NODE, {
        nodeId: ABS_NODE, identity: null, connectorProfile: null, profileApplied: true,
        unknownVariants: [],
        segments: [{
          name: 'Conditionals', description: null, origin: 0, space: 253,
          children: [30, 38].map((address) => ({
            kind: 'leaf', name: `Event ${address}`, description: null,
            elementType: 'eventId', address, size: 8, space: 253,
            path: ['Conditionals', `Event ${address}`],
            value: { type: 'eventId', bytes: [2, 1, 1, 1, 1, 1, 1, 1], hex: '0201010101010101' },
            eventRole: 'Consumer', constraints: null, readOnly: false,
          })),
        }],
      } as unknown as Parameters<typeof nodeTreeStore.setTree>[1]);

      const result = await orch.removeBlockSignalsFacility({ facilityId: 'abs-1' });

      expect(result).toEqual({ ok: true });
      const drafts = new Map(configChangesStore.draftEntries().map((d) => [d.key, d.value]));
      const first = drafts.get(editKeyForLeaf(ABS_NODE, 253, 30));
      const second = drafts.get(editKeyForLeaf(ABS_NODE, 253, 38));
      expect(first).toMatchObject({ type: 'eventId' });
      expect(second).toMatchObject({ type: 'eventId' });
      const hexOf = (v: unknown) => (v as { hex: string }).hex;
      expect(hexOf(first)).toMatch(/^[0-9A-F]{16}$/i);
      expect(hexOf(first)).not.toBe(hexOf(second));
      expect(drafts.get(editKeyForLeaf(ABS_NODE, 253, 10))).toEqual({ type: 'int', value: 0 });
    });

    it('restores drafts and keeps the facility when a node tree is missing', async () => {
      nodeTreeStore.reset();
      configEditor.applyEdit(editKeyForLeaf(ABS_NODE, 253, 10), { type: 'int', value: 5 });

      const result = await orch.removeBlockSignalsFacility({ facilityId: 'abs-1' });

      expect(result).toMatchObject({ ok: false });
      expect(facilitiesStore.facilities).toHaveLength(1);
      expect(channelsStore.channels).toHaveLength(1);
      expect(configChangesStore.draftEntries()).toEqual([
        { key: editKeyForLeaf(ABS_NODE, 253, 10), value: { type: 'int', value: 5 } },
      ]);
    });
  });

  it('generic deleteFacility routes an ABS facility through the release', async () => {
    await orch.deleteFacility('abs-1');

    expect(composeBlockSignalsRemoval).toHaveBeenCalledWith('abs-1');
    expect(facilitiesStore.facilities).toHaveLength(0);
    expect(configChangesStore.draftEntries()).toHaveLength(2);
  });

  it('generic deleteFacility throws and stages nothing when the ABS release is refused', async () => {
    const issues = [{ code: 'claimedSlotUnavailable', nodeKey: ABS_NODE, detail: 'not loaded' }];
    vi.mocked(composeBlockSignalsRemoval).mockResolvedValue({ edits: [], signalResources: [], logicPlacement: [], issues } as never);

    await expect(orch.deleteFacility('abs-1')).rejects.toBeInstanceOf(orch.BlockSignalsRemovalError);
    expect(facilitiesStore.facilities).toHaveLength(1);
  });
});

describe('createBlockSignalsFacility with released owners (Spec 020 / E2c B3)', () => {
  const RELEASE = [{ nodeKey: ABS_NODE, owner: 'Mast 2' }];
  const RELEASED = {
    edits: [
      { nodeKey: ABS_NODE, space: 253, address: 10, path: 'a', value: { type: 'int', value: 1 } },
      { nodeKey: ABS_NODE, space: 253, address: 30, path: 'e1', value: { type: 'freshEventId' } },
      { nodeKey: ABS_NODE, space: 253, address: 38, path: 'e2', value: { type: 'freshEventId' } },
    ],
    signalResources: [{ signalId: 's1', resources: { nodeKey: ABS_NODE, mast: 1, trackCircuits: [] } }],
    logicPlacement: [],
    issues: [],
  };

  function loadTree(): void {
    nodeTreeStore.setTree(ABS_NODE, {
      nodeId: ABS_NODE, identity: null, connectorProfile: null, profileApplied: true,
      unknownVariants: [],
      segments: [{
        name: 'Conditionals', description: null, origin: 0, space: 253,
        children: [30, 38].map((address) => ({
          kind: 'leaf', name: `Event ${address}`, description: null,
          elementType: 'eventId', address, size: 8, space: 253,
          path: ['Conditionals', `Event ${address}`],
          value: { type: 'eventId', bytes: [2, 1, 1, 1, 1, 1, 1, 1], hex: '0201010101010101' },
          eventRole: 'Consumer', constraints: null, readOnly: false,
        })),
      }],
    } as unknown as Parameters<typeof nodeTreeStore.setTree>[1]);
  }

  beforeEach(async () => {
    vi.restoreAllMocks();
    bowtieMetadataStore.clearAll();
    configChangesStore.clearAllDrafts();
    facilitiesStore.hydrateBaseline([]);
    channelsStore.hydrateBaseline([]);
    vi.mocked(composeBlockSignals).mockReset();
    vi.mocked(composeBlockSignals).mockResolvedValue(RELEASED as never);
    listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR, ABS_TEMPLATE]);
    behaviorTemplatesStore.reset();
    await behaviorTemplatesStore.loadBehaviorTemplates();
    nodeTreeStore.reset();
    loadTree();
  });

  it('composes with the released owners and stages their fresh event IDs beside the recipe values', async () => {
    const result = await orch.createBlockSignalsFacility({ ...absArgs(), release: RELEASE });

    expect(result).toEqual({ ok: true });
    expect((vi.mocked(composeBlockSignals).mock.calls[0][0] as ComposeBlockSignalsRequest).release).toEqual(RELEASE);
    const drafts = new Map(configChangesStore.draftEntries().map((d) => [d.key, d.value]));
    const first = drafts.get(editKeyForLeaf(ABS_NODE, 253, 30)) as { type: string; hex: string };
    const second = drafts.get(editKeyForLeaf(ABS_NODE, 253, 38)) as { type: string; hex: string };
    expect(first.type).toBe('eventId');
    expect(first.hex).toMatch(/^050101018800[0-9A-F]{4}$/i);
    expect(first.hex).not.toBe(second.hex);
    expect(drafts.get(editKeyForLeaf(ABS_NODE, 253, 10))).toEqual({ type: 'int', value: 1 });
  });

  it('rolls everything back when a released owner\'s node tree is not loaded', async () => {
    nodeTreeStore.reset();
    configEditor.applyEdit(editKeyForLeaf(ABS_NODE, 253, 10), { type: 'int', value: 5 });

    const result = await orch.createBlockSignalsFacility({ ...absArgs(), release: RELEASE });

    expect(result).toMatchObject({ ok: false });
    expect(facilitiesStore.pendingCreations).toHaveLength(0);
    expect(channelsStore.channels).toHaveLength(0);
    expect(configChangesStore.draftEntries()).toEqual([
      { key: editKeyForLeaf(ABS_NODE, 253, 10), value: { type: 'int', value: 5 } },
    ]);
  });
});
