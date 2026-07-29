/**
 * Spec 018 / S4 (D1, D2) tests for `effectiveLayoutStore.channelUsageMap`
 * and `unboundChannelsForRole` — ADR-0004 single-merge owner of the
 * channel ↔ facility-slot derivation.
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

const { effectiveLayoutStore } = await import('$lib/layout/effectiveLayoutStore.svelte');
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');

// Facility-slot binding compatibility (Option B, template-owned): the
// `shared` flag on a slot distinguishes a shared-observer binding (any
// number of claims coexist) from an exclusive-claim binding (only one
// claim tolerated). Block Indicator's slots are exclusive; ABS's `input`
// slot is a shared observer of the same block-detection channel.
const BLOCK_INDICATOR: BehaviorTemplate = {
  templateId: 'block-indicator',
  displayName: 'Block Indicator',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1, shared: false },
    { label: 'output', displayLabel: 'indicator', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1, shared: false },
  ],
  mapping: [
    { producerState: 'occupied', consumerCommand: 'lit' },
    { producerState: 'clear', consumerCommand: 'unlit' },
  ],
  compilationTarget: 'composed',
  rules: [],
};

const ABS_3_ASPECT_SIGNAL: BehaviorTemplate = {
  templateId: 'abs-3-aspect-signal',
  displayName: 'ABS 3-Aspect Signal',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1, shared: true },
    { label: 'output', displayLabel: 'signal', kind: 'consumer', requiredRole: 'signal-aspect', minChannels: 1, maxChannels: 1, shared: false },
    { label: 'downstream-signal', displayLabel: 'downstream', kind: 'producer', requiredRole: 'signal-aspect', minChannels: 0, maxChannels: 1, shared: true },
  ],
  mapping: [],
  compilationTarget: 'compiled',
  rules: [],
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

function facility(id: string, name: string, slots: Record<string, string[]>): Facility {
  return {
    facilityId: id,
    templateId: 'block-indicator',
    name,
    slotBindings: { input: [], output: [], ...slots },
  };
}

function absFacility(id: string, name: string, slots: Record<string, string[]>): Facility {
  return {
    facilityId: id,
    templateId: 'abs-3-aspect-signal',
    name,
    slotBindings: { input: [], output: [], 'downstream-signal': [], ...slots },
  };
}

beforeEach(async () => {
  channelsStore.reset();
  facilitiesStore.reset();
  behaviorTemplatesStore.reset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR, ABS_3_ASPECT_SIGNAL]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
});

describe('channelUsageMap', () => {
  it('is empty when no facilities have bound channels', () => {
    channelsStore.hydrateBaseline([bod(1), bod(2)]);
    expect(effectiveLayoutStore.channelUsageMap.size).toBe(0);
  });

  it('records one entry per (channel, facility, slot)', () => {
    channelsStore.hydrateBaseline([bod(1), lamp(1)]);
    facilitiesStore.hydrateBaseline([
      facility('f-1', 'Block 5', { input: ['ch-bod-1'], output: ['ch-lamp-1'] }),
    ]);
    const map = effectiveLayoutStore.channelUsageMap;
    expect(map.get('ch-bod-1')).toEqual([
      { facilityId: 'f-1', facilityName: 'Block 5', slotLabel: 'input', shared: false },
    ]);
    expect(map.get('ch-lamp-1')).toEqual([
      { facilityId: 'f-1', facilityName: 'Block 5', slotLabel: 'output', shared: false },
    ]);
  });

  it('reflects rebind across facilities (old usage drops, new appears)', () => {
    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([
      facility('f-1', 'Block 5', { input: ['ch-bod-1'] }),
      facility('f-2', 'Block 6', {}),
    ]);
    expect(effectiveLayoutStore.channelUsageMap.get('ch-bod-1')?.[0].facilityId).toBe('f-1');

    facilitiesStore.detachChannel('f-1', 'input', 'ch-bod-1');
    facilitiesStore.attachChannel('f-2', 'input', 'ch-bod-1');
    expect(effectiveLayoutStore.channelUsageMap.get('ch-bod-1')?.[0].facilityId).toBe('f-2');
  });
});

describe('unboundChannelsForRole', () => {
  it('returns all role-matching channels when no facility has bindings', () => {
    channelsStore.hydrateBaseline([bod(1), bod(2), lamp(1)]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('block-occupancy');
    expect(unbound.map((c) => c.id).sort()).toEqual(['ch-bod-1', 'ch-bod-2']);
  });

  it('excludes channels already bound anywhere', () => {
    channelsStore.hydrateBaseline([bod(1), bod(2), bod(3)]);
    facilitiesStore.hydrateBaseline([facility('f-1', 'B5', { input: ['ch-bod-1'] })]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('block-occupancy');
    expect(unbound.map((c) => c.id).sort()).toEqual(['ch-bod-2', 'ch-bod-3']);
  });

  it('honours excludeIds (rebind pre-selects currently-bound channel)', () => {
    channelsStore.hydrateBaseline([bod(1), bod(2)]);
    facilitiesStore.hydrateBaseline([facility('f-1', 'B5', { input: ['ch-bod-1'] })]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('block-occupancy', {
      excludeIds: new Set(['ch-bod-1']),
    });
    expect(unbound.map((c) => c.id).sort()).toEqual(['ch-bod-1', 'ch-bod-2']);
  });

  it('filters by role (ignores lamp-indicator when block-occupancy requested)', () => {
    channelsStore.hydrateBaseline([bod(1), lamp(1)]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('lamp-indicator');
    expect(unbound.map((c) => c.id)).toEqual(['ch-lamp-1']);
  });
});

// Facility-slot binding compatibility matrix (Option B, template-owned):
// one exclusive claim may coexist with any number of shared observers; a
// second exclusive claim is rejected. Block Indicator's `input` slot is
// exclusive; ABS's `input` slot is a shared observer of the same channel.
describe('unboundChannelsForRole — shared-observer vs exclusive-claim compatibility', () => {
  it('keeps a channel used only by an ABS shared-observer slot eligible for a Block Indicator exclusive slot', () => {
    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([absFacility('f-abs', 'Signal 3', { input: ['ch-bod-1'] })]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('block-occupancy', { shared: false });
    expect(unbound.map((c) => c.id)).toEqual(['ch-bod-1']);
  });

  it('excludes a channel already exclusively claimed by an existing Block Indicator from another exclusive request', () => {
    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([facility('f-1', 'Block 5', { input: ['ch-bod-1'] })]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('block-occupancy', { shared: false });
    expect(unbound).toEqual([]);
  });

  it('excludes a channel with mixed ABS shared-observer + Block Indicator exclusive usage from another exclusive request', () => {
    channelsStore.hydrateBaseline([bod(1)]);
    facilitiesStore.hydrateBaseline([
      absFacility('f-abs', 'Signal 3', { input: ['ch-bod-1'] }),
      facility('f-1', 'Block 5', { input: ['ch-bod-1'] }),
    ]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('block-occupancy', { shared: false });
    expect(unbound).toEqual([]);
  });

  it('still excludes role-incompatible channels regardless of shared-observer status', () => {
    channelsStore.hydrateBaseline([bod(1), lamp(1)]);
    facilitiesStore.hydrateBaseline([absFacility('f-abs', 'Signal 3', { input: ['ch-bod-1'] })]);
    const unbound = effectiveLayoutStore.unboundChannelsForRole('lamp-indicator', { shared: false });
    expect(unbound.map((c) => c.id)).toEqual(['ch-lamp-1']);
  });
});

