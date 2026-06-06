/**
 * Store-level tests for the editable bowtie preview merge logic.
 *
 * Per ADR-0004 / S2c the merge is owned by `effectiveLayoutStore` in
 * `$lib/layout`, which composes:
 *   - `buildEffectiveBowtiePreview()` (catalog × tree × metadata × layout)
 *   - the pending-deletion filter on `bowtieMetadataStore`
 *
 * These tests exercise the merge through the facade and verify the four
 * scenarios that previously lived under `EditableBowtiePreviewStore`:
 *
 *   1. Layout bowtie visible when catalog is null (Bug 4 fix)
 *   2. Layout bowtie shows empty entries before any tree is loaded
 *   3. Entries appear after a tree with matching eventId is populated
 *   4. Catalog card correctly merges with tree-derived entries
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NodeConfigTree, LeafConfigNode } from '$lib/types/nodeTree';
import type { BowtieCatalog, EventSlotEntry } from '$lib/api/tauri';
import type { LayoutFile } from '$lib/types/bowtie';

// ─── Shared test event ID ────────────────────────────────────────────────────

const TEST_EVENT_HEX = '02.01.57.00.02.D9.00.06';
const TEST_EVENT_BYTES = [0x02, 0x01, 0x57, 0x00, 0x02, 0xD9, 0x00, 0x06];

// ─── Mock setup ──────────────────────────────────────────────────────────────

// Mutable state objects that tests can manipulate to drive store behavior.
const mockCatalogState = { catalog: null as BowtieCatalog | null };
const mockLayoutState = { layout: null as LayoutFile | null };
const mockTreesMap = new Map<string, NodeConfigTree>();
const mockMetadataEdits = new Map<string, any>();
const mockNodeInfoMap = new Map<string, any>();
// Controllable role-classification map — tests set entries to exercise Bug 2 fix.
const mockRoleClassificationsMap = new Map<string, { role: string }>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

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
    get isDirty() { return false; },
    getMetadata(_eventIdHex: string) { return undefined; },
    getDirtyFields(_eventIdHex: string) { return new Set<string>(); },
    get allEventIds() { return []; },
    getRoleClassification(key: string) { return mockRoleClassificationsMap.get(key); },
    // ADR-0004: facade-level filter; no pending deletions in these store tests.
    hasPendingDeletion(_eventIdHex: string) { return false; },
  },
}));

// ─── Helpers ─────────────────────────────────────────────────────────────────

function makeEventIdLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Event ID',
    description: null,
    elementType: 'eventId',
    address: 100,
    size: 8,
    space: 253,
    path: ['seg:0', 'elem:0#1', 'elem:0'],
    value: { type: 'eventId', bytes: TEST_EVENT_BYTES, hex: TEST_EVENT_HEX },
    eventRole: 'Producer',
    constraints: null,
    ...overrides,
  };
}

function makePlaceholderLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Placeholder Event ID',
    description: null,
    elementType: 'eventId',
    address: 120,
    size: 8,
    space: 253,
    path: ['seg:0', 'elem:9#1', 'elem:0'],
    value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 1], hex: '00.00.00.00.00.00.00.01' },
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

function makeLayout(eventIdHex: string, name: string): LayoutFile {
  return {
    schemaVersion: '1.0',
    bowties: {
      [eventIdHex]: { name, tags: [] },
    },
    roleClassifications: {},
  };
}

function makeCatalogWithCard(eventIdHex: string): BowtieCatalog {
  const entry: EventSlotEntry = {
    node_key: '020157000001',
    node_name: 'Test Node',
    element_path: ['seg:0', 'elem:0#1', 'elem:0'],
    element_label: 'Config.Event ID',
    element_description: null,
    event_id: TEST_EVENT_BYTES,
    role: 'Producer',
  };
  return {
    bowties: [{
      event_id_hex: eventIdHex,
      event_id_bytes: TEST_EVENT_BYTES,
      producers: [entry],
      consumers: [{
        ...entry,
        node_key: '020157000002',
        node_name: 'Node 2',
        element_path: ['seg:0', 'elem:1#1', 'elem:0'],
        role: 'Consumer',
      }],
      ambiguous_entries: [],
      name: null,
      tags: [],
      state: 'Active',
    }],
    built_at: '2026-01-01T00:00:00Z',
    source_node_count: 2,
    total_slots_scanned: 10,
  };
}

// ─── Tests ───────────────────────────────────────────────────────────────────

// Import AFTER mocks are in place so module-level singletons pick them up.
// Per ADR-0004, the merge is exercised through the layout facade.
const { bowtieCatalogStore } = await import('$lib/stores/bowties.svelte');
const { effectiveLayoutStore } = await import('$lib/layout');

// Test-local alias: tests below read `editableBowtiePreviewStore.preview` as
// a shorthand for the facade's preview. The facade owns the merge and applies
// the pending-deletion filter; in these tests no deletions are pending, so
// the alias surfaces exactly what the merge function produces.
const editableBowtiePreviewStore = effectiveLayoutStore;

beforeEach(() => {
  mockCatalogState.catalog = null;
  mockLayoutState.layout = null;
  mockTreesMap.clear();
  mockMetadataEdits.clear();
  mockNodeInfoMap.clear();
  mockRoleClassificationsMap.clear();
  bowtieCatalogStore.reset();
});

describe('effectiveLayoutStore.preview (ADR-0004 / S2c)', () => {
  // ── Scenario 1: Layout bowtie visible when catalog is null ────────────────

  it('shows layout bowtie even when catalog is null (Bug 4)', () => {
    // Arrange: layout has a bowtie, catalog not yet built
    mockLayoutState.layout = makeLayout(TEST_EVENT_HEX, 'Test Bowtie');

    // Act
    const preview = editableBowtiePreviewStore.preview;

    // Assert: the bowtie from the layout should appear
    expect(preview.bowties.length).toBe(1);
    expect(preview.bowties[0].eventIdHex).toBe(TEST_EVENT_HEX);
    expect(preview.bowties[0].name).toBe('Test Bowtie');
    expect(preview.bowties[0].state).toBe('planning');
  });

  // ── Scenario 2: Layout bowtie has empty entries before tree load ──────────

  it('layout bowtie has no producer/consumer entries before tree is loaded', () => {
    // Arrange: layout bowtie exists, but no node trees are loaded yet
    mockLayoutState.layout = makeLayout(TEST_EVENT_HEX, 'Test Bowtie');
    // mockTreesMap is empty — simulates "node discovered but CDI not read"

    // Act
    const preview = editableBowtiePreviewStore.preview;

    // Assert: card exists but no slots populated
    expect(preview.bowties.length).toBe(1);
    expect(preview.bowties[0].producers.length).toBe(0);
    expect(preview.bowties[0].consumers.length).toBe(0);
  });

  // ── Scenario 3: Entries appear after tree is populated ────────────────────

  it('shows producer/consumer entries after tree with matching eventId is loaded', () => {
    // Arrange: layout + tree with a producer and a consumer for the same event ID
    mockLayoutState.layout = makeLayout(TEST_EVENT_HEX, 'Test Bowtie');

    const producerLeaf = makeEventIdLeaf({
      address: 100,
      path: ['seg:0', 'elem:0#1', 'elem:0'],
      eventRole: 'Producer',
    });
    const consumerLeaf = makeEventIdLeaf({
      address: 200,
      path: ['seg:0', 'elem:1#1', 'elem:0'],
      eventRole: 'Consumer',
    });

    const tree = makeTree('02.01.57.00.00.01', [producerLeaf, consumerLeaf]);
    mockTreesMap.set('02.01.57.00.00.01', tree);

    // Act
    const preview = editableBowtiePreviewStore.preview;

    // Assert: both entries should now be present
    expect(preview.bowties.length).toBe(1);
    expect(preview.bowties[0].producers.length).toBe(1);
    expect(preview.bowties[0].consumers.length).toBe(1);
  });

  // ── Scenario 5: JS-side role classification overrides Rust tree eventRole ──

  it('uses JS-side role classification to place ambiguous slot as producer (Bug 2 fix)', () => {
    // Arrange: layout bowtie + tree with one Ambiguous leaf that was classified as Producer
    mockLayoutState.layout = makeLayout(TEST_EVENT_HEX, 'Test Bowtie');

    const nodeId = '02.01.57.00.00.01';
    const leafPath = ['seg:0', 'elem:0#1', 'elem:0'];
    const leaf = makeEventIdLeaf({
      path: leafPath,
      eventRole: null, // Ambiguous/unclassified in Rust tree
    });
    mockTreesMap.set(nodeId, makeTree(nodeId, [leaf]));

    // User classified this slot as Producer via the picker
    const slotKey = `${nodeId}:${leafPath.join('/')}`;
    mockRoleClassificationsMap.set(slotKey, { role: 'Producer' });

    // Act
    const preview = editableBowtiePreviewStore.preview;

    // Assert: entry must appear as a producer, not a consumer
    const card = preview.bowties.find(b => b.eventIdHex === TEST_EVENT_HEX);
    expect(card).toBeDefined();
    expect(card!.producers.length).toBe(1);
    expect(card!.consumers.length).toBe(0);
  });

  // ── Scenario 4: Catalog card merges with tree entries ─────────────────────

  it('catalog card includes tree-derived entries for known event IDs', () => {
    // Arrange: catalog has a card with 1 producer + 1 consumer
    const catalog = makeCatalogWithCard(TEST_EVENT_HEX);
    bowtieCatalogStore.setCatalog(catalog);

    // Also have a tree loaded with a THIRD entry (another consumer on a different node)
    const extraConsumerLeaf = makeEventIdLeaf({
      address: 300,
      path: ['seg:0', 'elem:2#1', 'elem:0'],
      eventRole: 'Consumer',
    });
    const tree = makeTree('02.01.57.00.00.03', [extraConsumerLeaf]);
    mockTreesMap.set('02.01.57.00.00.03', tree);

    // Act
    const preview = editableBowtiePreviewStore.preview;

    // Assert: ADR-0004 / S2c — the preview is a single derivation that always
    // merges tree-discovered entries with catalog cards. The third entry
    // surfaces even when the catalog already has the event ID, because the
    // tree is authoritative for *current* slot membership (the catalog may
    // be a snapshot from before the latest CDI read).
    const card = preview.bowties.find(b => b.eventIdHex === TEST_EVENT_HEX);
    expect(card).toBeDefined();
    expect(card!.producers.length).toBe(1);
    expect(card!.consumers.length).toBe(2);
    // The new tree-discovered entry marks the card as dirty (elements changed).
    expect(card!.dirtyFields.has('elements')).toBe(true);
  });

  it('derives offline bowties from loaded trees when layout metadata is empty', () => {
    const producerLeaf = makeEventIdLeaf({
      path: ['seg:0', 'elem:0#1', 'elem:0'],
      eventRole: 'Producer',
    });
    const consumerLeaf = makeEventIdLeaf({
      address: 200,
      path: ['seg:0', 'elem:1#1', 'elem:0'],
      eventRole: 'Consumer',
    });

    mockTreesMap.set('02.01.57.00.00.01', makeTree('02.01.57.00.00.01', [producerLeaf, consumerLeaf]));

    const preview = editableBowtiePreviewStore.preview;

    expect(preview.bowties).toHaveLength(1);
    expect(preview.bowties[0].eventIdHex).toBe(TEST_EVENT_HEX);
    expect(preview.bowties[0].producers).toHaveLength(1);
    expect(preview.bowties[0].consumers).toHaveLength(1);
    expect(preview.bowties[0].state).toBe('active');
  });

  it('ignores placeholder event IDs when deriving offline bowties from trees', () => {
    mockTreesMap.set('02.01.57.00.00.01', makeTree('02.01.57.00.00.01', [makePlaceholderLeaf()]));

    const preview = editableBowtiePreviewStore.preview;

    expect(preview.bowties).toHaveLength(0);
  });

  it('filters out single-entry normal events when deriving offline bowties from trees', () => {
    mockTreesMap.set('02.01.57.00.00.01', makeTree('02.01.57.00.00.01', [makeEventIdLeaf()]));

    const preview = editableBowtiePreviewStore.preview;

    expect(preview.bowties).toHaveLength(0);
  });

  it('keeps single-entry well-known events when deriving offline bowties from trees', () => {
    const wellKnownLeaf = makeEventIdLeaf({
      value: {
        type: 'eventId',
        bytes: [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF],
        hex: '01.00.00.00.00.00.FF.FF',
      },
    });
    mockTreesMap.set('02.01.57.00.00.01', makeTree('02.01.57.00.00.01', [wellKnownLeaf]));

    const preview = editableBowtiePreviewStore.preview;

    expect(preview.bowties).toHaveLength(1);
    expect(preview.bowties[0].eventIdHex).toBe('01.00.00.00.00.00.FF.FF');
  });

  // ── Bug 3 regression: ambiguous entries must be enriched with element_label ──

  it('enriches ambiguous entries with element_label from tree (Bug 3)', () => {
    // Arrange: catalog has a card with an ambiguous entry (no element_label from Rust)
    const ambiguousEntry: EventSlotEntry = {
      node_key: '020157000001',
      node_name: 'Test Node',
      element_path: ['seg:0', 'elem:0#1', 'elem:0'],
      element_description: 'Some CDI description',
      event_id: TEST_EVENT_BYTES,
      role: 'Ambiguous',
      // Note: no element_label — Rust doesn't send it
    };
    bowtieCatalogStore.setCatalog({
      bowties: [{
        event_id_hex: TEST_EVENT_HEX,
        event_id_bytes: TEST_EVENT_BYTES,
        producers: [],
        consumers: [],
        ambiguous_entries: [ambiguousEntry],
        name: 'Test Bowtie',
        tags: [],
        state: 'Incomplete',
      }],
      built_at: '2026-01-01T00:00:00Z',
      source_node_count: 1,
      total_slots_scanned: 5,
    });

    // No tree loaded — fallback path: element_label = element_path.join('.')
    const preview = editableBowtiePreviewStore.preview;
    const card = preview.bowties.find(b => b.eventIdHex === TEST_EVENT_HEX);
    expect(card).toBeDefined();
    expect(card!.ambiguousEntries).toHaveLength(1);
    // Bug 3 regression: without enrichment, element_label would be undefined.
    // With enrichment, it falls back to element_path.join('.').
    expect(card!.ambiguousEntries[0].element_label).toBe('seg:0.elem:0#1.elem:0');
  });

  it('enriches ambiguous entries with tree-derived label when tree is available (Bug 3)', () => {
    // Arrange: catalog with ambiguous entry + matching tree loaded
    const nodeId = '02.01.57.00.00.01';
    const ambiguousEntry: EventSlotEntry = {
      node_key: nodeId,
      node_name: 'Test Node',
      element_path: ['seg:0', 'elem:0#1', 'elem:0'],
      element_description: 'Some CDI description',
      event_id: TEST_EVENT_BYTES,
      role: 'Ambiguous',
    };
    bowtieCatalogStore.setCatalog({
      bowties: [{
        event_id_hex: TEST_EVENT_HEX,
        event_id_bytes: TEST_EVENT_BYTES,
        producers: [],
        consumers: [],
        ambiguous_entries: [ambiguousEntry],
        name: 'Test Bowtie',
        tags: [],
        state: 'Incomplete',
      }],
      built_at: '2026-01-01T00:00:00Z',
      source_node_count: 1,
      total_slots_scanned: 5,
    });

    // Load a tree so enrichment resolves the real label
    const leaf = makeEventIdLeaf({ eventRole: null });
    mockTreesMap.set(nodeId, makeTree(nodeId, [leaf]));

    const preview = editableBowtiePreviewStore.preview;
    const card = preview.bowties.find(b => b.eventIdHex === TEST_EVENT_HEX);
    expect(card).toBeDefined();
    expect(card!.ambiguousEntries).toHaveLength(1);
    // With tree available, enrichment produces a tree-derived label (e.g. "Event ID")
    expect(card!.ambiguousEntries[0].element_label).toBeDefined();
    expect(card!.ambiguousEntries[0].element_label).not.toBe('');
  });
});

// ─── Display threshold: single-slot unnamed cards are classification-only ────

describe('display threshold for single-slot unnamed catalog cards', () => {
  const SINGLE_SLOT_EVENT_HEX = '05.02.01.02.02.00.01.00';
  const SINGLE_SLOT_EVENT_BYTES = [0x05, 0x02, 0x01, 0x02, 0x02, 0x00, 0x01, 0x00];

  function makeCatalogWithSingleSlotCard(): BowtieCatalog {
    return {
      bowties: [{
        event_id_hex: SINGLE_SLOT_EVENT_HEX,
        event_id_bytes: SINGLE_SLOT_EVENT_BYTES,
        producers: [{
          node_key: '020157000001',
          node_name: 'Test Node',
          element_path: ['seg:0', 'elem:0'],
          element_description: null,
          event_id: SINGLE_SLOT_EVENT_BYTES,
          role: 'Producer',
        }],
        consumers: [],
        ambiguous_entries: [],
        name: null,
        tags: [],
        state: 'Incomplete',
      }],
      built_at: '2026-01-01T00:00:00Z',
      source_node_count: 1,
      total_slots_scanned: 10,
    };
  }

  it('preview excludes single-slot unnamed cards', () => {
    bowtieCatalogStore.setCatalog(makeCatalogWithSingleSlotCard());
    const preview = editableBowtiePreviewStore.preview;
    expect(preview.bowties).toHaveLength(0);
  });

  it('preview includes single-slot unnamed cards when bowtie exists in layout', () => {
    // User created this bowtie explicitly. Even though only one side has the
    // matching event ID in the snapshot (offline consumer change not yet applied),
    // the bowtie must remain visible because it's in the layout file.
    bowtieCatalogStore.setCatalog(makeCatalogWithSingleSlotCard());
    mockLayoutState.layout = makeLayout(SINGLE_SLOT_EVENT_HEX, undefined as unknown as string);
    // Remove the name so it's truly unnamed
    mockLayoutState.layout!.bowties[SINGLE_SLOT_EVENT_HEX] = { name: undefined as unknown as string, tags: [] };
    const preview = editableBowtiePreviewStore.preview;
    expect(preview.bowties).toHaveLength(1);
    expect(preview.bowties[0].eventIdHex).toBe(SINGLE_SLOT_EVENT_HEX);
  });

  it('displayableBowties includes single-slot unnamed cards when in layout', () => {
    bowtieCatalogStore.setCatalog(makeCatalogWithSingleSlotCard());
    mockLayoutState.layout = { schemaVersion: '1.0', bowties: { [SINGLE_SLOT_EVENT_HEX]: { tags: [] } as any }, roleClassifications: {} };
    expect(bowtieCatalogStore.displayableBowties.length).toBe(1);
  });

  it('preview includes single-slot cards when they have a name', () => {
    const catalog = makeCatalogWithSingleSlotCard();
    catalog.bowties[0].name = 'BTN A Pressed';
    bowtieCatalogStore.setCatalog(catalog);
    const preview = editableBowtiePreviewStore.preview;
    expect(preview.bowties).toHaveLength(1);
    expect(preview.bowties[0].name).toBe('BTN A Pressed');
  });

  it('nodeSlotMap excludes entries from single-slot unnamed cards', () => {
    bowtieCatalogStore.setCatalog(makeCatalogWithSingleSlotCard());
    const map = bowtieCatalogStore.nodeSlotMap;
    expect(map.size).toBe(0);
  });

  it('nodeSlotMap includes entries from multi-slot cards', () => {
    const catalog = makeCatalogWithCard(TEST_EVENT_HEX);
    bowtieCatalogStore.setCatalog(catalog);
    const map = bowtieCatalogStore.nodeSlotMap;
    expect(map.size).toBe(2); // producer + consumer
  });

  it('nodeSlotMap includes entries from single-slot named cards', () => {
    const catalog = makeCatalogWithSingleSlotCard();
    catalog.bowties[0].name = 'BTN A Pressed';
    bowtieCatalogStore.setCatalog(catalog);
    const map = bowtieCatalogStore.nodeSlotMap;
    expect(map.size).toBe(1);
  });

  it('displayableBowties count excludes single-slot unnamed cards', () => {
    const catalog = makeCatalogWithSingleSlotCard();
    // Add a real multi-slot card too
    const multiSlotCard = makeCatalogWithCard(TEST_EVENT_HEX).bowties[0];
    catalog.bowties.push(multiSlotCard);
    bowtieCatalogStore.setCatalog(catalog);
    expect(bowtieCatalogStore.displayableBowties.length).toBe(1);
  });
});

// ─── getRoleForSlot: authoritative role lookup across all catalog cards ──────

describe('getRoleForSlot', () => {
  const SINGLE_SLOT_EVENT_HEX = '05.02.01.02.02.00.01.00';
  const SINGLE_SLOT_EVENT_BYTES = [0x05, 0x02, 0x01, 0x02, 0x02, 0x00, 0x01, 0x00];

  it('returns Producer for a slot in a catalog card\'s producers list', () => {
    const catalog = makeCatalogWithCard(TEST_EVENT_HEX);
    bowtieCatalogStore.setCatalog(catalog);
    // The producer entry has node_id '02.01.57.00.00.01' and path ['seg:0', 'elem:0#1', 'elem:0']
    const role = bowtieCatalogStore.getRoleForSlot('02.01.57.00.00.01', ['seg:0', 'elem:0#1', 'elem:0']);
    expect(role).toBe('Producer');
  });

  it('returns Consumer for a slot in a catalog card\'s consumers list', () => {
    const catalog = makeCatalogWithCard(TEST_EVENT_HEX);
    bowtieCatalogStore.setCatalog(catalog);
    const role = bowtieCatalogStore.getRoleForSlot('02.01.57.00.00.02', ['seg:0', 'elem:1#1', 'elem:0']);
    expect(role).toBe('Consumer');
  });

  it('returns role from sub-threshold single-slot cards', () => {
    // Single-slot unnamed card — not displayable, but role should still be found
    bowtieCatalogStore.setCatalog({
      bowties: [{
        event_id_hex: SINGLE_SLOT_EVENT_HEX,
        event_id_bytes: SINGLE_SLOT_EVENT_BYTES,
        producers: [{
          node_key: '020157000001',
          node_name: 'Test Node',
          element_path: ['seg:0', 'elem:3'],
          element_description: null,
          event_id: SINGLE_SLOT_EVENT_BYTES,
          role: 'Producer',
        }],
        consumers: [],
        ambiguous_entries: [],
        name: null,
        tags: [],
        state: 'Incomplete',
      }],
      built_at: '2026-01-01T00:00:00Z',
      source_node_count: 1,
      total_slots_scanned: 10,
    });
    const role = bowtieCatalogStore.getRoleForSlot('02.01.57.00.00.01', ['seg:0', 'elem:3']);
    expect(role).toBe('Producer');
  });

  it('returns null for a slot not in any catalog card', () => {
    bowtieCatalogStore.setCatalog(makeCatalogWithCard(TEST_EVENT_HEX));
    const role = bowtieCatalogStore.getRoleForSlot('99.99.99.99.99.99', ['seg:0', 'elem:0']);
    expect(role).toBeNull();
  });

  it('returns null when catalog is empty', () => {
    const role = bowtieCatalogStore.getRoleForSlot('02.01.57.00.00.01', ['seg:0', 'elem:0']);
    expect(role).toBeNull();
  });
});
