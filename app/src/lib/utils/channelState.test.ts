import { describe, it, expect } from 'vitest';
import {
  deriveChannelState,
  deriveSignalAspectState,
  channelStateLabel,
  channelStateClass,
} from './channelState';

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
    expect(channelStateLabel({ role: 'signal-aspect', state: 'stop' })).toBe('Stop');
    expect(channelStateLabel({ role: 'signal-aspect', state: 'approach' })).toBe('Approach');
    expect(channelStateLabel({ role: 'signal-aspect', state: 'clear' })).toBe('Clear');
    expect(channelStateLabel({ role: 'signal-aspect', state: 'dark' })).toBe('Dark');
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

  it('prefixes signal-aspect states to avoid CSS conflicts', () => {
    expect(channelStateClass({ role: 'signal-aspect', state: 'stop' })).toBe('signal-stop');
    expect(channelStateClass({ role: 'signal-aspect', state: 'approach' })).toBe('signal-approach');
    expect(channelStateClass({ role: 'signal-aspect', state: 'clear' })).toBe('signal-clear');
    expect(channelStateClass({ role: 'signal-aspect', state: 'dark' })).toBe('signal-dark');
  });
});

describe('deriveSignalAspectState', () => {
  const redOn = '0501010101000001';
  const redOff = '0501010101000002';
  const greenOn = '0501010101000003';
  const greenOff = '0501010101000004';

  it('returns no-config when no event IDs provided', () => {
    const events = new Map<string, number>();
    expect(deriveSignalAspectState(events, undefined, undefined, undefined, undefined)).toEqual({
      kind: 'no-config',
    });
  });

  it('returns unknown when all IDs known but none seen', () => {
    const events = new Map<string, number>();
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      kind: 'unknown',
    });
  });

  it('returns stop when redOn more recent than redOff AND greenOff more recent than greenOn', () => {
    const events = new Map<string, number>([
      [redOn, 2000],
      [redOff, 1000],
      [greenOn, 1000],
      [greenOff, 2000],
    ]);
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      role: 'signal-aspect',
      state: 'stop',
    });
  });

  it('returns approach when both redOn and greenOn most recent', () => {
    const events = new Map<string, number>([
      [redOn, 2000],
      [redOff, 1000],
      [greenOn, 2000],
      [greenOff, 1000],
    ]);
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      role: 'signal-aspect',
      state: 'approach',
    });
  });

  it('returns clear when redOff more recent AND greenOn more recent', () => {
    const events = new Map<string, number>([
      [redOn, 1000],
      [redOff, 2000],
      [greenOn, 2000],
      [greenOff, 1000],
    ]);
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      role: 'signal-aspect',
      state: 'clear',
    });
  });

  it('returns dark when both redOff and greenOff most recent', () => {
    const events = new Map<string, number>([
      [redOn, 1000],
      [redOff, 2000],
      [greenOn, 1000],
      [greenOff, 2000],
    ]);
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      role: 'signal-aspect',
      state: 'dark',
    });
  });

  it('returns stop when only redOn seen (partial events — unseen defaults to off)', () => {
    const events = new Map<string, number>([[redOn, 1000]]);
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      role: 'signal-aspect',
      state: 'stop',
    });
  });

  it('returns clear when only greenOn seen', () => {
    const events = new Map<string, number>([[greenOn, 1000]]);
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      role: 'signal-aspect',
      state: 'clear',
    });
  });

  it('returns dark when only redOff seen (red off, green defaults to off)', () => {
    const events = new Map<string, number>([[redOff, 1000]]);
    expect(deriveSignalAspectState(events, redOn, redOff, greenOn, greenOff)).toEqual({
      role: 'signal-aspect',
      state: 'dark',
    });
  });
});
