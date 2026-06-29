/**
 * Spec 018 / S4 tests for `facilityOrchestrator`.
 *
 * Drives the real `facilitiesStore` / `channelsStore` / `behaviorTemplatesStore`
 * singletons with mocked IPC: the orchestrator is the only seam under test,
 * so its role-validation + atomic-rebind contract is verified at the store
 * mutation level.
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
  createChannels: async (channels: InformationChannel[]) => channels,
  renameChannel: async () => undefined,
  deleteChannels: async () => undefined,
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
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

describe('selectChannelForSlot — select mode', () => {
  it('attaches the channel when the role matches', () => {
    orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-1', mode: 'select',
    });
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-bod-1']);
  });

  it('throws RoleMismatchError when the channel role does not match the slot', () => {
    expect(() => orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-lamp-1', mode: 'select',
    })).toThrow(orch.RoleMismatchError);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
  });

  it('throws UnknownReferenceError when the channel id is unknown', () => {
    expect(() => orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'nope', mode: 'select',
    })).toThrow(orch.UnknownReferenceError);
  });
});

describe('selectChannelForSlot — rebind mode (atomic swap)', () => {
  beforeEach(() => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5', slotBindings: { input: ['ch-bod-1'], output: [] } },
    ]);
  });

  it('detaches previous then attaches new', () => {
    orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-2', mode: 'rebind', previousChannelId: 'ch-bod-1',
    });
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-bod-2']);
  });

  it('throws when previousChannelId is missing', () => {
    expect(() => orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-2', mode: 'rebind',
    })).toThrow();
  });

  it('rejects role-mismatched rebind without touching the slot', () => {
    expect(() => orch.selectChannelForSlot({
      facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-lamp-1', mode: 'rebind', previousChannelId: 'ch-bod-1',
    })).toThrow(orch.RoleMismatchError);
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual(['ch-bod-1']);
  });
});

describe('removeFromSlot', () => {
  it('detaches the channel; no-op when already absent', () => {
    facilitiesStore.hydrateBaseline([
      { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5', slotBindings: { input: ['ch-bod-1'], output: [] } },
    ]);
    orch.removeFromSlot({ facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-1' });
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
    // Second call is a no-op (does not throw).
    orch.removeFromSlot({ facilityId: 'f-1', slotLabel: 'input', channelId: 'ch-bod-1' });
    expect(facilitiesStore.facilities[0].slotBindings.input).toEqual([]);
  });
});
