import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import { hasUnsavedPromptChanges as deriveUnsavedPromptChanges } from '$lib/orchestration/unsavedChangesGuard';
import { deriveSaveControlsViewState } from '$lib/components/ElementCardDeck/saveControlsPresenter';
import type { SaveControlsViewState } from '$lib/components/ElementCardDeck/saveControlsPresenter';
import type { SaveProgress } from '$lib/types/nodeTree';

export interface ChangeTrackerSnapshot {
  saveControls: SaveControlsViewState;
  hasUnsavedChanges: boolean;
}

class ChangeTrackerStore {
  deriveSnapshot(saveProgressState: SaveProgress['state']): ChangeTrackerSnapshot {
    const saveControls = deriveSaveControlsViewState({
      bowtieMetadataEditCount: bowtieMetadataStore.editCount,
      bowtieMetadataIsDirty: bowtieMetadataStore.isDirty,
      configDraftCount: configChangesStore.draftEntries().length,
      connectorWarningCount: connectorSelectionsStore.totalWarningCount,
      layoutIsDirty: layoutStore.isDirty,
      layoutIsOfflineMode: layoutStore.isOfflineMode,
      offlineDraftCount: offlineChangesStore.draftCount,
      offlineDraftRows: offlineChangesStore.draftRows,
      revertedPersistedCount: offlineChangesStore.revertedPersistedCount,
      saveProgressState,
      treeNodeIds: [...nodeTreeStore.trees.keys()],
    });

    return {
      saveControls,
      hasUnsavedChanges: deriveUnsavedPromptChanges(
        nodeTreeStore.trees.keys(),
        bowtieMetadataStore.isDirty,
        offlineChangesStore.draftCount,
        layoutStore.isDirty,
        offlineChangesStore.revertedPersistedCount,
      ),
    };
  }

  deriveSaveControlsState(saveProgressState: SaveProgress['state']): SaveControlsViewState {
    return this.deriveSnapshot(saveProgressState).saveControls;
  }

  hasUnsavedChanges(): boolean {
    return this.deriveSnapshot('idle').hasUnsavedChanges;
  }
}

export const changeTrackerStore = new ChangeTrackerStore();
