import {
  previewBlockSignals,
  type EntranceEnd,
  type ExitEnd,
  type FacilitySignalSettings,
  type Movements,
  type SelectedDetectedBlock,
  type SignalSettings,
  type TravelDirection,
} from '$lib/api/signaling';
import type { TurnoutShape } from '$lib/components/BlockSignals/trackGeometry';
import {
  blockSignalsEditorStore as store,
  type BlockSignalsStep,
} from '$lib/stores/blockSignalsEditor.svelte';

/**
 * Sequences the transient Automatic Block Signals editor: each edit updates
 * the store, then asks the backend for a fresh preview. Only the newest
 * response is accepted; cancel and layout reset discard any in flight.
 */
class BlockSignalsEditorOrchestrator {
  private requestGeneration = 0;

  start(initialName: string): void {
    this.requestGeneration += 1;
    store.start(initialName);
  }

  navigate(step: BlockSignalsStep): void {
    store.navigate(step);
  }

  cancel(): void {
    this.requestGeneration += 1;
    store.reset();
  }

  resetForNewLayout(): void {
    this.cancel();
  }

  /** Drawing-only change: no new preview is needed. */
  setTurnoutShape(turnoutId: string, shape: TurnoutShape): void {
    store.setTurnoutShape(turnoutId, shape);
  }

  /** Apply an edit to the request, then refresh the preview. */
  async edit(change: () => void): Promise<void> {
    change();
    await this.refreshPreview();
  }

  private async refreshPreview(): Promise<void> {
    const requestGeneration = ++this.requestGeneration;
    if (store.blocks.length === 0) {
      store.clearPreview();
      return;
    }

    try {
      const preview = await previewBlockSignals(store.previewRequest());
      if (requestGeneration === this.requestGeneration) {
        store.acceptPreview(preview);
      }
    } catch (error) {
      if (requestGeneration === this.requestGeneration) {
        store.setError(error instanceof Error ? error.message : String(error));
      }
    }
  }
}

export const blockSignalsEditorOrchestrator = new BlockSignalsEditorOrchestrator();
const o = blockSignalsEditorOrchestrator;

export const startBlockSignalsEditor = (initialName: string): void => o.start(initialName);
export const navigateBlockSignalsEditor = (step: BlockSignalsStep): void => o.navigate(step);
export const cancelBlockSignalsEditor = (): void => o.cancel();
export const resetBlockSignalsEditorForNewLayout = (): void => o.resetForNewLayout();
export const setTurnoutShape = (turnoutId: string, shape: TurnoutShape): void => o.setTurnoutShape(turnoutId, shape);

export const addSignaledBlock = (block: SelectedDetectedBlock) => o.edit(() => store.addBlock(block));
export const removeSignaledBlock = (blockId: string) => o.edit(() => store.removeBlock(blockId));
export const moveSignaledBlock = (blockId: string, delta: -1 | 1) => o.edit(() => store.moveBlock(blockId, delta));
export const setJoinedToPrevious = (blockId: string, joined: boolean) =>
  o.edit(() => store.setJoinedToPrevious(blockId, joined));
/** Resolves to the new turnout's ID once the preview has refreshed. */
export async function addTurnout(blockId: string, afterTurnoutId?: string): Promise<string> {
  let turnoutId = '';
  await o.edit(() => { turnoutId = store.addTurnout(blockId, afterTurnoutId); });
  return turnoutId;
}
export const renameTurnout = (turnoutId: string, name: string) => o.edit(() => store.renameTurnout(turnoutId, name));
export const removeTurnout = (turnoutId: string) => o.edit(() => store.removeTurnout(turnoutId));
export const moveTurnout = (turnoutId: string, delta: -1 | 1) => o.edit(() => store.moveTurnout(turnoutId, delta));
export const setFacilityName = (name: string) => o.edit(() => store.setFacilityName(name));
export const setBlockSignalsDirection = (direction: TravelDirection) => o.edit(() => store.setDirection(direction));
export const setBlockSignalsMovements = (movements: Movements) => o.edit(() => store.setMovements(movements));
export const setEntranceEnd = (end: EntranceEnd) => o.edit(() => store.setEntranceEnd(end));
export const setExitEnd = (end: ExitEnd) => o.edit(() => store.setExitEnd(end));
export const setFacilitySignalSettings = (change: Partial<FacilitySignalSettings>) =>
  o.edit(() => store.setSettings(change));
export const setSignalOverride = (signalId: string, settings: SignalSettings | null) =>
  o.edit(() => store.setSignalOverride(signalId, settings));
export const setSignalName = (signalId: string, name: string) => o.edit(() => store.setSignalName(signalId, name));

/** The intents the editor components emit. */
export const blockSignalsEditorActions = {
  navigate: navigateBlockSignalsEditor,
  cancel: cancelBlockSignalsEditor,
  setFacilityName,
  addBlock: addSignaledBlock,
  removeBlock: removeSignaledBlock,
  moveBlock: moveSignaledBlock,
  setJoinedToPrevious,
  addTurnout,
  renameTurnout,
  removeTurnout,
  moveTurnout,
  setTurnoutShape,
  setDirection: setBlockSignalsDirection,
  setMovements: setBlockSignalsMovements,
  setEntranceEnd,
  setExitEnd,
  setSettings: setFacilitySignalSettings,
  setSignalOverride,
  setSignalName,
};
export type BlockSignalsEditorActions = typeof blockSignalsEditorActions;
