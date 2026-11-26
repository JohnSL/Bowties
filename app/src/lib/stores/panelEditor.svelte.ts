import type { SelectedDetectedBlock } from '$lib/api/signaling';
import type { PanelWiringBoard } from '$lib/api/panels';
import { canonicalizeNodeId } from '$lib/utils/nodeRoster';
import { generateUuidV4 } from '$lib/utils/uuid';
import { seatPin } from '$lib/utils/pinSeat';

export type PanelEditorStep = 'blocks' | 'wiring';

export interface PanelLamp {
  nodeKey: string;
  output: number;
}

export interface PanelEditorEntry {
  entryId: string;
  blockId: string;
  blockName: string;
  lamp?: PanelLamp;
}

export interface PanelEditorMessage {
  kind: 'warn' | 'ok';
  text: string;
}

/** Wiring step-bar state: nothing yet, entries still needing lamps, or all wired. */
export type PanelWiringStepState = 'none' | 'allWired' | { needLamp: number };

export interface PanelStepBar {
  blocksCount: number;
  wiring: PanelWiringStepState;
}

const samePin = (a: PanelLamp, b: PanelLamp): boolean =>
  a.output === b.output && canonicalizeNodeId(a.nodeKey) === canonicalizeNodeId(b.nodeKey);

/**
 * Transient input for the Panel editor. Held only while the editor is open;
 * nothing here is saved or staged. The orchestrator owns sequencing.
 */
class PanelEditorStore {
  private _open = $state(false);
  private _facilityId = $state('');
  private _name = $state('');
  private _step = $state<PanelEditorStep>('blocks');
  private _entries = $state<PanelEditorEntry[]>([]);
  private _selectedEntryId = $state<string | undefined>(undefined);
  private _boardNodeKey = $state<string | undefined>(undefined);
  private _message = $state<PanelEditorMessage | undefined>(undefined);
  private _finishError = $state<string | null>(null);
  private _boards = $state<PanelWiringBoard[]>([]);
  private _factsLoaded = $state(false);

  get open(): boolean { return this._open; }
  get facilityId(): string { return this._facilityId; }
  get name(): string { return this._name; }
  get step(): PanelEditorStep { return this._step; }
  /** Fixed in S1; the record's default. */
  get litWhen(): 'occupied' { return 'occupied'; }
  get entries(): PanelEditorEntry[] { return this._entries; }
  get selectedEntryId(): string | undefined { return this._selectedEntryId; }
  get boardNodeKey(): string | undefined { return this._boardNodeKey; }
  get message(): PanelEditorMessage | undefined { return this._message; }
  get finishError(): string | null { return this._finishError; }
  /** Wiring facts per board, as loaded for this editor session. */
  get boards(): PanelWiringBoard[] { return this._boards; }
  get factsLoaded(): boolean { return this._factsLoaded; }

  get stepBar(): PanelStepBar {
    const count = this._entries.length;
    if (count === 0) return { blocksCount: 0, wiring: 'none' };
    const need = this._entries.filter((e) => !e.lamp).length;
    return { blocksCount: count, wiring: need === 0 ? 'allWired' : { needLamp: need } };
  }

  lampFor(entryId: string): PanelLamp | null {
    return this._entries.find((e) => e.entryId === entryId)?.lamp ?? null;
  }

  /** The entry whose lamp sits on this pin. */
  entryHolding(nodeKey: string, output: number): PanelEditorEntry | undefined {
    return this._entries.find((e) => e.lamp && samePin(e.lamp, { nodeKey, output }));
  }

  start(name: string): void {
    this.clear();
    this._open = true;
    this._name = name;
    this._facilityId = generateUuidV4();
  }

  goTo(step: PanelEditorStep): void {
    this._step = step;
  }

  addBlock(block: SelectedDetectedBlock): void {
    if (this._entries.some((e) => e.blockId === block.blockId)) return;
    this._entries = [
      ...this._entries,
      { entryId: generateUuidV4(), blockId: block.blockId, blockName: block.name },
    ];
  }

  removeEntry(entryId: string): void {
    this._entries = this._entries.filter((e) => e.entryId !== entryId);
    if (this._selectedEntryId === entryId) this._selectedEntryId = undefined;
  }

  selectEntry(entryId: string | undefined): void {
    this._selectedEntryId = entryId;
    this._message = undefined;
  }

  setBoard(nodeKey: string | undefined): void { this._boardNodeKey = nodeKey; }

  acceptFacts(boards: PanelWiringBoard[]): void {
    this._boards = boards;
    this._factsLoaded = true;
  }

  setMessage(message: PanelEditorMessage | undefined): void { this._message = message; }
  setFinishError(message: string | null): void { this._finishError = message; }

  /**
   * Seat the entry's lamp on a pin: free places it, its own pin clears it, and
   * a pin another entry holds moves to this entry. Returns the entry that lost
   * its lamp, if any.
   */
  seatLamp(entryId: string, nodeKey: string, output: number): PanelEditorEntry | null {
    const assignments: Record<string, PanelLamp | null> = Object.fromEntries(
      this._entries.map((e) => [e.entryId, e.lamp ?? null]),
    );
    const { assignments: next, displaced } = seatPin(assignments, entryId, { nodeKey, output }, {
      taken: 'move',
      samePin,
    });
    const lost = displaced ? this._entries.find((e) => e.entryId === displaced) ?? null : null;
    this._entries = this._entries.map((e) => {
      const { lamp: _, ...rest } = e;
      const lamp = next[e.entryId];
      return lamp ? { ...rest, lamp } : rest;
    });
    this._finishError = null;
    return lost;
  }

  reset(): void { this.clear(); }

  resetForNewLayout(): void { this.clear(); }

  private clear(): void {
    this._open = false;
    this._facilityId = '';
    this._name = '';
    this._step = 'blocks';
    this._entries = [];
    this._selectedEntryId = undefined;
    this._boardNodeKey = undefined;
    this._message = undefined;
    this._finishError = null;
    this._boards = [];
    this._factsLoaded = false;
  }
}

export const panelEditorStore = new PanelEditorStore();
