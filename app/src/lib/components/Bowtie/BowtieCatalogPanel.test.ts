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

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const mockReadComplete = { value: false };
const mockHasLayoutFile = { value: false };
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
    getTree: () => null,
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
  setModifiedValue: vi.fn(),
}));

vi.mock('$lib/utils/eventIds', () => ({
  generateFreshEventIdForNode: vi.fn(() => '00.00.00.00.00.00.00.00'),
}));

// ─── Tests ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockReadComplete.value = false;
  mockHasLayoutFile.value = false;
  mockPreviewCards.length = 0;
});

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
});
