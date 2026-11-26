import { invoke } from '@tauri-apps/api/core';
import type { TreeConfigValue } from '$lib/types/nodeTree';
import type { NodeLogicPlacement, SignalResources } from '$lib/api/facilities';

/** A detected block in the layout that can be added to the list. */
export interface SelectedDetectedBlock {
  blockId: string;
  name: string;
}

/** A hand-thrown turnout entered by name; its position source is not chosen yet. */
export interface PlaceholderTurnout {
  turnoutId: string;
  name: string;
}

/** A block in the facility's list, in travel order. */
export interface ListedBlock extends SelectedDetectedBlock {
  /** No signals between this block and the one before it. Ignored on the first block. */
  joinedToPrevious: boolean;
  turnouts: PlaceholderTurnout[];
}

export type TravelDirection = 'eastward' | 'westward';
export type Movements = 'oneDirection' | 'bothDirections';
export type EntranceEnd = { kind: 'noPlant' } | { kind: 'plant'; plantId: string };
export type ExitEnd =
  | { kind: 'nothingBeyond' }
  | { kind: 'treatAsStop' }
  | { kind: 'plant'; plantId: string };
export type TurnoutNotLined = 'stop' | 'restricting';
export type ApproachLighting = 'alwaysLit' | 'whenApproached';
export type AspectCount = 2 | 3 | 4;

export interface FacilitySignalSettings {
  aspects: AspectCount;
  turnoutNotLined: TurnoutNotLined;
  approachLighting: ApproachLighting;
}

export interface SignalSettings {
  aspects: AspectCount;
  approachLighting: ApproachLighting;
}

export interface SignalOverride extends SignalSettings {
  signalId: string;
}

export interface BlockSignalsPreviewRequest {
  facilityId: string;
  facilityName: string;
  /** The direction the block list is written in. */
  direction: TravelDirection;
  movements: Movements;
  blocks: ListedBlock[];
  entranceEnd: EntranceEnd;
  exitEnd: ExitEnd;
  settings: FacilitySignalSettings;
  signalOverrides: SignalOverride[];
  /** Signal names by signal ID. */
  signalNames: Record<string, string>;
  /**
   * Untracked owners (a mast or Direct Lamp row no facility claims) released
   * so their pins read as free. Optional; empty when omitted.
   */
  release?: ReleasedOwner[];
}

/** An untracked owner named for release; the backend never releases one that is not listed. */
export interface ReleasedOwner {
  nodeKey: string;
  /** "Mast N" or "Direct lamp N", the form the output hosts report owners in. */
  owner: string;
}

export type SignalIndication = 'stop' | 'restricting' | 'approach' | 'advanceApproach' | 'clear';

export interface SignalOutputHost {
  nodeKey: string;
  nodeName: string;
  style: {
    styleId: 'two-driver-rg-signal';
    bindingKind: 'signalLccRgOutputs';
    appearances: {
      stop: Array<'red' | 'green'>;
      approach: Array<'red' | 'green'>;
      clear: Array<'red' | 'green'>;
    };
  };
  evidence: 'complete' | 'unavailable' | 'invalid';
  evidenceDetail?: string;
  outputs: Array<{
    ordinal: number;
    label: string;
    owner: string | null;
    /** The Bowties facility that owns this pin's mast; absent when configured outside Bowties. */
    facility?: { facilityId: string; name: string };
  }>;
}

export type NextSignal =
  | { kind: 'notUsed' }
  | { kind: 'nothingBeyond' }
  | { kind: 'treatedAsStop' }
  | { kind: 'signal'; signalId: string }
  | { kind: 'plantEntrance'; plantId: string };

export type RuleCondition =
  | { kind: 'blockOccupied' }
  | { kind: 'turnoutNotLined'; turnoutId: string }
  | { kind: 'blockClear' }
  | { kind: 'clearNothingBeyondTreatedAsStop' }
  | { kind: 'clearNextAtStop' }
  | { kind: 'clearNextAtApproach' }
  | { kind: 'clearNextPermissive' };

export interface SignalPreview {
  signalId: string;
  name: string;
  /** The geographic direction of the movements this signal governs. */
  governs: TravelDirection;
  run: 'withList' | 'againstList';
  entranceBlockId: string;
  /** The blocks in this signal's signal block, in its direction of travel. */
  protectedBlockIds: string[];
  turnoutIds: string[];
  settings: SignalSettings;
  overridden: boolean;
  next: NextSignal;
  /** Conditions and resulting indications, most restrictive first. */
  rules: Array<{ condition: RuleCondition; indication: SignalIndication }>;
  observation: 'unavailable';
}

