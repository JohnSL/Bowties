import { invoke } from '@tauri-apps/api/core';
import type {
  ComposeIssue,
  EntranceEnd,
  ExitEnd,
  FacilitySignalSettings,
  ListedBlock,
  Movements,
  RemovalEdit,
  TravelDirection,
} from '$lib/api/signaling';

/**
 * Per-slot binding list (Spec 018 / S4 — D8 cardinality contract).
 * Empty array = unbound; one or more entries = bound to those channels.
 * The cap is enforced backend-side per template `SlotDefinition.maxChannels`.
 */
export type SlotBinding = string[];

/// Facility wire form (matches `bowties_core::layout::facilities::Facility`).
export interface Facility {
  facilityId: string;
  templateId: string;
  name: string;
  slotBindings: Record<string, SlotBinding>;
  /** Compiled Automatic Block Signals intent; absent on other templates. */
  blockSignals?: BlockSignalsRecord;
  /** Panel intent; absent on other templates. */
  panel?: PanelRecord;
}

/// Wire form of `bowties_core::layout::facilities::PanelRecord`.
export type LitWhen = 'occupied' | 'clear';

export interface PanelRecord {
  litWhen: LitWhen;
  /** Entries in panel order. */
  entries: PanelEntry[];
}

export interface PanelEntry {
  entryId: string;
  blockId: string;
  lampChannelId?: string;
  litWhen?: LitWhen;
}

/// Wire form of `bowties_core::layout::facilities::BlockSignalsRecord`.
export interface BlockSignalsRecord {
  direction: TravelDirection;
  movements: Movements;
  entranceEnd: EntranceEnd;
  exitEnd: ExitEnd;
  blocks: ListedBlock[];
  settings: FacilitySignalSettings;
  signals: SignalRecord[];
}

/// Wire form of `bowties_core::layout::facilities::SignalRecord`.
export interface SignalRecord {
  signalId: string;
  name: string;
  protectedBlock: string;
  channelId: string;
  resources: SignalResources;
}

/// The board resources one signal uses: its Mast and the Track Circuits it reads (1-based).
export interface SignalResources {
  nodeKey: string;
  mast: number;
  trackCircuits: number[];
}

/// One signal's group of consecutive Logic on a board (1-based `first`).
export interface PlacedLogicGroup {
  facilityId: string;
  signalId: string;
  first: number;
  length: number;
}

/// The recorded Logic placement of one board (wire form of `NodeLogicPlacement`).
export interface NodeLogicPlacement {
  nodeKey: string;
  groups: PlacedLogicGroup[];
}

/// The facilities document as the app reads it: facilities plus per-board Logic placement.
export interface FacilitiesListing {
  facilities: Facility[];
  logicPlacement: NodeLogicPlacement[];
}

/// Derived status of a facility (never persisted on the entity).
export type FacilityStatus = 'Incomplete' | 'Wired';

/// Fetch the facility inventory for the active layout.
export async function listFacilities(): Promise<FacilitiesListing> {
  return invoke<FacilitiesListing>('list_facilities');
}

/** One slot a facility claims, as the removal confirmation lists it. */
export interface FacilityRemovalClaim {
  nodeKey: string;
  kind: 'mast' | 'logic' | 'trackCircuit' | 'directLamp';
  index: number;
  /** A Direct Lamp row's effective Lamp Selection; null when 0 or unset. */
  lampSelection: number | null;
}

/**
 * With any issue, `edits` and `logicPlacement` are empty (`claims` still lists
 * what was claimed). `logicPlacement` has one entry per board that held this
 * facility's Logic: the groups that remain there (empty when none do).
 */
export interface FacilityRemovalResponse {
  edits: RemovalEdit[];
  logicPlacement: NodeLogicPlacement[];
  issues: ComposeIssue[];
  claims: FacilityRemovalClaim[];
}

/**
 * Read-only, kind-agnostic: the edits that return every slot the facility
 * claims (masts, Logic, Track Circuits, panel lamp rows) to its unused state,
 * for the caller to stage when removing it. The Logic's recipe events come back
 * as `freshEventId` for the caller to replace with generated event IDs. A node
 * whose configuration is not loaded, or a claimed slot that is missing, comes
 * back as a `claimedSlotUnavailable` issue with no edits.
 */
export async function composeFacilityRemoval(facilityId: string): Promise<FacilityRemovalResponse> {
  return invoke<FacilityRemovalResponse>('compose_facility_removal', { facilityId });
}
