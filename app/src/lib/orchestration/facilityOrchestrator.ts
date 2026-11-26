/** Workflow owner for Panel and Automatic Block Signals staging and removal. */

import type { InformationChannel } from '$lib/api/channels';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { configEditor } from '$lib/stores/configEditor.svelte';
import { configChangesStore, type ConfigDraftEntry } from '$lib/stores/configChanges.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import { syncLayoutDrafts } from '$lib/api/layout';
import { canonicalEventIdHex } from '$lib/utils/serialize';
import { editKeyForLeaf, parseEditKey } from '$lib/utils/editKey';
import {
  composeBlockSignals,
  type BlockSignalsPreviewRequest,
  type ComposeIssue,
  type ReleasedOwner,
  type RemovalEdit,
  type SignalWiring,
} from '$lib/api/signaling';
import { composeFacilityRemoval, type BlockSignalsRecord, type Facility, type FacilityRemovalClaim, type FacilityRemovalResponse, type NodeLogicPlacement, type PanelRecord } from '$lib/api/facilities';
import { composePanel, type ComposePanelIssue } from '$lib/api/panels';
import type { NodeConfigTree, TreeConfigValue } from '$lib/types/nodeTree';
import { generateFreshEventIdForNode } from '$lib/utils/eventIds';

/** Thrown when the referenced facility / slot / channel / template is unknown. */
export class UnknownReferenceError extends Error {
  constructor(public readonly what: 'facility' | 'slot' | 'channel' | 'template') {
    super(`Unknown ${what}`);
    this.name = 'UnknownReferenceError';
  }
}

/**
 * Push the current facility + channel draft deltas into
 * `LayoutState.drafts` so the compose IPC — which reads through
 * `LayoutState.effective_*` — observes the frontend's pending edits.
 *
 * The frontend still owns the draft layer per ADR-0012; this call is
 * the on-demand mirror described in ADR-0015. Callers send the
 * complete current delta set (idempotent w.r.t. re-sync).
 */
export async function syncDraftsForComposition(): Promise<void> {
  const deltas = [
    ...facilitiesStore.collectDeltas(),
    ...channelsStore.collectDeltas(),
  ];
  const configDrafts = configChangesStore.draftEntries().map(({ key, value }) => {
    const { normalizedNodeId, space, address } = parseEditKey(key);
    return { nodeKey: normalizedNodeId, space, address, value };
  });
  await syncLayoutDrafts(deltas, configDrafts);
}

/** User-owned channel IDs bound to the facility's slots (snapshot before delete). */
function userOwnedChannelIdsOf(facilityId: string): string[] {
  const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
  const ids: string[] = [];
  if (!facility) return ids;
  for (const channelId of Object.values(facility.slotBindings).flat()) {
    const channel = channelsStore.channels.find((c) => c.id === channelId);
    if (channel?.ownership === 'user-owned') ids.push(channelId);
  }
  return ids;
}

/**
 * Delete the facility, then its user-owned channels. Hardware-owned channels
 * are left alone: their lifecycle follows hardware config, not facility
 * membership. Facility first, then channels, so the cascade orchestrator
 * (which watches `channelsStore.channels`) finds no slot bindings referencing
 * the disappearing channel.
 */
function deleteFacilityAndChannels(facilityId: string, userOwnedChannelIds: string[]): void {
  facilitiesStore.deleteFacility(facilityId);
  for (const channelId of userOwnedChannelIds) {
    channelsStore.removeUserOwnedChannel(channelId);
  }
}

// ── Automatic Block Signals creation (Spec 020 / E1a) ────────────────────

const BLOCK_SIGNALS_TEMPLATE_ID = 'automatic-block-signals';
const BLOCK_SIGNALS_SLOT = 'signals';

export interface CreateBlockSignalsFacilityArgs {
  /** ID minted by the editor; the compose request and the facility share it. */
  facilityId: string;
  name: string;
  line: BlockSignalsPreviewRequest;
  wiring: SignalWiring[];
  /** Untracked owners released for this facility; their reset edits are staged with the recipe's. */
  release?: ReleasedOwner[];
  /** The facility's signals in order; channel IDs are assigned here. */
  signals: { signalId: string; name: string; protectedBlock: string }[];
}

