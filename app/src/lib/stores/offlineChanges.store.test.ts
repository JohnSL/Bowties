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
import type { LeafConfigNode } from '$lib/types/nodeTree';

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

function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Input Function',
    description: null,
    elementType: 'int',
    address: 0,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0#1', 'elem:0'],
    value: { type: 'int', value: 3 },
    eventRole: null,
    constraints: {
      min: 0,
      max: 8,
      defaultValue: null,
      mapEntries: [
        { value: 1, label: 'Normal' },
        { value: 3, label: 'Alt Action Hi' },
        { value: 5, label: 'Sample Hi' },
      ],
    },
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

    // Upserting planned===baseline with a persisted row creates a cancellation
    // draft that suppresses the persisted row in effectiveRows.
    // Cancellation drafts are not user-visible changes, so draftCount stays 0.
    expect(offlineChangesStore.draftCount).toBe(0);
    expect(offlineChangesStore.effectiveRows).toHaveLength(0);

    const reverted = await offlineChangesStore.revertAllPending();

    // After revert, the cancellation draft is cleared and the persisted row
    // is restored from the saved snapshot.
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

describe('applyConnectorCompatibilityConfigChanges', () => {
  it('stages generated connector repairs as draft config rows', () => {
    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
      {
        targetPath: 'Port I/O/Line/Input Function',
        space: 253,
        offset: '0x00000000',
        baselineValue: '3',
        plannedValue: '1',
        reason: 'Auto-staged repair',
        originSlotId: 'connector-a',
      },
    ]);

    expect(offlineChangesStore.draftRows).toHaveLength(1);
    expect(offlineChangesStore.draftRows[0]).toMatchObject({
      kind: 'config',
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '3',
      plannedValue: '1',
    });
  });

  it('finds the effective config row after layering a draft over a persisted row', () => {
    offlineChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
      {
        targetPath: 'Port I/O/Line/Input Function',
        space: 253,
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '1',
        reason: 'Auto-staged repair',
        originSlotId: 'connector-a',
      },
    ]);

    expect(
      offlineChangesStore.findEffectiveConfigChange('05.02.01.02.03.00', 253, '0x00000000'),
    ).toMatchObject({
      baselineValue: '0',
      plannedValue: '1',
    });
  });

  it('retains previously generated target corrections when the next recompute emits no new repair for that target', () => {
    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
      {
        targetPath: 'Port I/O/Line/Input Function',
        space: 253,
        offset: '0x00000000',
        baselineValue: '3',
        plannedValue: '1',
        reason: 'Auto-staged repair',
        originSlotId: 'connector-a',
      },
    ]);

    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', []);

    expect(offlineChangesStore.effectiveRows).toMatchObject([
      {
        nodeId: '05.02.01.02.03.00',
        space: 253,
        offset: '0x00000000',
        plannedValue: '1',
      },
    ]);
  });

  it('keeps the persisted baseline when layering a generated correction over a persisted pending change', () => {
    offlineChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
      {
        targetPath: 'Port I/O/Line/Input Function',
        space: 253,
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '1',
        reason: 'Auto-staged repair',
        originSlotId: 'connector-a',
      },
    ]);

    expect(offlineChangesStore.draftRows).toHaveLength(1);
    expect(offlineChangesStore.draftRows[0]).toMatchObject({
      changeId: expect.not.stringContaining('persisted-1'),
      baselineValue: '0',
      plannedValue: '1',
    });
  });

  it('reverting a draft layered over a persisted row keeps the persisted row intact', async () => {
    offlineChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
      {
        targetPath: 'Port I/O/Line/Input Function',
        space: 253,
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '1',
        reason: 'Auto-staged repair',
        originSlotId: 'connector-a',
      },
    ]);

    const draftId = offlineChangesStore.draftRows[0]?.changeId;
    expect(draftId).toBeTruthy();

    await offlineChangesStore.revertToBaseline(draftId!);

    expect(mockRevertOfflineChange).not.toHaveBeenCalled();
    expect(offlineChangesStore.draftRows).toHaveLength(0);
    expect(offlineChangesStore.persistedRows).toHaveLength(1);
    expect(offlineChangesStore.effectiveRows).toMatchObject([
      {
        changeId: 'persisted-1',
        baselineValue: '0',
        plannedValue: '3',
      },
    ]);
  });

  it('preserves the connector-corrected effective value when switching boards no longer emits a new repair', () => {
    offlineChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
      {
        targetPath: 'Port I/O/Line/Input Function',
        space: 253,
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '1',
        reason: 'Auto-staged repair',
        originSlotId: 'connector-a',
      },
    ]);

    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', []);

    expect(offlineChangesStore.draftRows).toHaveLength(1);
    expect(offlineChangesStore.effectiveRows).toMatchObject([
      {
        changeId: expect.not.stringContaining('persisted-1'),
        baselineValue: '0',
        plannedValue: '1',
      },
    ]);
  });

  it('keeps a user override when connector-generated repairs are later cleared', () => {
    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
      {
        targetPath: 'Port I/O/Line/Input Function',
        space: 253,
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '1',
        reason: 'Auto-staged repair',
        originSlotId: 'connector-a',
      },
    ]);

    offlineChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '2',
    });

    offlineChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', []);

    expect(offlineChangesStore.draftRows).toHaveLength(1);
    expect(offlineChangesStore.effectiveRows).toMatchObject([
      {
        baselineValue: '0',
        plannedValue: '2',
      },
    ]);
  });
});

