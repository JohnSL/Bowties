import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';

const {
  syncRef,
  reconcileRef,
} = vi.hoisted(() => ({
  syncRef: {
    matchStatus: null as any,
    syncMode: null as any,
    isLoading: false,
    error: null as string | null,
    isApplying: false,
    canApply: true,
    applyCount: 1,
    applyResult: null as any,
    conflictRows: [] as any[],
    cleanRows: [{
      changeId: 'row-1',
      nodeId: '05.02.01.02.03.00',
      kind: 'config',
      space: 253,
      offset: '0x00000010',
      baselineValue: '10',
      plannedValue: '20',
      status: 'pending',
    }],
    alreadyAppliedCount: 0,
    nodeMissingRows: [] as any[],
    session: {
      conflictRows: [] as any[],
      cleanRows: [] as any[],
      alreadyAppliedCount: 0,
      nodeMissingRows: [] as any[],
    } as any,
    setMode: vi.fn(async () => {}),
    loadSession: vi.fn(async () => {}),
    dismiss: vi.fn(),
    applySelected: vi.fn(async () => ({
      applied: ['row-1'],
      readOnlyCleared: [],
      failed: [],
    })),
  },
  reconcileRef: vi.fn(async () => {}),
}));

vi.mock('$lib/stores/syncPanel.svelte', () => ({
  syncPanelStore: syncRef,
}));

vi.mock('$lib/orchestration/syncApplyOrchestrator', () => ({
  reconcileOfflineTreesAfterSyncApply: reconcileRef,
}));

import SyncPanel from './SyncPanel.svelte';

beforeEach(() => {
  vi.clearAllMocks();
  syncRef.session = {
    conflictRows: [],
    cleanRows: [...syncRef.cleanRows],
    alreadyAppliedCount: 0,
    nodeMissingRows: [],
  };
  syncRef.applyResult = null;
});

describe('SyncPanel lifecycle orchestration', () => {
  it('delegates post-apply rebuild sequencing to the sync orchestrator', async () => {
    render(SyncPanel, { visible: true });

    const applyButton = await waitFor(() => screen.getByRole('button', { name: /apply \(1\)/i }));
    await fireEvent.click(applyButton);

    await waitFor(() => {
      expect(reconcileRef).toHaveBeenCalledWith(
        {
          applied: ['row-1'],
          readOnlyCleared: [],
          failed: [],
        },
        syncRef.session,
      );
    });
  });

  it('dismisses immediately for bench mode without loading a session', async () => {
    syncRef.matchStatus = { classification: 'uncertain', overlapPercent: 42 };
    syncRef.syncMode = null;

    render(SyncPanel, { visible: true });

    const benchButton = await screen.findByRole('button', { name: /bench \/ other bus/i });
    await fireEvent.click(benchButton);

    await waitFor(() => {
      expect(syncRef.setMode).toHaveBeenCalledWith('bench_other_bus');
      expect(syncRef.loadSession).not.toHaveBeenCalled();
      expect(syncRef.dismiss).toHaveBeenCalledTimes(1);
    });
  });
});
