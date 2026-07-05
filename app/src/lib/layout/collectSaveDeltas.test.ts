/**
 * Spec 018 followup — atomic save fold. Companion to the existing
 * `dirtyAggregate.integration.test.ts`: mirrors ADR-0011's aggregate
 * contract for the *dual* signal — for every edit-bearing store that
 * contributes to `dirtyBreakdown`, the same store MUST contribute to
 * `collectAllSaveDeltas()`. ADR-0002 (backend owns persistence + atomic
 * save) fails when a store dirties the UI but drops its deltas at the
 * save boundary.
 *
 * The route-level bug this test pins: a user-owned lamp-indicator channel
 * created for a Wired Block Indicator saved cleanly on the facility side
 * (`attachChannelToSlot`) but never emitted a channel-create delta —
 * `+page.svelte`'s save-collect call omitted `channelsStore.collectDeltas()`.
 * With the delta collected here at a single seam, the omission is a
 * compile-time-visible field, not a copy-paste at the call site.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { InformationChannel } from '$lib/api/channels';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { LayoutEditDelta } from '$lib/types/bowtie';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn(), open: vi.fn() }));
vi.mock('$lib/api/layout', () => ({
  addPlaceholderBoardIpc: vi.fn(),
  getNodeTree: vi.fn(),
  listBundledProfiles: vi.fn(),
}));

const { bowtieMetadataStore } = await import('$lib/stores/bowtieMetadata.svelte');
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { connectorSelectionsStore } = await import(
  '$lib/stores/connectorSelections.svelte'
);
const { collectAllSaveDeltas } = await import('./collectSaveDeltas');
const { layoutScopedParticipants } = await import(
  '$lib/orchestration/layoutLifecycleOrchestrator'
);

const LIVE_A = '020157000001';

const BLOCK_INDICATOR: BehaviorTemplate = {
  templateId: 'block-indicator',
  displayName: 'Block Indicator',
  slots: [
    { label: 'Detector', displayLabel: 'Detector', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'Lamp', displayLabel: 'Lamp', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
  ],
  mapping: [],
};

const HW_CHANNEL: InformationChannel = {
  id: 'ch-hw-1',
  name: 'BOD A1',
  role: 'block-occupancy',
  style: 'bod-block-detector-input',
  ownership: 'hardware-owned',
  binding: { kind: 'connectorInput', nodeKey: LIVE_A, connector: 'A', input: 1 },
};

const USER_CHANNEL: InformationChannel = {
  id: 'ch-user-1',
  name: 'Block 5 Lamp',
  role: 'lamp-indicator',
  style: 'single-led-direct-lamp',
  ownership: 'user-owned',
  binding: { kind: 'lampRow', nodeKey: LIVE_A, rowOrdinal: 2 },
};

beforeEach(() => {
  bowtieMetadataStore.clearAll();
  facilitiesStore.reset();
  channelsStore.reset();
  connectorSelectionsStore.reset();
});

describe('collectAllSaveDeltas — empty baseline', () => {
  it('returns an empty array when no store is dirty', () => {
    expect(collectAllSaveDeltas()).toEqual([]);
  });
});

describe('collectAllSaveDeltas — channels store contributions (the fold)', () => {
  it('emits a channel-create delta for a user-owned channel (regression: was silently dropped by the route)', () => {
    channelsStore.createUserOwnedChannel({
      role: USER_CHANNEL.role,
      style: USER_CHANNEL.style,
      binding: USER_CHANNEL.binding,
      name: USER_CHANNEL.name,
    });

    const deltas = collectAllSaveDeltas();
    const channelCreate = deltas.find(
      (d): d is Extract<LayoutEditDelta, { type: 'createChannel' }> =>
        d.type === 'createChannel',
    );
    expect(channelCreate).toBeDefined();
    expect(channelCreate!.channel.ownership).toBe('user-owned');
    expect(channelCreate!.channel.name).toBe('Block 5 Lamp');
  });

  it('emits a channel-create delta for a hardware-owned pending channel', () => {
    channelsStore.addPendingChannels([HW_CHANNEL]);

    const deltas = collectAllSaveDeltas();
    expect(deltas).toContainEqual({
      type: 'createChannel',
      channel: HW_CHANNEL,
    } satisfies LayoutEditDelta);
  });

  it('emits a rename-channel delta when a baseline channel is renamed', () => {
    channelsStore.hydrateBaseline([HW_CHANNEL]);
    channelsStore.renameChannel(HW_CHANNEL.id, 'Renamed');

    expect(collectAllSaveDeltas()).toContainEqual({
      type: 'renameChannel',
      channelId: HW_CHANNEL.id,
      newName: 'Renamed',
    } satisfies LayoutEditDelta);
  });

  it('emits a delete-channel delta when a baseline channel is deleted', () => {
    channelsStore.hydrateBaseline([HW_CHANNEL]);
    channelsStore.deleteChannels([HW_CHANNEL.id]);

    expect(collectAllSaveDeltas()).toContainEqual({
      type: 'deleteChannel',
      channelId: HW_CHANNEL.id,
    } satisfies LayoutEditDelta);
  });
});

describe('collectAllSaveDeltas — cross-store aggregation', () => {
  it('aggregates deltas from every edit-bearing store in one pass', () => {
    // facility
    facilitiesStore.addFacility(BLOCK_INDICATOR, 'Block 5');
    // user-owned channel
    const userCh = channelsStore.createUserOwnedChannel({
      role: USER_CHANNEL.role,
      style: USER_CHANNEL.style,
      binding: USER_CHANNEL.binding,
      name: USER_CHANNEL.name,
    });
    // bowtie metadata
    bowtieMetadataStore.deleteBowtie('01.01.01.01.01.01.01.01');

    const deltas = collectAllSaveDeltas();
    const types = new Set(deltas.map((d) => d.type));

    expect(types.has('addFacility')).toBe(true);
    expect(types.has('createChannel')).toBe(true);
    expect(types.has('deleteBowtie')).toBe(true);

    // Ensure the channel create carries the created channel.
    expect(deltas).toContainEqual({
      type: 'createChannel',
      channel: expect.objectContaining({ id: userCh.id }),
    });
  });
});

describe('collectAllSaveDeltas — registry dispatch', () => {
  it('calls collectDeltas on every registry participant that implements it', () => {
    const fakeDelta: LayoutEditDelta = {
      type: 'deleteBowtie',
      bowtieEventId: '99.99.99.99.99.99.99.99',
    };
    const fakeParticipant = { collectDeltas: () => [fakeDelta] };
    layoutScopedParticipants.push(fakeParticipant);

    try {
      const deltas = collectAllSaveDeltas();
      expect(deltas).toContainEqual(fakeDelta);
    } finally {
      layoutScopedParticipants.pop();
    }
  });
});
