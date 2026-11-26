import { panelWiringFacts, type PanelWiringPin } from '$lib/api/panels';
import type { SelectedDetectedBlock } from '$lib/api/signaling';
import { createPanelFacility, syncDraftsForComposition } from '$lib/orchestration/facilityOrchestrator';
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

  addBlock(block: SelectedDetectedBlock): void {
    store.addBlock(block);
  }

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
      store.setMessage({
        kind: 'warn',
        text: `${pin.label} is in use by "${owner.description}" outside Bowties and cannot be used here.`,
      });
      return;
    }
    if (owner.kind === 'facility' || (owner.kind === 'panel' && !ownPanel)) return;

    const lost = store.seatLamp(entry.entryId, nodeKey, output);
    store.setMessage(lost
      ? {
          kind: 'warn',
          text: `Moved ${pin.label} from ${lost.blockName} to ${entry.blockName}. ${lost.blockName} now needs a lamp.`,
        }
      : undefined);
  }

  /** Create the panel from the editor's input; the editor closes only on success. */
  async finish(): Promise<void> {
    if (this.finishing || !store.open) return;
    this.finishing = true;
    const generation = ++this.requestGeneration;
    try {
      const result = await createPanelFacility({
        facilityId: store.facilityId,
        name: store.name,
        record: {
          litWhen: store.litWhen,
          entries: store.entries.map((e) => ({ entryId: e.entryId, blockId: e.blockId })),
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
      const facts = await panelWiringFacts();
      if (generation !== this.requestGeneration) return;
      store.acceptFacts(facts.boards);
      if (!store.boardNodeKey) store.setBoard(facts.boards[0]?.nodeKey);
    } catch (error) {
      if (generation === this.requestGeneration) {
        store.setFinishError(error instanceof Error ? error.message : String(error));
      }
    }
  }
}

export const panelEditorOrchestrator = new PanelEditorOrchestrator();
const o = panelEditorOrchestrator;

/** The intents the Panel editor components emit. */
export const panelEditorActions = {
  startPanelEditor: (name: string): void => o.start(name),
  addBlock: (block: SelectedDetectedBlock): void => o.addBlock(block),
  removeEntry: (entryId: string): void => o.removeEntry(entryId),
  goTo: (step: PanelEditorStep): Promise<void> => o.goTo(step),
  selectEntry: (entryId: string): void => o.selectEntry(entryId),
  chooseBoard: (nodeKey: string): void => o.chooseBoard(nodeKey),
  pinClick: (nodeKey: string, output: number): void => o.pinClick(nodeKey, output),
  finish: (): Promise<void> => o.finish(),
  cancel: (): void => o.cancel(),
};
export type PanelEditorActions = typeof panelEditorActions;