export type CreateBlockSignalsFacilityResult =
  | { ok: true }
  | { ok: false; issues: ComposeIssue[] }
  | { ok: false; error: unknown };

export type UpdateBlockSignalsFacilityArgs = CreateBlockSignalsFacilityArgs;
export type UpdateBlockSignalsFacilityResult = CreateBlockSignalsFacilityResult;

/** Workflow owner: create an Automatic Block Signals facility (see `stageBlockSignalsFacility`). */
export function createBlockSignalsFacility(
  args: CreateBlockSignalsFacilityArgs,
): Promise<CreateBlockSignalsFacilityResult> {
  return stageBlockSignalsFacility('create', args);
}

/**
 * Workflow owner: update a saved Automatic Block Signals facility. Same
 * pipeline as create; the facility keeps its ID, signal channels and slot
 * attachments, and its record, names and channel wiring are replaced.
 */
export function updateBlockSignalsFacility(
  args: UpdateBlockSignalsFacilityArgs,
): Promise<UpdateBlockSignalsFacilityResult> {
  return stageBlockSignalsFacility('update', args);
}

/**
 * Shared staging pipeline for Automatic Block Signals (create and update).
 *
 * Mirrors drafts so the backend sees other unsaved claims, composes with
 * fresh facts (wiring nodes' unsaved edits included, facilityId so an update
 * recompiles around its own mast and Logic), refuses on any issue without
 * staging anything, then stages the facility/record/channels and the
 * configuration edits as ordinary drafts. Any failure after the first staged
 * mutation restores everything staged here. Never composes bowties (the
 * template has no mapping).
 */
async function stageBlockSignalsFacility(
  target: 'create' | 'update',
  args: CreateBlockSignalsFacilityArgs,
): Promise<CreateBlockSignalsFacilityResult> {
  const { facilityId, name, line, wiring, signals, release } = args;
  const template = behaviorTemplatesStore.findByTemplateId(BLOCK_SIGNALS_TEMPLATE_ID);
  if (!template) return { ok: false, error: new UnknownReferenceError('template') };

  const priorFacility = target === 'update'
    ? facilitiesStore.facilities.find((f) => f.facilityId === facilityId)
    : undefined;
  if (target === 'update' && !priorFacility?.blockSignals) {
    return { ok: false, error: new UnknownReferenceError('facility') };
  }
  const savedSignals = priorFacility?.blockSignals?.signals ?? [];

  return runStagingTransaction({
    compose: () => composeBlockSignals({ facilityId, line, wiring, release }),
    stage: (tx, composed) => {
      const wiringBySignal = new Map(wiring.map((w) => [w.signalId, w]));
      const savedChannelIds = new Map(savedSignals.map((s) => [s.signalId, s.channelId]));
      const createdChannelIds: string[] = [];
      const recordSignals: BlockSignalsRecord['signals'] = signals.map((signal) => {
        const wired = wiringBySignal.get(signal.signalId);
        if (!wired) throw new Error(`No wiring for signal '${signal.signalId}'`);
        const resources = composed.signalResources.find((r) => r.signalId === signal.signalId)?.resources;
        if (!resources) throw new Error(`No board resources planned for signal '${signal.signalId}'`);
        const binding = {
          kind: 'signalLccRgOutputs' as const,
          nodeKey: wired.nodeKey,
          greenOutput: wired.greenOutput,
          redOutput: wired.redOutput,
        };
        const savedChannelId = savedChannelIds.get(signal.signalId);
        if (savedChannelId) {
          tx.renameChannel(savedChannelId, signal.name);
          tx.rebindChannel(savedChannelId, binding);
          return { ...signal, channelId: savedChannelId, resources };
        }
        const channel = tx.createUserOwnedChannel({
          role: 'signal-indication',
          style: 'two-driver-rg-signal',
          binding,
          name: signal.name,
        });
        createdChannelIds.push(channel.id);
        return { ...signal, channelId: channel.id, resources };
      });

      const record: BlockSignalsRecord = {
        direction: line.direction,
        movements: line.movements,
        entranceEnd: line.entranceEnd,
        exitEnd: line.exitEnd,
        blocks: line.blocks,
        settings: line.settings,
        signals: recordSignals,
      };
      if (target === 'create') {
        tx.addFacility(template, name, facilityId, record);
      } else {
        tx.setBlockSignalsRecord(facilityId, record);
        tx.renameFacility(facilityId, name);
      }
      for (const channelId of createdChannelIds) {
        tx.attachChannel(facilityId, BLOCK_SIGNALS_SLOT, channelId);
      }
      tx.logicPlacement(composed.logicPlacement);
      tx.configEdits(composed.edits, facilityId);
    },
    // Signals the edit turned off lose their channel, after the last throwing
    // step; Discard restores both through the draft layer (ADR-0012).
    commit: () => {
      const keptSignalIds = new Set(signals.map((s) => s.signalId));
      for (const saved of savedSignals) {
        if (keptSignalIds.has(saved.signalId)) continue;
        facilitiesStore.detachChannel(facilityId, BLOCK_SIGNALS_SLOT, saved.channelId);
        channelsStore.removeUserOwnedChannel(saved.channelId);
      }
    },
  });
}

