import type {
  BlockSignalsPreview,
  BlockSignalsPreviewRequest,
  EntranceEnd,
  ExitEnd,
  FacilitySignalSettings,
  ComposeIssue,
  ListedBlock,
  Movements,
  ReleasedOwner,
  SelectedDetectedBlock,
  SignalOutputHost,
  SignalSettings,
  SignalWiring,
  TravelDirection,
} from '$lib/api/signaling';
import type { BlockSignalsRecord } from '$lib/api/facilities';
import type { TurnoutShape } from '$lib/components/BlockSignals/trackGeometry';
import { canonicalizeNodeId } from '$lib/utils/nodeRoster';
import { generateUuidV4 } from '$lib/utils/uuid';
import { seatPin } from '$lib/utils/pinSeat';
import { previewRequestFromRecord } from '$lib/utils/blockSignalsPreviewRequest';

export type BlockSignalsStep = 'blocks' | 'aspects' | 'appearance' | 'wiring';

/** The editor's steps in order. A Review step is deferred (E3r in specs/020-block-signals-first-slice/slices.md). */
const STEP_ORDER: ReadonlyArray<BlockSignalsStep> = ['blocks', 'aspects', 'appearance', 'wiring'];

/** How a step stands, derived from the facility's data rather than its position. */
export type BlockSignalsStepStatus = 'complete' | 'needsWork' | 'todo' | 'unavailable';

export interface BlockSignalsStepState {
  id: BlockSignalsStep;
  status: BlockSignalsStepStatus;
  unavailableReason?: string;
}

/** The wiring chosen so far for one signal; `null` until chosen. */
export interface SignalWiringChoice {
  nodeKey: string;
  redOutput: number | null;
  greenOutput: number | null;
}

/** A signal in this edit that holds a pin, and the lamp it drives from it. */
export interface PinHolder {
  signalId: string;
  name: string;
  lamp: 'red' | 'green';
}

/** One thing on the Wiring page that needs the user: a missing choice or a check issue. */
export type WiringAttentionItem =
  | { kind: 'missing'; what: 'signalLcc' | 'redPin' | 'greenPin' }
  | { kind: 'issue'; issue: ComposeIssue };

/** Settings this release cannot compile; Done is refused while any is set. */
export type OutOfScopeSetting = 'aspects' | 'approachLighting' | 'movements';

const DEFAULT_SETTINGS: FacilitySignalSettings = {
  aspects: 3,
  turnoutNotLined: 'stop',
  approachLighting: 'alwaysLit',
};

export type BlockSignalsEditorMode = 'create' | 'edit';

/** A saved facility's effective state, as the orchestrator reads it for editing. */
export interface BlockSignalsEditSeed {
  facilityId: string;
  name: string;
  record: BlockSignalsRecord;
  /** Wiring per signal ID, from each signal channel's effective binding. */
  wiring: Record<string, SignalWiringChoice>;
}

/** What the editor components read; they never change the store directly. */
export interface BlockSignalsEditorView {
  readonly mode: BlockSignalsEditorMode;
  readonly step: BlockSignalsStep;
  readonly facilityName: string;
  readonly facilityId: string;
  readonly direction: TravelDirection;
  readonly movements: Movements;
  readonly blocks: ListedBlock[];
  readonly entranceEnd: EntranceEnd;
  readonly exitEnd: ExitEnd;
  readonly settings: FacilitySignalSettings;
  readonly signalOverrides: Record<string, SignalSettings>;
  readonly signalNames: Record<string, string>;
  readonly turnoutShapes: Record<string, TurnoutShape>;
  readonly preview: BlockSignalsPreview | null;
  readonly error: string | null;
  readonly steps: ReadonlyArray<BlockSignalsStepState>;
  readonly outputHosts: SignalOutputHost[];
  readonly wiringComplete: boolean;
  readonly outOfScopeSettings: OutOfScopeSetting[];
  readonly issues: ComposeIssue[];
  readonly finishError: string | null;
  readonly canFinish: boolean;
  /** What the Wiring page needs from the user for one signal. */
  wiringAttentionFor(signalId: string): WiringAttentionItem[];
  /** Items to resolve per signal, only for signals with any. */
  readonly wiringAttentionCounts: Record<string, number>;
  /** Check issues that belong to no signal and no node. */
  readonly facilityWiringIssues: ComposeIssue[];
  readonly signalCount: number;
  /** Signals with nothing left to resolve on the Wiring page. */
  readonly wiredCount: number;
  /** The next signal after `fromSignalId`, wrapping, that needs attention. */
  nextAttentionSignalId(fromSignalId: string | null): string | null;
  wiringFor(signalId: string): SignalWiringChoice | null;
  /** The other signal in this edit that holds a pin on a node, or null when none does. */
  pinHolder(signalId: string, nodeKey: string, output: number): PinHolder | null;
  canMoveTurnout(turnoutId: string, delta: -1 | 1): boolean;
}

