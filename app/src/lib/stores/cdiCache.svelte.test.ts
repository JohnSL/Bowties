import { beforeEach, describe, expect, it } from 'vitest';
import { cdiCacheStore } from './cdiCache.svelte';

describe('cdiCacheStore', () => {
  beforeEach(() => {
    cdiCacheStore.reset();
  });

  it('starts empty', () => {
    expect(cdiCacheStore.nodes.size).toBe(0);
    expect(cdiCacheStore.has('a')).toBe(false);
  });

  it('unions additions via add()', () => {
    cdiCacheStore.add(['a', 'b']);
    cdiCacheStore.add(['b', 'c']);
    expect([...cdiCacheStore.nodes].sort()).toEqual(['a', 'b', 'c']);
    expect(cdiCacheStore.has('c')).toBe(true);
  });

  it('ignores an empty add() without churning the set', () => {
    cdiCacheStore.add(['a']);
    const before = cdiCacheStore.nodes;
    cdiCacheStore.add([]);
    expect(cdiCacheStore.nodes).toBe(before);
  });

  it('replaces the set wholesale via replace()', () => {
    cdiCacheStore.add(['a', 'b']);
    cdiCacheStore.replace(['c']);
    expect([...cdiCacheStore.nodes]).toEqual(['c']);
    expect(cdiCacheStore.has('a')).toBe(false);
  });

  it('clears all entries via reset()', () => {
    cdiCacheStore.add(['a', 'b']);
    cdiCacheStore.reset();
    expect(cdiCacheStore.nodes.size).toBe(0);
  });

  it('reset() is a no-op when already empty', () => {
    const before = cdiCacheStore.nodes;
    cdiCacheStore.reset();
    expect(cdiCacheStore.nodes).toBe(before);
  });
});
