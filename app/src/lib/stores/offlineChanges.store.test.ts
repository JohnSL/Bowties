/**
 * Store-level tests for offlineChangesStore (offlineChanges.svelte.ts).
 *
 * Covers:
 * - revertToBaseline removes draft-only changes without backend IPC
 * - revertToBaseline calls backend IPC for persisted changes
 * - draftCount updates after revert
 * - isBusy lifecycle during revert
 * - Error handling
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { OfflineChangeRow } from '$lib/api/sync';

// ─── Mock backend IPC ─────────────────────────────────────────────────────────

const mockListOfflineChanges = vi.fn<() => Promise<OfflineChangeRow[]>>().mockResolvedValue([]);
const mockReplaceOfflineChanges = vi.fn().mockResolvedValue(undefined);
const mockRevertOfflineChange = vi.fn().mockResolvedValue(undefined);

vi.mock('$lib/api/sync', () => ({
  listOfflineChanges: mockListOfflineChanges,
  replaceOfflineChanges: mockReplaceOfflineChanges,
  revertOfflineChange: mockRevertOfflineChange,
}));

// ─── Import store AFTER mocks ────────────────────────────────────────────────

const { offlineChangesStore } = await import('$lib/stores/offlineChanges.svelte');

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeDraftRow(overrides: Partial<OfflineChangeRow> = {}): OfflineChangeRow {
  return {
    changeId: `draft-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    kind: 'config',
    nodeId: '05.02.01.02.03.00',
    space: 253,
    offset: '0x00000000',
    baselineValue: '0',
    plannedValue: '42',
    status: 'pending',
    ...overrides,
  };
}

// ─── Tests ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  offlineChangesStore.clear();
  vi.clearAllMocks();
  mockListOfflineChanges.mockResolvedValue([]);
});

describe('revertToBaseline — draft-only changes', () => {
  it('removes a draft change from the store', async () => {
    const row = makeDraftRow({ changeId: 'draft-1' });
    offlineChangesStore.upsertRow(row);
    expect(offlineChangesStore.draftCount).toBe(1);

    const result = await offlineChangesStore.revertToBaseline('draft-1');
    expect(result).toBe(true);
    expect(offlineChangesStore.draftCount).toBe(0);
  });

  it('does not call backend IPC for draft-only changes', async () => {
    const row = makeDraftRow({ changeId: 'draft-1' });
    offlineChangesStore.upsertRow(row);

    await offlineChangesStore.revertToBaseline('draft-1');
    expect(mockRevertOfflineChange).not.toHaveBeenCalled();
  });

  it('leaves other draft changes intact', async () => {
    const row1 = makeDraftRow({ changeId: 'draft-1', offset: '0x00000000' });
    const row2 = makeDraftRow({ changeId: 'draft-2', offset: '0x00000001' });
    offlineChangesStore.upsertRow(row1);
    offlineChangesStore.upsertRow(row2);
    expect(offlineChangesStore.draftCount).toBe(2);

    await offlineChangesStore.revertToBaseline('draft-1');
    expect(offlineChangesStore.draftCount).toBe(1);
    expect(offlineChangesStore.draftRows[0].changeId).toBe('draft-2');
  });
});

describe('revertToBaseline — persisted changes', () => {
  it('calls backend IPC for persisted changes', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-1' });
    offlineChangesStore.setRows([persistedRow]);
    expect(offlineChangesStore.pendingApplyCount).toBe(1);

    await offlineChangesStore.revertToBaseline('persisted-1');
    expect(mockRevertOfflineChange).toHaveBeenCalledWith('persisted-1');
  });

  it('removes the row from working persisted rows without reloading the saved snapshot', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-1' });
    offlineChangesStore.setRows([persistedRow]);

    await offlineChangesStore.revertToBaseline('persisted-1');

    expect(mockListOfflineChanges).not.toHaveBeenCalled();
    expect(offlineChangesStore.persistedRows).toHaveLength(0);
    expect(offlineChangesStore.savedRows).toHaveLength(1);
    expect(offlineChangesStore.savedRows[0].changeId).toBe('persisted-1');
  });
});

describe('revertToBaseline — error handling', () => {
  it('returns false when backend IPC fails', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-err' });
    offlineChangesStore.setRows([persistedRow]);
    mockRevertOfflineChange.mockRejectedValueOnce(new Error('IPC failed'));

    const result = await offlineChangesStore.revertToBaseline('persisted-err');
    expect(result).toBe(false);
  });

  it('clears isBusy even on error', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-err' });
    offlineChangesStore.setRows([persistedRow]);
    mockRevertOfflineChange.mockRejectedValueOnce(new Error('IPC failed'));

    await offlineChangesStore.revertToBaseline('persisted-err');
    expect(offlineChangesStore.isBusy).toBe(false);
  });
});

describe('revertToBaseline — isBusy lifecycle', () => {
  it('sets isBusy during operation and clears after', async () => {
    const row = makeDraftRow({ changeId: 'draft-busy' });
    offlineChangesStore.upsertRow(row);

    // After the operation completes, isBusy should be false
    await offlineChangesStore.revertToBaseline('draft-busy');
    expect(offlineChangesStore.isBusy).toBe(false);
  });
});

describe('revertToBaseline — persisted row updates working state only', () => {
  it('pendingApplyCount drops to 0 after persisted revert when backend returns empty list', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-dirty' });
    offlineChangesStore.setRows([persistedRow]);
    expect(offlineChangesStore.pendingApplyCount).toBe(1);

    await offlineChangesStore.revertToBaseline('persisted-dirty');

    expect(offlineChangesStore.pendingApplyCount).toBe(0);
    expect(offlineChangesStore.savedRows).toHaveLength(1);
  });

  it('revertToBaseline for a draft row does not call backend IPC (no save needed)', async () => {
    const row = makeDraftRow({ changeId: 'draft-no-dirty' });
    offlineChangesStore.upsertRow(row);

    await offlineChangesStore.revertToBaseline('draft-no-dirty');

    // Draft reverts never touch the backend — no opportunity to trigger dirty state
    expect(mockRevertOfflineChange).not.toHaveBeenCalled();
    expect(mockReplaceOfflineChanges).not.toHaveBeenCalled();
  });
});

describe('revertAllPending — saved vs in-memory layering', () => {
  it('restores persisted rows to saved snapshot and clears drafts', async () => {
    const savedRow = makeDraftRow({
      changeId: 'saved-1',
      nodeId: '05.02.01.02.03.00',
      offset: '0x00000010',
      baselineValue: '10',
      plannedValue: '20',
    });
    offlineChangesStore.setRows([savedRow]);

    offlineChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000010',
      baselineValue: '10',
      plannedValue: '10',
    });

    expect(offlineChangesStore.draftCount).toBe(1);
    expect(offlineChangesStore.effectiveRows.length).toBe(0);

    const reverted = await offlineChangesStore.revertAllPending();

    expect(reverted).toBe(1);
    expect(offlineChangesStore.draftCount).toBe(0);
    expect(offlineChangesStore.persistedRows).toHaveLength(1);
    expect(offlineChangesStore.persistedRows[0].changeId).toBe('saved-1');
    expect(offlineChangesStore.savedRows).toHaveLength(1);
    expect(offlineChangesStore.savedRows[0].changeId).toBe('saved-1');
  });

  it('keeps saved snapshot aligned after reloadFromBackend', async () => {
    const backendRow = makeDraftRow({ changeId: 'backend-1', offset: '0x00000022' });
    mockListOfflineChanges.mockImplementationOnce(async () => [backendRow]);

    await offlineChangesStore.reloadFromBackend();

    expect(offlineChangesStore.persistedRows).toHaveLength(1);
    expect(offlineChangesStore.savedRows).toHaveLength(1);
    expect(offlineChangesStore.persistedRows[0].changeId).toBe('backend-1');
    expect(offlineChangesStore.savedRows[0].changeId).toBe('backend-1');
  });
});