/**
 * Transient input for the Automatic Block Signals editor. It is held only
 * while the editor is open; nothing here is saved, staged or applied. The
 * store owns the user's input; the preview is derived from it by the backend
 * and never written back over it.
 */
class BlockSignalsEditorStore implements BlockSignalsEditorView {
  private _open = $state(false);
  private _mode = $state<BlockSignalsEditorMode>('create');
  private _step = $state<BlockSignalsStep>('blocks');
  private _furthest = $state(0);
  private _facilityName = $state('');
  private _facilityId = $state('');
  private _direction = $state<TravelDirection>('eastward');
  private _movements = $state<Movements>('oneDirection');
  private _blocks = $state<ListedBlock[]>([]);
  private _entranceEnd = $state<EntranceEnd>({ kind: 'noPlant' });
  private _exitEnd = $state<ExitEnd>({ kind: 'nothingBeyond' });
  private _settings = $state<FacilitySignalSettings>({ ...DEFAULT_SETTINGS });
  private _signalOverrides = $state<Record<string, SignalSettings>>({});
  private _signalNames = $state<Record<string, string>>({});
  private _turnoutShapes = $state<Record<string, TurnoutShape>>({});
  private _turnoutCount = 0;
  private _preview = $state<BlockSignalsPreview | null>(null);
  private _error = $state<string | null>(null);
  private _wiring = $state<Record<string, SignalWiringChoice>>({});
  private _issues = $state<ComposeIssue[]>([]);
  private _checkFresh = $state(false);
  private _finishError = $state<string | null>(null);
  private _released = $state<ReleasedOwner[]>([]);

  get open(): boolean { return this._open; }
  get mode(): BlockSignalsEditorMode { return this._mode; }
  get step(): BlockSignalsStep { return this._step; }
  get facilityName(): string { return this._facilityName; }
  get direction(): TravelDirection { return this._direction; }
  get movements(): Movements { return this._movements; }
  get blocks(): ListedBlock[] { return this._blocks; }
  get entranceEnd(): EntranceEnd { return this._entranceEnd; }
  get exitEnd(): ExitEnd { return this._exitEnd; }
  get settings(): FacilitySignalSettings { return this._settings; }
  get signalOverrides(): Record<string, SignalSettings> { return this._signalOverrides; }
  get signalNames(): Record<string, string> { return this._signalNames; }
  get turnoutShapes(): Record<string, TurnoutShape> { return this._turnoutShapes; }
  /** Untracked owners the user chose to release so their pins can be used. */
  get released(): ReleasedOwner[] { return this._released; }
  get preview(): BlockSignalsPreview | null { return this._preview; }
  get error(): string | null { return this._error; }
  get steps(): ReadonlyArray<BlockSignalsStepState> {
    return STEP_ORDER.map((id, index) => ({ id, ...this.stepStatus(id, index) }));
  }

