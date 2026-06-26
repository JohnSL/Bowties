import { describe, it, expect } from 'vitest';
import { deriveChannelState } from './channelState';

describe('deriveChannelState', () => {
  const occupied = '0501010101000001';
  const clear = '0501010101000002';

  it('returns no-config when no event IDs provided (Spec 017 / S2)', () => {
    const events = new Map<string, number>();
    expect(deriveChannelState(events, undefined, undefined)).toBe('no-config');
  });

  it('returns unknown when both event IDs known but neither event seen', () => {
    const events = new Map<string, number>();
    expect(deriveChannelState(events, occupied, clear)).toBe('unknown');
  });

  it('returns occupied when only occupied event seen', () => {
    const events = new Map<string, number>([[occupied, 1000]]);
    expect(deriveChannelState(events, occupied, clear)).toBe('occupied');
  });

  it('returns clear when only clear event seen', () => {
    const events = new Map<string, number>([[clear, 1000]]);
    expect(deriveChannelState(events, occupied, clear)).toBe('clear');
  });

  it('returns occupied when occupied is more recent', () => {
    const events = new Map<string, number>([
      [occupied, 2000],
      [clear, 1000],
    ]);
    expect(deriveChannelState(events, occupied, clear)).toBe('occupied');
  });

  it('returns clear when clear is more recent', () => {
    const events = new Map<string, number>([
      [occupied, 1000],
      [clear, 2000],
    ]);
    expect(deriveChannelState(events, occupied, clear)).toBe('clear');
  });

  it('returns clear when timestamps are equal (clear wins tie)', () => {
    const events = new Map<string, number>([
      [occupied, 1000],
      [clear, 1000],
    ]);
    // When equal, occupied > clear is false, so 'clear' wins
    expect(deriveChannelState(events, occupied, clear)).toBe('clear');
  });
});
