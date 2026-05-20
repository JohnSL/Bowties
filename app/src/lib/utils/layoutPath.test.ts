import { describe, expect, it } from 'vitest';
import { normalizeLayoutTitle } from './layoutPath';

describe('normalizeLayoutTitle', () => {
  it('strips .bowties.yaml extension', () => {
    expect(normalizeLayoutTitle('/layouts/yard.bowties.yaml')).toBe('yard');
  });

  it('strips .bowties.yml extension', () => {
    expect(normalizeLayoutTitle('/layouts/yard.bowties.yml')).toBe('yard');
  });

  it('strips .layout extension', () => {
    expect(normalizeLayoutTitle('/layouts/my-layout.layout')).toBe('my-layout');
  });

  it('strips .yaml extension', () => {
    expect(normalizeLayoutTitle('/layouts/test.yaml')).toBe('test');
  });

  it('strips .layout.d extension', () => {
    expect(normalizeLayoutTitle('/layouts/yard.layout.d')).toBe('yard');
  });

  it('returns basename without extension for plain names', () => {
    expect(normalizeLayoutTitle('/a/b/yard')).toBe('yard');
  });

  it('handles Windows paths', () => {
    expect(normalizeLayoutTitle('C:\\Users\\john\\yard.bowties.yaml')).toBe('yard');
  });

  it('returns null for null', () => {
    expect(normalizeLayoutTitle(null)).toBeNull();
  });

  it('returns null for undefined', () => {
    expect(normalizeLayoutTitle(undefined)).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(normalizeLayoutTitle('')).toBeNull();
  });
});