  private stepStatus(id: BlockSignalsStep, index: number): { status: BlockSignalsStepStatus; unavailableReason?: string } {
    if (id !== 'blocks' && this._blocks.length === 0) {
      return { status: 'unavailable', unavailableReason: 'Add a block first' };
    }
    const reached = this._mode === 'edit' || index <= this._furthest;
    if (!reached) return { status: 'todo' };
    const outOfScope = this.outOfScopeSettings;
    switch (id) {
      case 'blocks':
        if (outOfScope.includes('movements')) return { status: 'needsWork' };
        return { status: this._blocks.length > 0 ? 'complete' : 'todo' };
      case 'aspects':
        return { status: outOfScope.includes('aspects') || outOfScope.includes('approachLighting') ? 'needsWork' : 'complete' };
      case 'appearance':
        return { status: 'complete' };
      default:
        if (!this._preview || this._preview.signals.length === 0) return { status: 'todo' };
        return { status: this.wiringNeedsAttention ? 'needsWork' : 'complete' };
    }
  }
  get facilityId(): string { return this._facilityId; }

  /** Signal LCC hosts and their outputs as the backend preview reports them. */
  get outputHosts(): SignalOutputHost[] { return this._preview?.outputHosts ?? []; }

  /** Every signal in the preview has a node and two distinct outputs. */
  get wiringComplete(): boolean {
    const signals = this._preview?.signals ?? [];
    return signals.length > 0 && signals.every((signal) => {
      const choice = this._wiring[signal.signalId];
      return !!choice && choice.redOutput !== null && choice.greenOutput !== null;
    });
  }

  /** The one Wiring rule: any signal or facility item to resolve. */
  private get wiringNeedsAttention(): boolean {
    return Object.keys(this.wiringAttentionCounts).length > 0 || this.facilityWiringIssues.length > 0;
  }

  get signalCount(): number { return this._preview?.signals.length ?? 0; }
  get wiredCount(): number { return this.signalCount - Object.keys(this.wiringAttentionCounts).length; }

  get facilityWiringIssues(): ComposeIssue[] {
    return this._issues.filter((i) => !i.signalId && !i.nodeKey);
  }

  get wiringAttentionCounts(): Record<string, number> {
    const counts: Record<string, number> = {};
    for (const signal of this._preview?.signals ?? []) {
      const count = this.wiringAttentionFor(signal.signalId).length;
      if (count > 0) counts[signal.signalId] = count;
    }
    return counts;
  }

  wiringAttentionFor(signalId: string): WiringAttentionItem[] {
    const choice = this._wiring[signalId];
    const items: WiringAttentionItem[] = [];
    if (!choice) items.push({ kind: 'missing', what: 'signalLcc' });
    else {
      if (choice.redOutput === null) items.push({ kind: 'missing', what: 'redPin' });
      if (choice.greenOutput === null) items.push({ kind: 'missing', what: 'greenPin' });
    }
    const node = choice ? canonicalizeNodeId(choice.nodeKey) : null;
    for (const issue of this._issues) {
      const onThisNode = !issue.signalId && node !== null && !!issue.nodeKey && canonicalizeNodeId(issue.nodeKey) === node;
      if (issue.signalId === signalId || onThisNode) {
        items.push({ kind: 'issue', issue });
      }
    }
    return items;
  }

  nextAttentionSignalId(fromSignalId: string | null): string | null {
    const ids = (this._preview?.signals ?? []).map((s) => s.signalId);
    const start = fromSignalId === null ? -1 : ids.indexOf(fromSignalId);
    for (let step = 1; step <= ids.length; step++) {
      const id = ids[(start + step) % ids.length];
      if (id !== fromSignalId && this.wiringAttentionFor(id).length > 0) return id;
    }
    return null;
  }

  /** The wiring for the preview's signals; empty unless complete. */
  signalWirings(): SignalWiring[] {
    if (!this.wiringComplete) return [];
    return (this._preview?.signals ?? []).map((signal) => {
      const choice = this._wiring[signal.signalId];
      return {
        signalId: signal.signalId,
        nodeKey: choice.nodeKey,
        redOutput: choice.redOutput as number,
        greenOutput: choice.greenOutput as number,
      };
    });
  }

