/**
 * Facility orchestrator — workflow owner for Slot Binding (Spec 018 / S4 — D3).
 *
 * Owns the multi-step Select-channel / Add-channel / Remove-from-slot
 * workflows. Components (FacilitySlot, SelectChannelPicker) emit
 * intent; this module translates that intent into deterministic store
 * mutations + role-match validation, never reaching the IPC layer
 * directly (all persistence flows through the save orchestrator via
 * `facilitiesStore.collectDeltas()`). Rebind was retired in S6
 * (2026-07-01) — changing a slot's channel is now Remove + Select/Add.
 *
 * Boundary: orchestration layer per `code-placement-and-ownership.md`.
 * No DOM, no IPC; pure store coordination + validation.
 */

import type { ChannelRole, InformationChannel } from '$lib/api/channels';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configEditor } from '$lib/stores/configEditor.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import { composeFacilityBowties, type CompositionOp } from '$lib/api/facilityBowties';
import { syncLayoutDrafts } from '$lib/api/layout';
import { canonicalEventIdHex } from '$lib/utils/serialize';
import { editKeyForLeaf, parseEditKey } from '$lib/utils/editKey';
import {
  composeBlockSignals,
  composeBlockSignalsRemoval,
  type BlockSignalsPreviewRequest,
  type ComposeIssue,
  type ReleasedOwner,
  type RemovalEdit,
  type SignalWiring,
} from '$lib/api/signaling';
import type { BlockSignalsRecord, NodeLogicPlacement } from '$lib/api/facilities';
import type { NodeConfigTree, TreeConfigValue } from '$lib/types/nodeTree';
import { generateFreshEventIdForNode } from '$lib/utils/eventIds';
import { collectEventIdLeaves, findLeafByPath } from '$lib/types/nodeTree';

/** Thrown when the slot's required role does not match the chosen channel. */
export class RoleMismatchError extends Error {
  constructor(
    public readonly expected: ChannelRole,
    public readonly actual: ChannelRole,
  ) {
    super(`Slot requires role '${expected}' but channel has role '${actual}'`);
    this.name = 'RoleMismatchError';
  }
}

/** Thrown when the referenced facility / slot / channel / template is unknown. */
export class UnknownReferenceError extends Error {
  constructor(public readonly what: 'facility' | 'slot' | 'channel' | 'template') {
    super(`Unknown ${what}`);
    this.name = 'UnknownReferenceError';
  }
}

/**
 * Thrown when an attach would take a slot past its template
 * `maxChannels` cap (S4 D8). Post-Rebind-retirement (S6 D4) this is
 * the guard rail that stops a user from stacking channels into a
 * max=1 slot — Remove-from-slot first, then Select/Add.
 */
export class SlotAtMaxError extends Error {
  constructor(
    public readonly slotLabel: string,
    public readonly maxChannels: number,
  ) {
    super(`Slot '${slotLabel}' is at its ${maxChannels}-channel cap`);
    this.name = 'SlotAtMaxError';
  }
}

export interface SelectChannelForSlotArgs {
  facilityId: string;
  slotLabel: string;
  channelId: string;
}

export interface RemoveFromSlotArgs {
  facilityId: string;
  slotLabel: string;
  channelId: string;
}

/**
 * Attach a channel to a facility slot.
 *
 * The store's cardinality guard (S4 D8: `max_channels` in the template)
 * rejects an attach into an already-filled slot; the picker is the
 * discoverability layer that prevents that call in the UI. Rebind was
 * retired in S6 (2026-07-01) in favour of a two-step Remove-from-slot +
 * Select/Add sequence — see slice 018-S6 D4.
 */
export function selectChannelForSlot(args: SelectChannelForSlotArgs): Promise<void> {
  const { facilityId, slotLabel, channelId } = args;

  const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
  if (!facility) throw new UnknownReferenceError('facility');

  const template = behaviorTemplatesStore.findByTemplateId(facility.templateId);
  if (!template) throw new UnknownReferenceError('template');

  const slot = template.slots.find((s) => s.label === slotLabel);
  if (!slot) throw new UnknownReferenceError('slot');

  const channel: InformationChannel | undefined = channelsStore.channels.find(
    (c) => c.id === channelId,
  );
  if (!channel) throw new UnknownReferenceError('channel');

  if (channel.role !== slot.requiredRole) {
    throw new RoleMismatchError(slot.requiredRole as ChannelRole, channel.role);
  }

  // Post-Rebind-retirement (S6 D4) cardinality guard: block second
  // attach into a max=1 slot so callers must Remove-from-slot first.
  // Skip when the channel is already attached (harmless no-op).
  const currentBindings = facility.slotBindings[slotLabel] ?? [];
  const alreadyAttached = currentBindings.includes(channelId);
  if (
    !alreadyAttached &&
    slot.maxChannels !== null &&
    currentBindings.length >= slot.maxChannels
  ) {
    throw new SlotAtMaxError(slotLabel, slot.maxChannels);
  }

  facilitiesStore.attachChannel(facilityId, slotLabel, channelId);

  // Spec 018 / S6 (D2) — compose-on-Wired: cheap no-op when still Incomplete.
  return composeBowtiesIfWired(facilityId);
}