// ── Facility removal (Spec 020 / E2b, Spec 021 S4) ───────────────────────

export interface RemoveFacilityArgs {
  facilityId: string;
  /** Skip releasing the board's claimed slots: delete the facility and channels only. */
  leaveBoard?: boolean;
  /** The claims the user confirmed in the preview; a different plan now is refused. */
  previewedClaims?: readonly FacilityRemovalClaim[];
}

export type RemoveFacilityResult =
  | { ok: true }
  | { ok: false; issues: ComposeIssue[] }
  | { ok: false; error: unknown }
  | { ok: false; planChanged: true };

export type FacilityRemovalPreview =
  | { ok: true; claims: FacilityRemovalClaim[]; issues: ComposeIssue[] }
  | { ok: false; error: unknown };

/** Thrown when a removal is refused or fails and the caller wants an exception. */
export class FacilityRemovalError extends Error {
  readonly issues?: ComposeIssue[];
  readonly cause?: unknown;
  constructor(result: Exclude<RemoveFacilityResult, { ok: true }>) {
    super('Could not remove the facility');
    this.name = 'FacilityRemovalError';
    if ('issues' in result) this.issues = result.issues;
    if ('error' in result) this.cause = result.error;
  }
}

class RemovalPlanChangedError extends Error {}

function claimKey(claim: FacilityRemovalClaim): string {
  return `${claim.nodeKey}|${claim.kind}|${claim.index}`;
}

function sameClaims(a: readonly FacilityRemovalClaim[], b: readonly FacilityRemovalClaim[]): boolean {
  const keys = new Set(a.map(claimKey));
  return keys.size === new Set(b.map(claimKey)).size && b.every((c) => keys.has(claimKey(c)));
}

/**
 * Read-only: what removing the facility would release. Mirrors drafts so the
 * backend sees unsaved edits, then composes; stages nothing.
 */
export async function previewFacilityRemoval(facilityId: string): Promise<FacilityRemovalPreview> {
  try {
    await syncDraftsForComposition();
    const { claims, issues } = await composeFacilityRemoval(facilityId);
    return { ok: true, claims, issues };
  } catch (error) {
    return { ok: false, error };
  }
}

/**
 * Workflow owner: remove a facility, the only kind router. A facility that
 * claims board slots (Panel or Automatic Block Signals) releases them as
 * ordinary configuration drafts (unless `leaveBoard`), refuses on any issue or
 * when the plan differs from the previewed claims, and deletes the facility
 * and its user-owned channels. Any other facility keeps the generic path: tear
 * down its composed bowties, then delete it and its user-owned channels.
 *
 * Ordering is the rollback strategy: the only step that can throw
 * (`stageComposedEdits`) runs first, so a failure restores the prior drafts and
 * the facility/channels were never touched. The deletes that follow are
 * non-throwing store transitions.
 */