  /** Settings that this release cannot compile into a facility. */
  get outOfScopeSettings(): OutOfScopeSetting[] {
    const found: OutOfScopeSetting[] = [];
    const overrides = Object.values(this._signalOverrides);
    if (this._settings.aspects !== 3 || overrides.some((o) => o.aspects !== 3)) found.push('aspects');
    if (this._settings.approachLighting !== 'alwaysLit' || overrides.some((o) => o.approachLighting !== 'alwaysLit')) {
      found.push('approachLighting');
    }
    if (this._movements !== 'oneDirection') found.push('movements');
    return found;
  }

  get issues(): ComposeIssue[] { return this._issues; }
  get finishError(): string | null { return this._finishError; }

  /** Done is offered only for complete, in-scope wiring whose latest check found nothing. */
  get canFinish(): boolean {
    return this._step === 'wiring'
      && this.wiringComplete
      && this.outOfScopeSettings.length === 0
      && this._checkFresh
      && this._issues.length === 0;
  }

  /** Any edit makes the last compose check stale. */
  invalidateCheck(): void {
    this._checkFresh = false;
    this._finishError = null;
  }

  acceptCheck(issues: ComposeIssue[]): void {
    this._issues = issues;
    this._checkFresh = true;
  }

  clearCheck(): void {
    this._issues = [];
    this._checkFresh = false;
  }

  setFinishError(message: string): void { this._finishError = message; }

  wiringFor(signalId: string): SignalWiringChoice | null { return this._wiring[signalId] ?? null; }

  pinHolder(signalId: string, nodeKey: string, output: number): PinHolder | null {
    const node = canonicalizeNodeId(nodeKey);
    for (const other of this._preview?.signals ?? []) {
      const choice = this._wiring[other.signalId];
      if (other.signalId === signalId || !choice || canonicalizeNodeId(choice.nodeKey) !== node) continue;
      if (choice.redOutput === output) return { signalId: other.signalId, name: other.name, lamp: 'red' };
      if (choice.greenOutput === output) return { signalId: other.signalId, name: other.name, lamp: 'green' };
    }
    return null;
  }

  /** Choosing a different node clears the pins chosen on the previous one. */
  chooseWiringNode(signalId: string, nodeKey: string): void {
    if (this._wiring[signalId]?.nodeKey === nodeKey) return;
    this._wiring = { ...this._wiring, [signalId]: { nodeKey, redOutput: null, greenOutput: null } };
  }

  /**
   * Seat a lamp on a pin: a free pin places it, the lamp's own pin clears it,
   * and the other lamp's pin swaps the two (the other lamp takes this lamp's
   * old pin, possibly none). A pin another signal in this edit holds is
   * refused: no stealing.
   */
  seatLamp(signalId: string, lamp: 'red' | 'green', output: number): void {
    const choice = this._wiring[signalId];
    if (!choice || this.pinHolder(signalId, choice.nodeKey, output)) return;
    const { assignments } = seatPin(
      { red: choice.redOutput, green: choice.greenOutput },
      lamp,
      output,
      { taken: 'swap' },
    );
    this._wiring = {
      ...this._wiring,
      [signalId]: { ...choice, redOutput: assignments.red, greenOutput: assignments.green },
    };
  }

  /** Release an untracked owner; the same owner is held once. */
  releaseOwner(owner: ReleasedOwner): void {
    const key = canonicalizeNodeId(owner.nodeKey);
    if (this._released.some((r) => canonicalizeNodeId(r.nodeKey) === key && r.owner === owner.owner)) return;
    this._released = [...this._released, { ...owner }];
  }

  /** Release an owner and place the lamp on its (now free) pin in one step. */
  releaseAndPlace(signalId: string, lamp: 'red' | 'green', owner: ReleasedOwner, output: number): void {
    this.releaseOwner(owner);
    const own = this._wiring[signalId]?.[lamp === 'red' ? 'redOutput' : 'greenOutput'];
    if (own !== output) this.seatLamp(signalId, lamp, output);
  }

