/**
 * Tests for the startup orchestrator (Spec 013 / S6).
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { KnownLayoutEntry } from '$lib/api/startup';
import type { OpenLayoutResult } from '$lib/api/layout';
import {
  loadKnownLayouts,
  openLayoutFromRegistry,
  createNewLayout,
  removeKnownLayout,
  deriveLayoutNameFromPath,
} from './startupOrchestrator';

function makeEntry(over: Partial<KnownLayoutEntry> = {}): KnownLayoutEntry {
  return {
    name: 'Yard',
    path: 'D:/Layouts/yard.layout',
    lastOpened: '2026-05-01T00:00:00.000Z',
    ...over,
  };
}

function makeOpenResult(over: Partial<OpenLayoutResult> = {}): OpenLayoutResult {
  return {
    layoutId: 'layout-1',
    capturedAt: '2026-05-23T00:00:00.000Z',
    layout: {
      schemaVersion: '1.0',
      bowties: {},
      roleClassifications: {},
      connectorSelections: {},
    } as OpenLayoutResult['layout'],
    offlineMode: true,
    nodeCount: 0,
    pendingOfflineChangeCount: 0,
    partialNodes: [],
    nodeSnapshots: [],
    recoveryOccurred: false,
    ...over,
  };
}

function makeStore() {
  const setEntries = vi.fn();
  const setBusy = vi.fn();
  return { setEntries, setBusy };
}

describe('loadKnownLayouts', () => {
  it('fetches entries, sets them on the store, and toggles busy', async () => {
    const store = makeStore();
    const entries = [makeEntry()];
    const getKnownLayouts = vi.fn(async () => entries);

    await loadKnownLayouts({ api: { getKnownLayouts }, store });

    expect(getKnownLayouts).toHaveBeenCalledTimes(1);
    expect(store.setEntries).toHaveBeenCalledWith(entries);
    expect(store.setBusy).toHaveBeenNthCalledWith(1, true);
    expect(store.setBusy).toHaveBeenNthCalledWith(2, false);
  });

  it('sets an empty entries list and reports the error when the API fails', async () => {
    const store = makeStore();
    const onError = vi.fn();
    const err = new Error('backend down');
    const getKnownLayouts = vi.fn(async () => { throw err; });

    await loadKnownLayouts({ api: { getKnownLayouts }, store, onError });

    expect(onError).toHaveBeenCalledWith(err);
    expect(store.setEntries).toHaveBeenCalledWith([]);
    expect(store.setBusy).toHaveBeenNthCalledWith(2, false);
  });
});

describe('openLayoutFromRegistry', () => {
  it('opens the layout, calls onOpened, then upserts the entry and updates store', async () => {
    const store = makeStore();
    const updated = [makeEntry({ name: 'Renamed' })];
    const openLayout = vi.fn(async () => makeOpenResult());
    const addKnownLayout = vi.fn(async () => updated);
    const onOpened = vi.fn(async () => {});

    const result = await openLayoutFromRegistry({
      path: 'D:/Layouts/yard.layout',
      name: 'Renamed',
      openLayout,
      api: { addKnownLayout },
      store,
      onOpened,
    });

    expect(openLayout).toHaveBeenCalledWith('D:/Layouts/yard.layout');
    expect(onOpened).toHaveBeenCalledTimes(1);
    expect(onOpened).toHaveBeenCalledWith(result);
    expect(addKnownLayout).toHaveBeenCalledTimes(1);
    const arg = addKnownLayout.mock.calls[0][0];
    expect(arg.name).toBe('Renamed');
    expect(arg.path).toBe('D:/Layouts/yard.layout');
    expect(arg.lastOpened).toMatch(/\d{4}-\d{2}-\d{2}T/);
    expect(store.setEntries).toHaveBeenCalledWith(updated);
  });

  it('derives the name from the path when no name is provided', async () => {
    const store = makeStore();
    const addKnownLayout = vi.fn(async () => []);
    await openLayoutFromRegistry({
      path: 'D:/Layouts/freight-yard.layout',
      openLayout: async () => makeOpenResult(),
      api: { addKnownLayout },
      store,
      onOpened: () => {},
    });
    expect(addKnownLayout.mock.calls[0][0].name).toBe('freight-yard');
  });

  it('does not throw when the registry upsert fails after a successful open', async () => {
    const store = makeStore();
    const openLayout = vi.fn(async () => makeOpenResult());
    const addKnownLayout = vi.fn(async () => { throw new Error('disk full'); });
    const onOpened = vi.fn();

    await expect(openLayoutFromRegistry({
      path: 'D:/Layouts/yard.layout',
      openLayout,
      api: { addKnownLayout },
      store,
      onOpened,
    })).resolves.toBeDefined();

    expect(onOpened).toHaveBeenCalledTimes(1);
    expect(store.setEntries).not.toHaveBeenCalled();
  });

  it('does not register the layout when the open call fails', async () => {
    const store = makeStore();
    const openLayout = vi.fn(async () => { throw new Error('bad file'); });
    const addKnownLayout = vi.fn(async () => []);

    await expect(openLayoutFromRegistry({
      path: 'D:/Layouts/bad.layout',
      openLayout,
      api: { addKnownLayout },
      store,
      onOpened: () => {},
    })).rejects.toThrow(/bad file/);

    expect(addKnownLayout).not.toHaveBeenCalled();
  });
});

describe('createNewLayout', () => {
  let store: ReturnType<typeof makeStore>;
  let createNewLayoutCapture: ReturnType<typeof vi.fn>;
  let saveLayoutDirectory: ReturnType<typeof vi.fn>;
  let openLayout: ReturnType<typeof vi.fn>;
  let addKnownLayout: ReturnType<typeof vi.fn>;
  let onOpened: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    store = makeStore();
    createNewLayoutCapture = vi.fn(async () => ({ layoutId: 'l-1', createdAt: 'x' }));
    saveLayoutDirectory = vi.fn(async () => ({}));
    openLayout = vi.fn(async () => makeOpenResult());
    addKnownLayout = vi.fn(async () => []);
    onOpened = vi.fn(async () => {});
  });

  it('runs createNewLayoutCapture → saveLayoutDirectory → openLayout → addKnownLayout in order', async () => {
    const order: string[] = [];
    createNewLayoutCapture.mockImplementation(async () => { order.push('create'); return { layoutId: 'l-1', createdAt: 'x' }; });
    saveLayoutDirectory.mockImplementation(async () => { order.push('save'); return {}; });
    openLayout.mockImplementation(async () => { order.push('open'); return makeOpenResult(); });
    addKnownLayout.mockImplementation(async () => { order.push('register'); return []; });

    await createNewLayout({
      name: 'Yard',
      path: 'D:/Layouts/yard.layout',
      api: { addKnownLayout },
      lifecycle: { createNewLayoutCapture, saveLayoutDirectory, openLayout },
      store,
      onOpened,
    });

    expect(order).toEqual(['create', 'save', 'open', 'register']);
    expect(saveLayoutDirectory).toHaveBeenCalledWith('D:/Layouts/yard.layout', true, []);
    expect(openLayout).toHaveBeenCalledWith('D:/Layouts/yard.layout');
  });

  it('rejects when the name is empty or whitespace', async () => {
    await expect(createNewLayout({
      name: '   ',
      path: 'D:/Layouts/yard.layout',
      api: { addKnownLayout },
      lifecycle: { createNewLayoutCapture, saveLayoutDirectory, openLayout },
      store,
      onOpened,
    })).rejects.toThrow(/name/i);
    expect(createNewLayoutCapture).not.toHaveBeenCalled();
  });

  it('rejects when the path is empty', async () => {
    await expect(createNewLayout({
      name: 'Yard',
      path: '',
      api: { addKnownLayout },
      lifecycle: { createNewLayoutCapture, saveLayoutDirectory, openLayout },
      store,
      onOpened,
    })).rejects.toThrow(/path/i);
    expect(createNewLayoutCapture).not.toHaveBeenCalled();
  });
});

describe('removeKnownLayout', () => {
  it('removes the entry through the API and updates the store', async () => {
    const store = makeStore();
    const remaining = [makeEntry({ path: 'D:/Layouts/keep.layout' })];
    const removeApi = vi.fn(async () => remaining);

    await removeKnownLayout({
      path: 'D:/Layouts/yard.layout',
      api: { removeKnownLayout: removeApi },
      store,
    });

    expect(removeApi).toHaveBeenCalledWith('D:/Layouts/yard.layout');
    expect(store.setEntries).toHaveBeenCalledWith(remaining);
  });

  it('reports errors and leaves the store untouched on failure', async () => {
    const store = makeStore();
    const onError = vi.fn();
    const removeApi = vi.fn(async () => { throw new Error('nope'); });

    await removeKnownLayout({
      path: 'D:/Layouts/yard.layout',
      api: { removeKnownLayout: removeApi },
      store,
      onError,
    });

    expect(onError).toHaveBeenCalled();
    expect(store.setEntries).not.toHaveBeenCalled();
  });
});

describe('deriveLayoutNameFromPath', () => {
  it('strips the .layout extension', () => {
    expect(deriveLayoutNameFromPath('D:/Layouts/yard.layout')).toBe('yard');
    expect(deriveLayoutNameFromPath('/home/me/freight.LAYOUT')).toBe('freight');
  });

  it('handles Windows backslash paths', () => {
    expect(deriveLayoutNameFromPath('D:\\Layouts\\depot.layout')).toBe('depot');
  });

  it('returns the basename when there is no .layout extension', () => {
    expect(deriveLayoutNameFromPath('D:/Layouts/oddly-named')).toBe('oddly-named');
  });
});