/**
 * Remove the given channel from the specified slot. No-op if the
 * channel was not bound there. The facility itself remains; only the
 * slot binding is cleared.
 *
 * Spec 018 / S5 — user-owned-channel lifecycle: when the bound channel
 * has `ownership === 'user-owned'`, the orchestrator ALSO deletes the
 * channel after detaching (its lifecycle is its binding; the lamp row
 * becomes unclaimed and re-eligible for the next Add channel). For
 * hardware-owned channels the channel persists (its lifecycle follows
 * the hardware-config selection, per FR-018).
 */
export async function removeFromSlot(args: RemoveFromSlotArgs): Promise<void> {
  const { facilityId, slotLabel, channelId } = args;

  // Spec 018 / S6 (T13) — teardown BEFORE detach so the composer sees the
  // still-Wired shape and can re-derive the consumer leaves to overwrite.
  await tearDownFacilityBowties(facilityId);

  facilitiesStore.detachChannel(facilityId, slotLabel, channelId);

  const channel = channelsStore.channels.find((c) => c.id === channelId);
  if (channel?.ownership === 'user-owned') {
    channelsStore.removeUserOwnedChannel(channelId);
  }
}

// ── Add-channel flow (Spec 018 / S5 — atomic create + claim + bind) ──────

export interface AddChannelForSlotArgs {
  facilityId: string;
  slotLabel: string;
  /** Node hosting the lamp row that the new channel claims. */
  lampRowNodeKey: string;
  /** 1-based ordinal of the Direct Lamp Control / Lamp#N row. */
  rowOrdinal: number;
  /** Optional override; default is `"${facility.name} ${slotLabel}"`. */
  name?: string;
}

export interface AddChannelForSlotResult {
  channelId: string;
}

/**
 * Spec 018 / S5 — atomic Add channel for a consumer slot.
 *
 * Validates the slot role is `lamp-indicator`, derives a default name,
 * creates a user-owned `single-led-direct-lamp` channel via the
 * channels store's new draft bucket, and immediately attaches it to
 * the slot. If attach fails (e.g. cardinality), the just-created
 * draft is rolled back so neither store carries half a transaction.
 *
 * Both mutations land in the same draft set; at save time they travel
 * as a `createChannel` delta paired with an `attachChannelToSlot`
 * delta, applied atomically inside `save_layout_directory` (ADR-0002).
 */
export async function addChannelForSlot(
  args: AddChannelForSlotArgs,
): Promise<AddChannelForSlotResult> {
  const { facilityId, slotLabel, lampRowNodeKey, rowOrdinal, name } = args;

  const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
  if (!facility) throw new UnknownReferenceError('facility');

  const template = behaviorTemplatesStore.findByTemplateId(facility.templateId);
  if (!template) throw new UnknownReferenceError('template');

  const slot = template.slots.find((s) => s.label === slotLabel);
  if (!slot) throw new UnknownReferenceError('slot');

  if (slot.requiredRole !== 'lamp-indicator') {
    throw new RoleMismatchError('lamp-indicator', slot.requiredRole as ChannelRole);
  }

  // Post-Rebind-retirement cardinality guard: fail early before
  // creating the draft channel so no roll-back is needed.
  const currentBindings = facility.slotBindings[slotLabel] ?? [];
  if (slot.maxChannels !== null && currentBindings.length >= slot.maxChannels) {
    throw new SlotAtMaxError(slotLabel, slot.maxChannels);
  }

  const channel = channelsStore.createUserOwnedChannel({
    role: 'lamp-indicator',
    style: 'single-led-direct-lamp',
    binding: { kind: 'lampRow', nodeKey: lampRowNodeKey, rowOrdinal },
    name: name ?? `${facility.name} ${slotLabel}`,
  });

  const attached = facilitiesStore.attachChannel(facilityId, slotLabel, channel.id);
  if (!attached) {
    // attachChannel returns false only on no-op suppression; for a
    // freshly-created id that should never trigger. Treat any false
    // here as a roll-back signal anyway so the contract is "either
    // both mutations stick or neither does".
    channelsStore.removeUserOwnedChannel(channel.id);
    throw new Error(
      `Failed to attach channel '${channel.id}' to slot '${slotLabel}' on facility '${facilityId}'`,
    );
  }

  // Spec 018 / S6 (D2) — compose-on-Wired hook.
  await composeBowtiesIfWired(facilityId);

  return { channelId: channel.id };
}