export async function removeFacility(args: RemoveFacilityArgs): Promise<RemoveFacilityResult> {
  const { facilityId, leaveBoard = false, previewedClaims } = args;
  const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
  if (!facility) return { ok: false, error: new UnknownReferenceError('facility') };

  if (!facility.panel && !facility.blockSignals) {
    try {
      const userOwnedChannelIds = userOwnedChannelIdsOf(facilityId);
      deleteFacilityAndChannels(facilityId, userOwnedChannelIds);
      return { ok: true };
    } catch (error) {
      return { ok: false, error };
    }
  }

  const result = await runStagingTransaction<ComposeIssue, FacilityRemovalResponse>({
    sync: !leaveBoard,
    compose: async () => {
      if (leaveBoard) {
        return { issues: [], edits: [], logicPlacement: [], claims: [] };
      }
      const composed = await composeFacilityRemoval(facilityId);
      if (previewedClaims && !sameClaims(previewedClaims, composed.claims)) throw new RemovalPlanChangedError();
      return composed;
    },
    stage: (tx, composed) => tx.configEdits(composed.edits, facilityId),
    // After the delete, which already drops the facility's own groups: only a board
    // whose remaining groups differ from that (another facility's group moved) stages a delta.
    commit: (composed) => {
      deleteFacilityAndChannels(facilityId, userOwnedChannelIdsOf(facilityId));
      stageLogicPlacement(composed.logicPlacement);
    },
  });
  if (!result.ok && 'error' in result && result.error instanceof RemovalPlanChangedError) {
    return { ok: false, planChanged: true };
  }
  return result;
}

// ── Panel creation (Spec 021) ────────────────────────────────────────────

const PANEL_TEMPLATE_ID = 'panel';
const PANEL_LAMPS_SLOT = 'lamps';

export interface CreatePanelFacilityArgs {
  /** ID minted by the editor; the compose request and the facility share it. */
  facilityId: string;
  name: string;
  /** The panel's entries in order, without lamp channel IDs (assigned here). */
  record: PanelRecord;
  /** One per entry that has a lamp. */
  lamps: { entryId: string; nodeKey: string; output: number; name: string }[];
  /** Untracked owners the user chose to release so their pins can be used. */
  release?: ReleasedOwner[];
}

export type CreatePanelFacilityResult =
  | { ok: true }
  | { ok: false; issues: ComposePanelIssue[] }
  | { ok: false; error: unknown };

/**
 * Workflow owner: create a Panel facility. Composes the lamp wiring with
 * fresh facts, refuses on any issue with nothing staged, then stages one
 * user-owned lamp channel per assigned row, the facility with its record and
 * lamp attachments, and the configuration edits as ordinary drafts. Any
 * throw restores everything staged here. Never composes bowties.
 */
export async function createPanelFacility(
  args: CreatePanelFacilityArgs,
): Promise<CreatePanelFacilityResult> {
  const { facilityId, name, record, lamps, release } = args;
  const template = behaviorTemplatesStore.findByTemplateId(PANEL_TEMPLATE_ID);
  if (!template) return { ok: false, error: new UnknownReferenceError('template') };

  return runStagingTransaction({
    compose: () => composePanel({ facilityId, record, lamps, ...(release?.length ? { release } : {}) }),
    stage: (tx, composed) => {
      const lampNames = new Map(lamps.map((l) => [l.entryId, l.name]));
      const channelIdByEntry = new Map<string, string>();
      for (const assignment of composed.assignments) {
        const channel = tx.createUserOwnedChannel({
          role: 'lamp-indicator',
          style: 'single-led-direct-lamp',
          binding: { kind: 'lampRow', nodeKey: assignment.nodeKey, rowOrdinal: assignment.row },
          name: lampNames.get(assignment.entryId) ?? '',
        });
        channelIdByEntry.set(assignment.entryId, channel.id);
      }

      tx.addFacility(template, name, facilityId);
      tx.setPanelRecord(facilityId, {
        ...record,
        entries: record.entries.map((entry) => {
          const lampChannelId = channelIdByEntry.get(entry.entryId);
          return lampChannelId ? { ...entry, lampChannelId } : entry;
        }),
      });
      for (const channelId of channelIdByEntry.values()) {
        tx.attachChannel(facilityId, PANEL_LAMPS_SLOT, channelId);
      }
      tx.configEdits(composed.edits, facilityId);
    },
  });
}

export type UpdatePanelFacilityArgs = CreatePanelFacilityArgs;
export type UpdatePanelFacilityResult = CreatePanelFacilityResult;

