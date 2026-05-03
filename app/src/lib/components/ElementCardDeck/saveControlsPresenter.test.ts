import { describe, expect, it } from 'vitest';
import type { OfflineChangeRow } from '$lib/api/sync';
import type { LeafConfigNode, NodeConfigTree, SegmentNode } from '$lib/types/nodeTree';
import { deriveSaveControlsViewState } from './saveControlsPresenter';

function makeDirtyTree(nodeId: string, count = 1): NodeConfigTree {
  const leaves: LeafConfigNode[] = Array.from({ length: count }, (_, index) => ({
    kind: 'leaf',
    name: `Field ${index}`,
    description: null,
    elementType: 'int',
    address: index,
    size: 1,
    space: 253,
    path: ['seg:0', `elem:${index}`],
    value: { type: 'int', value: 0 },
    eventRole: null,
    constraints: null,
    modifiedValue: { type: 'int', value: 99 },
  }));

  const segment: SegmentNode = {
    name: 'Configuration',
    description: null,
    origin: 0,
    space: 253,
    children: leaves,
  };

  return { nodeId, identity: null, segments: [segment] };
}

function makeState(overrides: Partial<Parameters<typeof deriveSaveControlsViewState>[0]> = {}) {
  return deriveSaveControlsViewState({
    bowtieMetadataEditCount: 0,
    bowtieMetadataIsDirty: false,
    layoutIsDirty: false,
    layoutIsOfflineMode: false,
    offlineDraftCount: 0,
    offlineDraftRows: [],
    saveProgressState: 'idle',
    trees: new Map<string, NodeConfigTree>(),
    ...overrides,
  });
}

describe('deriveSaveControlsViewState', () => {
  it('derives online pending counts from dirty config leaves and metadata edits', () => {
    const state = makeState({
      bowtieMetadataEditCount: 2,
      bowtieMetadataIsDirty: true,
      trees: new Map([
        ['node-1', makeDirtyTree('node-1', 2)],
        ['node-2', makeDirtyTree('node-2', 1)],
      ]),
    });

    expect(state.dirtyCount).toBe(3);
    expect(state.dirtyNodeCount).toBe(2);
    expect(state.pendingEditCount).toBe(5);
    expect(state.pendingHintText).toBe('5 unsaved changes');
    expect(state.discardFieldCount).toBe(5);
    expect(state.discardNodeCount).toBe(2);
    expect(state.canSave).toBe(true);
  });

  it('derives offline pending counts from draft rows and keeps isDirty-only sessions at one edit', () => {
    const pendingRows: OfflineChangeRow[] = [
      {
        changeId: 'draft-1',
        kind: 'config',
        nodeId: 'node-1',
        baselineValue: '1',
        plannedValue: '2',
        status: 'pending',
      },
      {
        changeId: 'draft-2',
        kind: 'config',
        nodeId: 'node-2',
        baselineValue: '3',
        plannedValue: '4',
        status: 'pending',
      },
    ];

    const dirtyOnlyState = makeState({
      layoutIsDirty: true,
      layoutIsOfflineMode: true,
    });

    expect(dirtyOnlyState.pendingEditCount).toBe(1);
    expect(dirtyOnlyState.pendingHintText).toBe('1 unsaved edit');
    expect(dirtyOnlyState.discardFieldCount).toBe(1);
    expect(dirtyOnlyState.discardNodeCount).toBe(1);

    const draftState = makeState({
      layoutIsDirty: true,
      layoutIsOfflineMode: true,
      offlineDraftCount: 2,
      offlineDraftRows: pendingRows,
    });

    expect(draftState.pendingEditCount).toBe(2);
    expect(draftState.pendingHintText).toBe('2 unsaved edits');
    expect(draftState.discardFieldCount).toBe(2);
    expect(draftState.discardNodeCount).toBe(2);
    expect(draftState.offlineDirtyNodeCount).toBe(2);
  });

  it('counts layout-only dirty state in online pending and discard totals', () => {
    const state = makeState({
      layoutIsDirty: true,
      layoutIsOfflineMode: false,
    });

    expect(state.pendingEditCount).toBe(1);
    expect(state.pendingHintText).toBe('1 unsaved change');
    expect(state.discardFieldCount).toBe(1);
    expect(state.discardNodeCount).toBe(1);
  });

  it('disables saving while a save is already in progress', () => {
    const state = makeState({
      saveProgressState: 'saving',
      trees: new Map([['node-1', makeDirtyTree('node-1', 1)]]),
    });

    expect(state.isSaving).toBe(true);
    expect(state.canSave).toBe(false);
    expect(state.hasEdits).toBe(true);
  });
});