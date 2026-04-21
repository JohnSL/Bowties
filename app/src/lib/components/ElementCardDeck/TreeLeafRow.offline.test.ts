/**
 * Offline-mode component tests for TreeLeafRow.svelte.
 *
 * Covers:
 * - Draft offline change annotation shows "Unsaved offline edit: X -> Y"
 * - Clicking "Revert" on a draft row calls offlineChangesStore.revertToBaseline
 *   AND nodeTreeStore.setLeafModifiedValue(nodeId, path, null).
 * - Persisted offline change annotation shows "Bus: X | Pending: Y"
 * - Clicking "Revert" on a persisted row calls revertToBaseline AND setLeafModifiedValue(null).
 * - Revert button is disabled when offlineChangesStore.isBusy is true.
 * - Offline change indicators are suppressed while layoutOpenInProgress is true.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TreeLeafRow from './TreeLeafRow.svelte';
import type { LeafConfigNode } from '$lib/types/nodeTree';
import type { OfflineChangeRow } from '$lib/api/sync';

// ─── Helpers ──────────────────────────────────────────────────────────────────

const NODE_ID = '05.02.01.00.00.00';

// A minimal Svelte 4 readable store shim (needed for layoutOpenInProgress)
function readable<T>(value: T) {
  return {
    subscribe: (fn: (v: T) => void) => {
      fn(value);
      return () => {};
    },
  };
}

// ─── Mock stores that control offline-row visibility ─────────────────────────

const mockFindDraftConfigChange = vi.fn<() => OfflineChangeRow | null>().mockReturnValue(null);
const mockFindPersistedConfigChange = vi.fn<() => OfflineChangeRow | null>().mockReturnValue(null);
const mockRevertToBaseline = vi.fn().mockResolvedValue(true);
let mockIsBusy = false;

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: {
    get isBusy() {
      return mockIsBusy;
    },
    findDraftConfigChange: (...args: unknown[]) => mockFindDraftConfigChange(...args),
    findPersistedConfigChange: (...args: unknown[]) => mockFindPersistedConfigChange(...args),
    revertToBaseline: (...args: unknown[]) => mockRevertToBaseline(...args),
    upsertConfigChange: vi.fn(),
    pendingCount: 0,
  },
}));

const mockSetLeafModifiedValue = vi.fn();

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    setLeafModifiedValue: (...args: unknown[]) => mockSetLeafModifiedValue(...args),
    getTree: vi.fn().mockReturnValue(null),
    trees: new Map(),
    updateLeafValue: vi.fn(),
  },
}));

// layoutOpenInProgress controls suppressTransientIndicators — false = show indicators
let mockLayoutOpenInProgress = false;

vi.mock('$lib/stores/layoutOpenLifecycle', () => ({
  get layoutOpenInProgress() {
    return readable(mockLayoutOpenInProgress);
  },
  layoutOpenPhase: readable('idle'),
  setLayoutOpenPhase: vi.fn(),
  layoutOpenStatusText: readable(''),
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: {
    isOfflineMode: true,
    hasLayoutFile: true,
    isDirty: false,
    markDirty: vi.fn(),
    setConnected: vi.fn(),
    activeContext: null,
  },
}));

// ─── Stub other transitive dependencies ───────────────────────────────────────

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    nodeSlotMap: new Map(),
    effectiveNodeSlotMap: new Map(),
    getDisplayName: vi.fn((id: string) => id),
  },
}));

vi.mock('$lib/stores/bowtieFocus.svelte', () => ({
  bowtieFocusStore: {
    highlightedEventIdHex: null,
    focusBowtie: vi.fn(),
    clearFocus: vi.fn(),
  },
}));

vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: {
    navigationRequest: null,
    leafFocusRequest: null,
    focusConfigField: vi.fn(),
    clearNavigation: vi.fn(),
    clearLeafFocus: vi.fn(),
    clearFocus: vi.fn(),
  },
}));

vi.mock('$lib/stores/connectionRequest.svelte', () => ({
  connectionRequestStore: {
    isRequested: false,
    complete: vi.fn(),
    request: vi.fn(),
  },
}));

vi.mock('$lib/api/config', () => ({
  setModifiedValue: vi.fn().mockResolvedValue(undefined),
  triggerAction: vi.fn().mockResolvedValue(undefined),
}));

// ─── Test setup ───────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks();
  mockFindDraftConfigChange.mockReturnValue(null);
  mockFindPersistedConfigChange.mockReturnValue(null);
  mockRevertToBaseline.mockResolvedValue(true);
  mockIsBusy = false;
  mockLayoutOpenInProgress = false;
});

function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Speed',
    description: null,
    elementType: 'int',
    address: 100,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0'],
    value: { type: 'int', value: 3 },
    eventRole: null,
    constraints: null,
    ...overrides,
  };
}

function makeDraftRow(overrides: Partial<OfflineChangeRow> = {}): OfflineChangeRow {
  return {
    changeId: 'draft-1',
    kind: 'config',
    nodeId: NODE_ID,
    space: 253,
    offset: '0x00000064',
    baselineValue: '3',
    plannedValue: '7',
    status: 'pending',
    ...overrides,
  };
}

function makePersistedRow(overrides: Partial<OfflineChangeRow> = {}): OfflineChangeRow {
  return {
    changeId: 'persisted-1',
    kind: 'config',
    nodeId: NODE_ID,
    space: 253,
    offset: '0x00000064',
    baselineValue: '3',
    plannedValue: '7',
    status: 'pending',
    ...overrides,
  };
}

// ─── Draft offline row tests ──────────────────────────────────────────────────

describe('draft offline row (unsaved edit)', () => {
  it('shows "Unsaved offline edit: X -> Y" annotation when a draft row exists', () => {
    mockFindDraftConfigChange.mockReturnValue(makeDraftRow({ baselineValue: '3', plannedValue: '7' }));
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByText(/Unsaved offline edit: 3 -> 7/)).toBeInTheDocument();
  });

  it('shows a "Revert" button for the draft row', () => {
    mockFindDraftConfigChange.mockReturnValue(makeDraftRow());
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByRole('button', { name: /revert to baseline/i })).toBeInTheDocument();
  });

  it('calls offlineChangesStore.revertToBaseline with the changeId on click', async () => {
    mockFindDraftConfigChange.mockReturnValue(makeDraftRow({ changeId: 'draft-42' }));
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });

    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));

    expect(mockRevertToBaseline).toHaveBeenCalledWith('draft-42');
  });

  it('calls nodeTreeStore.setLeafModifiedValue(nodeId, path, null) on click — clears the edit overlay', async () => {
    const leaf = makeLeaf({ path: ['seg:0', 'elem:0'] });
    mockFindDraftConfigChange.mockReturnValue(makeDraftRow());
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });

    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));

    expect(mockSetLeafModifiedValue).toHaveBeenCalledWith(NODE_ID, ['seg:0', 'elem:0'], null);
  });

  it('revert button is disabled when offlineChangesStore.isBusy is true', () => {
    mockIsBusy = true;
    mockFindDraftConfigChange.mockReturnValue(makeDraftRow());
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByRole('button', { name: /revert to baseline/i })).toBeDisabled();
  });

  it('does not show draft annotation when no draft row exists', () => {
    mockFindDraftConfigChange.mockReturnValue(null);
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.queryByText(/Unsaved offline edit/)).not.toBeInTheDocument();
  });
});

// ─── Persisted offline row tests ──────────────────────────────────────────────

describe('persisted offline row (pending apply)', () => {
  it('shows "Bus: X | Pending: Y" annotation when a persisted row exists', () => {
    mockFindPersistedConfigChange.mockReturnValue(
      makePersistedRow({ baselineValue: '3', plannedValue: '7' }),
    );
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByText(/Bus: 3 \| Pending: 7/)).toBeInTheDocument();
  });

  it('shows a "Revert" button for the persisted row', () => {
    mockFindPersistedConfigChange.mockReturnValue(makePersistedRow());
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByRole('button', { name: /revert to baseline/i })).toBeInTheDocument();
  });

  it('calls offlineChangesStore.revertToBaseline with the changeId on click', async () => {
    mockFindPersistedConfigChange.mockReturnValue(
      makePersistedRow({ changeId: 'persisted-99' }),
    );
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });

    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));

    expect(mockRevertToBaseline).toHaveBeenCalledWith('persisted-99');
  });

  it('calls nodeTreeStore.setLeafModifiedValue(nodeId, path, null) on click — clears isOfflinePending overlay', async () => {
    const leaf = makeLeaf({ path: ['seg:0', 'elem:2'] });
    mockFindPersistedConfigChange.mockReturnValue(makePersistedRow());
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });

    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));

    expect(mockSetLeafModifiedValue).toHaveBeenCalledWith(NODE_ID, ['seg:0', 'elem:2'], null);
  });

  it('does NOT call markDirty after persisted revert — no false Save/Discard buttons', async () => {
    // Bug 1b: previously layoutStore.markDirty() was called here, causing
    // SaveControls to show "0 unsaved changes" with active Save/Discard buttons.
    const { layoutStore } = await import('$lib/stores/layout.svelte');
    mockFindPersistedConfigChange.mockReturnValue(makePersistedRow());
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });

    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));

    expect(layoutStore.markDirty).not.toHaveBeenCalled();
  });
});

// ─── Lifecycle suppression ────────────────────────────────────────────────────

describe('offline indicators suppressed during layout open', () => {
  it('does not show draft annotation while layout is opening (suppressTransientIndicators = true)', () => {
    mockLayoutOpenInProgress = true;
    mockFindDraftConfigChange.mockReturnValue(makeDraftRow());
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.queryByText(/Unsaved offline edit/)).not.toBeInTheDocument();
  });

  it('does not show persisted annotation while layout is opening', () => {
    mockLayoutOpenInProgress = true;
    mockFindPersistedConfigChange.mockReturnValue(makePersistedRow());
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.queryByText(/Bus:.*Pending:/)).not.toBeInTheDocument();
  });
});
