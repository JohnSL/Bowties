/**
 * Tests for BowtieCatalogPanel.svelte — CTA and not-ready branches.
 *
 * Covers the props introduced in this session:
 *   - hasUnreadNodes: when true, shows the "Read Node Configuration" CTA
 *     with node count and an active button
 *   - readingConfig: disables the CTA button while reading is in progress
 *   - nodesCount / unreadCount: appear in CTA text
 *   - readComplete=false + hasUnreadNodes=false: shows the "not ready" fallback
 *   - readComplete=true: shows the catalog content (or EmptyState)
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import BowtieCatalogPanel from './BowtieCatalogPanel.svelte';

// ─── Mocks ────────────────────────────────────────────────────────────────────

const {
  mockSetModifiedValue,
  mockTreeByNodeId,
  mockDraftConfigChange,
  mockUpsertConfigChange,
  mockCatalogState,
  mockLayoutState,
  mockClassifications,
  mockMetadata,
} = vi.hoisted(() => {
  const draftConfigChange = { value: null as any };
  return {
    mockSetModifiedValue: vi.fn(),
    mockTreeByNodeId: new Map<string, any>(),
    mockDraftConfigChange: draftConfigChange,
    mockUpsertConfigChange: vi.fn((change) => {
      draftConfigChange.value = {
        changeId: 'draft-1',
        kind: 'config',
        status: 'pending',
        ...change,
      };
    }),
    // Stateful mock backing for the real `buildEffectiveBowtiePreview` merge
    // when a test opts in by calling it directly via `vi.importActual`.
    mockCatalogState: { catalog: null as any },
    mockLayoutState: { layout: null as any },
    mockClassifications: new Map<string, { role: 'Producer' | 'Consumer' }>(),
    mockMetadata: new Map<string, any>(),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const mockReadComplete = { value: false };
const mockHasLayoutFile = { value: false };
const mockIsOfflineMode = { value: false };
const mockPreviewCards: any[] = [];

vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    get catalog() { return mockCatalogState.catalog; },
    get readComplete() { return mockReadComplete.value; },
    get displayableBowties() { return []; },
  },
  // ADR-0004: the editable preview is exposed through `$lib/layout`; this
  // module-level stub here only exists for compatibility with any legacy
  // imports that may still resolve through the bowties module.
  buildEffectiveBowtiePreview: () => ({ bowties: mockPreviewCards, hasUnsavedChanges: false }),
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: {
    get hasLayoutFile() { return mockHasLayoutFile.value; },
    get isOfflineMode() { return mockIsOfflineMode.value; },
    get layout() { return mockLayoutState.layout; },
  },
}));

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: {
    upsertConfigChange: mockUpsertConfigChange,
    findDraftConfigChange: vi.fn(() => mockDraftConfigChange.value),
    findPersistedConfigChange: vi.fn(() => null),
  },
}));

vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: {
    getAllTags: () => [],
    hasPendingDeletion: () => false,
    get isDirty() { return false; },
    getMetadata: (id: string) => mockMetadata.get(id),
    getDirtyFields: () => new Set<string>(),
    getRoleClassification: (key: string) => mockClassifications.get(key),
    get allEventIds() { return [] as string[]; },
    classifyRole: (key: string, role: 'Producer' | 'Consumer') =>
      mockClassifications.set(key, { role }),
  },
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() { return mockTreeByNodeId; },
    getTree: (nodeId: string) => mockTreeByNodeId.get(nodeId),
  },
}));

vi.mock('$lib/stores/connectionRequest.svelte', () => ({
  connectionRequestStore: {
    get pendingRequest() { return null; },
    clearRequest: vi.fn(),
  },
}));

vi.mock('$lib/stores/bowtieFocus.svelte', () => ({
  bowtieFocusStore: {
    get focusRequest() { return null; },
    focusBowtie: vi.fn(),
    get highlightedEventIdHex() { return null; },
  },
}));

vi.mock('$lib/api/config', () => ({
  setModifiedValue: mockSetModifiedValue,
}));

const mockApplyEdit = vi.fn();
vi.mock('$lib/stores/configEditor.svelte', () => ({
  configEditor: {
    applyEdit: (...args: unknown[]) => mockApplyEdit(...args),
  },
}));

vi.mock('$lib/orchestration/configDraftOrchestrator', () => ({
  flushDraftToBackend: vi.fn(),
}));

vi.mock('$lib/utils/eventIds', async (importOriginal) => {
  const actual = await importOriginal<typeof import('$lib/utils/eventIds')>();
  return {
    ...actual,
    generateFreshEventIdForNode: vi.fn(() => '00.00.00.00.00.00.00.00'),
  };
});

// ─── Tests ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockReadComplete.value = false;
  mockHasLayoutFile.value = false;
  mockIsOfflineMode.value = false;
  mockPreviewCards.length = 0;
  mockTreeByNodeId.clear();
  mockDraftConfigChange.value = null;
  mockCatalogState.catalog = null;
  mockLayoutState.layout = null;
  mockClassifications.clear();
  mockMetadata.clear();
  vi.clearAllMocks();
});

function makeEventLeaf(overrides: Record<string, any> = {}) {
  return {
    kind: 'leaf',
    name: 'Event ID',
    description: null,
    elementType: 'eventId',
    address: 512,
    size: 8,
    space: 253,
    path: ['seg:0', 'elem:1'],
    value: {
      type: 'eventId',
      bytes: [0x05, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x01],
      hex: '05.01.01.01.00.00.00.01',
    },
    eventRole: 'Consumer',
    constraints: null,
    actionValue: 0,
    hintSlider: null,
    hintRadio: false,
    modifiedValue: null,
    writeState: null,
    writeError: null,
    readOnly: false,
    ...overrides,
  };
}

function makeTree(nodeId: string, leaves: any[]) {
  return {
    nodeId,
    identity: {
      manufacturer: 'ACME',
      model: 'Node',
      hardwareVersion: null,
      softwareVersion: null,
    },
    segments: [
      {
        name: 'Configuration',
        description: null,
        origin: 0,
        space: 253,
        children: leaves,
      },
    ],
  };
}

describe('BowtieCatalogPanel — CTA (hasUnreadNodes=true)', () => {
  it('shows the "Read Node Configuration" button', () => {
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: true,
        onReadConfig: vi.fn(),
        nodesCount: 3,
        unreadCount: 3,
        readingConfig: false,
      },
    });
    expect(screen.getByRole('button', { name: /read node configuration/i })).toBeInTheDocument();
  });

  it('displays the node count in the description', () => {
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: true,
        onReadConfig: vi.fn(),
        nodesCount: 2,
        unreadCount: 2,
        readingConfig: false,
      },
    });
    expect(screen.getByText(/2 nodes discovered/i)).toBeInTheDocument();
  });

  it('shows singular "node" for a single node', () => {
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: true,
        onReadConfig: vi.fn(),
        nodesCount: 1,
        unreadCount: 1,
        readingConfig: false,
      },
    });
    expect(screen.getByText(/1 node discovered/i)).toBeInTheDocument();
  });

  it('calls onReadConfig when the button is clicked', async () => {
    const onReadConfig = vi.fn();
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: true,
        onReadConfig,
        nodesCount: 2,
        unreadCount: 2,
        readingConfig: false,
      },
    });
    await fireEvent.click(screen.getByRole('button', { name: /read node configuration/i }));
    expect(onReadConfig).toHaveBeenCalledOnce();
  });

  it('disables the button while readingConfig is true', () => {
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: true,
        onReadConfig: vi.fn(),
        nodesCount: 2,
        unreadCount: 2,
        readingConfig: true,
      },
    });
    expect(screen.getByRole('button', { name: /read node configuration/i })).toBeDisabled();
  });

  it('shows the unread badge count', () => {
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: true,
        onReadConfig: vi.fn(),
        nodesCount: 3,
        unreadCount: 3,
        readingConfig: false,
      },
    });
    expect(screen.getByText(/3 unread/i)).toBeInTheDocument();
  });
});

describe('BowtieCatalogPanel — not-ready fallback (hasUnreadNodes=false, readComplete=false)', () => {
  it('shows the "not ready" message instead of the CTA', () => {
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: false,
        readingConfig: false,
      },
    });
    expect(screen.queryByRole('button', { name: /read node configuration/i })).toBeNull();
    expect(screen.getByText(/bowties will be available after cdi reads complete/i)).toBeInTheDocument();
  });

  it('does not show the blocker when an offline layout already has bowties to edit', () => {
    mockHasLayoutFile.value = true;
    mockPreviewCards.push({
      eventIdHex: '01.02.03.04.05.06.07.08',
      eventIdBytes: [1, 2, 3, 4, 5, 6, 7, 8],
      producers: [],
      consumers: [],
      ambiguousEntries: [],
      name: 'Offline Bowtie',
      tags: [],
      state: 'planning',
      isDirty: false,
      dirtyFields: new Set<string>(),
      newEntryKeys: new Set<string>(),
    });

    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: false,
        readingConfig: false,
      },
    });

    expect(screen.queryByText(/bowties will be available after cdi reads complete/i)).toBeNull();
    expect(screen.getByRole('list', { name: /bowtie connections/i })).toBeInTheDocument();
    expect(screen.getByText(/offline bowtie/i)).toBeInTheDocument();
  });
});

describe('BowtieCatalogPanel — catalog content (readComplete=true, hasUnreadNodes=false)', () => {
  it('does not show the CTA or not-ready message', () => {
    mockReadComplete.value = true;
    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: false,
        readingConfig: false,
      },
    });
    expect(screen.queryByRole('button', { name: /read node configuration/i })).toBeNull();
    expect(screen.queryByText(/bowties will be available/i)).toBeNull();
  });

  it('removes an offline consumer through draft changes instead of live proxy writes', async () => {
    mockReadComplete.value = true;
    mockHasLayoutFile.value = true;
    mockIsOfflineMode.value = true;

    const nodeId = '0201570002D9';
    mockTreeByNodeId.set(
      nodeId,
      makeTree(nodeId, [
        makeEventLeaf({
          address: 513,
          path: ['seg:0', 'elem:2'],
          value: {
            type: 'eventId',
            bytes: [0x05, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x02],
            hex: '0501010100000002',
          },
        }),
      ]),
    );

    mockPreviewCards.push({
      eventIdHex: '0501010100000001',
      eventIdBytes: [5, 1, 1, 1, 0, 0, 0, 1],
      producers: [
        {
          node_key: '020157000001',
          node_name: 'Producer Node',
          element_path: ['seg:0', 'elem:0'],
          element_label: 'Producer 1',
          element_description: null,
          event_id: [5, 1, 1, 1, 0, 0, 0, 1],
          role: 'Producer',
        },
      ],
      consumers: [
        {
          node_key: nodeId,
          node_name: 'Offline Node',
          element_path: ['seg:0', 'elem:2'],
          element_label: 'Consumer A',
          element_description: null,
          event_id: [5, 1, 1, 1, 0, 0, 0, 1],
          role: 'Consumer',
        },
        {
          node_key: '0201570002DA',
          node_name: 'Offline Node B',
          element_path: ['seg:0', 'elem:3'],
          element_label: 'Consumer B',
          element_description: null,
          event_id: [5, 1, 1, 1, 0, 0, 0, 1],
          role: 'Consumer',
        },
      ],
      ambiguousEntries: [],
      name: 'Offline Bowtie',
      tags: [],
      state: 'active',
      isDirty: false,
      dirtyFields: new Set<string>(),
      newEntryKeys: new Set<string>(),
    });

    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: false,
        readingConfig: false,
      },
    });

    await fireEvent.click(screen.getByRole('button', { name: /remove consumer consumer a/i }));
    expect(screen.getByText(/remove entry\?/i)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: /^remove$/i }));

    // After refactor: remove goes through configEditor.applyEdit + flushDraftToBackend
    expect(mockApplyEdit).toHaveBeenCalled();
    const [key, value] = mockApplyEdit.mock.calls[0];
    expect(key).toContain(nodeId.replace(/\./g, '').toUpperCase());
    expect(value.type).toBe('eventId');
    expect(value.hex).toBe('0000000000000000');
    // Should NOT use the old setModifiedValue IPC directly
    expect(mockSetModifiedValue).not.toHaveBeenCalled();
  });
});

// ─── Reclassified-ambiguous → last-consumer remove flow ─────────────────────
//
// Regression pin for the merge-Owner symmetry fix in
// `buildEffectiveBowtiePreview`. Before the fix, an entry moved from
// `card.ambiguous_entries` into `consumers` via a pending role classification
// kept `role: 'Ambiguous'`, so `confirmRemove`'s
// `isLastConsumer = ... && entry.role === 'Consumer'` was false,
// `willBecomeEmpty` was false, and the "Remove last element?" delete-bowtie
// dialog was skipped — the removal silently fell into `doRemoveElement`.
//
// This test drives the panel through the REAL merge (via `vi.importActual`)
// so it observes whatever role the current implementation of the merge Owner
// produces.

describe('BowtieCatalogPanel — reclassified-ambiguous last-consumer remove', () => {
  const CARD_HEX = '0201570002D90006';
  const CARD_BYTES = [0x02, 0x01, 0x57, 0x00, 0x02, 0xD9, 0x00, 0x06];

  it('reaches the delete-bowtie confirmation when removing a reclassified last consumer', async () => {
    mockReadComplete.value = true;

    const consumerNodeId = '020157000001';
    const consumerPath = ['seg:0', 'elem:9#1', 'elem:0'];
    const producerNodeId = '020157000002';

    // Catalog: 1 producer + 1 ambiguous entry that the user will classify as Consumer.
    const catalog = {
      bowties: [{
        event_id_hex: CARD_HEX,
        event_id_bytes: CARD_BYTES,
        producers: [{
          node_key: producerNodeId,
          node_name: 'Producer Peer',
          element_path: ['seg:0', 'elem:0'],
          element_label: 'Producer Slot',
          element_description: null,
          event_id: CARD_BYTES,
          role: 'Producer',
        }],
        consumers: [],
        ambiguous_entries: [{
          node_key: consumerNodeId,
          node_name: 'Consumer Node',
          element_path: consumerPath,
          element_label: 'Unknown Slot',
          element_description: 'Ambiguous slot on consumer node',
          event_id: CARD_BYTES,
          role: 'Ambiguous',
        }],
        name: 'Reclassify Bowtie',
        tags: [],
        state: 'Incomplete',
      }],
      built_at: '2026-07-03T00:00:00Z',
      source_node_count: 2,
      total_slots_scanned: 2,
    };
    mockCatalogState.catalog = catalog;

    // Tree for the consumer node so the entry survives leaf reconciliation.
    mockTreeByNodeId.set(consumerNodeId, makeTree(consumerNodeId, [
      makeEventLeaf({
        address: 900,
        path: consumerPath,
        eventRole: null,
        value: { type: 'eventId', bytes: CARD_BYTES, hex: CARD_HEX },
      }),
    ]));

    // Pending role classification: user picked Consumer.
    const slotKey = `${consumerNodeId}:${consumerPath.join('/')}`;
    mockClassifications.set(slotKey, { role: 'Consumer' });

    // Invoke the REAL merge (bypassing the module-level stub that returns
    // `mockPreviewCards`) so the test observes whatever shape the current
    // merge Owner produces. Before the fix, the reclassified entry keeps
    // `role: 'Ambiguous'`; after the fix, it becomes `role: 'Consumer'`.
    //
    // `vi.importActual` returns the real module with its own store singleton
    // — seed that singleton so the real merge's `bowtieCatalogStore.catalog`
    // read finds the test catalog.
    const bowties = await vi.importActual<typeof import('$lib/stores/bowties.svelte')>(
      '$lib/stores/bowties.svelte',
    );
    bowties.bowtieCatalogStore.setCatalog(catalog as any);
    mockPreviewCards.push(...bowties.buildEffectiveBowtiePreview().bowties);

    render(BowtieCatalogPanel, {
      props: {
        hasUnreadNodes: false,
        readingConfig: false,
      },
    });

    // Click the remove-× on the reclassified consumer row, then the
    // "Remove" button in the entry-removal confirmation dialog.
    // element_label is tree-enriched to "Configuration.Event ID" by the merge.
    await fireEvent.click(screen.getByRole('button', { name: /remove consumer configuration\.event id/i }));
    expect(screen.getByText(/remove entry\?/i)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: /^remove$/i }));

    // The card had exactly 1 producer + 1 (reclassified) consumer, so removing
    // the consumer triggers the last-element delete-bowtie confirmation.
    expect(screen.getByText(/remove last element\?/i)).toBeInTheDocument();
  });
});
