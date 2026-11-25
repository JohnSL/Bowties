import {
  composeBlockSignals,
  previewBlockSignals,
  type EntranceEnd,
  type ExitEnd,
  type FacilitySignalSettings,
  type Movements,
  type ReleasedOwner,
  type SelectedDetectedBlock,
  type SignalSettings,
  type TravelDirection,
} from '$lib/api/signaling';
import type { TurnoutShape } from '$lib/components/BlockSignals/trackGeometry';
import {
  createBlockSignalsFacility,
  syncDraftsForComposition,
  updateBlockSignalsFacility,
} from '$lib/orchestration/facilityOrchestrator';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import {
  blockSignalsEditorStore as store,
  type BlockSignalsStep,
  type SignalWiringChoice,
} from '$lib/stores/blockSignalsEditor.svelte';

/**
 * Sequences the transient Automatic Block Signals editor: each edit updates
 * the store, then asks the backend for a fresh preview. On the wiring step a
 * complete wiring is also checked by compose (no writes). Only the newest
 * preview or check is accepted; cancel, Done and layout reset discard any in
 * flight. Done hands the editor's input to the facility workflow.
 */
class BlockSignalsEditorOrchestrator {
  private requestGeneration = 0;
  private finishing = false;

  start(initialName: string): void {
    this.requestGeneration += 1;
    store.start(initialName);
  }

  /** Open the editor on a saved facility's effective state and check it against the backend. */
  async startEdit(facilityId: string): Promise<void> {
    const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
    const record = facility?.blockSignals;
    if (!facility || !record) return;
    const wiring: Record<string, SignalWiringChoice> = {};
    for (const signal of record.signals) {
      const binding = channelsStore.channels.find((c) => c.id === signal.channelId)?.binding;
      if (binding?.kind === 'signalLccRgOutputs') {
        wiring[signal.signalId] = {
          nodeKey: binding.nodeKey,
          redOutput: binding.redOutput,
          greenOutput: binding.greenOutput,
        };
      }
    }
    this.requestGeneration += 1;
    store.startEdit({ facilityId, name: facility.name, record, wiring });
    await this.edit(() => {});
  }

  async navigate(step: BlockSignalsStep): Promise<void> {
    store.navigate(step);
    await this.refreshCheck();
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

  /** Wiring choices change no request, so no new preview is needed. */
  async wire(change: () => void): Promise<void> {
    store.invalidateCheck();
    change();
    await this.refreshCheck();
  }

  /** Create the facility from the editor's input; the editor closes only on success. */
  async finish(): Promise<void> {
    if (this.finishing || !store.canFinish || !store.preview) return;
    this.finishing = true;
    const generation = ++this.requestGeneration;
    try {
      const stage = store.mode === 'edit' ? updateBlockSignalsFacility : createBlockSignalsFacility;
      const result = await stage({
        facilityId: store.facilityId,
        name: store.facilityName,
        line: store.previewRequest(),
        wiring: store.signalWirings(),
        release: store.released,
        signals: store.preview.signals.map((signal) => ({
          signalId: signal.signalId,
          name: signal.name,
          protectedBlock: signal.protectedBlockIds[0],
        })),
      });
      if (generation !== this.requestGeneration) return;
      if (result.ok) {
        this.requestGeneration += 1;
        store.reset();
      } else if ('issues' in result) {
        store.acceptCheck(result.issues);
      } else {
        const { error } = result;
        store.setFinishError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      this.finishing = false;
    }
  }

  /** Apply an edit to the request, then refresh the preview. */
  async edit(change: () => void): Promise<void> {
    store.invalidateCheck();
    change();
    if (await this.refreshPreview()) await this.refreshCheck();
  }

  /** Compose-check complete wiring on the wiring step; otherwise clear the check. */
  private async refreshCheck(): Promise<void> {
    if (store.step !== 'wiring' || !store.wiringComplete) {
      store.clearCheck();
      return;
    }
    const requestGeneration = ++this.requestGeneration;
    const wiring = store.signalWirings();
    try {
      await syncDraftsForComposition();
      const response = await composeBlockSignals({
        facilityId: store.facilityId,
        line: store.previewRequest(),
        wiring,
        release: store.released,
      });
      if (requestGeneration === this.requestGeneration) store.acceptCheck(response.issues);
    } catch (error) {
      if (requestGeneration === this.requestGeneration) {
        store.clearCheck();
        store.setFinishError(error instanceof Error ? error.message : String(error));
      }
    }
  }

  /** Resolves true when this request is still the newest once settled. */
  private async refreshPreview(): Promise<boolean> {
    const requestGeneration = ++this.requestGeneration;
    if (store.blocks.length === 0) {
      store.clearPreview();
      return true;
    }

    try {
      await syncDraftsForComposition();
      const preview = await previewBlockSignals(store.previewRequest());
      if (requestGeneration === this.requestGeneration) {
        store.acceptPreview(preview);
      }
    } catch (error) {
      if (requestGeneration === this.requestGeneration) {
        store.setError(error instanceof Error ? error.message : String(error));
      }
    }
    return requestGeneration === this.requestGeneration;
  }
}

export const blockSignalsEditorOrchestrator = new BlockSignalsEditorOrchestrator();
const o = blockSignalsEditorOrchestrator;

export const startBlockSignalsEditor = (initialName: string): void => o.start(initialName);
export const startEditBlockSignalsEditor = (facilityId: string): Promise<void> => o.startEdit(facilityId);
export const navigateBlockSignalsEditor = (step: BlockSignalsStep): Promise<void> => o.navigate(step);
export const finishBlockSignalsEditor = (): Promise<void> => o.finish();
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
export const chooseWiringNode = (signalId: string, nodeKey: string) => o.wire(() => store.chooseWiringNode(signalId, nodeKey));
/** Seat a lamp on a pin: place it, clear it from its own pin, or swap with the other lamp. */
export const placeLamp = (signalId: string, lamp: 'red' | 'green', output: number) => o.wire(() => store.seatLamp(signalId, lamp, output));
/** Release an untracked owner and place the lamp on its pin; the release rides every later preview and check. */
export const releaseAndPlaceOutput = (signalId: string, lamp: 'red' | 'green', owner: ReleasedOwner, output: number) =>
  o.edit(() => store.releaseAndPlace(signalId, lamp, owner, output));

/** The intents the editor components emit. */
export const blockSignalsEditorActions = {
  navigate: navigateBlockSignalsEditor,
  finish: finishBlockSignalsEditor,
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
  chooseWiringNode,
  placeLamp,
  releaseAndPlaceOutput,
};
export type BlockSignalsEditorActions = typeof blockSignalsEditorActions;