// ── Facility bowtie composition + teardown (Spec 018 / S6 — D2 + T13) ────

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

/**
 * Compose the facility's bowties into the draft layer when the facility
 * has become Wired. No-op when `facilityStatus !== 'Wired'` — cheap to
 * call from every attach path since the guard runs before any IPC.
 *
 * On Wired, first mirrors the frontend draft state into
 * `LayoutState.drafts` (Spec 018 / S6 bugfix — the compose IPC reads
 * facility + channel data through the effective drafts-over-saved
 * view), then dispatches each returned [`CompositionOp`] via
 * `configEditor.applyEdit` (consumer leaf writes) and
 * `bowtieMetadataStore.createBowtie` (metadata rows with the
 * `createdByFacility` back-reference). All edits land in the draft
 * stores; the save flow picks them up unchanged.
 */
export async function composeBowtiesIfWired(facilityId: string): Promise<void> {
  if (effectiveLayoutStore.facilityStatus(facilityId) !== 'Wired') return;

  await syncDraftsForComposition();
  const ops = await composeFacilityBowties(facilityId);
  applyCompositionOps(ops);
}

/**
 * Reverse the composition side effects for a facility — the "inverse
 * of composition" primitive that every teardown caller shares.
 *
 * Composition writes to two places (bowtie metadata rows +
 * `EventID` consumer leaves via `configEditor`). Historically the
 * teardown reversal only worked when called on a still-Wired shape,
 * because the composer IPC was the only way to know which consumer
 * leaves belonged to the facility. Callers that reached teardown on
 * an already-Incomplete facility (`_cascadeDetach` after runtime
 * channel loss, and the 2026-07-03 load-time repair for ghost
 * bindings) skipped the leaf reset, so metadata was deleted but the
 * `EventID` leaves survived and the backend's CDI-scan catalog
 * re-produced the bowtie on the next open — a symmetry violation
 * that persisted through save+reopen.
 *
 * Consolidated reversal uses two lookup strategies:
 *
 *   1. Composer-forward — when the facility is still Wired the
 *      backend composer walks template → slots → channels → consumer
 *      leaves and returns exact leaf paths. Fast and precise.
 *   2. Metadata-driven fallback — for every event id hex whose
 *      `BowtieMetadata.createdByFacility === facilityId`, scan every
 *      loaded config tree for `EventID` leaves whose effective value
 *      equals that hex and reset each match. Slower but works when
 *      the facility structure is broken (empty slot, ghost binding).
 *
 * All resets stage as `configEditor.applyEdit` draft edits and ride
 * the standard save flow (ADR-0012 extension 2026-07-03).
 */
async function resetComposedLeavesForFacility(facilityId: string): Promise<void> {
  if (effectiveLayoutStore.facilityStatus(facilityId) === 'Wired') {
    await syncDraftsForComposition();
    const ops = await composeFacilityBowties(facilityId);
    for (const op of ops) {
      const tree = nodeTreeStore.getTree(op.consumerNodeKey);
      if (!tree) continue;
      const leaf = findLeafByPath(tree, op.consumerLeafPath);
      if (!leaf) continue;
      const fresh = freshEventIdValue(op.consumerNodeKey, tree);
      if (!fresh) continue;
      configEditor.applyEdit(editKeyForLeaf(op.consumerNodeKey, leaf.space, leaf.address), fresh);
    }
    return;
  }

  const targetHexes = new Set(bowtieMetadataStore.bowtiesForFacility(facilityId));
  if (targetHexes.size === 0) return;
  for (const [nodeKey, tree] of nodeTreeStore.trees) {
    for (const leaf of collectEventIdLeaves(tree)) {
      const value = effectiveLayoutStore.effectiveValue(nodeKey, leaf);
      if (!value || value.type !== 'eventId') continue;
      if (!targetHexes.has(value.hex)) continue;
      const fresh = freshEventIdValue(nodeKey, tree);
      if (!fresh) continue;
      configEditor.applyEdit(editKeyForLeaf(nodeKey, leaf.space, leaf.address), fresh);
    }
  }
}

