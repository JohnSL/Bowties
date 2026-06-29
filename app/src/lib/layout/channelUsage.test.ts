/**
 * Spec 018 / S4 (D1, D2) tests for `effectiveLayoutStore.channelUsageMap`
 * and `unboundChannelsForRole` — ADR-0004 single-merge owner of the
 * channel ↔ facility-slot derivation.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';

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

beforeEach(() => {
  channelsStore.reset();
  facilitiesStore.reset();
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
      { facilityId: 'f-1', facilityName: 'Block 5', slotLabel: 'input' },
    ]);
    expect(map.get('ch-lamp-1')).toEqual([
      { facilityId: 'f-1', facilityName: 'Block 5', slotLabel: 'output' },
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