/**
 * Workflow owner: update a saved Panel facility. Same pipeline as create; the
 * facility keeps its ID and the lamp channels of lamps that stay (renamed, and
 * rebound only when the board or row changed), new lamps get channels, and in
 * the commit phase lamps the edit dropped lose their channel. The record, the
 * attaches and the detaches all land in this one synchronous pass, so the
 * record's lamp channel IDs equal the `lamps` slot set when the batch ends.
 * Dropped-row resets come back from the compose as part of the edits.
 */
export async function updatePanelFacility(
  args: UpdatePanelFacilityArgs,
): Promise<UpdatePanelFacilityResult> {
  const { facilityId, name, record, lamps, release } = args;
  const priorFacility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
  if (!priorFacility?.panel) return { ok: false, error: new UnknownReferenceError('facility') };
  const savedLampByEntry = new Map(
    priorFacility.panel.entries.flatMap((e) => (e.lampChannelId ? [[e.entryId, e.lampChannelId] as const] : [])),
  );

  return runStagingTransaction({
    compose: () => composePanel({ facilityId, record, lamps, ...(release?.length ? { release } : {}) }),
    stage: (tx, composed) => {
      const lampNames = new Map(lamps.map((l) => [l.entryId, l.name]));
      const channelIdByEntry = new Map<string, string>();
      const createdChannelIds: string[] = [];
      for (const assignment of composed.assignments) {
        const lampName = lampNames.get(assignment.entryId) ?? '';
        const binding = { kind: 'lampRow' as const, nodeKey: assignment.nodeKey, rowOrdinal: assignment.row };
        const savedChannelId = savedLampByEntry.get(assignment.entryId);
        const saved = savedChannelId ? channelsStore.channels.find((c) => c.id === savedChannelId) : undefined;
        if (savedChannelId && saved) {
          if (saved.name !== lampName) tx.renameChannel(savedChannelId, lampName);
          const bound = saved.binding;
          if (bound.kind !== 'lampRow' || bound.nodeKey !== binding.nodeKey || bound.rowOrdinal !== binding.rowOrdinal) {
            tx.rebindChannel(savedChannelId, binding);
          }
          channelIdByEntry.set(assignment.entryId, savedChannelId);
        } else {
          const channel = tx.createUserOwnedChannel({
            role: 'lamp-indicator',
            style: 'single-led-direct-lamp',
            binding,
            name: lampName,
          });
          createdChannelIds.push(channel.id);
          channelIdByEntry.set(assignment.entryId, channel.id);
        }
      }

      tx.setPanelRecord(facilityId, {
        ...record,
        entries: record.entries.map((entry) => {
          const lampChannelId = channelIdByEntry.get(entry.entryId);
          return lampChannelId ? { ...entry, lampChannelId } : entry;
        }),
      });
      if (priorFacility.name !== name) tx.renameFacility(facilityId, name);
      for (const channelId of createdChannelIds) {
        tx.attachChannel(facilityId, PANEL_LAMPS_SLOT, channelId);
      }
      tx.configEdits(composed.edits, facilityId);
    },
    // Lamps the edit dropped lose their channel, after the last throwing step;
    // Discard restores both through the draft layer (ADR-0012).
    commit: (composed) => {
      const keptEntryIds = new Set(composed.assignments.map((a) => a.entryId));
      for (const [entryId, channelId] of savedLampByEntry) {
        if (keptEntryIds.has(entryId)) continue;
        facilitiesStore.detachChannel(facilityId, PANEL_LAMPS_SLOT, channelId);
        channelsStore.removeUserOwnedChannel(channelId);
      }
    },
  });
}

// ── Staging transaction (ADR-0012: one staging pipeline) ─────────────────

/**
 * Store mutations for one facility-staging transaction. Each method applies the
 * mutation and records its own inverse at that moment, so a throw anywhere
 * later replays exactly what was staged, in reverse. Throwing store
 * transitions only: deletes belong in the transaction's `commit` phase.
 */
interface StagingTransaction {
  createUserOwnedChannel(input: Parameters<typeof channelsStore.createUserOwnedChannel>[0]): InformationChannel;
  renameChannel(channelId: string, name: string): void;
  rebindChannel(channelId: string, binding: InformationChannel['binding']): void;
  addFacility(...args: Parameters<typeof facilitiesStore.addFacility>): Facility;
  setBlockSignalsRecord(facilityId: string, record: BlockSignalsRecord): void;
  setPanelRecord(facilityId: string, record: PanelRecord): void;
  renameFacility(facilityId: string, name: string): void;
  /** Throws when the store refuses the attach, so the transaction rolls back. */
  attachChannel(facilityId: string, slotLabel: string, channelId: string): void;
  logicPlacement(boards: readonly NodeLogicPlacement[]): void;
  configEdits(edits: readonly RemovalEdit[], facilityId: string): void;
}

