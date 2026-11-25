/**
 * Regression tests for placeholder NodeKey handling in the config draft
 * orchestrator (Spec 014 / S8.5).
 *
 * The bus-sync pending changes queue only accepts real LCC NodeIDs — when a
 * placeholder NodeKey (`placeholder:<uuidv4>`) leaks into the recorded Pending Change rows
 * the backend rejects it with "Invalid NodeID hex string length: 44". These
 * tests pin that:
 *   - `stageConfigDraftsAsPendingChanges` stages placeholder-keyed drafts like
 *     real-node drafts and leaves all drafts in `configChangesStore` (the Save
 *     clears them once the rows are recorded).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

const {
  configChangesStoreRef,
  pendingChangesStoreRef,
  nodeTreeStoreRef,
} = vi.hoisted(() => ({
  configChangesStoreRef: {
    _entries: [] as { key: string; value: unknown }[],
    draftEntries: vi.fn(),
    visibleValue: vi.fn(),
    revert: vi.fn((key: string) => {
      configChangesStoreRef._entries = configChangesStoreRef._entries.filter((e) => e.key !== key);
      return true;
    }),
  },
  pendingChangesStoreRef: {
    upsertConfigChange: vi.fn(),
    findDraftConfigChange: vi.fn(() => null),
    findPersistedConfigChange: vi.fn(() => null),
  },
  nodeTreeStoreRef: {
    trees: new Map(),
  },
}));

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: configChangesStoreRef,
}));

vi.mock('$lib/stores/pendingChanges.svelte', () => ({
  pendingChangesStore: pendingChangesStoreRef,
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: nodeTreeStoreRef,
}));

import { stageConfigDraftsAsPendingChanges } from './configDraftOrchestrator';

beforeEach(() => {
  vi.clearAllMocks();
  configChangesStoreRef._entries = [];
  configChangesStoreRef.draftEntries.mockImplementation(() =>
    [...configChangesStoreRef._entries],
  );
  configChangesStoreRef.visibleValue.mockImplementation((key: string) => {
    const found = configChangesStoreRef._entries.find((e) => e.key === key);
    return found ? found.value : null;
  });
});

describe('stageConfigDraftsAsPendingChanges', () => {
  it('stages real-node drafts into pendingChangesStore and keeps them as drafts', () => {
    configChangesStoreRef._entries = [
      { key: '050201020300:253:100', value: { type: 'int', value: 7 } },
    ];

    stageConfigDraftsAsPendingChanges();

    expect(pendingChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(1);
    const arg = pendingChangesStoreRef.upsertConfigChange.mock.calls[0][0];
    expect(arg.nodeId).toBe('050201020300');
    expect(arg.space).toBe(253);
    expect(arg.offset).toBe('0x00000064');
    // E3b-1 B2: staging does not clear the drafts; the Save clears them only
    // after the rows are recorded in the layout file.
    expect(configChangesStoreRef.revert).not.toHaveBeenCalled();
    expect(configChangesStoreRef._entries).toHaveLength(1);
  });

  it('stages placeholder NodeKey drafts alongside real-node drafts (S8.12 unification)', () => {
    const placeholderKey =
      'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100';
    configChangesStoreRef._entries = [
      { key: placeholderKey, value: { type: 'string', value: 'Yard 1' } },
      { key: '050201020300:253:100', value: { type: 'int', value: 7 } },
    ];

    stageConfigDraftsAsPendingChanges();

    // Both drafts staged — placeholder and real node.
    expect(pendingChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(2);
    expect(
      pendingChangesStoreRef.upsertConfigChange.mock.calls[0][0].nodeId,
    ).toBe('placeholder:01234567-89ab-cdef-0123-456789abcdef');
    expect(
      pendingChangesStoreRef.upsertConfigChange.mock.calls[1][0].nodeId,
    ).toBe('050201020300');

    // Drafts stay until the Save has recorded the rows.
    expect(configChangesStoreRef._entries).toHaveLength(2);
  });

  it('stages placeholder-only drafts without throwing', () => {
    configChangesStoreRef._entries = [
      {
        key: 'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100',
        value: { type: 'string', value: 'Yard 1' },
      },
    ];

    expect(() => stageConfigDraftsAsPendingChanges()).not.toThrow();
    expect(pendingChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(1);
    expect(
      pendingChangesStoreRef.upsertConfigChange.mock.calls[0][0].nodeId,
    ).toBe('placeholder:01234567-89ab-cdef-0123-456789abcdef');
  });
});

// ─── S8.8 bug-closing contract ───────────────────────────────────────────────
//
// Spec 014 / S8.8 / T1 — regression contract for the unified placeholder
// edit transport. Before S8.8, `stageConfigDraftsAsPendingChanges` filtered
// placeholder-keyed drafts out of the offline staging path, so a placeholder
// field edit never reached disk and was lost on layout reload (the bug
// surfaced as "Save failed: Invalid NodeID hex string length: 44" in earlier
// builds and was patched into a silent drop in S8.5).
//
// Post-S8.8 architectural target: a placeholder is just a `NodeSnapshot`
// whose identity is a UUID. Edits ride the same `PendingChangeRow` channel
// as real-node edits. The backend accepts the
// placeholder NodeKey verbatim (no `NodeID::from_hex_string` validation).
//
// This test MUST stay green forever — it is the regression contract that
// keeps the workaround from creeping back in. Do not delete when refactoring.
describe('stageConfigDraftsAsPendingChanges — S8.8 placeholder unification', () => {
  it('stages placeholder NodeKey drafts into pendingChangesStore alongside real-node drafts', () => {
    const placeholderNodeKey = 'placeholder:01234567-89ab-cdef-0123-456789abcdef';
    const placeholderEditKey = `${placeholderNodeKey}:253:100`;
    configChangesStoreRef._entries = [
      { key: placeholderEditKey, value: { type: 'string', value: 'Yard 1' } },
    ];

    stageConfigDraftsAsPendingChanges();

    // The placeholder draft is staged into pendingChangesStore using its
    // NodeKey verbatim (not stripped, not validated as 12-hex). The IPC
    // accepts this shape post-S8.8-T3.
    expect(pendingChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(1);
    const arg = pendingChangesStoreRef.upsertConfigChange.mock.calls[0][0];
    expect(arg.nodeId).toBe(placeholderNodeKey);
    expect(arg.space).toBe(253);
    expect(arg.offset).toBe('0x00000064');
    expect(arg.plannedValue).toBe('Yard 1');

    // Same lifecycle as a real-node edit: the Save clears the draft once the
    // row is recorded.
    expect(configChangesStoreRef._entries).toHaveLength(1);
  });
});
