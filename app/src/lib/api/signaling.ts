import { invoke } from '@tauri-apps/api/core';
import type { ChannelBinding, ChannelRole } from '$lib/api/channels';

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
   * A transient selection only. It reuses the established role/style/binding
   * vocabulary but does not create, stage, or save an Information Channel.
   */
  bindings?: SignalBindingSelection[];
}

export type SignalOutputBinding = Extract<
  ChannelBinding,
  { kind: 'signalLccRgOutputs' }
>;

export interface SignalBindingSelection {
  signalId: string;
  role: Extract<ChannelRole, 'signal-indication'>;
  style: 'two-driver-rg-signal';
  binding: SignalOutputBinding;
}

export type SignalIndication = 'stop' | 'restricting' | 'approach' | 'advanceApproach' | 'clear';

export interface SignalAppearance {
  indication: 'stop' | 'approach' | 'clear';
  outputs: Array<'red' | 'green'>;
}

export interface SignalDisplay {
  role: Extract<ChannelRole, 'signal-indication'>;
  style: 'two-driver-rg-signal';
  binding: SignalOutputBinding | null;
  appearances: SignalAppearance[];
}

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
  display?: SignalDisplay;
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
  capability: {
    kind: 'incompletePreview';
    stage: 'unavailable';
    save: 'unavailable';
    apply: 'unavailable';
  };
  issues: ProposalIssue[];
}

export async function previewBlockSignals(
  request: BlockSignalsPreviewRequest,
): Promise<BlockSignalsPreview> {
  return invoke<BlockSignalsPreview>('preview_block_signals', { request });
}
