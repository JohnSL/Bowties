import type { SelectedDetectedBlock } from '$lib/api/signaling';
import type { ReleasedOwner } from '$lib/api/signaling';
import type { PanelWiringBoard } from '$lib/api/panels';
import type { LitWhen } from '$lib/api/facilities';
import { canonicalizeNodeId } from '$lib/utils/nodeRoster';
import { generateUuidV4 } from '$lib/utils/uuid';
import { withReleasedOwner } from '$lib/utils/releasedOwners';
import { seatPin } from '$lib/utils/pinSeat';
import { moveItem, moveItemTo } from '$lib/utils/moveInList';

export type PanelEditorStep = 'blocks' | 'wiring';
export type PanelEditorMode = 'create' | 'edit';

export interface PanelLamp {
  nodeKey: string;
  output: number;
}

export interface PanelEditorEntry {
  entryId: string;
  blockId: string;
  blockName: string;
  lamp?: PanelLamp;
  litWhen?: LitWhen;
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

/** A saved panel's effective state, as the editor opens on it. */
export interface PanelEditSeed {
  facilityId: string;
  name: string;
  litWhen: LitWhen;
  entries: PanelEditorEntry[];
}

/** An outside owner the user may release so its pin can be used (the view shows the release card). */
export interface PendingRelease {
  owner: ReleasedOwner;
  nodeKey: string;
  output: number;
  description: string;
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
  private _mode = $state<PanelEditorMode>('create');
  private _litWhen = $state<LitWhen>('occupied');
  private _step = $state<PanelEditorStep>('blocks');
  private _entries = $state<PanelEditorEntry[]>([]);
  private _selectedEntryId = $state<string | undefined>(undefined);
  private _boardNodeKey = $state<string | undefined>(undefined);
  private _message = $state<PanelEditorMessage | undefined>(undefined);
  private _finishError = $state<string | null>(null);
  private _boards = $state<PanelWiringBoard[]>([]);
  private _factsLoaded = $state(false);
  private _releases = $state<ReleasedOwner[]>([]);
  private _pendingRelease = $state<PendingRelease | null>(null);
  private _releaseIssues = $state<{ nodeKey: string; detail: string }[]>([]);

  get open(): boolean { return this._open; }
  get facilityId(): string { return this._facilityId; }
  get name(): string { return this._name; }
  get step(): PanelEditorStep { return this._step; }
  get mode(): PanelEditorMode { return this._mode; }
  get litWhen(): LitWhen { return this._litWhen; }
  get entries(): PanelEditorEntry[] { return this._entries; }
  get selectedEntryId(): string | undefined { return this._selectedEntryId; }
  get boardNodeKey(): string | undefined { return this._boardNodeKey; }
  get message(): PanelEditorMessage | undefined { return this._message; }
  get finishError(): string | null { return this._finishError; }
  /** Wiring facts per board, as loaded for this editor session. */
  get boards(): PanelWiringBoard[] { return this._boards; }
  get factsLoaded(): boolean { return this._factsLoaded; }
  /** Untracked owners the user chose to release so their pins can be used. */
  get releases(): ReleasedOwner[] { return this._releases; }
  get pendingRelease(): PendingRelease | null { return this._pendingRelease; }
  /** Releases the backend could not honour, from the latest wiring facts. */
  get releaseIssues(): { nodeKey: string; detail: string }[] { return this._releaseIssues; }

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

