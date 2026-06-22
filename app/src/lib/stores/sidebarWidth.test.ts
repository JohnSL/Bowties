/**
 * Unit tests for sidebarWidthStore.
 *
 * Validates:
 * - Default width is 240px
 * - setWidth clamps to min/max constraints
 * - Persistence to localStorage on setWidth
 * - Restoration from localStorage on creation
 * - reset() returns to default
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

// Mock localStorage before importing the store
const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => { store[key] = value; }),
    removeItem: vi.fn((key: string) => { delete store[key]; }),
    clear: () => { store = {}; },
  };
})();

Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock });

describe('sidebarWidthStore', () => {
  beforeEach(() => {
    localStorageMock.clear();
    vi.resetModules();
  });

  async function importStore() {
    const { sidebarWidthStore } = await import('./sidebarWidth');
    return sidebarWidthStore;
  }

  it('defaults to 240px when no localStorage value exists', async () => {
    const store = await importStore();
    expect(get(store)).toBe(240);
  });

  it('restores width from localStorage on creation', async () => {
    localStorageMock.getItem.mockReturnValueOnce('300');
    const store = await importStore();
    expect(get(store)).toBe(300);
  });

  it('ignores invalid localStorage values and falls back to default', async () => {
    localStorageMock.getItem.mockReturnValueOnce('not-a-number');
    const store = await importStore();
    expect(get(store)).toBe(240);
  });

  it('setWidth updates the store value', async () => {
    const store = await importStore();
    store.setWidth(320);
    expect(get(store)).toBe(320);
  });

  it('setWidth persists to localStorage', async () => {
    const store = await importStore();
    store.setWidth(280);
    expect(localStorageMock.setItem).toHaveBeenCalledWith('bowties:sidebarWidth', '280');
  });

  it('setWidth clamps below minimum (160px)', async () => {
    const store = await importStore();
    store.setWidth(100);
    expect(get(store)).toBe(160);
  });

  it('setWidth clamps above maximum (600px)', async () => {
    const store = await importStore();
    store.setWidth(800);
    expect(get(store)).toBe(600);
  });

  it('reset() returns to default width', async () => {
    const store = await importStore();
    store.setWidth(400);
    store.reset();
    expect(get(store)).toBe(240);
  });

  it('reset() clears the localStorage entry', async () => {
    const store = await importStore();
    store.setWidth(400);
    store.reset();
    expect(localStorageMock.removeItem).toHaveBeenCalledWith('bowties:sidebarWidth');
  });

  it('clamps localStorage value that exceeds max', async () => {
    localStorageMock.getItem.mockReturnValueOnce('9999');
    const store = await importStore();
    expect(get(store)).toBe(600);
  });

  it('clamps localStorage value below min', async () => {
    localStorageMock.getItem.mockReturnValueOnce('50');
    const store = await importStore();
    expect(get(store)).toBe(160);
  });
});