export type ProposalIssue =
  | { kind: 'duplicateBlock'; blockId: string }
  | { kind: 'unsupportedAspectCount'; signalId: string | null; aspects: number }
  | { kind: 'turnoutPositionUnavailable'; turnoutId: string }
  | { kind: 'missingDetectionFacts'; blockId: string; missingStates: string[] }
  | { kind: 'missingHardwareBindings'; requiredBindings: string[] }
  | { kind: 'invalidSignalBinding'; signalId: string; reason: string }
  | { kind: 'duplicateSignalOutputs'; signalId: string; nodeKey: string; output: number }
  | { kind: 'signalOutputConflict'; signalId: string; nodeKey: string; output: number; owner: string }
  | {
    kind: 'signalOutputHeldBySignal';
    signalId: string;
    nodeKey: string;
    output: number;
    holderSignalId: string;
    holderName: string;
  }
  | {
    kind: 'insufficientSignalOutputCapacity';
    signalId: string;
    nodeKey: string;
    required: number;
    available: number;
  }
  | { kind: 'unsupportedSignalStyle'; signalId: string; style: string; nodeKey: string }
  | { kind: 'unavailableSignalHardware'; signalId: string; reason: string };

export interface BlockSignalsPreview {
  facilityName: string;
  direction: TravelDirection;
  movements: Movements;
  blocks: ListedBlock[];
  entranceEnd: EntranceEnd;
  exitEnd: ExitEnd;
  signals: SignalPreview[];
  outputHosts?: SignalOutputHost[];
  /**
   * Every mast outside Bowties that drives a pin on an output host, with the
   * Logic that sets its aspects.
   */
  outsideOwners?: OutsideOwner[];
  capability: {
    kind: 'incompletePreview';
    stage: 'unavailable';
    save: 'unavailable';
    apply: 'unavailable';
  };
  issues: ProposalIssue[];
}

export interface OutsideOwner {
  nodeKey: string;
  /** "Mast N". */
  owner: string;
  /** Logic numbers on the same node whose actions set the mast's aspects. */
  matchedLogic: number[];
  /** The subset of `matchedLogic` that also sends other events; kept, with only the matching actions reset. */
  keptLogic: number[];
}

export async function previewBlockSignals(
  request: BlockSignalsPreviewRequest,
): Promise<BlockSignalsPreview> {
  return invoke<BlockSignalsPreview>('preview_block_signals', { request });
}

/** The red/green wiring chosen for one signal. */
export interface SignalWiring {
  signalId: string;
  nodeKey: string;
  redOutput: number;
  greenOutput: number;
}

/** An unsaved configuration edit, keyed as the draft store keys it. */
export interface ComposeBlockSignalsRequest {
  /** The facility being composed; its own mast and Logic do not count as in use. */
  facilityId?: string;
  line: BlockSignalsPreviewRequest;
  wiring: SignalWiring[];
  /**
   * Untracked owners released for this facility. Their reset edits come back
   * merged with the recipe's (recipe wins on a shared leaf) and their pins and
   * slots count as free.
   */
  release?: ReleasedOwner[];
}

/** A generated configuration edit; pass `nodeKey/space/address/value` to `configEditor.applyEdit`. */
export interface ComposedEdit {
  nodeKey: string;
  space: number;
  address: number;
  /** Compiler CDI path, for diagnostics. */
  path: string;
  value: TreeConfigValue;
}

/** A removal asks the caller to generate a new event ID for the leaf, breaking its connections. */
export interface FreshEventIdValue {
  type: 'freshEventId';
}

/** An edit that may ask for a fresh event ID: a removal's or a release's. */
export interface RemovalEdit extends Omit<ComposedEdit, 'value'> {
  value: TreeConfigValue | FreshEventIdValue;
}

/** One signal's board resources as the compiler chose them. */
export interface SignalResourcesEntry {
  signalId: string;
  resources: SignalResources;
}

export type ComposeIssueCode =
  | 'wiringMissing'
  | 'bindingInvalid'
  | 'hardwareUnavailable'
  | 'styleUnsupported'
  | 'outputDuplicate'
  | 'outputInUse'
  | 'outputCapacity'
  | 'noFreeMast'
  | 'noFreeLogic'
  | 'claimedSlotUnavailable'
  | 'logicUndefinedFunction'
  | 'logicUnterminatedGroup'
  | 'logicUnread'
  | 'releaseInvalid'
  | 'occupancyEventsMissing'
  | 'aspectEventsMissing'
  | 'noFreeTrackCircuit'
  | 'linkAddressMissing'
  | 'unsupportedMovements'
  | 'unsupportedEnds'
  | 'unsupportedAspects'
  | 'unsupportedApproachLighting';

export interface ComposeIssue {
  code: ComposeIssueCode;
  signalId: string | null;
  nodeKey: string | null;
  detail: string;
  bindingIssue?: ProposalIssue;
  /** The Logic numbers a Logic placement issue names; absent for other issues. */
  logic?: number[];
}

/**
 * With any issue, `edits`, `signalResources` and `logicPlacement` are empty.
 * `logicPlacement` is the wiring board's whole new placement. Edits from
 * released owners, vacated Logic and released Track Circuits may carry `freshEventId`.
 */
export interface ComposeBlockSignalsResponse {
  edits: RemovalEdit[];
  signalResources: SignalResourcesEntry[];
  logicPlacement: NodeLogicPlacement[];
  issues: ComposeIssue[];
}

/** Read-only: generates the edits for the caller to stage; writes nothing. */
export async function composeBlockSignals(
  request: ComposeBlockSignalsRequest,
): Promise<ComposeBlockSignalsResponse> {
  return invoke<ComposeBlockSignalsResponse>('compose_block_signals', { request });
}
