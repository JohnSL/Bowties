/**
 * Store-level tests for EditableBowtiePreviewStore.
 *
 * Tests the reactive preview derivation that merges catalog, layout,
 * metadata, and tree data.  Exercises the four scenarios identified
 * as untested:
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
  },
}));

vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: {
    get isDirty() { return false; },
    getMetadata(_eventIdHex: string) { return undefined; },
    get allEventIds() { return []; },
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
    node_id: '02.01.57.00.00.01',
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
        node_id: '02.01.57.00.00.02',
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

// Import AFTER mocks are in place so module-level singleton picks them up.
const { editableBowtiePreviewStore, bowtieCatalogStore } = await import('$lib/stores/bowties.svelte');

beforeEach(() => {
  mockCatalogState.catalog = null;
  mockLayoutState.layout = null;
  mockTreesMap.clear();
  mockMetadataEdits.clear();
  mockNodeInfoMap.clear();
  bowtieCatalogStore.reset();
});

describe('EditableBowtiePreviewStore.preview', () => {
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

    // Assert: catalog's original 1P+1C plus the new tree consumer = 1P + 2C
    const card = preview.bowties.find(b => b.eventIdHex === TEST_EVENT_HEX);
    expect(card).toBeDefined();
    expect(card!.producers.length).toBe(1);
    expect(card!.consumers.length).toBe(2);
    expect(card!.isDirty).toBe(true); // new entry makes it dirty
  });
});
