/**
 * Offline-mode component tests for TreeLeafRow.svelte — post edit-layer refactor.
 *
 * TreeLeafRow now reads display state from configChangesStore.changeLayers()
 * and routes edits through configEditor.applyEdit(). These tests verify:
 * - Draft layer annotation shows "Unsaved offline edit: {from} -> {to}"
 * - Persisted offline pending annotation shows "Bus: {baseline} | Pending: {pending}"
 * - Clicking "Revert" on a draft calls configChangesStore.revert()
 * - Clicking "Revert" on a persisted row calls offlineChangesStore.revertToBaseline()
 * - Annotations are suppressed while layoutOpenInProgress is true.
 */

import '@testing-library/jest-dom/vitest';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import TreeLeafRow from './TreeLeafRow.svelte';
import type { LeafConfigNode } from '$lib/types/nodeTree';
import type { ChangeLayer } from '$lib/stores/configChanges.svelte';

// ─── Helpers ──────────────────────────────────────────────────────────────────

const NODE_ID = '05.02.01.00.00.00';

function readable<T>(value: T) {
  return {
    subscribe: (fn: (v: T) => void) => {
      fn(value);
      return () => {};
    },
  };
}

// ─── Mock configChangesStore (display layer) ──────────────────────────────────

let mockChangeLayers: ChangeLayer[] = [];
let mockVisibleValue: import('$lib/types/nodeTree').TreeConfigValue | null = null;

const mockRevert = vi.fn();

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    changeLayers: () => mockChangeLayers,
    visibleValue: () => mockVisibleValue,
    revert: (...args: unknown[]) => mockRevert(...args),
    hasDraftsForNode: vi.fn().mockReturnValue(false),
    draftEntries: vi.fn().mockReturnValue([]),
  },
}));

vi.mock('$lib/stores/configEditor.svelte', () => ({
  configEditor: { applyEdit: vi.fn() },
}));

vi.mock('$lib/orchestration/configDraftOrchestrator', () => ({
  flushDraftToBackend: vi.fn(),
}));

// ─── Mock offlineChangesStore (persisted row revert) ─────────────────────────

const mockRevertToBaseline = vi.fn().mockResolvedValue(true);
const mockFindPersistedConfigChange = vi.fn().mockReturnValue(null);
let mockIsBusy = false;

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: {
    get isBusy() { return mockIsBusy; },
    findPersistedConfigChange: (...args: unknown[]) => mockFindPersistedConfigChange(...args),
    revertToBaseline: (...args: unknown[]) => mockRevertToBaseline(...args),
  },
}));

let mockLayoutOpenInProgress = false;

vi.mock('$lib/stores/layoutOpenLifecycle', () => ({
  get layoutOpenInProgress() { return readable(mockLayoutOpenInProgress); },
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

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    nodeSlotMap: new Map(),
    effectiveNodeSlotMap: new Map(),
    getDisplayName: vi.fn((id: string) => id),
    getRoleForSlot: vi.fn().mockReturnValue(null),
  },
}));
vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    trees: new Map(),
    getTree: vi.fn().mockReturnValue(null),
  },
}));
vi.mock('$lib/types/nodeTree', async () => {
  const actual = await vi.importActual('$lib/types/nodeTree');
  return {
    ...actual,
    buildElementLabel: vi.fn((_tree: any, leaf: any) => leaf.name ?? 'Unknown'),
  };
});
vi.mock('$lib/stores/bowtieFocus.svelte', () => ({
  bowtieFocusStore: { highlightedEventIdHex: null, focusBowtie: vi.fn(), clearFocus: vi.fn() },
}));
vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: {
    navigationRequest: null, leafFocusRequest: null,
    focusConfigField: vi.fn(), clearNavigation: vi.fn(), clearLeafFocus: vi.fn(), clearFocus: vi.fn(),
  },
}));
vi.mock('$lib/stores/connectionRequest.svelte', () => ({
  connectionRequestStore: { isRequested: false, complete: vi.fn(), request: vi.fn() },
}));
vi.mock('$lib/api/config', () => ({
  setModifiedValue: vi.fn().mockResolvedValue(undefined),
  triggerAction: vi.fn().mockResolvedValue(undefined),
}));

// ─── Test setup ───────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks();
  mockChangeLayers = [];
  mockVisibleValue = null;
  mockRevertToBaseline.mockResolvedValue(true);
  mockFindPersistedConfigChange.mockReturnValue(null);
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

// ─── Draft layer annotation tests ─────────────────────────────────────────────