  /** Open the editor on a saved panel, keeping its facility ID. */
  startEdit(seed: PanelEditSeed): void {
    this.clear();
    this._open = true;
    this._mode = 'edit';
    this._facilityId = seed.facilityId;
    this._name = seed.name;
    this._litWhen = seed.litWhen;
    this._entries = seed.entries.map((e) => ({ ...e, ...(e.lamp ? { lamp: { ...e.lamp } } : {}) }));
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

  addBlocks(blocks: SelectedDetectedBlock[]): void {
    for (const block of blocks) this.addBlock(block);
  }

  moveEntry(entryId: string, direction: -1 | 1): void {
    const index = this._entries.findIndex((entry) => entry.entryId === entryId);
    const entries = moveItem(this._entries, index, direction);
    if (entries !== this._entries) this._entries = entries;
  }

  moveEntryTo(entryId: string, toIndex: number): void {
    const index = this._entries.findIndex((entry) => entry.entryId === entryId);
    const entries = moveItemTo(this._entries, index, toIndex);
    if (entries !== this._entries) this._entries = entries;
  }

  setLitWhen(value: LitWhen): void { this._litWhen = value; this._finishError = null; }

  setEntryLitWhen(entryId: string, value: LitWhen | undefined): void {
    this._entries = this._entries.map((entry) => {
      if (entry.entryId !== entryId) return entry;
      const { litWhen: _, ...rest } = entry;
      return value ? { ...rest, litWhen: value } : rest;
    });
    this._finishError = null;
  }

  removeEntry(entryId: string): void {
    this._entries = this._entries.filter((e) => e.entryId !== entryId);
    if (this._selectedEntryId === entryId) this._selectedEntryId = undefined;
  }

  selectEntry(entryId: string | undefined): void {
    this._selectedEntryId = entryId;
    const lamp = this._entries.find((entry) => entry.entryId === entryId)?.lamp;
    if (lamp) this._boardNodeKey = lamp.nodeKey;
    this._message = undefined;
    this._pendingRelease = null;
  }

  setBoard(nodeKey: string | undefined): void { this._boardNodeKey = nodeKey; }

  acceptFacts(boards: PanelWiringBoard[], releaseIssues: { nodeKey: string; detail: string }[] = []): void {
    this._boards = boards;
    this._releaseIssues = releaseIssues;
    this._factsLoaded = true;
  }

  setPendingRelease(pending: PendingRelease | null): void { this._pendingRelease = pending; }
  setMessage(message: PanelEditorMessage | undefined): void { this._message = message; }
  setFinishError(message: string | null): void { this._finishError = message; }

  /**
   * Seat the entry's lamp on a pin: free places it, its own pin clears it, and
   * a pin another entry holds is refused (that entry must clear it first).
   */
  seatLamp(entryId: string, nodeKey: string, output: number): void {
    const holder = this.entryHolding(nodeKey, output);
    if (holder && holder.entryId !== entryId) return;
    const assignments: Record<string, PanelLamp | null> = Object.fromEntries(
      this._entries.map((e) => [e.entryId, e.lamp ?? null]),
    );
    const { assignments: next } = seatPin(assignments, entryId, { nodeKey, output }, { samePin });
    this._entries = this._entries.map((e) => {
      const { lamp: _, ...rest } = e;
      const lamp = next[e.entryId];
      return lamp ? { ...rest, lamp } : rest;
    });
    this._finishError = null;
  }

  /** Release an untracked owner and seat the entry's lamp on its (now free) pin. */
  releaseAndSeat(entryId: string, owner: ReleasedOwner, nodeKey: string, output: number): void {
    this._releases = withReleasedOwner(this._releases, owner);
    this._pendingRelease = null;
    this.seatLamp(entryId, nodeKey, output);
  }

  reset(): void { this.clear(); }

  resetForNewLayout(): void { this.clear(); }

  private clear(): void {
    this._open = false;
    this._facilityId = '';
    this._name = '';
    this._mode = 'create';
    this._litWhen = 'occupied';
    this._step = 'blocks';
    this._entries = [];
    this._selectedEntryId = undefined;
    this._boardNodeKey = undefined;
    this._message = undefined;
    this._finishError = null;
    this._boards = [];
    this._factsLoaded = false;
    this._releases = [];
    this._pendingRelease = null;
    this._releaseIssues = [];
  }
}

export const panelEditorStore = new PanelEditorStore();
