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

export type BlockSignalsStep = 'blocks' | 'aspects' | 'appearance' | 'wiring' | 'review';

/** The editor's steps in order; Review is listed but deferred to a later release. */
const STEP_ORDER: ReadonlyArray<BlockSignalsStep> = ['blocks', 'aspects', 'appearance', 'wiring', 'review'];

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
  issuesForSignal(signalId: string): ComposeIssue[];
  issuesForNode(nodeKey: string): ComposeIssue[];
  wiringFor(signalId: string): SignalWiringChoice | null;
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
    if (id === 'review') return { status: 'unavailable', unavailableReason: 'Not in this release' };
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
        if (this.wiringComplete) return { status: 'complete' };
        return { status: this._preview && this._preview.signals.length > 0 ? 'needsWork' : 'todo' };
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
  issuesForSignal(signalId: string): ComposeIssue[] { return this._issues.filter((i) => i.signalId === signalId); }
  issuesForNode(nodeKey: string): ComposeIssue[] { return this._issues.filter((i) => i.nodeKey === nodeKey); }

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

  /** Choosing a different node clears the pins chosen on the previous one. */
  chooseWiringNode(signalId: string, nodeKey: string): void {
    if (this._wiring[signalId]?.nodeKey === nodeKey) return;
    this._wiring = { ...this._wiring, [signalId]: { nodeKey, redOutput: null, greenOutput: null } };
  }

  /** Red and green must differ; a clashing choice is ignored. */
  chooseRedOutput(signalId: string, output: number): void { this.choosePin(signalId, 'redOutput', 'greenOutput', output); }
  chooseGreenOutput(signalId: string, output: number): void { this.choosePin(signalId, 'greenOutput', 'redOutput', output); }

  private choosePin(signalId: string, pin: 'redOutput' | 'greenOutput', other: 'redOutput' | 'greenOutput', output: number): void {
    const choice = this._wiring[signalId];
    if (!choice || choice[other] === output) return;
    this._wiring = { ...this._wiring, [signalId]: { ...choice, [pin]: output } };
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
    if (lamp === 'red') this.chooseRedOutput(signalId, output);
    else this.chooseGreenOutput(signalId, output);
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
    const { record } = seed;
    this._open = true;
    this._mode = 'edit';
    this._facilityId = seed.facilityId;
    this._facilityName = seed.name;
    this._direction = record.direction;
    this._movements = record.movements;
    this._blocks = record.blocks.map((block) => ({ ...block, turnouts: block.turnouts.map((t) => ({ ...t })) }));
    this._entranceEnd = record.entranceEnd;
    this._exitEnd = record.exitEnd;
    this._settings = { ...record.settings };
    this._signalNames = Object.fromEntries(record.signals.map((s) => [s.signalId, s.name]));
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
