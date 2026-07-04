/**
 * Regression + unit tests for the config draft backend mirror
 * (2026-07-03 extension of ADR-0012). Drives the `reconcile(entries)`
 * seam directly instead of the `$effect.root` reactive path — mirrors
 * the `facilityCascadeOrchestrator` test style.
 *
 * The bug this suite pins:
 *
 *   Facility composition (`facilityOrchestrator.composeBowtiesIfWired`)
 *   stages consumer-leaf EventID edits through `configEditor.applyEdit`,
 *   which writes ONLY to `configChangesStore` (sync, no IPC). Before the
 *   mirror existed, no code observed those drafts and forwarded them to
 *   `setModifiedValue`, so a connected Save saw an empty
 *   `NodeProxy.modified_value` map and wrote nothing to the bus. The
 *   catalog then rebuilt from live state and the composed bowties
 *   disappeared. The mirror orchestrator is the reactive owner that
 *   closes that gap.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConfigDraftEntry } from '$lib/stores/configChanges.svelte';
import type { TreeConfigValue } from '$lib/types/nodeTree';

const { setModifiedValueRef, layoutStoreRef, nodeTreeStoreRef } = vi.hoisted(() => ({
  setModifiedValueRef: vi.fn(async () => {}),
  layoutStoreRef: { isConnected: true },
  nodeTreeStoreRef: { trees: new Map<string, unknown>() },
}));

vi.mock('$lib/api/config', () => ({
  setModifiedValue: setModifiedValueRef,
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: layoutStoreRef,
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: nodeTreeStoreRef,
}));

const { configDraftMirrorOrchestrator } = await import(
  './configDraftMirrorOrchestrator.svelte'
);

const REAL_KEY = '050201020300:253:100';
const REAL_DOTTED = '05.02.01.02.03.00';
const PLACEHOLDER_KEY =
  'placeholder:01234567-89ab-cdef-0123-456789abcdef:253:100';

function entry(key: string, value: TreeConfigValue): ConfigDraftEntry {
  return { key, value };
}

beforeEach(() => {
  vi.clearAllMocks();
  layoutStoreRef.isConnected = true;
  nodeTreeStoreRef.trees = new Map();
  // Reset the singleton's private last-seen map by stopping the mirror.
  configDraftMirrorOrchestrator.stopMirror();
});

describe('configDraftMirrorOrchestrator (2026-07-03 draft-backend mirror)', () => {
  it('emits setModifiedValue for a new draft while connected', () => {
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);

    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);

    expect(setModifiedValueRef).toHaveBeenCalledTimes(1);
    expect(setModifiedValueRef).toHaveBeenCalledWith(
      REAL_DOTTED,
      100,
      253,
      { type: 'int', value: 7 },
    );
  });

  it('regression: composed EventID drafts reach the backend on connected save', () => {
    // Reproduces the "connected save didn't write config values" bug.
    // Before the mirror, `configEditor.applyEdit` writes from facility
    // composition (or resetComposedLeavesForFacility) were invisible to
    // the backend at Save time because no callsite invoked
    // flushDraftToBackend on the composed leaves.
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);
    const composedEventId: TreeConfigValue = {
      type: 'eventId',
      bytes: [0x02, 0x01, 0x57, 0x00, 0x00, 0x02, 0xd9, 0x00],
      hex: '02.01.57.00.00.02.D9.00',
    };

    configDraftMirrorOrchestrator.reconcile([entry(REAL_KEY, composedEventId)]);

    expect(setModifiedValueRef).toHaveBeenCalledTimes(1);
    expect(setModifiedValueRef).toHaveBeenCalledWith(
      REAL_DOTTED,
      100,
      253,
      composedEventId,
    );
  });

  it('does NOT emit while the bus is offline', () => {
    layoutStoreRef.isConnected = false;
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);

    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);

    expect(setModifiedValueRef).not.toHaveBeenCalled();
  });

  it('skips placeholder NodeKeys (matches legacy flushDraftToBackend guard)', () => {
    configDraftMirrorOrchestrator.reconcile([
      entry(PLACEHOLDER_KEY, { type: 'string', value: 'Yard 1' }),
    ]);

    expect(setModifiedValueRef).not.toHaveBeenCalled();
  });

  it('does not re-emit for an unchanged draft on the next reconcile pass', () => {
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);
    const value: TreeConfigValue = { type: 'int', value: 7 };

    configDraftMirrorOrchestrator.reconcile([entry(REAL_KEY, value)]);
    configDraftMirrorOrchestrator.reconcile([entry(REAL_KEY, value)]);

    expect(setModifiedValueRef).toHaveBeenCalledTimes(1);
  });

  it('re-emits when a draft value changes', () => {
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);

    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);
    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 8 }),
    ]);

    expect(setModifiedValueRef).toHaveBeenCalledTimes(2);
    expect(setModifiedValueRef).toHaveBeenLastCalledWith(
      REAL_DOTTED,
      100,
      253,
      { type: 'int', value: 8 },
    );
  });

  it('emits no IPC when a draft is removed (baseline update / prune already handled backend)', () => {
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);

    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);
    setModifiedValueRef.mockClear();

    // Draft pruned/reverted — entries snapshot no longer includes the key.
    configDraftMirrorOrchestrator.reconcile([]);

    expect(setModifiedValueRef).not.toHaveBeenCalled();
  });

  it('drafts staged while offline are not re-emitted on reconnect', () => {
    // Plan caveat: connect state is checked inside the mirror body, not as a
    // reactive dependency. A draft added while offline is "acknowledged"
    // (last-seen advances) so a later connection does not flush the backlog.
    // "Flush pending drafts on connect" is a deliberate follow-up, not this
    // orchestrator's job.
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);
    layoutStoreRef.isConnected = false;
    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);
    expect(setModifiedValueRef).not.toHaveBeenCalled();

    layoutStoreRef.isConnected = true;
    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);

    expect(setModifiedValueRef).not.toHaveBeenCalled();
  });

  it('falls back to the normalized NodeID when the tree store has no matching key', () => {
    // No entry in nodeTreeStore.trees — findDottedNodeId returns null and
    // the emitter passes the normalized 12-hex form to setModifiedValue.
    nodeTreeStoreRef.trees = new Map();

    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);

    expect(setModifiedValueRef).toHaveBeenCalledWith(
      '050201020300',
      100,
      253,
      { type: 'int', value: 7 },
    );
  });

  it('stopMirror resets last-seen so the next start re-baselines cleanly', () => {
    nodeTreeStoreRef.trees = new Map([[REAL_DOTTED, {}]]);

    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);
    expect(setModifiedValueRef).toHaveBeenCalledTimes(1);

    configDraftMirrorOrchestrator.stopMirror();

    // After stop, the mirror forgets what it has seen; the same draft is
    // emitted again on the next reconcile so a fresh layout-open cycle
    // does not silently drop in-flight drafts.
    configDraftMirrorOrchestrator.reconcile([
      entry(REAL_KEY, { type: 'int', value: 7 }),
    ]);
    expect(setModifiedValueRef).toHaveBeenCalledTimes(2);
  });
});