function beginStagingTransaction(): { tx: StagingTransaction; rollback: () => void } {
  const inverses: (() => void)[] = [];
  const tx: StagingTransaction = {
    createUserOwnedChannel(input) {
      const channel = channelsStore.createUserOwnedChannel(input);
      inverses.push(() => channelsStore.removeUserOwnedChannel(channel.id));
      return channel;
    },
    renameChannel(channelId, name) {
      const prior = channelsStore.channels.find((c) => c.id === channelId);
      channelsStore.renameChannel(channelId, name);
      if (prior) inverses.push(() => channelsStore.renameChannel(channelId, prior.name));
    },
    rebindChannel(channelId, binding) {
      const prior = channelsStore.channels.find((c) => c.id === channelId);
      channelsStore.rebindChannel(channelId, binding);
      if (prior) inverses.push(() => channelsStore.rebindChannel(channelId, prior.binding));
    },
    addFacility(...args) {
      const facility = facilitiesStore.addFacility(...args);
      inverses.push(() => facilitiesStore.deleteFacility(facility.facilityId));
      return facility;
    },
    setBlockSignalsRecord(facilityId, record) {
      const prior = facilitiesStore.facilities.find((f) => f.facilityId === facilityId)?.blockSignals;
      facilitiesStore.setBlockSignalsRecord(facilityId, record);
      if (prior) inverses.push(() => facilitiesStore.setBlockSignalsRecord(facilityId, prior));
    },
    setPanelRecord(facilityId, record) {
      const prior = facilitiesStore.facilities.find((f) => f.facilityId === facilityId)?.panel;
      facilitiesStore.setPanelRecord(facilityId, record);
      if (prior) inverses.push(() => facilitiesStore.setPanelRecord(facilityId, prior));
    },
    renameFacility(facilityId, name) {
      const prior = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
      facilitiesStore.renameFacility(facilityId, name);
      if (prior) inverses.push(() => facilitiesStore.renameFacility(facilityId, prior.name));
    },
    attachChannel(facilityId, slotLabel, channelId) {
      if (!facilitiesStore.attachChannel(facilityId, slotLabel, channelId)) {
        throw new Error(`Failed to attach channel '${channelId}' to facility '${facilityId}'`);
      }
      inverses.push(() => facilitiesStore.detachChannel(facilityId, slotLabel, channelId));
    },
    logicPlacement(boards) {
      const prior = placementOf(boards);
      inverses.push(() => stageLogicPlacement(prior));
      stageLogicPlacement(boards);
    },
    configEdits(edits, facilityId) {
      const priorDrafts = new Map(configChangesStore.draftEntries().map((d) => [d.key, d]));
      // Recorded before staging: a throw part-way through still restores the leaves staged so far.
      inverses.push(() => restoreConfigDrafts(edits, priorDrafts));
      stageComposedEdits(edits, facilityId);
    },
  };
  return {
    tx,
    rollback: () => {
      for (const inverse of inverses.reverse()) inverse();
    },
  };
}

/**
 * One staging pipeline for compiled facilities. Mirrors drafts and composes
 * with fresh facts (skipped when `sync` is false), refuses on any issue with
 * nothing staged, runs `stage` through the transaction's recorded-inverse
 * mutations (any throw replays them in reverse), then runs the non-throwing
 * `commit` phase (deletes) after the last throwing step.
 *
 * Each facility kind supplies its own typed `stage` and `commit`; the record
 * setters and channel plans stay per-kind.
 */
