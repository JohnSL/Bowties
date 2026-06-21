import { describe, expect, it } from 'vitest';
import { normalizeLayoutTitle } from './layoutPath';

describe('normalizeLayoutTitle', () => {
  it('returns folder name from Unix path', () => {
    expect(normalizeLayoutTitle('/layouts/yard')).toBe('yard');
  });

  it('returns folder name from nested path', () => {
    expect(normalizeLayoutTitle('/a/b/my-layout')).toBe('my-layout');
  });

  it('handles Windows paths', () => {
    expect(normalizeLayoutTitle('C:\\Users\\john\\yard')).toBe('yard');
  });

  it('strips trailing slashes', () => {
    expect(normalizeLayoutTitle('/layouts/yard/')).toBe('yard');
    expect(normalizeLayoutTitle('C:\\Layouts\\Yard\\')).toBe('Yard');
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