describe('draft layer (unsaved edit)', () => {
  it('shows "Unsaved offline edit: X -> Y" when a draft layer exists over baseline', () => {
    mockChangeLayers = [
      { type: 'draft', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    mockVisibleValue = { type: 'int', value: 7 };
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByText(/Unsaved offline edit: 3 → 7/)).toBeInTheDocument();
  });

  it('shows persisted planned value as "from" when draft is on top of offline pending', () => {
    mockChangeLayers = [
      { type: 'draft', value: { type: 'int', value: 3 } },
      { type: 'offlinePending', value: { type: 'int', value: 5 } },
      { type: 'baseline', value: { type: 'int', value: 0 } },
    ];
    mockVisibleValue = { type: 'int', value: 3 };
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByText(/Unsaved offline edit: 5 → 3/)).toBeInTheDocument();
  });

  it('shows a "Revert" button for the draft layer', () => {
    mockChangeLayers = [
      { type: 'draft', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    mockVisibleValue = { type: 'int', value: 7 };
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByRole('button', { name: /revert to baseline/i })).toBeInTheDocument();
  });

  it('calls configChangesStore.revert on click', async () => {
    mockChangeLayers = [
      { type: 'draft', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    mockVisibleValue = { type: 'int', value: 7 };
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));
    expect(mockRevert).toHaveBeenCalled();
  });

  it('does not show draft annotation when no draft layer exists', () => {
    mockChangeLayers = [{ type: 'baseline', value: { type: 'int', value: 3 } }];
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.queryByText(/Unsaved offline edit/)).not.toBeInTheDocument();
  });
});

// ─── Persisted offline pending tests ──────────────────────────────────────────

describe('persisted offline pending', () => {
  it('shows "Bus: X | Pending: Y" when offline pending layer exists', () => {
    mockChangeLayers = [
      { type: 'offlinePending', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    mockVisibleValue = { type: 'int', value: 7 };
    mockFindPersistedConfigChange.mockReturnValue({
      changeId: 'persisted-1', kind: 'config', nodeId: NODE_ID,
      space: 253, offset: '0x00000064', baselineValue: '3', plannedValue: '7', status: 'pending',
    });
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.getByText(/Bus: 3 \| Pending: 7/)).toBeInTheDocument();
  });

  it('renders offline-pending row with the pending style', () => {
    mockChangeLayers = [
      { type: 'offlinePending', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    mockVisibleValue = { type: 'int', value: 7 };
    mockFindPersistedConfigChange.mockReturnValue({
      changeId: 'persisted-1', kind: 'config', nodeId: NODE_ID,
      space: 253, offset: '0x00000064', baselineValue: '3', plannedValue: '7', status: 'pending',
    });
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    const row = screen.getByRole('listitem');
    expect(row).toHaveClass('offline-pending');
    expect(row).not.toHaveClass('dirty');
  });

  it('calls offlineChangesStore.revertToBaseline on persisted revert click', async () => {
    mockChangeLayers = [
      { type: 'offlinePending', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    mockVisibleValue = { type: 'int', value: 7 };
    mockFindPersistedConfigChange.mockReturnValue({
      changeId: 'persisted-99', kind: 'config', nodeId: NODE_ID,
      space: 253, offset: '0x00000064', baselineValue: '3', plannedValue: '7', status: 'pending',
    });
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));
    expect(mockRevertToBaseline).toHaveBeenCalledWith('persisted-99');
  });

  it('does not mark layout dirty after persisted revert (tracked via revertedPersistedCount instead)', async () => {
    const { layoutStore } = await import('$lib/stores/layout.svelte');
    mockChangeLayers = [
      { type: 'offlinePending', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    mockVisibleValue = { type: 'int', value: 7 };
    mockFindPersistedConfigChange.mockReturnValue({
      changeId: 'persisted-1', kind: 'config', nodeId: NODE_ID,
      space: 253, offset: '0x00000064', baselineValue: '3', plannedValue: '7', status: 'pending',
    });
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    await fireEvent.click(screen.getByRole('button', { name: /revert to baseline/i }));
    expect(layoutStore.markDirty).not.toHaveBeenCalled();
  });
});

// ─── Lifecycle suppression ────────────────────────────────────────────────────

describe('indicators suppressed during layout open', () => {
  it('does not show draft annotation while layout is opening', () => {
    mockLayoutOpenInProgress = true;
    mockChangeLayers = [
      { type: 'draft', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.queryByText(/Unsaved offline edit/)).not.toBeInTheDocument();
  });

  it('does not show persisted annotation while layout is opening', () => {
    mockLayoutOpenInProgress = true;
    mockChangeLayers = [
      { type: 'offlinePending', value: { type: 'int', value: 7 } },
      { type: 'baseline', value: { type: 'int', value: 3 } },
    ];
    render(TreeLeafRow, { props: { leaf: makeLeaf(), nodeId: NODE_ID } });
    expect(screen.queryByText(/Bus:.*Pending:/)).not.toBeInTheDocument();
  });
});