async function runStagingTransaction<I, C extends { issues: I[] }>(plan: {
  sync?: boolean;
  compose: () => Promise<C>;
  stage: (tx: StagingTransaction, composed: C) => void;
  commit?: (composed: C) => void;
}): Promise<{ ok: true } | { ok: false; issues: I[] } | { ok: false; error: unknown }> {
  let composed: C;
  try {
    if (plan.sync ?? true) await syncDraftsForComposition();
    composed = await plan.compose();
  } catch (error) {
    return { ok: false, error };
  }
  if (composed.issues.length > 0) return { ok: false, issues: composed.issues };

  const { tx, rollback } = beginStagingTransaction();
  try {
    plan.stage(tx, composed);
  } catch (error) {
    rollback();
    return { ok: false, error };
  }
  plan.commit?.(composed);
  return { ok: true };
}

// ── Private helpers ─────────────────────────────────────────────────────

/** Stage each board's whole placement as the compiler planned it (a no-op where it matches). */
function stageLogicPlacement(boards: readonly NodeLogicPlacement[]): void {
  for (const board of boards) facilitiesStore.setLogicPlacement(board.nodeKey, board.groups);
}

/** The effective placement of the boards `planned` names (none left = empty groups), to restore on rollback. */
function placementOf(planned: readonly NodeLogicPlacement[]): NodeLogicPlacement[] {
  const current = facilitiesStore.logicPlacement;
  return planned.map(({ nodeKey }) => ({
    nodeKey,
    groups: current.find((board) => board.nodeKey === nodeKey)?.groups ?? [],
  }));
}

/**
 * A concrete event ID value that no leaf of the node's tree (including staged
 * drafts) holds yet, or null when the generator yields no valid ID. Staging the
 * returned value before the next call keeps successive IDs distinct.
 */
function freshEventIdValue(nodeKey: string, tree: NodeConfigTree): TreeConfigValue | null {
  const bytes = hexToBytes(generateFreshEventIdForNode(nodeKey, tree));
  return bytes ? { type: 'eventId', bytes, hex: canonicalEventIdHex(bytes) } : null;
}

/**
 * Stage composed edits (a facility's recipe, a release or a removal), replacing each `freshEventId` request with a generated
 * event ID. Every needed node tree is checked first so a missing one throws
 * before anything is staged; each ID is staged before the next is generated.
 */
function stageComposedEdits(edits: readonly RemovalEdit[], facilityId: string): void {
  for (const edit of edits) {
    if (edit.value.type === 'freshEventId' && !nodeTreeStore.getTree(edit.nodeKey)) {
      throw new Error(`Configuration for node ${edit.nodeKey} is not loaded`);
    }
  }
  for (const edit of edits) {
    let value: TreeConfigValue | null;
    if (edit.value.type === 'freshEventId') {
      value = freshEventIdValue(edit.nodeKey, nodeTreeStore.getTree(edit.nodeKey)!);
      if (!value) throw new Error(`Could not generate an event ID for node ${edit.nodeKey}`);
    } else {
      value = edit.value;
    }
    configEditor.applyEdit(editKeyForLeaf(edit.nodeKey, edit.space, edit.address), value, { facilityId });
  }
}

/** Put each edited leaf's draft back to its prior value (or drop it if it had none). */
function restoreConfigDrafts(
  edits: readonly { nodeKey: string; space: number; address: number }[],
  priorDrafts: ReadonlyMap<string, ConfigDraftEntry>,
): void {
  for (const edit of edits) {
    const key = editKeyForLeaf(edit.nodeKey, edit.space, edit.address);
    const prior = priorDrafts.get(key);
    if (prior) configChangesStore.set(key, prior.value, prior.origin);
    else configChangesStore.revert(key);
  }
}

/** Stage configuration edits as ordinary drafts so they ride the existing save path. */
function applyConfigEdits(
  edits: readonly { nodeKey: string; space: number; address: number; value: TreeConfigValue; facilityId: string }[],
): void {
  for (const edit of edits) {
    configEditor.applyEdit(
      editKeyForLeaf(edit.nodeKey, edit.space, edit.address),
      edit.value,
      { facilityId: edit.facilityId },
    );
  }
}

function hexToBytes(hex: string): number[] | null {
  const cleaned = hex.replace(/\./g, '');
  if (cleaned.length !== 16) return null;
  const bytes: number[] = [];
  for (let i = 0; i < cleaned.length; i += 2) {
    const byte = parseInt(cleaned.substring(i, i + 2), 16);
    if (Number.isNaN(byte)) return null;
    bytes.push(byte);
  }
  return bytes;
}
