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
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import { composeFacilityBowties, type CompositionOp } from '$lib/api/facilityBowties';
import { syncLayoutDrafts } from '$lib/api/layout';
import { canonicalEventIdHex } from '$lib/utils/serialize';
import { editKeyForLeaf } from '$lib/utils/editKey';
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
async function syncDraftsForComposition(): Promise<void> {
  const deltas = [
    ...facilitiesStore.collectDeltas(),
    ...channelsStore.collectDeltas(),
  ];
  await syncLayoutDrafts(deltas);
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
      const freshHex = generateFreshEventIdForNode(op.consumerNodeKey, tree);
      const bytes = hexToBytes(freshHex);
      if (!bytes) continue;
      configEditor.applyEdit(
        editKeyForLeaf(op.consumerNodeKey, leaf.space, leaf.address),
        { type: 'eventId', bytes, hex: canonicalEventIdHex(bytes) },
      );
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
      const freshHex = generateFreshEventIdForNode(nodeKey, tree);
      const bytes = hexToBytes(freshHex);
      if (!bytes) continue;
      configEditor.applyEdit(
        editKeyForLeaf(nodeKey, leaf.space, leaf.address),
        { type: 'eventId', bytes, hex: canonicalEventIdHex(bytes) },
      );
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
 * Delete the facility, first tearing down its composed bowties. Called
 * by the facility card's Delete action; the S5 removeFromSlot flow
 * still runs for the user-owned bound channels via the store's own
 * cleanup on facility deletion.
 */
export async function deleteFacility(facilityId: string): Promise<void> {
  await tearDownFacilityBowties(facilityId);
  facilitiesStore.deleteFacility(facilityId);
}

// ── Private helpers ─────────────────────────────────────────────────────

function applyCompositionOps(ops: readonly CompositionOp[]): void {
  for (const op of ops) {
    const bytes = op.eventIdBytes.slice();
    const hex = canonicalEventIdHex(bytes);
    // Dispatch the consumer leaf write via the config editor so the
    // draft flows through the existing save path.
    configEditor.applyEdit(
      editKeyForLeaf(op.consumerNodeKey, op.consumerLeafSpace, op.consumerLeafAddress),
      { type: 'eventId', bytes, hex },
    );
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
