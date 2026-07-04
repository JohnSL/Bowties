import { describe, it, expect } from 'vitest';
import { getStyleEventMapping } from './channelStyles';

describe('channelStyles registry', () => {
  it('returns the BOD detector input mapping for "bod-block-detector-input"', () => {
    expect(getStyleEventMapping('bod-block-detector-input')).toEqual({
      occupied: { producerLeafIndex: 0 },
      clear: { producerLeafIndex: 1 },
    });
  });

  it('returns the single-LED direct-lamp consumer mapping for "single-led-direct-lamp"', () => {
    expect(getStyleEventMapping('single-led-direct-lamp')).toEqual({
      lit: { consumerLeafIndex: 0 },
      unlit: { consumerLeafIndex: 1 },
    });
  });

  it('returns undefined for an unknown style id', () => {
    expect(getStyleEventMapping('not-a-real-style')).toBeUndefined();
  });

  it('returns undefined for the empty string', () => {
    expect(getStyleEventMapping('')).toBeUndefined();
  });
});
