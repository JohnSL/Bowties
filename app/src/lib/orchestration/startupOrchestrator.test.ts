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
      path: 'D:/Layouts/freight-yard',
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
  let closeLayout: ReturnType<typeof vi.fn>;
  let createNewLayoutCapture: ReturnType<typeof vi.fn>;
  let saveLayoutDirectory: ReturnType<typeof vi.fn>;
  let openLayout: ReturnType<typeof vi.fn>;
  let addKnownLayout: ReturnType<typeof vi.fn>;
  let onOpened: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    store = makeStore();
    closeLayout = vi.fn(async () => {});
    createNewLayoutCapture = vi.fn(async () => ({ layoutId: 'l-1', createdAt: 'x' }));
    saveLayoutDirectory = vi.fn(async () => ({}));
    openLayout = vi.fn(async () => makeOpenResult());
    addKnownLayout = vi.fn(async () => []);
    onOpened = vi.fn(async () => {});
  });

  it('runs closeLayout → createNewLayoutCapture → saveLayoutDirectory → openLayout → addKnownLayout in order', async () => {
    const order: string[] = [];
    closeLayout.mockImplementation(async () => { order.push('close'); });
    createNewLayoutCapture.mockImplementation(async () => { order.push('create'); return { layoutId: 'l-1', createdAt: 'x' }; });
    saveLayoutDirectory.mockImplementation(async () => { order.push('save'); return {}; });
    openLayout.mockImplementation(async () => { order.push('open'); return makeOpenResult(); });
    addKnownLayout.mockImplementation(async () => { order.push('register'); return []; });

    await createNewLayout({
      name: 'Yard',
      path: 'D:/Layouts/yard.layout',
      api: { addKnownLayout },
      lifecycle: { closeLayout, createNewLayoutCapture, saveLayoutDirectory, openLayout },
      store,
      onOpened,
    });

    expect(order).toEqual(['close', 'create', 'save', 'open', 'register']);
    expect(saveLayoutDirectory).toHaveBeenCalledWith('D:/Layouts/yard.layout', true, []);
    expect(openLayout).toHaveBeenCalledWith('D:/Layouts/yard.layout');
  });

  it('rejects when the name is empty or whitespace', async () => {
    await expect(createNewLayout({
      name: '   ',
      path: 'D:/Layouts/yard.layout',
      api: { addKnownLayout },
      lifecycle: { closeLayout, createNewLayoutCapture, saveLayoutDirectory, openLayout },
      store,
      onOpened,
    })).rejects.toThrow(/name/i);
    expect(closeLayout).not.toHaveBeenCalled();
    expect(createNewLayoutCapture).not.toHaveBeenCalled();
  });

  it('rejects when the path is empty', async () => {
    await expect(createNewLayout({
      name: 'Yard',
      path: '',
      api: { addKnownLayout },
      lifecycle: { closeLayout, createNewLayoutCapture, saveLayoutDirectory, openLayout },
      store,
      onOpened,
    })).rejects.toThrow(/path/i);
    expect(closeLayout).not.toHaveBeenCalled();
    expect(createNewLayoutCapture).not.toHaveBeenCalled();
  });
});

/**
 * R7 regression: composed-seam test. The R7 bug — open a layout with a
 * placeholder, close it, create a new layout, and the placeholder
 * reappears — was a leak across three modules. Wire the *real*
 * `layoutLifecycleOrchestrator.closeLayout` into `createNewLayout`'s
 * lifecycle, seed a placeholder, and assert it is gone before
 * `saveLayoutDirectory` runs (the leak point in the old flow).
 */
describe('createNewLayout + layoutLifecycleOrchestrator integration', () => {
  it('clears placeholder roster before saving the new layout (R7)', async () => {
    const { layoutLifecycleOrchestrator } = await import('./layoutLifecycleOrchestrator');
    const { nodeRoster } = await import('$lib/stores/nodeRoster.svelte');
    const { layoutStore } = await import('$lib/stores/layout.svelte');
    const { nodeInfoStore } = await import('$lib/stores/nodeInfo');
    const { get } = await import('svelte/store');

    layoutStore.reset();
    nodeRoster.clearLayoutScope();
    nodeInfoStore.set(new Map());

    const PH = 'placeholder:99999999-2222-4333-8444-555555555555';
    const map = new Map(get(nodeInfoStore));
    map.set(PH, {
      node_id: [0, 0, 0, 0, 0, 0], alias: 0,
      snip_data: { manufacturer: 'M', model: 'M', hardware_version: '', software_version: '', user_name: 'X', user_description: '' },
      snip_status: 'Complete', connection_status: 'NotApplicable',
      last_verified: '', last_seen: '', cdi: null, pip_flags: null, pip_status: 'NotSupported',
    } as never);
    nodeInfoStore.set(map);
    nodeRoster.addPlaceholder({
      nodeKey: PH, profileStem: 'stem',
      info: get(nodeInfoStore).get(PH)!,
      tree: { segments: [] } as never,
    });

    expect(nodeRoster.placeholderEntries.length).toBe(1);

    const store = makeStore();
    const saveLayoutDirectory = vi.fn(async () => {
      // Pin the seam: by the time the new layout is being persisted,
      // the prior placeholder must already be gone from the frontend
      // roster (and the backend's clear_layout_scope must have fired).
      expect(nodeRoster.placeholderEntries.length).toBe(0);
      return {};
    });

    await createNewLayout({
      name: 'Yard',
      path: 'D:/Layouts/yard.layout',
      api: { addKnownLayout: vi.fn(async () => []) },
      lifecycle: {
        // Wire the real lifecycle orchestrator's closeLayout (legacy_file
        // mode skips the backend IPC, so this isolates the frontend seam).
        closeLayout: () => layoutLifecycleOrchestrator.closeLayout({
          activeMode: 'legacy_file',
          closeLayoutIpc: vi.fn(async () => ({ closed: true })),
          clearRecentLayout: vi.fn(async () => {}),
          connected: false,
        }).then(() => undefined),
        createNewLayoutCapture: vi.fn(async () => ({ layoutId: 'l-1', createdAt: 'x' })),
        saveLayoutDirectory,
        openLayout: vi.fn(async () => makeOpenResult()),
      },
      store,
      onOpened: vi.fn(),
    });

    expect(saveLayoutDirectory).toHaveBeenCalledTimes(1);
    expect(nodeRoster.placeholderEntries.length).toBe(0);
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
  it('returns the folder name from a Unix path', () => {
    expect(deriveLayoutNameFromPath('D:/Layouts/yard')).toBe('yard');
    expect(deriveLayoutNameFromPath('/home/me/freight')).toBe('freight');
  });

  it('handles Windows backslash paths', () => {
    expect(deriveLayoutNameFromPath('D:\\Layouts\\depot')).toBe('depot');
  });

  it('strips trailing slashes', () => {
    expect(deriveLayoutNameFromPath('D:/Layouts/oddly-named/')).toBe('oddly-named');
  });
});
