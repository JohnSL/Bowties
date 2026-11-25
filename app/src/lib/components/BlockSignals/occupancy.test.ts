import { describe, expect, it } from 'vitest';
import type { InformationChannel } from '$lib/api/channels';
import { blockOccupancyMap } from './occupancy';

const channel = (id: string, role: string) => ({ id, role }) as unknown as InformationChannel;
const ids = new Map<string, Record<string, string>>([
  ['b1', { occupied: 'E1', clear: 'E2' }],
  ['b2', { occupied: 'E3', clear: 'E4' }],
  ['lamp', { lit: 'E5', unlit: 'E6' }],
]);

describe('blockOccupancyMap', () => {
  const channels = [channel('b1', 'block-occupancy'), channel('b2', 'block-occupancy'), channel('b3', 'block-occupancy'), channel('lamp', 'lamp-indicator')];

  it('maps each occupancy channel to occupied, clear or unavailable and ignores other roles', () => {
    const events = new Map([['E1', 2000], ['E2', 1000], ['E4', 500]]);
    const map = blockOccupancyMap(channels, ids, events);

    expect(map.get('b1')).toBe('occupied');
    expect(map.get('b2')).toBe('clear');
    expect(map.get('b3')).toBe('unavailable');
    expect(map.has('lamp')).toBe(false);
  });

  it('is unavailable for every block when no events have been seen', () => {
    const map = blockOccupancyMap(channels, ids, new Map());
    expect([...map.values()].every((value) => value === 'unavailable')).toBe(true);
  });
});
