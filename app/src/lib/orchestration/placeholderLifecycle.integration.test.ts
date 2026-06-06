/**
 * Spec 014 / S8.5 / T12 — placeholder lifecycle end-to-end tests.
 *
 * Exercises the two quickstart scenarios:
 *   A) add → edit → close (discard) → reopen → absent
 *   B) add → edit User Name leaf → save → reopen → present, with the
 *      `addPlaceholderBoard` delta composed for the backend
 *
 * These tests drive the real orchestrator and store wiring; only the
 * outer-edge IPCs (`addPlaceholderBoardIpc`, `getNodeTree`,
 * `listBundledProfiles`, save IPC) and the `+page.svelte` lifecycle reset
 * callbacks are mocked. This mirrors the integration seam where
 * placeholders cross from in-memory roster to persisted layout snapshots.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import type { LayoutEditDelta, LayoutFile } from '$lib/types/bowtie';

const addPlaceholderBoardIpcMock = vi.fn();
const getNodeTreeMock = vi.fn();
const listBundledProfilesIpc = vi.fn();

vi.mock('$lib/api/layout', () => ({
  addPlaceholderBoardIpc: addPlaceholderBoardIpcMock,
  getNodeTree: getNodeTreeMock,
  listBundledProfiles: listBundledProfilesIpc,
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn(), open: vi.fn() }));

const { nodeInfoStore } = await import('$lib/stores/nodeInfo');
const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');
const { configReadNodesStore } = await import('$lib/stores/configReadStatus');
const { nodeRoster } = await import('$lib/stores/nodeRoster.svelte');
const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
const { addPlaceholderBoard } = await import('./placeholderBoardOrchestrator');
const { saveLayoutOrchestrated } = await import('./saveLayoutOrchestrator');
const { editKeyForLeaf } = await import('$lib/utils/editKey');

const STUB_TREE = { segments: [] } as unknown as Parameters<
  typeof nodeTreeStore.setTree
>[1];

const STUB_PROFILES = [
  {
    stem: 'RR-CirKits_Tower-LCC',
    manufacturer: 'RR-CirKits',
    model: 'Tower-LCC',
  },
];

const EMPTY_LAYOUT: LayoutFile = {
  schemaVersion: '1.0',
  bowties: {},
  roleClassifications: {},
};

let addCallCounter = 0;

beforeEach(() => {
  addCallCounter = 0;
  addPlaceholderBoardIpcMock.mockReset();
  getNodeTreeMock.mockReset();
  listBundledProfilesIpc.mockReset();
  listBundledProfilesIpc.mockResolvedValue(STUB_PROFILES);
  addPlaceholderBoardIpcMock.mockImplementation(async () => {
    addCallCounter++;
    const hex = addCallCounter.toString(16).padStart(12, '0');
    return { nodeKey: `placeholder:${hex.slice(0,8)}-${hex.slice(8,12)}-4000-8000-000000000000` };
  });
  getNodeTreeMock.mockResolvedValue(STUB_TREE);
  nodeInfoStore.set(new Map());
  configReadNodesStore.set(new Set());
  nodeTreeStore.reset();
  nodeRoster.clearLayoutScope();
  configChangesStore.clearAllDrafts();
});

describe('S8.5 / T12 — placeholder lifecycle (Scenario A: discard)', () => {
  it('removes the placeholder from every in-memory store when the layout is closed without saving', async () => {
    // 1. Open layout (implicit via empty stores)
    // 2. Add a placeholder
    const { nodeKey } = await addPlaceholderBoard({
      profileStem: 'RR-CirKits_Tower-LCC',
    });
    expect(get(nodeInfoStore).has(nodeKey)).toBe(true);
    expect(nodeRoster.has(nodeKey)).toBe(true);

    // 3. Set a config value through the standard draft path
    const key = editKeyForLeaf(nodeKey, 253, 100);
    configChangesStore.set(key, { type: 'int', value: 7 });
    expect(configChangesStore.draftEntries().length).toBeGreaterThan(0);

    // 4. Simulate close-without-save: the +page.svelte resetLayoutStore
    //    callback (T9) resets every store including inMemoryPlaceholders.
    nodeInfoStore.set(new Map());
    nodeTreeStore.reset();
    configReadNodesStore.set(new Set());
    nodeRoster.clearLayoutScope();
    configChangesStore.clearAllDrafts();

    // 5. Reopen layout (mocked as empty snapshot list — backend returns
    //    no placeholder for this key because save was never called).
    // 6. Assert the placeholder NodeKey is absent everywhere.
    expect(get(nodeInfoStore).has(nodeKey)).toBe(false);
    expect(nodeRoster.has(nodeKey)).toBe(false);
    expect(nodeTreeStore.trees.has(nodeKey)).toBe(false);
  });
});

describe('S8.11 — placeholder lifecycle (Scenario B: save)', () => {
  it(
    'composes an addNode delta on save and clears the in-memory roster ' +
      'while leaving the synthesized snapshot in nodeInfoStore',
    async () => {
      // 1. Add the placeholder
      const { nodeKey } = await addPlaceholderBoard({
        profileStem: 'RR-CirKits_Tower-LCC',
      });

      // 2. Edit the User Name leaf via the standard draft path. The leaf
      //    address is illustrative — what matters is that an edit exists.
      const userNameKey = editKeyForLeaf(nodeKey, 251, 0);
      configChangesStore.set(userNameKey, {
        type: 'string',
        value: 'East Yard Tower',
      });

      // 3. Save. The orchestrator must append exactly one
      //    addNode delta carrying our nodeKey.
      const saveFile = vi.fn(async (_path: string, deltas: LayoutEditDelta[]) => ({
        warnings: [],
        layout: EMPTY_LAYOUT,
        persistedNodeIds: [nodeKey],
        capturedDeltas: deltas,
      }));
      const rebuildCatalog = vi.fn(async () => ({
        bowties: [],
        built_at: '',
        source_node_count: 0,
        total_slots_scanned: 0,
      }));
      const setCatalog = vi.fn();
      const clearMetadata = vi.fn();
      const markClean = vi.fn();
      const hydrateLayout = vi.fn();
      const setActiveContext = vi.fn();
      const updatePartialCaptureNodes = vi.fn();
      const clearPersistedDrafts = vi.fn(() => configChangesStore.clearAllDrafts());
      const clearPersistedPlaceholders = vi.fn((keys: string[]) => {
        nodeRoster.markPlaceholdersPersisted(keys);
      });

      await saveLayoutOrchestrated({
        saveFile,
        rebuildCatalog,
        setCatalog,
        clearMetadata,
        markClean,
        hydrateLayout,
        path: '/tmp/layout',
        deltas: [],
        inMemorySnapshotKeys: nodeRoster.placeholderEntries.map((e) => e.nodeKey),
        setActiveContext,
        updatePartialCaptureNodes,
        getPendingChangeCount: () => 0,
        clearPersistedDrafts,
        clearPersistedPlaceholders,
      });

      // Verify the delta sent to the backend
      expect(saveFile).toHaveBeenCalledTimes(1);
      const [, sentDeltas] = saveFile.mock.calls[0];
      const addNodeDeltas = sentDeltas.filter(
        (d) => d.type === 'addNode',
      );
      expect(addNodeDeltas).toHaveLength(1);
      expect(addNodeDeltas[0]).toMatchObject({
        type: 'addNode',
        nodeKey,
      });

      // 4. Post-save: profile-stem tracking is cleared (no longer
      //    "unsaved"), but the synthesized snapshot remains in nodeInfoStore
      //    (the placeholder is now a "saved node" from the frontend's
      //    perspective and reload would restore it from backend snapshots).
      const entryAfterSave = nodeRoster.allEntries.find((e) => e.nodeKey === nodeKey);
      expect(entryAfterSave).toBeDefined();
      expect(entryAfterSave?.profileStem).toBeUndefined();
      expect(get(nodeInfoStore).has(nodeKey)).toBe(true);

      // 5. Sidebar label resolves to the edited User Name path: the
      //    standard draft layer would surface the edit; here we just
      //    check that the synthesized snip carries an empty user_name
      //    (the implicit-naming fallback) and the edit is committed in
      //    the draft layer prior to clearPersistedDrafts.
      const snip = get(nodeInfoStore).get(nodeKey)?.snip_data;
      expect(snip?.user_name).toBe('');
      expect(snip?.manufacturer).toBe('RR-CirKits');
      expect(snip?.model).toBe('Tower-LCC');
      // After clearPersistedDrafts ran the draft is gone — the
      // post-save effective value would come from the persisted snapshot
      // returned by the backend on the next reload.
      expect(configChangesStore.draftEntries()).toHaveLength(0);
    },
  );
});