/**
 * Reverse the composition side effects of a facility — the exact
 * inverse of `composeBowtiesIfWired`. Callable from every path that
 * needs to un-Wire a facility (user Remove-from-slot, runtime cascade
 * from hardware channel loss, load-time repair of a ghost binding,
 * facility delete). See `resetComposedLeavesForFacility` for the
 * two-strategy leaf-lookup contract that makes this safe regardless
 * of the facility's current Wired-ness.
 */
export async function tearDownFacilityBowties(facilityId: string): Promise<void> {
  await resetComposedLeavesForFacility(facilityId);
  for (const hex of bowtieMetadataStore.bowtiesForFacility(facilityId)) {
    bowtieMetadataStore.deleteBowtie(hex);
  }
}

/**
 * Delete the facility, first tearing down its composed bowties and
 * removing any user-owned channels that were bound to its slots.
 * Hardware-owned channels are left alone — their lifecycle follows
 * hardware config, not facility membership.
 *
 * Ordering matters: channel removal MUST happen AFTER facility deletion
 * so the cascade orchestrator (which watches `channelsStore.channels`)
 * cannot find slot bindings referencing the disappearing channel and
 * interfere with our teardown/delete sequence.
 */
export async function deleteFacility(facilityId: string): Promise<void> {
  // An Automatic Block Signals facility owns claimed board slots; deleting it
  // generically would strand them. Route through the release workflow (Spec
  // 020 / E2b) and surface a refusal as a typed error.
  const target = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
  if (target?.templateId === BLOCK_SIGNALS_TEMPLATE_ID && target.blockSignals) {
    const result = await removeBlockSignalsFacility({ facilityId });
    if (!result.ok) throw new BlockSignalsRemovalError(result);
    return;
  }

  const userOwnedChannelIds = userOwnedChannelIdsOf(facilityId);
  await tearDownFacilityBowties(facilityId);
  deleteFacilityAndChannels(facilityId, userOwnedChannelIds);
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

/** Facility first, then channels, so the cascade orchestrator finds no slot bindings. */
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

  let composed;
  try {
    await syncDraftsForComposition();
    composed = await composeBlockSignals({ facilityId, line, wiring, release });
  } catch (error) {
    return { ok: false, error };
  }
  if (composed.issues.length > 0) return { ok: false, issues: composed.issues };

  const priorDrafts = new Map(configChangesStore.draftEntries().map((d) => [d.key, d.value]));
  const priorPlacement = placementOf(composed.logicPlacement);
  const priorChannels = new Map(channelsStore.channels.map((c) => [c.id, c]));
  const createdChannelIds: string[] = [];
  let facilityAdded = false;
  try {
    const wiringBySignal = new Map(wiring.map((w) => [w.signalId, w]));
    const savedChannelIds = new Map(
      (priorFacility?.blockSignals?.signals ?? []).map((s) => [s.signalId, s.channelId]),
    );
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
        channelsStore.renameChannel(savedChannelId, signal.name);
        channelsStore.rebindChannel(savedChannelId, binding);
        return { ...signal, channelId: savedChannelId, resources };
      }
      const channel = channelsStore.createUserOwnedChannel({
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
      facilitiesStore.addFacility(template, name, facilityId, record);
      facilityAdded = true;
    } else {
      facilitiesStore.setBlockSignalsRecord(facilityId, record);
      facilitiesStore.renameFacility(facilityId, name);
    }
    for (const channelId of createdChannelIds) {
      if (!facilitiesStore.attachChannel(facilityId, BLOCK_SIGNALS_SLOT, channelId)) {
        throw new Error(`Failed to attach channel '${channelId}' to facility '${facilityId}'`);
      }
    }

    stageLogicPlacement(composed.logicPlacement);
    stageComposedEdits(composed.edits);
    return { ok: true };
  } catch (error) {
    restoreConfigDrafts(composed.edits, priorDrafts);
    if (facilityAdded) facilitiesStore.deleteFacility(facilityId);
    stageLogicPlacement(priorPlacement);
    for (const channelId of createdChannelIds) channelsStore.removeUserOwnedChannel(channelId);
    if (priorFacility?.blockSignals) {
      facilitiesStore.setBlockSignalsRecord(facilityId, priorFacility.blockSignals);
      facilitiesStore.renameFacility(facilityId, priorFacility.name);
      for (const s of priorFacility.blockSignals.signals) {
        const prior = priorChannels.get(s.channelId);
        if (!prior) continue;
        channelsStore.renameChannel(prior.id, prior.name);
        channelsStore.rebindChannel(prior.id, prior.binding);
      }
    }
    return { ok: false, error };
  }
}