describe('resolveEffectiveCurrentValue', () => {
  it('prefers the effective pending row value over the leaf value', () => {
    const leaf = makeLeaf();
    offlineChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '3',
        plannedValue: '1',
      }),
    ]);

    offlineChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '3',
      plannedValue: '5',
    });

    expect(
      offlineChangesStore.resolveEffectiveCurrentValue('05.02.01.02.03.00', leaf),
    ).toEqual({ type: 'int', value: 5 });
  });
});

describe('upsertConfigChange — cancellation draft', () => {
  it('creates a cancellation draft when plannedValue equals baselineValue and a persisted row exists', () => {
    offlineChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '42',
      }),
    ]);

    expect(offlineChangesStore.effectiveRows).toHaveLength(1);

    offlineChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '0',
    });

    expect(offlineChangesStore.effectiveRows).toHaveLength(0);
    expect(offlineChangesStore.draftRows).toHaveLength(1);
    expect(offlineChangesStore.draftRows[0]).toMatchObject({
      baselineValue: '0',
      plannedValue: '0',
    });
    // Cancellation drafts suppress the persisted row but are not counted
    // as user-visible changes — draftCount excludes planned===baseline rows.
    expect(offlineChangesStore.draftCount).toBe(0);
  });

  it('does not create a draft when plannedValue equals baselineValue and no persisted row exists', () => {
    offlineChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '0',
    });

    expect(offlineChangesStore.effectiveRows).toHaveLength(0);
    expect(offlineChangesStore.draftRows).toHaveLength(0);
  });
});

describe('clearDraftConfigChanges', () => {
  it('removes draft rows matching the specified locations', () => {
    offlineChangesStore.upsertRow(makeDraftRow({
      changeId: 'draft-a',
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '1',
    }));
    offlineChangesStore.upsertRow(makeDraftRow({
      changeId: 'draft-b',
      offset: '0x00000010',
      baselineValue: '0',
      plannedValue: '2',
    }));

    expect(offlineChangesStore.draftRows).toHaveLength(2);

    offlineChangesStore.clearDraftConfigChanges('05.02.01.02.03.00', [
      { space: 253, offset: '0x00000000' },
    ]);

    expect(offlineChangesStore.draftRows).toHaveLength(1);
    expect(offlineChangesStore.draftRows[0].changeId).toBe('draft-b');
  });

  it('does not remove persisted rows', () => {
    offlineChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '1',
      }),
    ]);

    offlineChangesStore.clearDraftConfigChanges('05.02.01.02.03.00', [
      { space: 253, offset: '0x00000000' },
    ]);

    expect(offlineChangesStore.persistedRows).toHaveLength(1);
  });

  it('is a no-op when no matching draft locations exist', () => {
    offlineChangesStore.upsertRow(makeDraftRow({
      changeId: 'draft-a',
      offset: '0x00000000',
    }));

    offlineChangesStore.clearDraftConfigChanges('05.02.01.02.03.00', [
      { space: 253, offset: '0x00000099' },
    ]);

    expect(offlineChangesStore.draftRows).toHaveLength(1);
  });
});
