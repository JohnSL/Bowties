/**
 * T020: Vitest unit tests for PendingEditsStore.
 *
 * Tests add/remove, state queries, state transitions, per-node and per-segment
 * queries, clearAll, and auto-remove on value revert.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { pendingEditsStore, makePendingEditKey } from '$lib/stores/pendingEdits.svelte';
import type { PendingEdit, TreeConfigValue, LeafConstraints } from '$lib/types/nodeTree';

// ─── Test helpers ─────────────────────────────────────────────────────────────

const NODE_A = '05.01.01.01.03.00';
const NODE_B = '05.01.01.01.03.01';

function makeEdit(
  nodeId: string,
  space: number,
  address: number,
  original: TreeConfigValue,
  pending: TreeConfigValue,
  overrides?: Partial<PendingEdit>,
): PendingEdit {
  const key = makePendingEditKey(nodeId, space, address);
  return {
    key,
    nodeId,
    segmentOrigin: 0,
    segmentName: 'Configuration',
    address,
    space,
    size: 4,
    elementType: 'int',
    fieldPath: ['seg:0', 'elem:0'],
    fieldLabel: 'Test Field',
    originalValue: original,
    pendingValue: pending,
    validationState: 'valid',
    validationMessage: null,
    writeState: 'dirty',
    writeError: null,
    constraints: null,
    ...overrides,
  };
}

function intVal(n: number): TreeConfigValue {
  return { type: 'int', value: n };
}

function strVal(s: string): TreeConfigValue {
  return { type: 'string', value: s };
}

// ─── Reset state before each test ────────────────────────────────────────────

beforeEach(() => {
  pendingEditsStore.clearAll();
});

// ─── Basic add / remove ───────────────────────────────────────────────────────

describe('setEdit / removeEdit', () => {
  it('adds an edit and makes it retrievable', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    const edit = makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(42));
    pendingEditsStore.setEdit(key, edit);

    expect(pendingEditsStore.getEdit(key)).toMatchObject({ pendingValue: intVal(42) });
    expect(pendingEditsStore.dirtyCount).toBe(1);
  });

  it('removes an edit by key', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)));
    pendingEditsStore.removeEdit(key);

    expect(pendingEditsStore.getEdit(key)).toBeUndefined();
    expect(pendingEditsStore.dirtyCount).toBe(0);
  });

  it('auto-removes when pendingValue equals originalValue (revert)', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(5), intVal(5)));

    expect(pendingEditsStore.getEdit(key)).toBeUndefined();
    expect(pendingEditsStore.dirtyCount).toBe(0);
  });

  it('replaces existing edit with same key', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)));
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(99)));

    expect(pendingEditsStore.getEdit(key)?.pendingValue).toEqual(intVal(99));
    expect(pendingEditsStore.dirtyCount).toBe(1);
  });
});

// ─── Dirty count / hasPendingEdits ────────────────────────────────────────────

describe('dirtyCount / hasPendingEdits', () => {
  it('starts with zero edits', () => {
    expect(pendingEditsStore.dirtyCount).toBe(0);
    expect(pendingEditsStore.hasPendingEdits).toBe(false);
  });

  it('increments when edits are added', () => {
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 100),
      makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)),
    );
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 200),
      makeEdit(NODE_A, 0xfd, 200, intVal(0), intVal(2)),
    );
    expect(pendingEditsStore.dirtyCount).toBe(2);
    expect(pendingEditsStore.hasPendingEdits).toBe(true);
  });
});

// ─── hasInvalid ───────────────────────────────────────────────────────────────

describe('hasInvalid', () => {
  it('returns false when all edits are valid', () => {
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 100),
      makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1), { validationState: 'valid' }),
    );
    expect(pendingEditsStore.hasInvalid).toBe(false);
  });

  it('returns true when at least one edit is invalid', () => {
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 100),
      makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1), { validationState: 'valid' }),
    );
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 200),
      makeEdit(NODE_A, 0xfd, 200, intVal(0), intVal(999), { validationState: 'invalid' }),
    );
    expect(pendingEditsStore.hasInvalid).toBe(true);
  });
});

// ─── State transitions ────────────────────────────────────────────────────────

describe('markWriting / markError / markClean', () => {
  const key = makePendingEditKey(NODE_A, 0xfd, 100);

  beforeEach(() => {
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(42)));
  });

  it('dirty → writing: sets writeState to "writing"', () => {
    pendingEditsStore.markWriting(key);
    expect(pendingEditsStore.getEdit(key)?.writeState).toBe('writing');
  });

  it('writing → error: sets writeState to "error" with message', () => {
    pendingEditsStore.markWriting(key);
    pendingEditsStore.markError(key, 'Timeout after 3 retries');
    const edit = pendingEditsStore.getEdit(key);
    expect(edit?.writeState).toBe('error');
    expect(edit?.writeError).toBe('Timeout after 3 retries');
  });

  it('writing → clean: removes the edit (write succeeded)', () => {
    pendingEditsStore.markWriting(key);
    pendingEditsStore.markClean(key);
    expect(pendingEditsStore.getEdit(key)).toBeUndefined();
    expect(pendingEditsStore.dirtyCount).toBe(0);
  });

  it('markWriting is a no-op for unknown key', () => {
    pendingEditsStore.markWriting('nonexistent:253:0');
    // should not throw, and store unchanged
    expect(pendingEditsStore.dirtyCount).toBe(1);
  });

  it('markError is a no-op for unknown key', () => {
    pendingEditsStore.markError('nonexistent:253:0', 'oops');
    expect(pendingEditsStore.dirtyCount).toBe(1);
  });
});

// ─── Per-node queries ─────────────────────────────────────────────────────────

describe('getDirtyForNode', () => {
  it('returns only edits for the specified node', () => {
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 100),
      makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)),
    );
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_B, 0xfd, 100),
      makeEdit(NODE_B, 0xfd, 100, intVal(0), intVal(2)),
    );

    const forA = pendingEditsStore.getDirtyForNode(NODE_A);
    expect(forA).toHaveLength(1);
    expect(forA[0].nodeId).toBe(NODE_A);
  });

  it('excludes edits in "writing" state', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)));
    pendingEditsStore.markWriting(key);

    expect(pendingEditsStore.getDirtyForNode(NODE_A)).toHaveLength(0);
  });

  it('returns empty array when node has no edits', () => {
    expect(pendingEditsStore.getDirtyForNode('99.99.99.99.99.99')).toEqual([]);
  });
});

// ─── Per-segment queries ──────────────────────────────────────────────────────

describe('getDirtyForSegment', () => {
  it('returns only edits matching node and segment origin', () => {
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 100),
      makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1), { segmentOrigin: 0 }),
    );
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 500),
      makeEdit(NODE_A, 0xfd, 500, intVal(0), intVal(2), { segmentOrigin: 256 }),
    );

    const seg0 = pendingEditsStore.getDirtyForSegment(NODE_A, 0);
    expect(seg0).toHaveLength(1);
    expect(seg0[0].address).toBe(100);

    const seg256 = pendingEditsStore.getDirtyForSegment(NODE_A, 256);
    expect(seg256).toHaveLength(1);
    expect(seg256[0].address).toBe(500);
  });
});

// ─── clearAll / clearForNode ──────────────────────────────────────────────────

describe('clearAll / clearForNode', () => {
  beforeEach(() => {
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 100),
      makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)),
    );
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_B, 0xfd, 200),
      makeEdit(NODE_B, 0xfd, 200, intVal(0), intVal(2)),
    );
  });

  it('clearAll removes all edits', () => {
    pendingEditsStore.clearAll();
    expect(pendingEditsStore.dirtyCount).toBe(0);
  });

  it('clearForNode removes only edits for that node', () => {
    pendingEditsStore.clearForNode(NODE_A);
    expect(pendingEditsStore.dirtyCount).toBe(1);
    expect(pendingEditsStore.getDirtyForNode(NODE_B)).toHaveLength(1);
  });
});

// ─── allEdits snapshot ────────────────────────────────────────────────────────

describe('allEdits', () => {
  it('returns a flat array of all edits', () => {
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 100),
      makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)),
    );
    pendingEditsStore.setEdit(
      makePendingEditKey(NODE_A, 0xfd, 200),
      makeEdit(NODE_A, 0xfd, 200, strVal('old'), strVal('new'), { elementType: 'string' }),
    );

    expect(pendingEditsStore.allEdits).toHaveLength(2);
  });
});

// ─── T043: US5 — Error state transitions ─────────────────────────────────────

describe('T043: error state transitions', () => {
  beforeEach(() => pendingEditsStore.clearAll());

  it('markWriting transitions dirty → writing', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)));
    expect(pendingEditsStore.getEdit(key)?.writeState).toBe('dirty');

    pendingEditsStore.markWriting(key);
    expect(pendingEditsStore.getEdit(key)?.writeState).toBe('writing');
  });

  it('markError transitions writing → error with message', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)));
    pendingEditsStore.markWriting(key);

    pendingEditsStore.markError(key, 'Connection timeout');

    const edit = pendingEditsStore.getEdit(key);
    expect(edit?.writeState).toBe('error');
    expect(edit?.writeError).toBe('Connection timeout');
  });

  it('re-editing an error field transitions writeState back to dirty', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 100);
    // Set up field in error state
    const edit = makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1));
    pendingEditsStore.setEdit(key, edit);
    pendingEditsStore.markWriting(key);
    pendingEditsStore.markError(key, 'Timeout');
    expect(pendingEditsStore.getEdit(key)?.writeState).toBe('error');

    // Re-edit the field with a new value
    const reedited = makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(2));
    pendingEditsStore.setEdit(key, reedited);

    expect(pendingEditsStore.getEdit(key)?.writeState).toBe('dirty');
    expect(pendingEditsStore.getEdit(key)?.writeError).toBeNull();
  });

  it('getRetryableForSegment returns both dirty and error edits', () => {
    const key1 = makePendingEditKey(NODE_A, 0xfd, 100);
    const key2 = makePendingEditKey(NODE_A, 0xfd, 200);
    const key3 = makePendingEditKey(NODE_A, 0xfd, 300);

    pendingEditsStore.setEdit(key1, makeEdit(NODE_A, 0xfd, 100, intVal(0), intVal(1)));
    pendingEditsStore.setEdit(key2, makeEdit(NODE_A, 0xfd, 200, intVal(0), intVal(2)));
    pendingEditsStore.setEdit(key3, makeEdit(NODE_A, 0xfd, 300, intVal(0), intVal(3)));

    // Mark key2 as error (simulating save failure)
    pendingEditsStore.markWriting(key2);
    pendingEditsStore.markError(key2, 'Write failed');

    // Mark key3 as clean (succeeded)
    pendingEditsStore.markClean(key3);

    const retryable = pendingEditsStore.getRetryableForSegment(NODE_A, 0);
    expect(retryable).toHaveLength(2);  // key1 (dirty) + key2 (error), not key3 (clean/removed)

    const retryableKeys = retryable.map(e => e.key);
    expect(retryableKeys).toContain(key1);
    expect(retryableKeys).toContain(key2);
    expect(retryableKeys).not.toContain(key3);
  });

  it('markClean removes the edit (key3 is gone after success)', () => {
    const key = makePendingEditKey(NODE_A, 0xfd, 400);
    pendingEditsStore.setEdit(key, makeEdit(NODE_A, 0xfd, 400, intVal(0), intVal(99)));
    pendingEditsStore.markWriting(key);
    pendingEditsStore.markClean(key);

    expect(pendingEditsStore.getEdit(key)).toBeUndefined();
  });
});