// ── Automatic Block Signals removal (Spec 020 / E2b) ─────────────────────

export interface RemoveBlockSignalsFacilityArgs {
  facilityId: string;
  /** Skip releasing the board's mast and Logic: delete the facility and channels only. */
  leaveBoard?: boolean;
}

export type RemoveBlockSignalsFacilityResult = CreateBlockSignalsFacilityResult;

/** Thrown by the generic `deleteFacility` when an ABS release is refused or fails. */
export class BlockSignalsRemovalError extends Error {
  readonly issues?: ComposeIssue[];
  readonly cause?: unknown;
  constructor(result: { ok: false; issues: ComposeIssue[] } | { ok: false; error: unknown } | { ok: false }) {
    super('Could not remove the Automatic Block Signals facility');
    this.name = 'BlockSignalsRemovalError';
    if ('issues' in result) this.issues = result.issues;
    if ('error' in result) this.cause = result.error;
  }
}

/**
 * Workflow owner: remove an Automatic Block Signals facility, releasing its
 * mast and Logic as ordinary configuration drafts (unless
 * `leaveBoard`) and deleting the facility and its user-owned signal channels.
 *
 * Ordering is the rollback strategy: the only step that can throw
 * (`stageComposedEdits`) runs first, so a failure restores the prior drafts and
 * the facility/channels were never touched. The deletes that follow are
 * non-throwing store transitions. Refuses on any release issue with nothing
 * staged. Never tears down bowties (the template has no mapping).
 */
export async function removeBlockSignalsFacility(
  args: RemoveBlockSignalsFacilityArgs,
): Promise<RemoveBlockSignalsFacilityResult> {
  const { facilityId, leaveBoard = false } = args;
  const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId);
  if (!facility?.blockSignals) {
    return { ok: false, error: new UnknownReferenceError('facility') };
  }

  let edits: readonly RemovalEdit[] = [];
  let placement: NodeLogicPlacement[] = [];
  if (!leaveBoard) {
    try {
      await syncDraftsForComposition();
      const composed = await composeBlockSignalsRemoval(facilityId);
      if (composed.issues.length > 0) return { ok: false, issues: composed.issues };
      edits = composed.edits;
      placement = composed.logicPlacement;
    } catch (error) {
      return { ok: false, error };
    }
  }

  const priorDrafts = new Map(configChangesStore.draftEntries().map((d) => [d.key, d.value]));
  try {
    stageComposedEdits(edits);
  } catch (error) {
    restoreConfigDrafts(edits, priorDrafts);
    return { ok: false, error };
  }
  deleteFacilityAndChannels(facilityId, userOwnedChannelIdsOf(facilityId));
  // After the delete, which already drops the facility's own groups: only a board
  // whose remaining groups differ from that (another facility's group moved) stages a delta.
  stageLogicPlacement(placement);
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
function stageComposedEdits(edits: readonly RemovalEdit[]): void {
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
    configEditor.applyEdit(editKeyForLeaf(edit.nodeKey, edit.space, edit.address), value);
  }
}

/** Put each edited leaf's draft back to its prior value (or drop it if it had none). */
function restoreConfigDrafts(
  edits: readonly { nodeKey: string; space: number; address: number }[],
  priorDrafts: ReadonlyMap<string, TreeConfigValue>,
): void {
  for (const edit of edits) {
    const key = editKeyForLeaf(edit.nodeKey, edit.space, edit.address);
    const prior = priorDrafts.get(key);
    if (prior) configChangesStore.set(key, prior);
    else configChangesStore.revert(key);
  }
}

/** Stage configuration edits as ordinary drafts so they ride the existing save path. */
function applyConfigEdits(
  edits: readonly { nodeKey: string; space: number; address: number; value: TreeConfigValue }[],
): void {
  for (const edit of edits) {
    configEditor.applyEdit(editKeyForLeaf(edit.nodeKey, edit.space, edit.address), edit.value);
  }
}

function applyCompositionOps(ops: readonly CompositionOp[]): void {
  for (const op of ops) {
    const bytes = op.eventIdBytes.slice();
    const hex = canonicalEventIdHex(bytes);
    applyConfigEdits([{
      nodeKey: op.consumerNodeKey,
      space: op.consumerLeafSpace,
      address: op.consumerLeafAddress,
      value: { type: 'eventId', bytes, hex },
    }]);
    // Register the bowtie with the createdByFacility back-reference.
    bowtieMetadataStore.createBowtie(hex, op.bowtieName, {
      createdByFacility: op.createdByFacility,
    });
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
