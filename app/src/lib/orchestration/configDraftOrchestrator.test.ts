/**
 * Regression tests for placeholder NodeKey handling in the config draft
 * orchestrator (Spec 014 / S8.5).
 *
 * The bus-sync offline changes queue only accepts real LCC NodeIDs — when a
 * placeholder NodeKey (`placeholder:<uuidv4>`) leaks into `replace_offline_changes`
 * the backend rejects it with "Invalid NodeID hex string length: 44". These
 * tests pin that:
 *   - `stageDraftsForOfflineSave` skips placeholder-keyed drafts and leaves
 *     them in `configChangesStore` (so the user's edits survive in-memory
 *     across a save until S9 wires them into the on-disk NodeSnapshot).
 *   - `flushDraftToBackend` does not call `setModifiedValue` for placeholders.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

const {
  configChangesStoreRef,
  offlineChangesStoreRef,
  setModifiedValueRef,
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
  offlineChangesStoreRef: {
    upsertConfigChange: vi.fn(),
    findDraftConfigChange: vi.fn(() => null),
    findPersistedConfigChange: vi.fn(() => null),
  },
  setModifiedValueRef: vi.fn(async () => {}),
  nodeTreeStoreRef: {
    trees: new Map(),
  },
}));

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: configChangesStoreRef,
}));

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: offlineChangesStoreRef,
}));

vi.mock('$lib/api/config', () => ({
  setModifiedValue: setModifiedValueRef,
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: nodeTreeStoreRef,
}));

import { stageDraftsForOfflineSave, flushDraftToBackend } from './configDraftOrchestrator';

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

describe('stageDraftsForOfflineSave', () => {
  it('stages real-node drafts into offlineChangesStore and clears them', () => {
    configChangesStoreRef._entries = [
      { key: '050201020300:253:100', value: { type: 'int', value: 7 } },
    ];

    stageDraftsForOfflineSave();

    expect(offlineChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(1);
    const arg = offlineChangesStoreRef.upsertConfigChange.mock.calls[0][0];
    expect(arg.nodeId).toBe('050201020300');
    expect(arg.space).toBe(253);
    expect(arg.offset).toBe('0x00000064');
    expect(configChangesStoreRef.revert).toHaveBeenCalledWith('050201020300:253:100');
    expect(configChangesStoreRef._entries).toHaveLength(0);
  });

  it('stages placeholder NodeKey drafts alongside real-node drafts (S8.12 unification)', () => {
    const placeholderKey =
      'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100';
    configChangesStoreRef._entries = [
      { key: placeholderKey, value: { type: 'string', value: 'Yard 1' } },
      { key: '050201020300:253:100', value: { type: 'int', value: 7 } },
    ];

    stageDraftsForOfflineSave();

    // Both drafts staged — placeholder and real node.
    expect(offlineChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(2);
    expect(
      offlineChangesStoreRef.upsertConfigChange.mock.calls[0][0].nodeId,
    ).toBe('placeholder:01234567-89ab-cdef-0123-456789abcdef');
    expect(
      offlineChangesStoreRef.upsertConfigChange.mock.calls[1][0].nodeId,
    ).toBe('050201020300');

    // Both drafts reverted from configChangesStore.
    expect(configChangesStoreRef._entries).toHaveLength(0);
  });

  it('stages placeholder-only drafts without throwing', () => {
    configChangesStoreRef._entries = [
      {
        key: 'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100',
        value: { type: 'string', value: 'Yard 1' },
      },
    ];

    expect(() => stageDraftsForOfflineSave()).not.toThrow();
    expect(offlineChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(1);
    expect(
      offlineChangesStoreRef.upsertConfigChange.mock.calls[0][0].nodeId,
    ).toBe('placeholder:01234567-89ab-cdef-0123-456789abcdef');
  });
});

describe('flushDraftToBackend', () => {
  it('does not call setModifiedValue for placeholder NodeKeys', () => {
    configChangesStoreRef._entries = [
      {
        key: 'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100',
        value: { type: 'string', value: 'Yard 1' },
      },
    ];

    flushDraftToBackend(
      'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100',
    );

    expect(setModifiedValueRef).not.toHaveBeenCalled();
  });

  it('calls setModifiedValue for real-node NodeIDs', () => {
    configChangesStoreRef._entries = [
      { key: '050201020300:253:100', value: { type: 'int', value: 7 } },
    ];

    flushDraftToBackend('050201020300:253:100');

    expect(setModifiedValueRef).toHaveBeenCalledTimes(1);
    expect(setModifiedValueRef).toHaveBeenCalledWith(
      '050201020300',
      100,
      253,
      { type: 'int', value: 7 },
    );
  });
});

// ─── S8.8 bug-closing contract ───────────────────────────────────────────────
//
// Spec 014 / S8.8 / T1 — regression contract for the unified placeholder
// edit transport. Before S8.8, `stageDraftsForOfflineSave` filtered
// placeholder-keyed drafts out of the offline staging path, so a placeholder
// field edit never reached disk and was lost on layout reload (the bug
// surfaced as "Save failed: Invalid NodeID hex string length: 44" in earlier
// builds and was patched into a silent drop in S8.5).
//
// Post-S8.8 architectural target: a placeholder is just a `NodeSnapshot`
// whose identity is a UUID. Edits ride the same `OfflineChangeRow` channel
// as real-node edits. The `replace_offline_changes` IPC accepts the
// placeholder NodeKey verbatim (no `NodeID::from_hex_string` validation).
//
// This test MUST stay green forever — it is the regression contract that
// keeps the workaround from creeping back in. Do not delete when refactoring.
describe('stageDraftsForOfflineSave — S8.8 placeholder unification', () => {
  it('stages placeholder NodeKey drafts into offlineChangesStore alongside real-node drafts', () => {
    const placeholderNodeKey = 'placeholder:01234567-89ab-cdef-0123-456789abcdef';
    const placeholderEditKey = `${placeholderNodeKey}:253:100`;
    configChangesStoreRef._entries = [
      { key: placeholderEditKey, value: { type: 'string', value: 'Yard 1' } },
    ];

    stageDraftsForOfflineSave();

    // The placeholder draft is staged into offlineChangesStore using its
    // NodeKey verbatim (not stripped, not validated as 12-hex). The IPC
    // accepts this shape post-S8.8-T3.
    expect(offlineChangesStoreRef.upsertConfigChange).toHaveBeenCalledTimes(1);
    const arg = offlineChangesStoreRef.upsertConfigChange.mock.calls[0][0];
    expect(arg.nodeId).toBe(placeholderNodeKey);
    expect(arg.space).toBe(253);
    expect(arg.offset).toBe('0x00000064');
    expect(arg.plannedValue).toBe('Yard 1');

    // The draft is cleared from configChangesStore — same lifecycle as a
    // real-node edit. No more "placeholder drafts survive in-memory across
    // save" workaround.
    expect(configChangesStoreRef._entries).toHaveLength(0);
  });
});
