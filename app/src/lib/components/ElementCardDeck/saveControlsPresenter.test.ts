import { describe, expect, it, vi, beforeEach } from 'vitest';
import type { OfflineChangeRow } from '$lib/api/sync';
import { deriveSaveControlsViewState } from './saveControlsPresenter';

// ── Mock configChangesStore ───────────────────────────────────────────────────

const mockHasDraftsForNode = vi.fn().mockReturnValue(false);

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    hasDraftsForNode: (...args: unknown[]) => mockHasDraftsForNode(...args),
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  mockHasDraftsForNode.mockReturnValue(false);
});

function makeState(overrides: Partial<Parameters<typeof deriveSaveControlsViewState>[0]> = {}) {
  return deriveSaveControlsViewState({
    bowtieMetadataEditCount: 0,
    bowtieMetadataIsDirty: false,
    configDraftCount: 0,
    connectorWarningCount: 0,
    layoutIsDirty: false,
    layoutIsOfflineMode: false,
    offlineDraftCount: 0,
    offlineDraftRows: [],
    revertedPersistedCount: 0,
    saveProgressState: 'idle',
    treeNodeIds: [],
    ...overrides,
  });
}

describe('deriveSaveControlsViewState', () => {
  it('derives online pending counts from config drafts and metadata edits', () => {
    mockHasDraftsForNode.mockReturnValue(true);

    const state = makeState({
      bowtieMetadataEditCount: 2,
      bowtieMetadataIsDirty: true,
      configDraftCount: 3,
      treeNodeIds: ['node-1', 'node-2'],
    });

    expect(state.dirtyCount).toBe(3);
    expect(state.dirtyNodeCount).toBe(2);
    expect(state.pendingEditCount).toBe(5);
    expect(state.pendingHintText).toBe('5 unsaved changes');
    expect(state.discardFieldCount).toBe(5);
    expect(state.discardNodeCount).toBe(2);
    expect(state.canSave).toBe(true);
  });

  it('derives offline pending counts from draft rows and keeps isDirty-only sessions at one edit', () => {
    const pendingRows: OfflineChangeRow[] = [
      {
        changeId: 'draft-1',
        kind: 'config',
        nodeId: 'node-1',
        baselineValue: '1',
        plannedValue: '2',
        status: 'pending',
      },
      {
        changeId: 'draft-2',
        kind: 'config',
        nodeId: 'node-2',
        baselineValue: '3',
        plannedValue: '4',
        status: 'pending',
      },
    ];

    const dirtyOnlyState = makeState({
      layoutIsDirty: true,
      layoutIsOfflineMode: true,
    });

    expect(dirtyOnlyState.pendingEditCount).toBe(1);
    expect(dirtyOnlyState.pendingHintText).toBe('1 unsaved edit');
    expect(dirtyOnlyState.discardFieldCount).toBe(1);
    expect(dirtyOnlyState.discardNodeCount).toBe(1);

    mockHasDraftsForNode.mockReturnValue(true);

    const draftState = makeState({
      layoutIsDirty: true,
      layoutIsOfflineMode: true,
      configDraftCount: 2,
      treeNodeIds: ['node-1', 'node-2'],
    });

    expect(draftState.pendingEditCount).toBe(3);
    expect(draftState.pendingHintText).toBe('3 unsaved edits');
    expect(draftState.discardFieldCount).toBe(3);
    expect(draftState.discardNodeCount).toBe(2);

    mockHasDraftsForNode.mockReturnValue(false);
  });

  it('counts only layout dirty when offline draft count is zero (cancellation-only)', () => {
    const state = makeState({
      layoutIsDirty: true,
      layoutIsOfflineMode: true,
      offlineDraftCount: 0,
    });

    expect(state.pendingEditCount).toBe(1);
    expect(state.pendingHintText).toBe('1 unsaved edit');
    expect(state.discardFieldCount).toBe(1);
  });

  it('counts reverted persisted rows as unsaved edits', () => {
    const state = makeState({
      layoutIsOfflineMode: true,
      revertedPersistedCount: 2,
    });

    expect(state.pendingEditCount).toBe(2);
    expect(state.pendingHintText).toBe('2 unsaved edits');
    expect(state.hasEdits).toBe(true);
    expect(state.discardFieldCount).toBe(2);
  });

  it('counts layout-only dirty state in online pending and discard totals', () => {
    const state = makeState({
      layoutIsDirty: true,
      layoutIsOfflineMode: false,
    });

    expect(state.pendingEditCount).toBe(1);
    expect(state.pendingHintText).toBe('1 unsaved change');
    expect(state.discardFieldCount).toBe(1);
    expect(state.discardNodeCount).toBe(1);
  });

  it('disables saving while a save is already in progress', () => {
    mockHasDraftsForNode.mockReturnValue(true);

    const state = makeState({
      saveProgressState: 'saving',
      configDraftCount: 1,
      treeNodeIds: ['node-1'],
    });

    expect(state.isSaving).toBe(true);
    expect(state.canSave).toBe(false);
    expect(state.hasEdits).toBe(true);
  });

  it('includes config drafts in offline mode pending and discard counts', () => {
    mockHasDraftsForNode.mockReturnValue(true);

    const state = makeState({
      layoutIsOfflineMode: true,
      configDraftCount: 1,
      offlineDraftCount: 0,
      treeNodeIds: ['node-1'],
    });

    expect(state.pendingEditCount).toBe(1);
    expect(state.pendingHintText).toBe('1 unsaved edit');
    expect(state.discardFieldCount).toBe(1);
    expect(state.hasEdits).toBe(true);
    expect(state.canSave).toBe(true);
  });

  it('counts only config drafts in offline mode (offline draft rows are persistence staging)', () => {
    mockHasDraftsForNode.mockReturnValue(true);

    const state = makeState({
      layoutIsOfflineMode: true,
      configDraftCount: 2,
      offlineDraftCount: 1,
      treeNodeIds: ['node-1'],
    });

    expect(state.pendingEditCount).toBe(2);
    expect(state.pendingHintText).toBe('2 unsaved edits');
    expect(state.discardFieldCount).toBe(2);
  });
});