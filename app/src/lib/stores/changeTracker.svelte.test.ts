import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NodeConfigTree } from '$lib/types/nodeTree';

const {
  treesRef,
  bowtieRef,
  layoutRef,
  offlineRef,
  presenterRef,
  unsavedRef,
} = vi.hoisted(() => ({
  treesRef: { map: new Map<string, NodeConfigTree>() },
  bowtieRef: { editCount: 0, isDirty: false },
  layoutRef: { isDirty: false, isOfflineMode: false },
  offlineRef: { draftCount: 0, draftRows: [] as any[] },
  presenterRef: {
    deriveSaveControlsViewState: vi.fn().mockReturnValue({
      hasEdits: false,
      pendingEditCount: 0,
      pendingHintText: '0 unsaved changes',
      canSave: false,
      dirtyCount: 0,
      dirtyNodeCount: 0,
      discardFieldCount: 0,
      discardNodeCount: 0,
      hasConfigEdits: false,
      hasMetadataEdits: false,
      hasOfflineEdits: false,
      isSaving: false,
      offlineDirtyNodeCount: 0,
    }),
  },
  unsavedRef: {
    hasUnsavedPromptChanges: vi.fn().mockReturnValue(false),
  },
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() {
      return treesRef.map;
    },
  },
}));

vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: bowtieRef,
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: layoutRef,
}));

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: offlineRef,
}));

vi.mock('$lib/components/ElementCardDeck/saveControlsPresenter', () => ({
  deriveSaveControlsViewState: presenterRef.deriveSaveControlsViewState,
}));

vi.mock('$lib/orchestration/unsavedChangesGuard', () => ({
  hasUnsavedPromptChanges: unsavedRef.hasUnsavedPromptChanges,
}));

import { changeTrackerStore } from './changeTracker.svelte';

describe('changeTrackerStore', () => {
  beforeEach(() => {
    treesRef.map = new Map();
    bowtieRef.editCount = 0;
    bowtieRef.isDirty = false;
    layoutRef.isDirty = false;
    layoutRef.isOfflineMode = false;
    offlineRef.draftCount = 0;
    offlineRef.draftRows = [];
    presenterRef.deriveSaveControlsViewState.mockClear();
    unsavedRef.hasUnsavedPromptChanges.mockClear();
  });

  it('derives snapshot saveControls state from current aggregate inputs', () => {
    const tree = { nodeId: 'node-1', identity: null, segments: [] } as unknown as NodeConfigTree;
    treesRef.map.set('node-1', tree);
    bowtieRef.editCount = 2;
    bowtieRef.isDirty = true;
    layoutRef.isDirty = true;
    layoutRef.isOfflineMode = true;
    offlineRef.draftCount = 3;
    offlineRef.draftRows = [{ changeId: '1', status: 'pending', nodeId: 'node-1' }];

    const snapshot = changeTrackerStore.deriveSnapshot('idle');

    expect(snapshot.saveControls).toEqual({
      hasEdits: false,
      pendingEditCount: 0,
      pendingHintText: '0 unsaved changes',
      canSave: false,
      dirtyCount: 0,
      dirtyNodeCount: 0,
      discardFieldCount: 0,
      discardNodeCount: 0,
      hasConfigEdits: false,
      hasMetadataEdits: false,
      hasOfflineEdits: false,
      isSaving: false,
      offlineDirtyNodeCount: 0,
    });

    expect(presenterRef.deriveSaveControlsViewState).toHaveBeenCalledWith({
      bowtieMetadataEditCount: 2,
      bowtieMetadataIsDirty: true,
      layoutIsDirty: true,
      layoutIsOfflineMode: true,
      offlineDraftCount: 3,
      offlineDraftRows: [{ changeId: '1', status: 'pending', nodeId: 'node-1' }],
      saveProgressState: 'idle',
      trees: treesRef.map,
    });
  });

  it('derives snapshot unsaved prompt signal from the same aggregate sources', () => {
    const tree = { nodeId: 'node-1', identity: null, segments: [] } as unknown as NodeConfigTree;
    treesRef.map.set('node-1', tree);
    bowtieRef.isDirty = true;
    offlineRef.draftCount = 1;
    layoutRef.isDirty = true;
    unsavedRef.hasUnsavedPromptChanges.mockReturnValueOnce(true);

    const snapshot = changeTrackerStore.deriveSnapshot('idle');

    expect(snapshot.hasUnsavedChanges).toBe(true);
    expect(unsavedRef.hasUnsavedPromptChanges).toHaveBeenCalledWith(
      treesRef.map.values(),
      true,
      1,
      true,
    );
  });

  it('keeps compatibility helpers aligned with snapshot fields', () => {
    unsavedRef.hasUnsavedPromptChanges.mockReturnValueOnce(true).mockReturnValueOnce(true);

    const saveControls = changeTrackerStore.deriveSaveControlsState('idle');
    const hasUnsavedChanges = changeTrackerStore.hasUnsavedChanges();

    expect(saveControls).toEqual(changeTrackerStore.deriveSnapshot('idle').saveControls);
    expect(hasUnsavedChanges).toBe(true);
  });
});