  start(initialName: string): void {
    this.clear();
    this._open = true;
    this._facilityName = initialName;
    this._facilityId = generateUuidV4();
  }

  /** Open the editor on a saved facility, seeding all input from its effective state. */
  startEdit(seed: BlockSignalsEditSeed): void {
    this.clear();
    const request = previewRequestFromRecord(seed.facilityId, seed.name, seed.record);
    this._open = true;
    this._mode = 'edit';
    this._facilityId = request.facilityId;
    this._facilityName = request.facilityName;
    this._direction = request.direction;
    this._movements = request.movements;
    this._blocks = request.blocks;
    this._entranceEnd = request.entranceEnd;
    this._exitEnd = request.exitEnd;
    this._settings = request.settings;
    this._signalNames = request.signalNames;
    this._wiring = Object.fromEntries(Object.entries(seed.wiring).map(([id, w]) => [id, { ...w }]));
  }

  navigate(step: BlockSignalsStep): void {
    const state = this.steps.find((item) => item.id === step);
    if (!state || state.status === 'unavailable') return;
    this._step = step;
    this._furthest = Math.max(this._furthest, STEP_ORDER.indexOf(step));
  }
  setFacilityName(name: string): void { this._facilityName = name; }
  setDirection(direction: TravelDirection): void { this._direction = direction; }
  setMovements(movements: Movements): void { this._movements = movements; }
  setEntranceEnd(end: EntranceEnd): void { this._entranceEnd = end; }
  setExitEnd(end: ExitEnd): void { this._exitEnd = end; }

  setSettings(change: Partial<FacilitySignalSettings>): void {
    this._settings = { ...this._settings, ...change };
  }

  /** Give a signal its own settings, or `null` to use the facility's again. */
  setSignalOverride(signalId: string, settings: SignalSettings | null): void {
    const { [signalId]: _, ...rest } = this._signalOverrides;
    this._signalOverrides = settings ? { ...rest, [signalId]: settings } : rest;
  }

  setSignalName(signalId: string, name: string): void {
    this._signalNames = { ...this._signalNames, [signalId]: name };
  }

  addBlock(block: SelectedDetectedBlock): void {
    if (this._blocks.some((item) => item.blockId === block.blockId)) return;
    this._blocks = [...this._blocks, { ...block, joinedToPrevious: false, turnouts: [] }];
  }

  removeBlock(blockId: string): void {
    this._blocks = this._blocks.filter((block) => block.blockId !== blockId);
  }

  /** Move a block, with its turnouts, one place up (-1) or down (+1). */
  moveBlock(blockId: string, delta: -1 | 1): void {
    const index = this._blocks.findIndex((block) => block.blockId === blockId);
    const target = index + delta;
    if (index < 0 || target < 0 || target >= this._blocks.length) return;
    const blocks = [...this._blocks];
    [blocks[index], blocks[target]] = [blocks[target], blocks[index]];
    this._blocks = blocks;
  }

  setJoinedToPrevious(blockId: string, joined: boolean): void {
    this._blocks = this._blocks.map((block) =>
      block.blockId === blockId ? { ...block, joinedToPrevious: joined } : block);
  }

  /**
   * Add a placeholder turnout to a block: after `afterTurnoutId` when given,
   * otherwise first in the block. Returns the new turnout's ID.
   */
  addTurnout(blockId: string, afterTurnoutId?: string): string {
    this._turnoutCount += 1;
    const turnout = { turnoutId: `turnout:${generateUuidV4()}`, name: `Turnout ${this._turnoutCount}` };
    this._blocks = this._blocks.map((block) => {
      if (block.blockId !== blockId) return block;
      const at = afterTurnoutId ? block.turnouts.findIndex((t) => t.turnoutId === afterTurnoutId) + 1 : 0;
      const turnouts = [...block.turnouts];
      turnouts.splice(at, 0, turnout);
      return { ...block, turnouts };
    });
    return turnout.turnoutId;
  }

