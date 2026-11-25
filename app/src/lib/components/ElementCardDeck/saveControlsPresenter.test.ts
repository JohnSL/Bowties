import { describe, expect, it } from 'vitest';
import type { PendingChangeRow } from '$lib/api/sync';
import type { DirtyBreakdown } from '$lib/layout';
import { deriveSaveControlsViewState } from './saveControlsPresenter';

function emptyBreakdown(): DirtyBreakdown {
  return {
    config: 0,
    configNodes: 0,
    configChanges: 0,
    metadata: 0,
    channels: 0,
    facilities: 0,
    connectorSelections: 0,
    pendingDrafts: 0,
    revertedPendingChanges: 0,
    layoutStruct: 0,
    unsavedNewNodes: 0,
    unsavedRemovedNodes: 0,
    waitingPendingChanges: 0,
  };
}

function makeState(
  breakdown: Partial<DirtyBreakdown> = {},
  overrides: Partial<Parameters<typeof deriveSaveControlsViewState>[0]> = {},
) {
  return deriveSaveControlsViewState({
    breakdown: { ...emptyBreakdown(), ...breakdown },
    connectorWarningCount: 0,
    layoutIsOfflineMode: false,
    offlineDraftRows: [],
    saveProgressState: 'idle',
    ...overrides,
  });
}

describe('deriveSaveControlsViewState — one wording, offline or connected', () => {
  it.each([false, true])('configuration only reads "N configuration changes" (offline=%s)', (offline) => {
    const state = makeState(
      { config: 3, configNodes: 2, configChanges: 3 },
      { layoutIsOfflineMode: offline },
    );

    expect(state.pendingHintText).toBe('3 configuration changes');
    expect(state.pendingEditCount).toBe(3);
    expect(state.discardFieldCount).toBe(3);
    expect(state.discardNodeCount).toBe(2);
    expect(state.canSave).toBe(true);
  });

  it('uses the singular for one configuration change', () => {
    const state = makeState({ config: 1, configNodes: 1, configChanges: 1 });
    expect(state.pendingHintText).toBe('1 configuration change');
  });

  it.each([false, true])('configuration plus layout edits adds "+ other edits" (offline=%s)', (offline) => {
    const state = makeState(
      { config: 2, configNodes: 1, configChanges: 2, metadata: 2, facilities: 1 },
      { layoutIsOfflineMode: offline },
    );

    expect(state.pendingHintText).toBe('2 configuration changes + other edits');
    expect(state.pendingEditCount).toBe(5);
    expect(state.hasMetadataEdits).toBe(true);
  });

  it.each([
    ['bowtie names', { metadata: 1 }],
    ['facilities', { facilities: 1 }],
    ['channels', { channels: 2 }],
    ['connector selections', { connectorSelections: 1 }],
    ['layout structure', { layoutStruct: 1 }],
    ['new nodes', { unsavedNewNodes: 1 }],
    ['removed nodes', { unsavedRemovedNodes: 1 }],
  ])('%s alone reads "Unsaved changes" and can be saved', (_name, breakdown) => {
    const state = makeState(breakdown as Partial<DirtyBreakdown>);

    expect(state.pendingHintText).toBe('Unsaved changes');
    expect(state.hasEdits).toBe(true);
    expect(state.canSave).toBe(true);
  });

  it('counts a reverted saved row and a configuration draft once each through configChanges', () => {
    const state = makeState(
      { config: 1, configNodes: 1, configChanges: 2, revertedPendingChanges: 1 },
      { layoutIsOfflineMode: true },
    );

    expect(state.pendingHintText).toBe('2 configuration changes');
    expect(state.discardFieldCount).toBe(2);
  });

  it('counts a Pending Change draft row that has no config draft (connector repair)', () => {
    const state = makeState({ pendingDrafts: 1, configChanges: 1 });

    expect(state.hasEdits).toBe(true);
    expect(state.pendingHintText).toBe('1 configuration change');
  });

  it('disables saving while a save is already in progress', () => {
    const state = makeState(
      { config: 1, configNodes: 1, configChanges: 1 },
      { saveProgressState: 'saving' },
    );

    expect(state.isSaving).toBe(true);
    expect(state.canSave).toBe(false);
    expect(state.hasEdits).toBe(true);
  });

  it('sums every edit family into the totals', () => {
    const state = makeState({
      facilities: 1,
      channels: 2,
      connectorSelections: 1,
      config: 3,
      configNodes: 2,
      configChanges: 3,
      metadata: 1,
    });

    expect(state.pendingEditCount).toBe(1 + 2 + 1 + 3 + 1);
    expect(state.discardFieldCount).toBe(1 + 2 + 1 + 3 + 1);
    expect(state.discardNodeCount).toBe(2);
  });
});

describe('deriveSaveControlsViewState — rows waiting for the board', () => {
  it('shows waiting rows on their own line without making them unsaved', () => {
    const state = makeState({ waitingPendingChanges: 3 });

    expect(state.waitingHintText).toBe('3 waiting to be written to the board');
    expect(state.hasEdits).toBe(false);
    expect(state.canSave).toBe(false);
    expect(state.pendingHintText).toBe('');
    expect(state.hasWaitingChanges).toBe(true);
  });

  it('keeps unsaved wording and waiting wording separate', () => {
    const state = makeState({ config: 1, configNodes: 1, configChanges: 1, waitingPendingChanges: 1 });

    expect(state.pendingHintText).toBe('1 configuration change');
    expect(state.waitingHintText).toBe('1 waiting to be written to the board');
  });

  it('offers Review in Sync Panel only while connected with rows waiting', () => {
    expect(makeState({ waitingPendingChanges: 1 }, { layoutIsConnected: true }).canReviewWaiting).toBe(true);
    expect(makeState({ waitingPendingChanges: 1 }, { layoutIsConnected: false }).canReviewWaiting).toBe(false);
    expect(makeState({}, { layoutIsConnected: true }).canReviewWaiting).toBe(false);
  });

  it('has no waiting text when nothing is waiting', () => {
    const state = makeState();
    expect(state.waitingHintText).toBe('');
    expect(state.hasWaitingChanges).toBe(false);
  });
});

describe('deriveSaveControlsViewState — offlineDirtyNodeCount', () => {
  it('counts distinct nodeIds in pending offline rows', () => {
    const rows: PendingChangeRow[] = [
      {
        changeId: 'd1',
        kind: 'config',
        nodeId: 'n1',
        baselineValue: '1',
        plannedValue: '2',
        status: 'pending',
      },
      {
        changeId: 'd2',
        kind: 'config',
        nodeId: 'n1',
        baselineValue: '3',
        plannedValue: '4',
        status: 'pending',
      },
      {
        changeId: 'd3',
        kind: 'config',
        nodeId: 'n2',
        baselineValue: '5',
        plannedValue: '6',
        status: 'pending',
      },
    ];

    const state = makeState({}, { offlineDraftRows: rows });
    expect(state.offlineDirtyNodeCount).toBe(2);
  });
});

describe('deriveSaveControlsViewState — empty baseline', () => {
  it('reports no edits and disables save', () => {
    const state = makeState();
    expect(state.hasEdits).toBe(false);
    expect(state.canSave).toBe(false);
    expect(state.pendingEditCount).toBe(0);
    expect(state.pendingHintText).toBe('');
  });
});
