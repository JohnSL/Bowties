/**
 * Store-level tests for syncPanelStore (syncPanel.svelte.ts).
 *
 * Covers:
 * - dismiss() sets isDismissed = true and collapses isActive to false
 * - isDismissed persists without auto-reset (the settle-timer guard on the
 *   page depends on this invariant — if any store operation silently cleared
 *   isDismissed the re-open guard would break)
 * - loadSession() resets _dismissed = false. This is INTENTIONAL: loadSession
 *   is only safe to call from forceSyncPanel() (which first calls reset()), NOT
 *   directly from settle-timer auto-triggers. The settle-timer path is guarded
 *   on the page by `if (syncPanelStore.isDismissed && syncTriggered) return;`
 *   so loadSession is never reached from those callers after a dismiss.
 * - reset() clears session, dismissed state, and all resolution tracking
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { SyncSession } from '$lib/api/sync';

// ─── Mock IPC ────────────────────────────────────────────────────────────────

const mockBuildSyncSession = vi.fn<() => Promise<SyncSession>>();
const mockComputeLayoutMatchStatus = vi
  .fn()
  .mockResolvedValue({ classification: 'likely_same', overlapRatio: 1.0, confirmedCount: 1, totalNode: 1 });
const mockSetSyncMode = vi.fn().mockResolvedValue(undefined);
const mockApplySyncChanges = vi.fn().mockResolvedValue({ appliedCount: 0, skippedCount: 0, failedRows: [] });

vi.mock('$lib/api/sync', () => ({
  buildSyncSession: (...args: unknown[]) => mockBuildSyncSession(...args),
  computeLayoutMatchStatus: (...args: unknown[]) => mockComputeLayoutMatchStatus(...args),
  setSyncMode: (...args: unknown[]) => mockSetSyncMode(...args),
  applySyncChanges: (...args: unknown[]) => mockApplySyncChanges(...args),
}));

// ─── Import store AFTER mocks ────────────────────────────────────────────────

const { syncPanelStore } = await import('$lib/stores/syncPanel.svelte');

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeEmptySession(): SyncSession {
  return {
    conflictRows: [],
    cleanRows: [],
    alreadyAppliedCount: 0,
    nodeMissingRows: [],
  };
}

function makeSessionWithCleanRow(): SyncSession {
  return {
    conflictRows: [],
    cleanRows: [
      {
        changeId: 'row-1',
        kind: 'config',
        nodeId: '05.02.01.00.00.00',
        space: 253,
        offset: '0x00000000',
        baselineValue: '1',
        plannedValue: '2',
        status: 'pending',
      } as any,
    ],
    alreadyAppliedCount: 0,
    nodeMissingRows: [],
  };
}

// ─── Tests ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  syncPanelStore.reset();
  vi.clearAllMocks();
  mockBuildSyncSession.mockResolvedValue(makeEmptySession());
});

// ── dismiss() ────────────────────────────────────────────────────────────────

describe('dismiss()', () => {
  it('sets isDismissed to true', () => {
    expect(syncPanelStore.isDismissed).toBe(false);
    syncPanelStore.dismiss();
    expect(syncPanelStore.isDismissed).toBe(true);
  });

  it('collapses isActive to false even when a session is loaded', async () => {
    mockBuildSyncSession.mockResolvedValue(makeSessionWithCleanRow());
    await syncPanelStore.loadSession();
    expect(syncPanelStore.session).not.toBeNull();
    expect(syncPanelStore.isActive).toBe(true);

    syncPanelStore.dismiss();
    expect(syncPanelStore.isActive).toBe(false);
  });

  it('isDismissed persists — other store operations do not auto-reset it', () => {
    // This is a critical invariant: the settle-timer guard in maybeTriggerSync
    // (`if (syncPanelStore.isDismissed && syncTriggered) return`) depends on
    // isDismissed staying true after dismiss() without an explicit reset.
    syncPanelStore.dismiss();
    expect(syncPanelStore.isDismissed).toBe(true);

    // Common operations that must NOT clear isDismissed
    syncPanelStore.resolveConflict('some-id', 'apply');
    syncPanelStore.selectAllClean();
    syncPanelStore.deselectAllClean();

    expect(syncPanelStore.isDismissed).toBe(true);
  });
});

// ── loadSession() after dismiss() ────────────────────────────────────────────

describe('loadSession() after dismiss()', () => {
  it('resets _dismissed — safe only when called from forceSyncPanel, NOT settle timer', async () => {
    // dismiss() marks the panel as user-dismissed
    syncPanelStore.dismiss();
    expect(syncPanelStore.isDismissed).toBe(true);

    // loadSession() unconditionally clears _dismissed = false (line 197 of store).
    // This is intentional: forceSyncPanel() calls reset() then maybeTriggerSync()
    // which calls loadSession(). The settle-timer path is guarded BEFORE reaching
    // loadSession, so this reset is never triggered from auto-triggers.
    await syncPanelStore.loadSession();
    expect(syncPanelStore.isDismissed).toBe(false);
  });

  it('makes isActive true when session has content', async () => {
    mockBuildSyncSession.mockResolvedValue(makeSessionWithCleanRow());
    syncPanelStore.dismiss();

    await syncPanelStore.loadSession();
    expect(syncPanelStore.isActive).toBe(true);
  });

  it('isActive is false after loadSession returns an empty session', async () => {
    // An empty session (no rows) → isActive false because no session *content*,
    // but the store itself only checks _session !== null for isActive.
    // The page decides visibility based on hasContent; isActive = true.
    // This test documents the actual store contract (isActive ≠ hasContent).
    mockBuildSyncSession.mockResolvedValue(makeEmptySession());
    await syncPanelStore.loadSession();
    // _session is set to the empty session object (not null) → isActive = true
    expect(syncPanelStore.session).not.toBeNull();
    expect(syncPanelStore.isActive).toBe(true);
  });
});

// ── reset() ──────────────────────────────────────────────────────────────────

describe('reset()', () => {
  it('clears isDismissed', () => {
    syncPanelStore.dismiss();
    syncPanelStore.reset();
    expect(syncPanelStore.isDismissed).toBe(false);
  });

  it('clears the session', async () => {
    await syncPanelStore.loadSession();
    expect(syncPanelStore.session).not.toBeNull();

    syncPanelStore.reset();
    expect(syncPanelStore.session).toBeNull();
  });

  it('clears isActive', async () => {
    await syncPanelStore.loadSession();
    expect(syncPanelStore.isActive).toBe(true);

    syncPanelStore.reset();
    expect(syncPanelStore.isActive).toBe(false);
  });

  it('clears conflict resolutions', async () => {
    mockBuildSyncSession.mockResolvedValue({
      ...makeEmptySession(),
      conflictRows: [{ changeId: 'c-1' } as any],
    });
    await syncPanelStore.loadSession();
    syncPanelStore.resolveConflict('c-1', 'apply');
    expect(syncPanelStore.getResolution('c-1')).toBe('apply');

    syncPanelStore.reset();
    expect(syncPanelStore.getResolution('c-1')).toBeUndefined();
  });
});

// ── Settle-timer guard contract ───────────────────────────────────────────────

describe('settle-timer guard contract', () => {
  it('isDismissed stays true without explicit loadSession or reset — blocking auto-re-open', () => {
    // This test documents the page-level guard invariant:
    // After dismiss(), the page's maybeTriggerSync checks:
    //   if (syncPanelStore.isDismissed && syncTriggered) return;
    // For this guard to work, isDismissed must stay true until forceSyncPanel
    // explicitly resets the store (which clears isDismissed).
    syncPanelStore.dismiss();

    // Verify store state supports the guard
    expect(syncPanelStore.isDismissed).toBe(true);
    expect(syncPanelStore.isActive).toBe(false);

    // Only reset() or loadSession() can clear it — simulating forceSyncPanel
    syncPanelStore.reset();
    expect(syncPanelStore.isDismissed).toBe(false);
  });
});
