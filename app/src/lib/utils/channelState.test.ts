import { describe, it, expect } from 'vitest';
import { deriveChannelState, channelStateLabel, channelStateClass } from './channelState';

describe('deriveChannelState', () => {
  const occupied = '0501010101000001';
  const clear = '0501010101000002';
  const lit = '0501010101000003';
  const unlit = '0501010101000004';

  it('returns no-config when no event IDs provided (Spec 017 / S2)', () => {
    const events = new Map<string, number>();
    expect(deriveChannelState(events, undefined, undefined, 'block-occupancy')).toEqual({
      kind: 'no-config',
    });
  });

  it('returns unknown when both event IDs known but neither event seen', () => {
    const events = new Map<string, number>();
    expect(deriveChannelState(events, occupied, clear, 'block-occupancy')).toEqual({
      kind: 'unknown',
    });
  });

  it('returns occupied when only occupied event seen (block-occupancy)', () => {
    const events = new Map<string, number>([[occupied, 1000]]);
    expect(deriveChannelState(events, occupied, clear, 'block-occupancy')).toEqual({
      role: 'block-occupancy',
      state: 'occupied',
    });
  });

  it('returns clear when only clear event seen (block-occupancy)', () => {
    const events = new Map<string, number>([[clear, 1000]]);
    expect(deriveChannelState(events, occupied, clear, 'block-occupancy')).toEqual({
      role: 'block-occupancy',
      state: 'clear',
    });
  });

  it('returns occupied when occupied is more recent', () => {
    const events = new Map<string, number>([
      [occupied, 2000],
      [clear, 1000],
    ]);
    expect(deriveChannelState(events, occupied, clear, 'block-occupancy')).toEqual({
      role: 'block-occupancy',
      state: 'occupied',
    });
  });

  it('returns clear when clear is more recent', () => {
    const events = new Map<string, number>([
      [occupied, 1000],
      [clear, 2000],
    ]);
    expect(deriveChannelState(events, occupied, clear, 'block-occupancy')).toEqual({
      role: 'block-occupancy',
      state: 'clear',
    });
  });

  it('returns clear when timestamps are equal (clear wins tie)', () => {
    const events = new Map<string, number>([
      [occupied, 1000],
      [clear, 1000],
    ]);
    expect(deriveChannelState(events, occupied, clear, 'block-occupancy')).toEqual({
      role: 'block-occupancy',
      state: 'clear',
    });
  });

  it('returns lit when only lit event seen (lamp-indicator)', () => {
    const events = new Map<string, number>([[lit, 1000]]);
    expect(deriveChannelState(events, lit, unlit, 'lamp-indicator')).toEqual({
      role: 'lamp-indicator',
      state: 'lit',
    });
  });

  it('returns unlit when only unlit event seen (lamp-indicator)', () => {
    const events = new Map<string, number>([[unlit, 1000]]);
    expect(deriveChannelState(events, lit, unlit, 'lamp-indicator')).toEqual({
      role: 'lamp-indicator',
      state: 'unlit',
    });
  });

  it('returns lit when lit is more recent (lamp-indicator)', () => {
    const events = new Map<string, number>([
      [lit, 2000],
      [unlit, 1000],
    ]);
    expect(deriveChannelState(events, lit, unlit, 'lamp-indicator')).toEqual({
      role: 'lamp-indicator',
      state: 'lit',
    });
  });

  it('returns unlit when unlit is more recent (lamp-indicator)', () => {
    const events = new Map<string, number>([
      [lit, 1000],
      [unlit, 2000],
    ]);
    expect(deriveChannelState(events, lit, unlit, 'lamp-indicator')).toEqual({
      role: 'lamp-indicator',
      state: 'unlit',
    });
  });
});

describe('channelStateLabel', () => {
  it('formats each arm of the discriminated union', () => {
    expect(channelStateLabel({ kind: 'no-config' })).toBe('No config');
    expect(channelStateLabel({ kind: 'unknown' })).toBe('Unknown');
    expect(channelStateLabel({ role: 'block-occupancy', state: 'occupied' })).toBe('Occupied');
    expect(channelStateLabel({ role: 'block-occupancy', state: 'clear' })).toBe('Clear');
    expect(channelStateLabel({ role: 'lamp-indicator', state: 'lit' })).toBe('Lit');
    expect(channelStateLabel({ role: 'lamp-indicator', state: 'unlit' })).toBe('Unlit');
  });
});

describe('channelStateClass', () => {
  it('returns the discriminator-or-state string', () => {
    expect(channelStateClass({ kind: 'no-config' })).toBe('no-config');
    expect(channelStateClass({ kind: 'unknown' })).toBe('unknown');
    expect(channelStateClass({ role: 'block-occupancy', state: 'occupied' })).toBe('occupied');
    expect(channelStateClass({ role: 'block-occupancy', state: 'clear' })).toBe('clear');
    expect(channelStateClass({ role: 'lamp-indicator', state: 'lit' })).toBe('lit');
    expect(channelStateClass({ role: 'lamp-indicator', state: 'unlit' })).toBe('unlit');
  });
});
