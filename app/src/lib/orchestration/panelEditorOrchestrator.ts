import { panelWiringFacts, type PanelWiringPin } from '$lib/api/panels';
import type { SelectedDetectedBlock } from '$lib/api/signaling';
import type { LitWhen } from '$lib/api/facilities';
import { createPanelFacility, syncDraftsForComposition, updatePanelFacility } from '$lib/orchestration/facilityOrchestrator';
import { effectiveLayoutStore } from '$lib/layout';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { panelEditorStore as store, type PanelEditorStep } from '$lib/stores/panelEditor.svelte';

/**
 * Sequences the transient Panel editor. The store owns the user's input; this
 * orchestrator loads wiring facts (once per editor session, after syncing
 * drafts so owners reflect the effective layout), decides what a pin click
 * means from each pin's owner, and hands Done to the facility workflow.
 * Cancel, Done and layout reset invalidate any request in flight.
 */
class PanelEditorOrchestrator {
  private requestGeneration = 0;
  private finishing = false;

  start(name: string): void {
    this.requestGeneration += 1;
    store.start(name);
  }

  /** Open the editor on a saved panel's effective state (each lamp's pin is its row's Lamp Selection). */
  startEdit(facilityId: string): void {
    const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
    const record = facility?.panel;
    if (!facility || !record) return;
    this.requestGeneration += 1;
    store.startEdit({
      facilityId,
      name: facility.name,
      litWhen: record.litWhen,
      entries: effectiveLayoutStore.panelCardEntries(facilityId).map((e) => {
        const litWhen = record.entries.find((entry) => entry.entryId === e.entryId)?.litWhen;
        return {
          entryId: e.entryId,
          blockId: e.blockId,
          blockName: e.blockName,
          ...(litWhen ? { litWhen } : {}),
          ...(e.lamp && e.lamp.pin !== null ? { lamp: { nodeKey: e.lamp.nodeKey, output: e.lamp.pin } } : {}),
        };
      }),
    });
  }

  addBlock(block: SelectedDetectedBlock): void {
    store.addBlock(block);
  }

  addBlocks(blocks: SelectedDetectedBlock[]): void { store.addBlocks(blocks); }
  moveEntry(entryId: string, direction: -1 | 1): void { store.moveEntry(entryId, direction); }
  setLitWhen(value: LitWhen): void { store.setLitWhen(value); }
  setEntryLitWhen(entryId: string, value: LitWhen | undefined): void { store.setEntryLitWhen(entryId, value); }

  removeEntry(entryId: string): void {
    store.removeEntry(entryId);
  }

  async goTo(step: PanelEditorStep): Promise<void> {
    store.goTo(step);
    if (step === 'wiring') await this.loadFacts();
  }

  selectEntry(entryId: string): void {
    store.selectEntry(entryId);
  }

  chooseBoard(nodeKey: string): void {
    store.setBoard(nodeKey);
  }

  /** Act on a pin for the selected entry according to who holds it. */
  pinClick(nodeKey: string, output: number): void {
    const entryId = store.selectedEntryId;
    const entry = store.entries.find((e) => e.entryId === entryId);
    if (!entry) return;
    const pin = this.findPin(nodeKey, output);
    if (!pin) return;

    const { owner } = pin;
    const ownPanel = owner.kind === 'panel' && owner.facilityId === store.facilityId;
    if (owner.kind === 'outside') {
      store.setPendingRelease({
        owner: { nodeKey, owner: owner.row === null ? owner.description : `Direct lamp ${owner.row}` },
        nodeKey,
        output,
        description: owner.description,
      });
      return;
    }
    store.setPendingRelease(null);
    if (owner.kind === 'facility' || (owner.kind === 'panel' && !ownPanel)) return;

    store.seatLamp(entry.entryId, nodeKey, output);
    store.setMessage(undefined);
  }

  /** Release the pending outside owner, seat the lamp on its pin, and re-read facts with the release. */
  async confirmRelease(): Promise<void> {
    const pending = store.pendingRelease;
    const entryId = store.selectedEntryId;
    if (!pending || !entryId) return;
    store.releaseAndSeat(entryId, pending.owner, pending.nodeKey, pending.output);
    store.setMessage(undefined);
    await this.refreshFacts();
  }

