import { describe, expect, it } from 'vitest';
import type { OfflineChangeRow } from '$lib/api/sync';
import type { DirtyBreakdown } from '$lib/layout';
import { deriveSaveControlsViewState } from './saveControlsPresenter';

function emptyBreakdown(): DirtyBreakdown {
  return {
    config: 0,
    configNodes: 0,
    metadata: 0,
    channels: 0,
    facilities: 0,
    connectorSelections: 0,
    offlineDrafts: 0,
    offlineRevertedPersisted: 0,
    layoutStruct: 0,
    unsavedNewNodes: 0,
    unsavedRemovedNodes: 0,
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

describe('deriveSaveControlsViewState — config + metadata edits', () => {
  it('derives online pending counts from config drafts and metadata edits', () => {
    const state = makeState({
      config: 3,
      configNodes: 2,
      metadata: 2,
    });

    expect(state.dirtyCount).toBe(3);
    expect(state.dirtyNodeCount).toBe(2);
    expect(state.pendingEditCount).toBe(5);
    expect(state.pendingHintText).toBe('5 unsaved changes');
    expect(state.discardFieldCount).toBe(5);
    expect(state.discardNodeCount).toBe(2);
    expect(state.canSave).toBe(true);
    expect(state.hasMetadataEdits).toBe(true);
  });
});

describe('deriveSaveControlsViewState — offline mode', () => {
  it('keeps layout-only dirty sessions at one edit (hint reads "edit" not "change")', () => {
    const state = makeState(
      { layoutStruct: 1 },
      { layoutIsOfflineMode: true },
    );

    expect(state.pendingEditCount).toBe(1);
    expect(state.pendingHintText).toBe('1 unsaved edit');
    expect(state.discardFieldCount).toBe(1);
    expect(state.discardNodeCount).toBe(1);
  });

  it('counts config drafts in offline mode pending and discard totals', () => {
    const state = makeState(
      { config: 1, configNodes: 1, layoutStruct: 1 },
      { layoutIsOfflineMode: true },
    );

    expect(state.pendingEditCount).toBe(2);
    expect(state.pendingHintText).toBe('2 unsaved edits');
    expect(state.discardFieldCount).toBe(2);
    expect(state.hasEdits).toBe(true);
    expect(state.canSave).toBe(true);
  });

  it('counts reverted persisted rows as unsaved edits', () => {
    const state = makeState(
      { offlineRevertedPersisted: 2 },
      { layoutIsOfflineMode: true },
    );

    expect(state.pendingEditCount).toBe(2);
    expect(state.pendingHintText).toBe('2 unsaved edits');
    expect(state.hasEdits).toBe(true);
    expect(state.discardFieldCount).toBe(2);
  });
});

describe('deriveSaveControlsViewState — online mode', () => {
  it('counts layout-only dirty state in online pending and discard totals', () => {
    const state = makeState({ layoutStruct: 1 });

    expect(state.pendingEditCount).toBe(1);
    expect(state.pendingHintText).toBe('1 unsaved change');
    expect(state.discardFieldCount).toBe(1);
    expect(state.discardNodeCount).toBe(1);
  });

  it('disables saving while a save is already in progress', () => {
    const state = makeState(
      { config: 1, configNodes: 1 },
      { saveProgressState: 'saving' },
    );

    expect(state.isSaving).toBe(true);
    expect(state.canSave).toBe(false);
    expect(state.hasEdits).toBe(true);
  });
});

describe('deriveSaveControlsViewState — facilities and channels (S1.2)', () => {
  it('treats a facility-only edit as a savable change with the right hint', () => {
    const state = makeState({ facilities: 1 });

    expect(state.pendingEditCount).toBe(1);
    expect(state.pendingHintText).toBe('1 unsaved change');
    expect(state.hasEdits).toBe(true);
    expect(state.canSave).toBe(true);
    expect(state.discardFieldCount).toBe(1);
  });

  it('treats a channel-only edit as a savable change', () => {
    const state = makeState({ channels: 2 });

    expect(state.pendingEditCount).toBe(2);
    expect(state.pendingHintText).toBe('2 unsaved changes');
    expect(state.hasEdits).toBe(true);
    expect(state.canSave).toBe(true);
    expect(state.discardFieldCount).toBe(2);
  });

  it('sums facilities, channels, connector selections, config, and metadata into the totals', () => {
    const state = makeState({
      facilities: 1,
      channels: 2,
      connectorSelections: 1,
      config: 3,
      configNodes: 2,
      metadata: 1,
    });

    expect(state.pendingEditCount).toBe(1 + 2 + 1 + 3 + 1);
    expect(state.discardFieldCount).toBe(1 + 2 + 1 + 3 + 1);
    expect(state.discardNodeCount).toBe(2);
  });
});

describe('deriveSaveControlsViewState — offlineDirtyNodeCount', () => {
  it('counts distinct nodeIds in pending offline rows', () => {
    const rows: OfflineChangeRow[] = [
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
    expect(state.pendingHintText).toBe('0 unsaved changes');
  });
});
