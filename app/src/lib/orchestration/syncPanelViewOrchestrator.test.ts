import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ApplySyncResult, SyncSession } from '$lib/api/sync';
import {
  applySyncModeChoice,
  applySyncSelectionAndReconcile,
  hasSyncSessionRows,
} from './syncPanelViewOrchestrator';

function makeSession(overrides: Partial<SyncSession> = {}): SyncSession {
  return {
    conflictRows: [],
    cleanRows: [],
    alreadyAppliedCount: 0,
    nodeMissingRows: [],
    ...overrides,
  };
}

function makeStore(overrides: Partial<{
  session: SyncSession | null;
  setMode: ReturnType<typeof vi.fn>;
  loadSession: ReturnType<typeof vi.fn>;
  dismiss: ReturnType<typeof vi.fn>;
  applySelected: ReturnType<typeof vi.fn>;
}> = {}) {
  return {
    session: null,
    setMode: vi.fn(async () => {}),
    loadSession: vi.fn(async () => {}),
    dismiss: vi.fn(),
    applySelected: vi.fn(async () => null as ApplySyncResult | null),
    ...overrides,
  };
}

describe('hasSyncSessionRows', () => {
  it('returns false when the session has no actionable rows', () => {
    expect(hasSyncSessionRows(makeSession())).toBe(false);
  });

  it('returns true when any conflict, clean, or missing rows exist', () => {
    expect(hasSyncSessionRows(makeSession({ cleanRows: [{ changeId: 'row-1' } as any] }))).toBe(true);
    expect(hasSyncSessionRows(makeSession({ conflictRows: [{ changeId: 'row-1' } as any] }))).toBe(true);
    expect(hasSyncSessionRows(makeSession({ nodeMissingRows: [{ changeId: 'row-1' } as any] }))).toBe(true);
  });
});

describe('applySyncModeChoice', () => {
  let closePanel: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    closePanel = vi.fn();
  });

  it('dismisses immediately for bench mode without loading a session', async () => {
    const store = makeStore();

    await applySyncModeChoice(store, 'bench_other_bus', closePanel);

    expect(store.setMode).toHaveBeenCalledWith('bench_other_bus');
    expect(store.loadSession).not.toHaveBeenCalled();
    expect(store.dismiss).toHaveBeenCalledTimes(1);
    expect(closePanel).toHaveBeenCalledTimes(1);
  });

  it('dismisses target mode when the built session is empty', async () => {
    const store = makeStore({
      loadSession: vi.fn(async function (this: typeof store) {
        store.session = makeSession();
      }),
    });

    await applySyncModeChoice(store, 'target_layout_bus', closePanel);

    expect(store.loadSession).toHaveBeenCalledTimes(1);
    expect(store.dismiss).toHaveBeenCalledTimes(1);
    expect(closePanel).toHaveBeenCalledTimes(1);
  });

  it('keeps the panel open when target mode builds a non-empty session', async () => {
    const store = makeStore({
      loadSession: vi.fn(async () => {
        store.session = makeSession({ cleanRows: [{ changeId: 'row-1' } as any] });
      }),
    });

    await applySyncModeChoice(store, 'target_layout_bus', closePanel);

    expect(store.dismiss).not.toHaveBeenCalled();
    expect(closePanel).not.toHaveBeenCalled();
  });
});

describe('applySyncSelectionAndReconcile', () => {
  it('reconciles and dismisses after a fully successful apply', async () => {
    const reconcile = vi.fn(async () => {});
    const closePanel = vi.fn();
    const session = makeSession({ cleanRows: [{ changeId: 'row-1' } as any] });
    const store = makeStore({
      session,
      applySelected: vi.fn(async () => ({
        applied: ['row-1'],
        readOnlyCleared: [],
        failed: [],
      })),
    });

    const result = await applySyncSelectionAndReconcile(store, reconcile, closePanel);

    expect(result).toEqual({
      applied: ['row-1'],
      readOnlyCleared: [],
      failed: [],
    });
    expect(reconcile).toHaveBeenCalledWith(result, session);
    expect(store.dismiss).toHaveBeenCalledTimes(1);
    expect(closePanel).toHaveBeenCalledTimes(1);
  });

  it('keeps the panel open when apply returns failures', async () => {
    const reconcile = vi.fn(async () => {});
    const closePanel = vi.fn();
    const store = makeStore({
      session: makeSession({ conflictRows: [{ changeId: 'row-1' } as any] }),
      applySelected: vi.fn(async () => ({
        applied: [],
        readOnlyCleared: [],
        failed: [{ changeId: 'row-1', reason: 'write failed' }],
      })),
    });

    await applySyncSelectionAndReconcile(store, reconcile, closePanel);

    expect(reconcile).toHaveBeenCalledTimes(1);
    expect(store.dismiss).not.toHaveBeenCalled();
    expect(closePanel).not.toHaveBeenCalled();
  });
});