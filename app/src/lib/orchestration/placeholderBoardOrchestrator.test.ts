/**
 * Tests for placeholderBoardOrchestrator (Spec 014 / S8.10).
 *
 * Post-factory behaviour: add calls `addPlaceholderBoardIpc` (backend
 * factory), reads the tree via `getNodeTree`, and seeds the frontend roster.
 * Delete operates on in-memory stores only.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

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
const { configSidebarStore } = await import('$lib/stores/configSidebar');
const { addPlaceholderBoard, deletePlaceholderBoard } = await import('./placeholderBoardOrchestrator');

const UUID_V4_RE = /^placeholder:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

const STUB_TREE = { segments: [] } as unknown as Parameters<typeof nodeTreeStore.setTree>[1];

const STUB_PROFILES = [
  { stem: 'RR-CirKits_Tower-LCC', manufacturer: 'RR-CirKits', model: 'Tower-LCC' },
  { stem: 'Mustangpeak_TurnoutBoss', manufacturer: 'Mustangpeak Engineering', model: 'TurnoutBoss' },
];

let addCallCounter = 0;

beforeEach(() => {
  addCallCounter = 0;
  addPlaceholderBoardIpcMock.mockReset();
  getNodeTreeMock.mockReset();
  listBundledProfilesIpc.mockReset();
  listBundledProfilesIpc.mockResolvedValue(STUB_PROFILES);
  // Each call mints a distinct UUID key (simulating the backend factory).
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
  configSidebarStore.reset();
});

describe('placeholderBoardOrchestrator — addPlaceholderBoard', () => {
  it('calls backend factory and seeds every in-memory store', async () => {
    const { nodeKey } = await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });

    expect(nodeKey).toMatch(UUID_V4_RE);
    expect(addPlaceholderBoardIpcMock).toHaveBeenCalledWith('RR-CirKits_Tower-LCC');
    expect(getNodeTreeMock).toHaveBeenCalledWith(nodeKey);

    const info = get(nodeInfoStore).get(nodeKey);
    expect(info).toBeDefined();
    expect(info?.snip_data?.manufacturer).toBe('RR-CirKits');
    expect(info?.snip_data?.model).toBe('Tower-LCC');
    expect(info?.snip_data?.user_name).toBe('');
    expect(info?.node_id).toEqual([]);

    expect(nodeTreeStore.getTree(nodeKey)).toBe(STUB_TREE);
    expect(get(configReadNodesStore).has(nodeKey)).toBe(true);
    const entry = nodeRoster.allEntries.find((e) => e.nodeKey === nodeKey);
    expect(entry?.profileStem).toBe('RR-CirKits_Tower-LCC');
  });

  it('rejects an unknown profile stem before touching any store', async () => {
    await expect(addPlaceholderBoard({ profileStem: 'Nope_Nope' })).rejects.toThrow(
      /UnknownBundledProfile/,
    );

    expect(addPlaceholderBoardIpcMock).not.toHaveBeenCalled();
    expect(get(nodeInfoStore).size).toBe(0);
    expect(nodeRoster.placeholderEntries).toHaveLength(0);
  });

  it('generates distinct NodeKeys for back-to-back adds', async () => {
    const a = await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });
    const b = await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });
    expect(a.nodeKey).not.toEqual(b.nodeKey);
    expect(nodeRoster.placeholderEntries).toHaveLength(2);
  });
});

describe('placeholderBoardOrchestrator — deletePlaceholderBoard', () => {
  async function seedOnePlaceholder(): Promise<string> {
    const { nodeKey } = await addPlaceholderBoard({ profileStem: 'RR-CirKits_Tower-LCC' });
    return nodeKey;
  }

  it('returns false when the user declines the confirmation and leaves stores untouched', async () => {
    const nodeKey = await seedOnePlaceholder();

    const removed = await deletePlaceholderBoard({ nodeKey, confirm: async () => false });

    expect(removed).toBe(false);
    expect(get(nodeInfoStore).has(nodeKey)).toBe(true);
    expect(nodeTreeStore.getTree(nodeKey)).toBeDefined();
    expect(nodeRoster.has(nodeKey)).toBe(true);
  });

  it('removes the placeholder from every in-memory store when confirmed', async () => {
    const nodeKey = await seedOnePlaceholder();

    const removed = await deletePlaceholderBoard({ nodeKey, confirm: async () => true });

    expect(removed).toBe(true);
    expect(get(nodeInfoStore).has(nodeKey)).toBe(false);
    expect(nodeTreeStore.getTree(nodeKey)).toBeUndefined();
    expect(get(configReadNodesStore).has(nodeKey)).toBe(false);
    expect(nodeRoster.has(nodeKey)).toBe(false);
  });

  it('returns false (without prompting) for a NodeKey not in the in-memory roster', async () => {
    const confirm = vi.fn(async () => true);
    const removed = await deletePlaceholderBoard({
      nodeKey: 'placeholder:11111111-1111-4111-8111-111111111111',
      confirm,
    });
    expect(removed).toBe(false);
    expect(confirm).not.toHaveBeenCalled();
  });

  it('clears the sidebar selection when the deleted key was selected', async () => {
    const nodeKey = await seedOnePlaceholder();
    configSidebarStore.setSelectedNode(nodeKey);

    await deletePlaceholderBoard({ nodeKey, confirm: async () => true });

    expect(get(configSidebarStore).selectedNodeId).toBeNull();
  });

  it('leaves an unrelated sidebar selection intact', async () => {
    const nodeKey = await seedOnePlaceholder();
    const other = 'placeholder:99999999-9999-4999-8999-999999999999';
    configSidebarStore.setSelectedNode(other);

    await deletePlaceholderBoard({ nodeKey, confirm: async () => true });

    expect(get(configSidebarStore).selectedNodeId).toBe(other);
  });
});
