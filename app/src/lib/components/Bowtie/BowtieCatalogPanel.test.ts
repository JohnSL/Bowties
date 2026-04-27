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
  mockSetLeafModifiedValue,
  mockDraftConfigChange,
  mockUpsertConfigChange,
} = vi.hoisted(() => {
  const draftConfigChange = { value: null as any };
  return {
    mockSetModifiedValue: vi.fn(),
    mockTreeByNodeId: new Map<string, any>(),
    mockSetLeafModifiedValue: vi.fn(),
    mockDraftConfigChange: draftConfigChange,
    mockUpsertConfigChange: vi.fn((change) => {
      draftConfigChange.value = {
        changeId: 'draft-1',
        kind: 'config',
        status: 'pending',
        ...change,
      };
    }),
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
    get catalog() { return null; },
    get readComplete() { return mockReadComplete.value; },
  },
  editableBowtiePreviewStore: {
    get preview() { return { bowties: mockPreviewCards }; },
  },
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: {
    get hasLayoutFile() { return mockHasLayoutFile.value; },
    get isOfflineMode() { return mockIsOfflineMode.value; },
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
  },
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() { return new Map(); },
    getTree: (nodeId: string) => mockTreeByNodeId.get(nodeId),
    setLeafModifiedValue: mockSetLeafModifiedValue,
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

vi.mock('$lib/utils/eventIds', () => ({
  generateFreshEventIdForNode: vi.fn(() => '00.00.00.00.00.00.00.00'),
}));

// ─── Tests ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockReadComplete.value = false;
  mockHasLayoutFile.value = false;
  mockIsOfflineMode.value = false;
  mockPreviewCards.length = 0;
  mockTreeByNodeId.clear();
  mockDraftConfigChange.value = null;
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

    const nodeId = '02.01.57.00.02.D9';
    mockTreeByNodeId.set(
      nodeId,
      makeTree(nodeId, [
        makeEventLeaf({
          address: 513,
          path: ['seg:0', 'elem:2'],
          value: {
            type: 'eventId',
            bytes: [0x05, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x02],
            hex: '05.01.01.01.00.00.00.02',
          },
        }),
      ]),
    );

    mockPreviewCards.push({
      eventIdHex: '05.01.01.01.00.00.00.01',
      eventIdBytes: [5, 1, 1, 1, 0, 0, 0, 1],
      producers: [
        {
          node_id: '02.01.57.00.00.01',
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
          node_id: nodeId,
          node_name: 'Offline Node',
          element_path: ['seg:0', 'elem:2'],
          element_label: 'Consumer A',
          element_description: null,
          event_id: [5, 1, 1, 1, 0, 0, 0, 1],
          role: 'Consumer',
        },
        {
          node_id: '02.01.57.00.02.DA',
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

    expect(mockUpsertConfigChange).toHaveBeenCalledWith({
      nodeId,
      space: 253,
      offset: '0x00000201',
      baselineValue: '05.01.01.01.00.00.00.02',
      plannedValue: '00.00.00.00.00.00.00.00',
    });
    expect(mockSetLeafModifiedValue).toHaveBeenCalledWith(
      nodeId,
      ['seg:0', 'elem:2'],
      {
        type: 'eventId',
        bytes: [0, 0, 0, 0, 0, 0, 0, 0],
        hex: '00.00.00.00.00.00.00.00',
      },
    );
    expect(mockSetModifiedValue).not.toHaveBeenCalled();
  });
});
