/**
 * Store-level tests for pendingChangesStore (pendingChanges.svelte.ts).
 *
 * Covers:
 * - revertToBaseline removes draft-only changes without backend IPC
 * - revertToBaseline calls backend IPC for persisted changes
 * - draftCount updates after revert
 * - isBusy lifecycle during revert
 * - Error handling
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { PendingChangeRow } from '$lib/api/sync';
import type { LeafConfigNode } from '$lib/types/nodeTree';

// ─── Mock backend IPC ─────────────────────────────────────────────────────────

const mockListPendingChanges = vi.fn<() => Promise<PendingChangeRow[]>>().mockResolvedValue([]);
const mockRevertPendingChange = vi.fn().mockResolvedValue(undefined);
const mockSetPendingChange = vi.fn<(input: unknown) => Promise<string>>();

vi.mock('$lib/api/sync', () => ({
  listPendingChanges: mockListPendingChanges,
  revertPendingChange: mockRevertPendingChange,
  setPendingChange: mockSetPendingChange,
}));

// ─── Import store AFTER mocks ────────────────────────────────────────────────

const { pendingChangesStore } = await import('$lib/stores/pendingChanges.svelte');

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeDraftRow(overrides: Partial<PendingChangeRow> = {}): PendingChangeRow {
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
  pendingChangesStore.clear();
  vi.clearAllMocks();
  mockListPendingChanges.mockResolvedValue([]);
});

describe('revertToBaseline — draft-only changes', () => {
  it('removes a draft change from the store', async () => {
    const row = makeDraftRow({ changeId: 'draft-1' });
    pendingChangesStore.upsertRow(row);
    expect(pendingChangesStore.draftCount).toBe(1);

    const result = await pendingChangesStore.revertToBaseline('draft-1');
    expect(result).toBe(true);
    expect(pendingChangesStore.draftCount).toBe(0);
  });

  it('does not call backend IPC for draft-only changes', async () => {
    const row = makeDraftRow({ changeId: 'draft-1' });
    pendingChangesStore.upsertRow(row);

    await pendingChangesStore.revertToBaseline('draft-1');
    expect(mockRevertPendingChange).not.toHaveBeenCalled();
  });

  it('leaves other draft changes intact', async () => {
    const row1 = makeDraftRow({ changeId: 'draft-1', offset: '0x00000000' });
    const row2 = makeDraftRow({ changeId: 'draft-2', offset: '0x00000001' });
    pendingChangesStore.upsertRow(row1);
    pendingChangesStore.upsertRow(row2);
    expect(pendingChangesStore.draftCount).toBe(2);

    await pendingChangesStore.revertToBaseline('draft-1');
    expect(pendingChangesStore.draftCount).toBe(1);
    expect(pendingChangesStore.draftRows[0].changeId).toBe('draft-2');
  });
});

describe('revertToBaseline — persisted changes', () => {
  it('calls backend IPC for persisted changes', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-1' });
    pendingChangesStore.setRows([persistedRow]);
    expect(pendingChangesStore.pendingApplyCount).toBe(1);

    await pendingChangesStore.revertToBaseline('persisted-1');
    expect(mockRevertPendingChange).toHaveBeenCalledWith('persisted-1');
  });

  it('removes the row from working persisted rows without reloading the saved snapshot', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-1' });
    pendingChangesStore.setRows([persistedRow]);

    await pendingChangesStore.revertToBaseline('persisted-1');

    expect(mockListPendingChanges).not.toHaveBeenCalled();
    expect(pendingChangesStore.persistedRows).toHaveLength(0);
    expect(pendingChangesStore.savedRows).toHaveLength(1);
    expect(pendingChangesStore.savedRows[0].changeId).toBe('persisted-1');
  });
});

describe('revertToBaseline — error handling', () => {
  it('returns false when backend IPC fails', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-err' });
    pendingChangesStore.setRows([persistedRow]);
    mockRevertPendingChange.mockRejectedValueOnce(new Error('IPC failed'));

    const result = await pendingChangesStore.revertToBaseline('persisted-err');
    expect(result).toBe(false);
  });

  it('clears isBusy even on error', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-err' });
    pendingChangesStore.setRows([persistedRow]);
    mockRevertPendingChange.mockRejectedValueOnce(new Error('IPC failed'));

    await pendingChangesStore.revertToBaseline('persisted-err');
    expect(pendingChangesStore.isBusy).toBe(false);
  });
});

describe('revertToBaseline — isBusy lifecycle', () => {
  it('sets isBusy during operation and clears after', async () => {
    const row = makeDraftRow({ changeId: 'draft-busy' });
    pendingChangesStore.upsertRow(row);

    // After the operation completes, isBusy should be false
    await pendingChangesStore.revertToBaseline('draft-busy');
    expect(pendingChangesStore.isBusy).toBe(false);
  });
});

describe('revertToBaseline — persisted row updates working state only', () => {
  it('pendingApplyCount drops to 0 after persisted revert when backend returns empty list', async () => {
    const persistedRow = makeDraftRow({ changeId: 'persisted-dirty' });
    pendingChangesStore.setRows([persistedRow]);
    expect(pendingChangesStore.pendingApplyCount).toBe(1);

    await pendingChangesStore.revertToBaseline('persisted-dirty');

    expect(pendingChangesStore.pendingApplyCount).toBe(0);
    expect(pendingChangesStore.savedRows).toHaveLength(1);
  });

  it('revertToBaseline for a draft row does not call backend IPC (no save needed)', async () => {
    const row = makeDraftRow({ changeId: 'draft-no-dirty' });
    pendingChangesStore.upsertRow(row);

    await pendingChangesStore.revertToBaseline('draft-no-dirty');

    // Draft reverts never touch the backend — no opportunity to trigger dirty state
    expect(mockRevertPendingChange).not.toHaveBeenCalled();
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
    pendingChangesStore.setRows([savedRow]);

    pendingChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000010',
      baselineValue: '10',
      plannedValue: '10',
    });

    // Upserting planned===baseline with a persisted row creates a cancellation
    // draft that suppresses the persisted row in effectiveRows.
    // Cancellation drafts are not user-visible changes, so draftCount stays 0.
    expect(pendingChangesStore.draftCount).toBe(0);
    expect(pendingChangesStore.effectiveRows).toHaveLength(0);

    const reverted = await pendingChangesStore.revertAllPending();

    // After revert, the cancellation draft is cleared and the persisted row
    // is restored from the saved snapshot.
    expect(reverted).toBe(1);
    expect(pendingChangesStore.draftCount).toBe(0);
    expect(pendingChangesStore.persistedRows).toHaveLength(1);
    expect(pendingChangesStore.persistedRows[0].changeId).toBe('saved-1');
    expect(pendingChangesStore.savedRows).toHaveLength(1);
    expect(pendingChangesStore.savedRows[0].changeId).toBe('saved-1');
  });

  it('keeps saved snapshot aligned after reloadFromBackend', async () => {
    const backendRow = makeDraftRow({ changeId: 'backend-1', offset: '0x00000022' });
    mockListPendingChanges.mockImplementationOnce(async () => [backendRow]);

    await pendingChangesStore.reloadFromBackend();

    expect(pendingChangesStore.persistedRows).toHaveLength(1);
    expect(pendingChangesStore.savedRows).toHaveLength(1);
    expect(pendingChangesStore.persistedRows[0].changeId).toBe('backend-1');
    expect(pendingChangesStore.savedRows[0].changeId).toBe('backend-1');
  });
});

describe('applyConnectorCompatibilityConfigChanges', () => {
  it('stages generated connector repairs as draft config rows', () => {
    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
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

    expect(pendingChangesStore.draftRows).toHaveLength(1);
    expect(pendingChangesStore.draftRows[0]).toMatchObject({
      kind: 'config',
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '3',
      plannedValue: '1',
    });
  });

  it('finds the effective config row after layering a draft over a persisted row', () => {
    pendingChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
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
      pendingChangesStore.findEffectiveConfigChange('05.02.01.02.03.00', 253, '0x00000000'),
    ).toMatchObject({
      baselineValue: '0',
      plannedValue: '1',
    });
  });

  it('retains previously generated target corrections when the next recompute emits no new repair for that target', () => {
    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
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

    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', []);

    expect(pendingChangesStore.effectiveRows).toMatchObject([
      {
        nodeId: '05.02.01.02.03.00',
        space: 253,
        offset: '0x00000000',
        plannedValue: '1',
      },
    ]);
  });

  it('keeps the persisted baseline when layering a generated correction over a persisted pending change', () => {
    pendingChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
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

    expect(pendingChangesStore.draftRows).toHaveLength(1);
    expect(pendingChangesStore.draftRows[0]).toMatchObject({
      changeId: expect.not.stringContaining('persisted-1'),
      baselineValue: '0',
      plannedValue: '1',
    });
  });

  it('reverting a draft layered over a persisted row keeps the persisted row intact', async () => {
    pendingChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
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

    const draftId = pendingChangesStore.draftRows[0]?.changeId;
    expect(draftId).toBeTruthy();

    await pendingChangesStore.revertToBaseline(draftId!);

    expect(mockRevertPendingChange).not.toHaveBeenCalled();
    expect(pendingChangesStore.draftRows).toHaveLength(0);
    expect(pendingChangesStore.persistedRows).toHaveLength(1);
    expect(pendingChangesStore.effectiveRows).toMatchObject([
      {
        changeId: 'persisted-1',
        baselineValue: '0',
        plannedValue: '3',
      },
    ]);
  });

  it('preserves the connector-corrected effective value when switching boards no longer emits a new repair', () => {
    pendingChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '3',
      }),
    ]);

    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
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

    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', []);

    expect(pendingChangesStore.draftRows).toHaveLength(1);
    expect(pendingChangesStore.effectiveRows).toMatchObject([
      {
        changeId: expect.not.stringContaining('persisted-1'),
        baselineValue: '0',
        plannedValue: '1',
      },
    ]);
  });

  it('keeps a user override when connector-generated repairs are later cleared', () => {
    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', [
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

    pendingChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '2',
    });

    pendingChangesStore.applyConnectorCompatibilityConfigChanges('05.02.01.02.03.00', []);

    expect(pendingChangesStore.draftRows).toHaveLength(1);
    expect(pendingChangesStore.effectiveRows).toMatchObject([
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
    pendingChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '3',
        plannedValue: '1',
      }),
    ]);

    pendingChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '3',
      plannedValue: '5',
    });

    expect(
      pendingChangesStore.resolveEffectiveCurrentValue('05.02.01.02.03.00', leaf),
    ).toEqual({ type: 'int', value: 5 });
  });
});

describe('upsertConfigChange — cancellation draft', () => {
  it('creates a cancellation draft when plannedValue equals baselineValue and a persisted row exists', () => {
    pendingChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '42',
      }),
    ]);

    expect(pendingChangesStore.effectiveRows).toHaveLength(1);

    pendingChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '0',
    });

    expect(pendingChangesStore.effectiveRows).toHaveLength(0);
    expect(pendingChangesStore.draftRows).toHaveLength(1);
    expect(pendingChangesStore.draftRows[0]).toMatchObject({
      baselineValue: '0',
      plannedValue: '0',
    });
    // Cancellation drafts suppress the persisted row but are not counted
    // as user-visible changes — draftCount excludes planned===baseline rows.
    expect(pendingChangesStore.draftCount).toBe(0);
  });

  it('does not create a draft when plannedValue equals baselineValue and no persisted row exists', () => {
    pendingChangesStore.upsertConfigChange({
      nodeId: '05.02.01.02.03.00',
      space: 253,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '0',
    });

    expect(pendingChangesStore.effectiveRows).toHaveLength(0);
    expect(pendingChangesStore.draftRows).toHaveLength(0);
  });
});

describe('clearDraftConfigChanges', () => {
  it('removes draft rows matching the specified locations', () => {
    pendingChangesStore.upsertRow(makeDraftRow({
      changeId: 'draft-a',
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '1',
    }));
    pendingChangesStore.upsertRow(makeDraftRow({
      changeId: 'draft-b',
      offset: '0x00000010',
      baselineValue: '0',
      plannedValue: '2',
    }));

    expect(pendingChangesStore.draftRows).toHaveLength(2);

    pendingChangesStore.clearDraftConfigChanges('05.02.01.02.03.00', [
      { space: 253, offset: '0x00000000' },
    ]);

    expect(pendingChangesStore.draftRows).toHaveLength(1);
    expect(pendingChangesStore.draftRows[0].changeId).toBe('draft-b');
  });

  it('does not remove persisted rows', () => {
    pendingChangesStore.setRows([
      makeDraftRow({
        changeId: 'persisted-1',
        offset: '0x00000000',
        baselineValue: '0',
        plannedValue: '1',
      }),
    ]);

    pendingChangesStore.clearDraftConfigChanges('05.02.01.02.03.00', [
      { space: 253, offset: '0x00000000' },
    ]);

    expect(pendingChangesStore.persistedRows).toHaveLength(1);
  });

  it('is a no-op when no matching draft locations exist', () => {
    pendingChangesStore.upsertRow(makeDraftRow({
      changeId: 'draft-a',
      offset: '0x00000000',
    }));

    pendingChangesStore.clearDraftConfigChanges('05.02.01.02.03.00', [
      { space: 253, offset: '0x00000099' },
    ]);

    expect(pendingChangesStore.draftRows).toHaveLength(1);
  });
});

describe('recordDraftsToBackend (E3b-1: IDs this Save recorded)', () => {
  it('records each draft as a Pending Change and returns only those IDs, leaving earlier rows alone', async () => {
    const earlier = makeDraftRow({
      changeId: 'earlier-conflict', offset: '0x00000010', status: 'conflict', error: 'The board kept 5',
    });
    pendingChangesStore.setRows([earlier]);
    pendingChangesStore.upsertRow(makeDraftRow({ changeId: 'draft-a', offset: '0x00000000', plannedValue: '7' }));
    mockSetPendingChange.mockResolvedValue('backend-id-a');
    mockListPendingChanges.mockResolvedValue([
      earlier,
      makeDraftRow({ changeId: 'backend-id-a', offset: '0x00000000', plannedValue: '7' }),
    ]);

    const ids = await pendingChangesStore.recordDraftsToBackend();

    expect(ids).toEqual(['backend-id-a']);
    expect(mockSetPendingChange).toHaveBeenCalledTimes(1);
    expect(mockSetPendingChange).toHaveBeenCalledWith(expect.objectContaining({
      kind: 'config', offset: '0x00000000', plannedValue: '7',
    }));
    expect(pendingChangesStore.persistedRows.map((r) => r.changeId)).toEqual(['earlier-conflict', 'backend-id-a']);
    expect(pendingChangesStore.draftRows).toHaveLength(0);
  });
});

describe('still-waiting rows (E3b D3: failed and conflict rows are not applied)', () => {
  beforeEach(() => {
    pendingChangesStore.clear();
  });

  it('counts failed and conflict rows as waiting, but not applied or clean rows', () => {
    pendingChangesStore.setRows([
      makeDraftRow({ changeId: 'a', offset: '0x00000001', status: 'pending' }),
      makeDraftRow({ changeId: 'b', offset: '0x00000002', status: 'failed', error: 'timeout' }),
      makeDraftRow({ changeId: 'c', offset: '0x00000003', status: 'conflict', error: 'The board kept 5' }),
      makeDraftRow({ changeId: 'd', offset: '0x00000004', status: 'applied' }),
      makeDraftRow({ changeId: 'e', offset: '0x00000005', status: 'clean' }),
    ]);

    expect(pendingChangesStore.pendingCount).toBe(3);
  });

  it('finds a conflict row as the persisted change for its field', () => {
    pendingChangesStore.setRows([
      makeDraftRow({ changeId: 'c', offset: '0x00000003', status: 'conflict', error: 'The board kept 5' }),
    ]);

    expect(
      pendingChangesStore.findPersistedConfigChange('05.02.01.02.03.00', 253, '0x00000003')?.changeId,
    ).toBe('c');
  });
});
