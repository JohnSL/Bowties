// Spec 018 / S2 integration test for the new channel schema (role / style /
// ownership / binding). Drives the BOD-select auto-create path through the
// orchestrator's pure builder, the draft store, the legacy create/list IPCs,
// and back. Asserts the new schema is preserved end-to-end and that
// `channelType` / `hardwareRef` are GONE (ADR-0013 retirement).

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { InformationChannel } from '$lib/api/channels';
import type {
  ConnectorProfileView,
  ConnectorSelectionDocument,
} from '$lib/types/connectorProfile';

const listChannelsMock = vi.fn<() => Promise<InformationChannel[]>>(async () => []);
const createChannelsMock = vi.fn<(channels: InformationChannel[]) => Promise<InformationChannel[]>>(
  async (channels) => channels,
);
const renameChannelMock = vi.fn<(id: string, newName: string) => Promise<void>>(async () => {});
const deleteChannelsMock = vi.fn<(ids: string[]) => Promise<void>>(async () => {});

vi.mock('$lib/api/channels', async (importOriginal) => {
  const actual = await importOriginal<typeof import('$lib/api/channels')>();
  return {
    ...actual,
    listChannels: listChannelsMock,
    createChannels: createChannelsMock,
    renameChannel: renameChannelMock,
    deleteChannels: deleteChannelsMock,
  };
});

const { channelsStore } = await import('$lib/stores/channels.svelte');
const { buildAutoCreatedChannelsForSlot } = await import(
  '$lib/orchestration/connectorSelectionOrchestrator'
);
const { getStyleEventMapping } = await import('$lib/utils/channelStyles');

function makeBod8Profile(): ConnectorProfileView {
  return {
    nodeId: '05.02.01.02.03.00',
    carrierKey: 'rr-cirkits::tower-lcc',
    slots: [
      {
        slotId: 'connector-a',
        label: 'Connector A',
        order: 0,
        allowNoneInstalled: true,
        supportedDaughterboardIds: ['BOD-8-SM'],
        affectedPaths: [],
        supportedDaughterboardConstraints: [],
      },
    ],
    supportedDaughterboards: [
      {
        daughterboardId: 'BOD-8-SM',
        displayName: 'BOD-8-SM',
        kind: 'detection',
        channelInputs: [
          {
            channelType: 'block-occupancy',
            style: 'bod-block-detector-input',
            inputs: [1, 2, 3, 4, 5, 6, 7, 8],
          },
        ],
      },
    ],
  };
}

function makeBod8Document(): ConnectorSelectionDocument {
  return {
    nodeId: '05.02.01.02.03.00',
    carrierKey: 'rr-cirkits::tower-lcc',
    slotSelections: [
      { slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM', status: 'selected' },
    ],
  };
}

beforeEach(() => {
  channelsStore.reset();
  listChannelsMock.mockReset();
  listChannelsMock.mockResolvedValue([]);
  createChannelsMock.mockReset();
  createChannelsMock.mockImplementation(async (channels) => channels);
  renameChannelMock.mockReset();
  deleteChannelsMock.mockReset();
});

describe('Spec 018 / S2: channel schema (role/style/ownership/binding) end-to-end', () => {
  it('BOD-8 select → 8 channels with new shape → save → reload → preserved', async () => {
    // ── 1. Start with no channels persisted ─────────────────────────────────
    await channelsStore.loadChannels();
    expect(channelsStore.channels).toEqual([]);

    // ── 2. Simulate BOD-8 selection: build channels via the orchestrator's
    //      pure builder, then push them through the draft store as the
    //      orchestrator's step-4 lock does. ───────────────────────────────────
    const profile = makeBod8Profile();
    const document = makeBod8Document();
    const channels = buildAutoCreatedChannelsForSlot(
      profile,
      document,
      'West Yard',
      'connector-a',
    );

    expect(channels).toHaveLength(8);

    // ── 3. Every channel carries the new schema; the legacy fields are gone.
    for (let i = 0; i < 8; i++) {
      const ch = channels[i];
      expect(ch.name).toBe(`West Yard — Connector A — Input ${i + 1}`);
      expect(ch.role).toBe('block-occupancy');
      expect(ch.style).toBe('bod-block-detector-input');
      expect(ch.ownership).toBe('hardware-owned');
      expect(ch.binding).toEqual({
        kind: 'connectorInput',
        nodeKey: '050201020300',
        connector: 'connector-a',
        input: i + 1,
      });
      expect(ch.id).toBeTruthy();
      // Legacy fields must not exist on the new schema.
      expect((ch as unknown as Record<string, unknown>).channelType).toBeUndefined();
      expect((ch as unknown as Record<string, unknown>).hardwareRef).toBeUndefined();
    }

    // ── 4. Push to the draft store, then run the legacy save flush
    //      (createChannels IPC) — the on-wire shape must be the new schema. ─
    channelsStore.addPendingChannels(channels);
    expect(channelsStore.isDirty).toBe(true);
    expect(channelsStore.channels).toHaveLength(8);
    expect(channelsStore.grouped.get('block-occupancy')).toHaveLength(8);

    const { createChannels } = await import('$lib/api/channels');
    await createChannels(channelsStore.pendingCreations);
    expect(createChannelsMock).toHaveBeenCalledTimes(1);
    const sent = createChannelsMock.mock.calls[0][0];
    expect(sent).toHaveLength(8);
    expect(sent[0]).toEqual(channels[0]);

    // ── 5. Close + reopen: reset, then reload from a backend that returns
    //      the same shape. SC-002 — schema round-trip exact. ────────────────
    listChannelsMock.mockResolvedValue(channels);
    channelsStore.reset();
    expect(channelsStore.channels).toEqual([]);

    await channelsStore.loadChannels();
    expect(channelsStore.channels).toEqual(channels);
    expect(channelsStore.isDirty).toBe(false);

    // ── 6. Producer event-leaf mapping is sourced from the style registry,
    //      not a per-channelType hardcoded constant. ────────────────────────
    const mapping = getStyleEventMapping('bod-block-detector-input');
    expect(mapping).toEqual({
      occupied: { producerLeafIndex: 0 },
      clear: { producerLeafIndex: 1 },
    });
    expect(getStyleEventMapping('unknown-style')).toBeUndefined();
  });

  it('renaming a hydrated channel preserves the new schema on flush', async () => {
    const profile = makeBod8Profile();
    const document = makeBod8Document();
    const channels = buildAutoCreatedChannelsForSlot(
      profile,
      document,
      'West Yard',
      'connector-a',
    );
    listChannelsMock.mockResolvedValue(channels);
    await channelsStore.loadChannels();

    const target = channels[0];
    expect(channelsStore.renameChannel(target.id, 'West Yard Block 1')).toBe(true);
    expect(channelsStore.isDirty).toBe(true);

    const effective = channelsStore.channels.find((c) => c.id === target.id)!;
    expect(effective.name).toBe('West Yard Block 1');
    // Rename does not touch role/style/ownership/binding.
    expect(effective.role).toBe(target.role);
    expect(effective.style).toBe(target.style);
    expect(effective.ownership).toBe(target.ownership);
    expect(effective.binding).toEqual(target.binding);
  });
});
