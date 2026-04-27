import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';

const { syncRef, reconcileRef } = vi.hoisted(() => ({
  syncRef: {
    matchStatus: { classification: 'likely_same', overlapPercent: 100 },
    syncMode: 'target_layout_bus' as const,
    isLoading: false,
    error: null as string | null,
    isApplying: false,
    canApply: true,
    applyCount: 1,
    applyResult: null as any,
    conflictRows: [] as any[],
    cleanRows: [] as any[],
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
    applySelected: vi.fn(async () => null),
    getResolution: vi.fn(() => undefined),
    resolveConflict: vi.fn(),
    selectedCleanCount: 0,
    isCleanRowDeselected: vi.fn(() => false),
    toggleCleanRow: vi.fn(),
    selectAllClean: vi.fn(),
    deselectAllClean: vi.fn(),
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

function makeRow(overrides: Record<string, unknown> = {}) {
  return {
    changeId: 'row-1',
    nodeId: '02.01.57.00.02.D9',
    nodeName: 'West Yard',
    fieldLabel: 'Port I/O.Line(2).Event(0).Indicator',
    baselineValue: '02.01.57.00.02.D9.02.66',
    plannedValue: '02.01.57.00.02.D9.04.8A',
    busValue: '02.01.57.00.02.D9.02.66',
    resolution: 'unresolved',
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  syncRef.matchStatus = { classification: 'likely_same', overlapPercent: 100 };
  syncRef.syncMode = 'target_layout_bus';
  syncRef.isLoading = false;
  syncRef.error = null;
  syncRef.isApplying = false;
  syncRef.canApply = true;
  syncRef.applyCount = 1;
  syncRef.applyResult = null;
  syncRef.conflictRows = [];
  syncRef.cleanRows = [];
  syncRef.alreadyAppliedCount = 0;
  syncRef.nodeMissingRows = [];
  syncRef.selectedCleanCount = 0;
  syncRef.session = {
    conflictRows: [],
    cleanRows: [],
    alreadyAppliedCount: 0,
    nodeMissingRows: [],
  };
});

describe('SyncPanel display metadata', () => {
  it('renders conflict rows with field and node metadata', () => {
    const row = makeRow({ changeId: 'conflict-1', busValue: '02.01.57.00.02.D9.02.77' });
    syncRef.conflictRows = [row];
    syncRef.session = {
      conflictRows: [row],
      cleanRows: [],
      alreadyAppliedCount: 0,
      nodeMissingRows: [],
    };

    render(SyncPanel, { visible: true });

    expect(screen.getByText('Port I/O.Line(2).Event(0).Indicator')).toBeInTheDocument();
    expect(screen.getByText('West Yard')).toBeInTheDocument();
    expect(screen.getByText('02.01.57.00.02.D9')).toBeInTheDocument();
  });

  it('renders clean rows with field and node metadata when expanded', async () => {
    const row = makeRow({ changeId: 'clean-1' });
    syncRef.cleanRows = [row];
    syncRef.selectedCleanCount = 1;
    syncRef.session = {
      conflictRows: [],
      cleanRows: [row],
      alreadyAppliedCount: 0,
      nodeMissingRows: [],
    };

    render(SyncPanel, { visible: true });

    await fireEvent.click(screen.getByRole('button', { name: /clean changes/i }));

    expect(screen.getByText('Port I/O.Line(2).Event(0).Indicator')).toBeInTheDocument();
    expect(screen.getByText('West Yard')).toBeInTheDocument();
    expect(screen.getByText('02.01.57.00.02.D9')).toBeInTheDocument();
  });

  it('renders node-missing rows with field and node metadata', () => {
    const row = makeRow({ changeId: 'missing-1', busValue: undefined });
    syncRef.nodeMissingRows = [row];
    syncRef.session = {
      conflictRows: [],
      cleanRows: [],
      alreadyAppliedCount: 0,
      nodeMissingRows: [row],
    };

    render(SyncPanel, { visible: true });

    expect(screen.getByText('Port I/O.Line(2).Event(0).Indicator')).toBeInTheDocument();
    expect(screen.getByText('West Yard')).toBeInTheDocument();
    expect(screen.getByText('02.01.57.00.02.D9')).toBeInTheDocument();
    expect(screen.getByText('Not on bus')).toBeInTheDocument();
  });
});