/**
 * Tests for effectiveLayoutStore — ADR-0004 (S2c-T1).
 *
 * The store is the single derived read model that projects
 * (bowtieCatalogStore, layoutStore, bowtieMetadataStore, configChangesStore,
 *  nodeTreeStore) into the values the UI renders.  It subsumes the leaf-level
 * resolveValue/resolveRole helpers from ADR-0003 and the
 * EditableBowtiePreview fast/slow path branch in `bowties.svelte.ts`.
 *
 * These tests pin the contract for the three S2b-era bugs:
 *
 *  - Bug 1: drafts left over from a persisted save no longer matter — the
 *           derivation never reads `hasDraftsForNode` as a path switch.
 *  - Bug 2: a saved roleClassification on the underlying layout file is
 *           visible through `effectiveRole` and `slotsByRole` even when the
 *           tree leaf still has `eventRole: null`.
 *  - Bug 3: a pending `deleteBowtie` edit in bowtieMetadataStore removes the
 *           card from `effectiveBowties` immediately (before save).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { NodeConfigTree, LeafConfigNode, TreeConfigValue, EventRole } from '$lib/types/nodeTree';
import type { BowtieCatalog, EventSlotEntry } from '$lib/api/tauri';
import type { LayoutFile, RoleClassification } from '$lib/types/bowtie';
import { editKeyForLeaf } from '$lib/utils/editKey';

// ─── Shared event ID ─────────────────────────────────────────────────────────

const EVENT_HEX = '02.01.57.00.02.D9.00.06';
const EVENT_BYTES = [0x02, 0x01, 0x57, 0x00, 0x02, 0xD9, 0x00, 0x06];
const NODE_ID = '02.01.57.00.00.01';

// ─── Mock state containers ───────────────────────────────────────────────────

const mockCatalogState = { catalog: null as BowtieCatalog | null };
const mockLayoutState = { layout: null as LayoutFile | null };
const mockTreesMap = new Map<string, NodeConfigTree>();
const mockNodeInfoMap = new Map<string, any>();
// pending bowtie metadata edits keyed like the real store
const mockRoleClassifications = new Map<string, { role: 'Producer' | 'Consumer' }>();
const mockPendingDeletes = new Set<string>();
const mockPendingCreates = new Map<string, { name?: string }>();
const mockDrafts = new Map<string, TreeConfigValue>();
const mockOfflinePending = new Map<string, TreeConfigValue>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

vi.mock('$lib/stores/nodeInfo', () => ({
  nodeInfoStore: {
    subscribe: (fn: (val: any) => void) => { fn(mockNodeInfoMap); return () => {}; },
  },
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: {
    get layout() { return mockLayoutState.layout; },
  },
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() { return mockTreesMap; },
    getTree(nodeId: string) { return mockTreesMap.get(nodeId) ?? null; },
  },
}));

vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: {
    get isDirty() {
      return mockPendingDeletes.size > 0 || mockPendingCreates.size > 0;
    },
    getMetadata(eventIdHex: string) {
      const pendingCreate = mockPendingCreates.get(eventIdHex);
      if (pendingCreate) {
        return { name: pendingCreate.name, tags: [] };
      }
      const layout = mockLayoutState.layout;
      return layout?.bowties[eventIdHex];
    },
    getDirtyFields(eventIdHex: string) {
      const dirty = new Set<string>();
      if (mockPendingCreates.has(eventIdHex)) dirty.add('name');
      return dirty;
    },
    get allEventIds() {
      return Array.from(mockPendingCreates.keys());
    },
    hasPendingDeletion(eventIdHex: string) {
      return mockPendingDeletes.has(eventIdHex);
    },
    getRoleClassification(key: string): RoleClassification | undefined {
      // Match the real store: pending classify edits first, then layout.
      // We don't model pending edits in tests yet — fall through to layout.
      return mockLayoutState.layout?.roleClassifications[key];
    },
  },
}));

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    overrideValue(key: string): TreeConfigValue | null {
      return mockDrafts.get(key) ?? mockOfflinePending.get(key) ?? null;
    },
    visibleValue(key: string): TreeConfigValue | null {
      return mockDrafts.get(key) ?? mockOfflinePending.get(key) ?? null;
    },
    hasDraftsForNode(_nodeId: string): boolean {
      return mockDrafts.size > 0;
    },
  },
}));

// ─── Helpers ─────────────────────────────────────────────────────────────────

function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Event ID',
    description: null,
    elementType: 'eventId',
    address: 100,
    size: 8,
    space: 253,
    path: ['seg:0', 'elem:0#1', 'elem:0'],
    value: { type: 'eventId', bytes: EVENT_BYTES, hex: EVENT_HEX },
    eventRole: 'Producer',
    constraints: null,
    ...overrides,
  };
}

function makeTree(nodeId: string, leaves: LeafConfigNode[]): NodeConfigTree {
  return {
    nodeId,
    identity: null,
    segments: [{
      name: 'Config',
      description: null,
      origin: 0,
      space: 253,
      children: leaves.map(l => l),
    }],
  };
}

function makeLayoutWith(opts: {
  bowties?: Record<string, { name?: string; tags: string[] }>;
  roleClassifications?: Record<string, RoleClassification>;
} = {}): LayoutFile {
  return {
    schemaVersion: '1.0',
    bowties: opts.bowties ?? {},
    roleClassifications: opts.roleClassifications ?? {},
    connectorSelections: {},
  };
}

function makeCatalogCard(opts: Partial<{
  eventIdHex: string;
  producers: EventSlotEntry[];
  consumers: EventSlotEntry[];
  ambiguous: EventSlotEntry[];
  state: 'Active' | 'Incomplete' | 'Planning';
}> = {}): BowtieCatalog {
  return {
    bowties: [{
      event_id_hex: opts.eventIdHex ?? EVENT_HEX,
      event_id_bytes: EVENT_BYTES,
      producers: opts.producers ?? [],
      consumers: opts.consumers ?? [],
      ambiguous_entries: opts.ambiguous ?? [],
      name: null,
      tags: [],
      state: opts.state ?? 'Active',
    }],
    built_at: '2026-01-01T00:00:00Z',
    source_node_count: 1,
    total_slots_scanned: 1,
  };
}

function makeEntry(overrides: Partial<EventSlotEntry> = {}): EventSlotEntry {
  return {
    node_id: NODE_ID,
    node_name: 'Test Node',
    element_path: ['seg:0', 'elem:0#1', 'elem:0'],
    element_label: 'Event ID',
    element_description: null,
    event_id: EVENT_BYTES,
    role: 'Producer',
    ...overrides,
  };
}

// ─── Module under test (imported AFTER mocks) ────────────────────────────────

const { effectiveLayoutStore, bowtieCatalogStore } = await import('./effectiveLayoutStore.svelte');

beforeEach(() => {
  mockCatalogState.catalog = null;
  mockLayoutState.layout = null;
  mockTreesMap.clear();
  mockNodeInfoMap.clear();
  mockRoleClassifications.clear();
  mockPendingDeletes.clear();
  mockPendingCreates.clear();
  mockDrafts.clear();
  mockOfflinePending.clear();
  bowtieCatalogStore.reset();
});

// ─── effectiveBowties ────────────────────────────────────────────────────────

describe('effectiveLayoutStore.effectiveBowties', () => {
  it('returns catalog cards from the underlying catalog', () => {
    bowtieCatalogStore.setCatalog(makeCatalogCard({
      producers: [makeEntry({ role: 'Producer' })],
      consumers: [makeEntry({ node_id: '02.01.57.00.00.02', role: 'Consumer' })],
    }));

    const cards = effectiveLayoutStore.effectiveBowties;

    expect(cards).toHaveLength(1);
    expect(cards[0].eventIdHex).toBe(EVENT_HEX);
    expect(cards[0].producers).toHaveLength(1);
    expect(cards[0].consumers).toHaveLength(1);
  });

  it('removes a card with a pending deleteBowtie edit (Bug 3 — immediate delete)', () => {
    bowtieCatalogStore.setCatalog(makeCatalogCard({
      producers: [makeEntry()],
      consumers: [makeEntry({ node_id: '02.01.57.00.00.02', role: 'Consumer' })],
    }));
    mockPendingDeletes.add(EVENT_HEX);

    const cards = effectiveLayoutStore.effectiveBowties;

    expect(cards.find(c => c.eventIdHex === EVENT_HEX)).toBeUndefined();
  });

  it('includes layout-only bowties even when catalog is null', () => {
    mockLayoutState.layout = makeLayoutWith({
      bowties: { [EVENT_HEX]: { name: 'Planned', tags: [] } },
    });

    const cards = effectiveLayoutStore.effectiveBowties;
    expect(cards).toHaveLength(1);
    expect(cards[0].name).toBe('Planned');
  });

  it('includes pending-create-only bowties (no catalog, no layout entry)', () => {
    mockPendingCreates.set(EVENT_HEX, { name: 'New Bowtie' });

    const cards = effectiveLayoutStore.effectiveBowties;
    expect(cards).toHaveLength(1);
    expect(cards[0].eventIdHex).toBe(EVENT_HEX);
    expect(cards[0].name).toBe('New Bowtie');
  });

  it('a pending delete wins over a layout-only entry', () => {
    mockLayoutState.layout = makeLayoutWith({
      bowties: { [EVENT_HEX]: { name: 'Stale', tags: [] } },
    });
    mockPendingDeletes.add(EVENT_HEX);

    const cards = effectiveLayoutStore.effectiveBowties;
    expect(cards.find(c => c.eventIdHex === EVENT_HEX)).toBeUndefined();
  });
});

// ─── effectiveValue ──────────────────────────────────────────────────────────

describe('effectiveLayoutStore.effectiveValue', () => {
  it('returns the leaf value when no override layer applies', () => {
    const leaf = makeLeaf();
    const v = effectiveLayoutStore.effectiveValue(NODE_ID, leaf);
    expect(v?.type).toBe('eventId');
    expect((v as any).hex).toBe(EVENT_HEX);
  });

  it('returns the draft value over the leaf baseline', () => {
    const leaf = makeLeaf();
    const draftHex = '02.01.57.00.02.D9.00.99';
    const editKey = editKeyForLeaf(NODE_ID, leaf.space, leaf.address);
    mockDrafts.set(editKey, { type: 'eventId', bytes: EVENT_BYTES, hex: draftHex });

    const v = effectiveLayoutStore.effectiveValue(NODE_ID, leaf);
    expect((v as any).hex).toBe(draftHex);
  });

  it('returns the offlinePending value when there is no draft', () => {
    const leaf = makeLeaf();
    const offlineHex = '02.01.57.00.02.D9.00.AB';
    const editKey = editKeyForLeaf(NODE_ID, leaf.space, leaf.address);
    mockOfflinePending.set(editKey, { type: 'eventId', bytes: EVENT_BYTES, hex: offlineHex });

    const v = effectiveLayoutStore.effectiveValue(NODE_ID, leaf);
    expect((v as any).hex).toBe(offlineHex);
  });
});

// ─── effectiveRole ───────────────────────────────────────────────────────────

describe('effectiveLayoutStore.effectiveRole', () => {
  it('returns the saved roleClassification from the layout when leaf.eventRole is null (Bug 2)', () => {
    const leaf = makeLeaf({ eventRole: null });
    const slotKey = `${NODE_ID}:${leaf.path.join('/')}`;
    mockLayoutState.layout = makeLayoutWith({
      roleClassifications: { [slotKey]: { role: 'Consumer' } },
    });

    const role = effectiveLayoutStore.effectiveRole(NODE_ID, leaf);
    expect(role).toBe('Consumer');
  });

  it('returns the leaf baseline role when no override layer applies', () => {
    const leaf = makeLeaf({ eventRole: 'Producer' });
    const role = effectiveLayoutStore.effectiveRole(NODE_ID, leaf);
    expect(role).toBe('Producer');
  });

  it('returns null when no layer has a role for the slot', () => {
    const leaf = makeLeaf({ eventRole: null });
    const role = effectiveLayoutStore.effectiveRole(NODE_ID, leaf);
    expect(role).toBeNull();
  });

  it('catalog role wins over null leaf baseline', () => {
    // a leaf whose CDI marks it Ambiguous gets classified to Consumer
    // by some other catalog card containing this slot
    const leaf = makeLeaf({ eventRole: null, address: 200, path: ['seg:0', 'a', 'b'] });
    const slotEntry = makeEntry({
      role: 'Consumer',
      element_path: ['seg:0', 'a', 'b'],
    });
    bowtieCatalogStore.setCatalog(makeCatalogCard({
      consumers: [slotEntry],
    }));

    const role = effectiveLayoutStore.effectiveRole(NODE_ID, leaf);
    expect(role).toBe('Consumer');
  });
});

// ─── slotsByRole ─────────────────────────────────────────────────────────────

describe('effectiveLayoutStore.slotsByRole', () => {
  it('returns only leaves whose effective role matches', () => {
    const producerLeaf = makeLeaf({
      address: 100,
      path: ['seg:0', 'p', 'evt'],
      eventRole: 'Producer',
    });
    const consumerLeaf = makeLeaf({
      address: 200,
      path: ['seg:0', 'c', 'evt'],
      eventRole: 'Consumer',
    });
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, [producerLeaf, consumerLeaf]));

    const producers = effectiveLayoutStore.slotsByRole(NODE_ID, 'Producer');
    const consumers = effectiveLayoutStore.slotsByRole(NODE_ID, 'Consumer');

    expect(producers.map(l => l.address)).toEqual([100]);
    expect(consumers.map(l => l.address)).toEqual([200]);
  });

  it('honors saved roleClassifications when leaf.eventRole is null (Bug 2)', () => {
    const leaf = makeLeaf({
      address: 100,
      path: ['seg:0', 'x', 'evt'],
      eventRole: null,
    });
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, [leaf]));
    const slotKey = `${NODE_ID}:${leaf.path.join('/')}`;
    mockLayoutState.layout = makeLayoutWith({
      roleClassifications: { [slotKey]: { role: 'Consumer' } },
    });

    const consumers = effectiveLayoutStore.slotsByRole(NODE_ID, 'Consumer');
    expect(consumers).toHaveLength(1);
    expect(consumers[0].address).toBe(100);
  });

  it('null roleFilter returns all event-id leaves', () => {
    const a = makeLeaf({ address: 1, path: ['seg:0', 'a'], eventRole: 'Producer' });
    const b = makeLeaf({ address: 2, path: ['seg:0', 'b'], eventRole: null });
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, [a, b]));

    const all = effectiveLayoutStore.slotsByRole(NODE_ID, null);
    expect(all).toHaveLength(2);
  });
});

// ─── isSlotFree ──────────────────────────────────────────────────────────────

describe('effectiveLayoutStore.isSlotFree', () => {
  it('returns true when the leaf has no value', () => {
    const leaf = makeLeaf({ value: null });
    expect(effectiveLayoutStore.isSlotFree(NODE_ID, leaf)).toBe(true);
  });

  it('returns false when the leafs event ID participates in a catalog bowtie', () => {
    bowtieCatalogStore.setCatalog(makeCatalogCard({
      producers: [makeEntry()],
      consumers: [makeEntry({ node_id: '02.01.57.00.00.02', role: 'Consumer' })],
    }));
    const leaf = makeLeaf();
    expect(effectiveLayoutStore.isSlotFree(NODE_ID, leaf)).toBe(false);
  });

  it('returns false for placeholder (leading-zero) event IDs', () => {
    const leaf = makeLeaf({
      value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 1], hex: '00.00.00.00.00.00.00.01' },
    });
    expect(effectiveLayoutStore.isSlotFree(NODE_ID, leaf)).toBe(false);
  });

  it('returns true when the only card referencing this event has a pending delete (Bug 3)', () => {
    bowtieCatalogStore.setCatalog(makeCatalogCard({
      producers: [makeEntry()],
      consumers: [makeEntry({ node_id: '02.01.57.00.00.02', role: 'Consumer' })],
    }));
    mockPendingDeletes.add(EVENT_HEX);
    const leaf = makeLeaf();
    expect(effectiveLayoutStore.isSlotFree(NODE_ID, leaf)).toBe(true);
  });
});
