import { clearLayoutDrafts } from '$lib/api/layout';
import { layoutEditParticipants } from '$lib/orchestration/layoutEditParticipants';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';

/** Discard unsaved layout edits in their historical order. */
export async function discardLayoutEdits(): Promise<void> {
  await pendingChangesStore.revertAllPending();
  layoutEditParticipants.config.discard();
  layoutEditParticipants.metadata.discard();
  layoutEditParticipants.connectors.discard();
  layoutEditParticipants.channels.discard();
  layoutEditParticipants.facilities.discard();
  layoutEditParticipants.layout.discard();
  connectorSelectionsStore.hydrateFromLayout(layoutStore.layout);
  await clearLayoutDrafts();
}
