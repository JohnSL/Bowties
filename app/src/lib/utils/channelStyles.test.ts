import { describe, it, expect } from 'vitest';
import { getStyleEventMapping } from './channelStyles';

describe('channelStyles registry', () => {
  it('returns the BOD detector input mapping for "bod-block-detector-input"', () => {
    expect(getStyleEventMapping('bod-block-detector-input')).toEqual({
      occupied: { producerLeafIndex: 0 },
      clear: { producerLeafIndex: 1 },
    });
  });

  it('returns undefined for an unknown style id', () => {
    expect(getStyleEventMapping('not-a-real-style')).toBeUndefined();
  });

  it('returns undefined for the empty string', () => {
    expect(getStyleEventMapping('')).toBeUndefined();
  });
});
