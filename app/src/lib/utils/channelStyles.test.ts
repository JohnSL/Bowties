import { describe, it, expect } from 'vitest';
import { getStyleEventMapping, getStyleRowCount } from './channelStyles';

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

  it('returns the 2-LED bicolor aspect mapping for "2-led-bicolor-aspect"', () => {
    const mapping = getStyleEventMapping('2-led-bicolor-aspect');
    expect(mapping).toBeDefined();
    expect(mapping!.stop).toEqual({ consumerLeafIndex: 0 });
    expect(mapping!.approach).toEqual({ consumerLeafIndex: 2 });
    expect(mapping!.clear).toEqual({ consumerLeafIndex: 4 });
    expect(mapping!.dark).toEqual({ consumerLeafIndex: 6 });
  });

  it('returns undefined for an unknown style id', () => {
    expect(getStyleEventMapping('not-a-real-style')).toBeUndefined();
  });

  it('returns undefined for the empty string', () => {
    expect(getStyleEventMapping('')).toBeUndefined();
  });

  it('returns row count 1 for single-led-direct-lamp', () => {
    expect(getStyleRowCount('single-led-direct-lamp')).toBe(1);
  });

  it('returns row count 2 for 2-led-bicolor-aspect', () => {
    expect(getStyleRowCount('2-led-bicolor-aspect')).toBe(2);
  });

  it('returns row count 0 for unknown styles', () => {
    expect(getStyleRowCount('not-a-real-style')).toBe(0);
  });
});