  /** Create the panel from the editor's input; the editor closes only on success. */
  async finish(): Promise<void> {
    if (this.finishing || !store.open) return;
    this.finishing = true;
    const generation = ++this.requestGeneration;
    try {
      const save = store.mode === 'edit' ? updatePanelFacility : createPanelFacility;
      const result = await save({
        facilityId: store.facilityId,
        name: store.name,
        release: store.releases,
        record: {
          litWhen: store.litWhen,
          entries: store.entries.map((e) => ({ entryId: e.entryId, blockId: e.blockId, ...(e.litWhen ? { litWhen: e.litWhen } : {}) })),
        },
        lamps: store.entries
          .filter((e) => e.lamp)
          .map((e) => ({
            entryId: e.entryId,
            nodeKey: e.lamp!.nodeKey,
            output: e.lamp!.output,
            name: e.blockName,
          })),
      });
      if (generation !== this.requestGeneration) return;
      if (result.ok) {
        this.requestGeneration += 1;
        store.reset();
      } else if ('issues' in result) {
        store.setFinishError(result.issues.map((i) => i.detail).join('\n'));
      } else {
        const { error } = result;
        store.setFinishError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      this.finishing = false;
    }
  }

  cancel(): void {
    this.requestGeneration += 1;
    store.reset();
  }

  resetForNewLayout(): void {
    this.cancel();
  }

  private findPin(nodeKey: string, output: number): PanelWiringPin | undefined {
    return store.boards.find((b) => b.nodeKey === nodeKey)?.pins.find((p) => p.output === output);
  }

  private async loadFacts(): Promise<void> {
    if (store.factsLoaded) return;
    const generation = ++this.requestGeneration;
    try {
      await syncDraftsForComposition();
      await this.readFacts(generation);
    } catch (error) {
      this.failFacts(generation, error);
    }
  }

  /** Re-read facts with the releases chosen so far; the newest request wins. */
  private async refreshFacts(): Promise<void> {
    const generation = ++this.requestGeneration;
    try {
      await this.readFacts(generation);
    } catch (error) {
      this.failFacts(generation, error);
    }
  }

  private async readFacts(generation: number): Promise<void> {
    const facts = await panelWiringFacts(undefined, store.releases);
    if (generation !== this.requestGeneration) return;
    store.acceptFacts(facts.boards, facts.releaseIssues);
    if (!store.boardNodeKey) store.setBoard(facts.boards[0]?.nodeKey);
  }

  private failFacts(generation: number, error: unknown): void {
    if (generation === this.requestGeneration) {
      store.setFinishError(error instanceof Error ? error.message : String(error));
    }
  }
}

export const panelEditorOrchestrator = new PanelEditorOrchestrator();
const o = panelEditorOrchestrator;

/** The intents the Panel editor components emit. */
export const panelEditorActions = {
  startPanelEditor: (name: string): void => o.start(name),
  startPanelEdit: (facilityId: string): void => o.startEdit(facilityId),
  addBlock: (block: SelectedDetectedBlock): void => o.addBlock(block),
  addBlocks: (blocks: SelectedDetectedBlock[]): void => o.addBlocks(blocks),
  moveEntry: (entryId: string, direction: -1 | 1): void => o.moveEntry(entryId, direction),
  setLitWhen: (value: LitWhen): void => o.setLitWhen(value),
  setEntryLitWhen: (entryId: string, value: LitWhen | undefined): void => o.setEntryLitWhen(entryId, value),
  removeEntry: (entryId: string): void => o.removeEntry(entryId),
  goTo: (step: PanelEditorStep): Promise<void> => o.goTo(step),
  selectEntry: (entryId: string): void => o.selectEntry(entryId),
  chooseBoard: (nodeKey: string): void => o.chooseBoard(nodeKey),
  pinClick: (nodeKey: string, output: number): void => o.pinClick(nodeKey, output),
  confirmRelease: (): Promise<void> => o.confirmRelease(),
  finish: (): Promise<void> => o.finish(),
  cancel: (): void => o.cancel(),
};
export type PanelEditorActions = typeof panelEditorActions;