  renameTurnout(turnoutId: string, name: string): void {
    this._blocks = this._blocks.map((block) => ({
      ...block,
      turnouts: block.turnouts.map((t) => (t.turnoutId === turnoutId ? { ...t, name } : t)),
    }));
  }

  removeTurnout(turnoutId: string): void {
    this._blocks = this._blocks.map((block) => ({
      ...block,
      turnouts: block.turnouts.filter((t) => t.turnoutId !== turnoutId),
    }));
    const { [turnoutId]: _, ...shapes } = this._turnoutShapes;
    this._turnoutShapes = shapes;
  }

  /** Whether a turnout can step up (-1) or down (+1), within its block or into the next one. */
  canMoveTurnout(turnoutId: string, delta: -1 | 1): boolean {
    const index = this._blocks.findIndex((block) => block.turnouts.some((t) => t.turnoutId === turnoutId));
    if (index < 0) return false;
    const position = this._blocks[index].turnouts.findIndex((t) => t.turnoutId === turnoutId);
    const inside = delta < 0 ? position > 0 : position < this._blocks[index].turnouts.length - 1;
    return inside || (index + delta >= 0 && index + delta < this._blocks.length);
  }

  /** Step a turnout up or down within its block, and on into the neighbouring block. */
  moveTurnout(turnoutId: string, delta: -1 | 1): void {
    if (!this.canMoveTurnout(turnoutId, delta)) return;
    const blocks = this._blocks.map((block) => ({ ...block, turnouts: [...block.turnouts] }));
    const index = blocks.findIndex((block) => block.turnouts.some((t) => t.turnoutId === turnoutId));
    const turnouts = blocks[index].turnouts;
    const position = turnouts.findIndex((t) => t.turnoutId === turnoutId);
    const target = position + delta;
    if (target >= 0 && target < turnouts.length) {
      [turnouts[position], turnouts[target]] = [turnouts[target], turnouts[position]];
    } else {
      const [turnout] = turnouts.splice(position, 1);
      const neighbour = blocks[index + delta].turnouts;
      if (delta < 0) neighbour.push(turnout);
      else neighbour.unshift(turnout);
    }
    this._blocks = blocks;
  }

  setTurnoutShape(turnoutId: string, shape: TurnoutShape): void {
    this._turnoutShapes = { ...this._turnoutShapes, [turnoutId]: shape };
  }

  previewRequest(): BlockSignalsPreviewRequest {
    return {
      facilityId: this._facilityId,
      facilityName: this._facilityName,
      direction: this._direction,
      movements: this._movements,
      blocks: this._blocks,
      entranceEnd: this._entranceEnd,
      exitEnd: this._exitEnd,
      settings: this._settings,
      signalOverrides: Object.entries(this._signalOverrides).map(([signalId, settings]) => ({ signalId, ...settings })),
      signalNames: this._signalNames,
      release: this._released,
    };
  }

  acceptPreview(preview: BlockSignalsPreview): void {
    this._preview = preview;
    this._error = null;
  }

  setError(error: string): void {
    this._error = error;
    this._preview = null;
  }

  clearPreview(): void {
    this._preview = null;
    this._error = null;
  }

  reset(): void {
    this.clear();
  }

  resetForNewLayout(): void { this.reset(); }

  private clear(): void {
    this._open = false;
    this._mode = 'create';
    this._step = 'blocks';
    this._furthest = 0;
    this._facilityName = '';
    this._facilityId = '';
    this._direction = 'eastward';
    this._movements = 'oneDirection';
    this._blocks = [];
    this._entranceEnd = { kind: 'noPlant' };
    this._exitEnd = { kind: 'nothingBeyond' };
    this._settings = { ...DEFAULT_SETTINGS };
    this._signalOverrides = {};
    this._signalNames = {};
    this._turnoutShapes = {};
    this._turnoutCount = 0;
    this._preview = null;
    this._error = null;
    this._wiring = {};
    this._issues = [];
    this._checkFresh = false;
    this._finishError = null;
    this._released = [];
  }
}

export const blockSignalsEditorStore = new BlockSignalsEditorStore();
