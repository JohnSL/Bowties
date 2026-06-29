/**
 * Facility orchestrator — workflow owner for Slot Binding (Spec 018 / S4 — D3).
 *
 * Owns the multi-step Select-channel / Rebind / Remove-from-slot
 * workflows. Components (FacilitySlot, SelectChannelPicker) emit
 * intent; this module translates that intent into deterministic store
 * mutations + role-match validation, never reaching the IPC layer
 * directly (all persistence flows through the save orchestrator via
 * `facilitiesStore.collectDeltas()`).
 *
 * Boundary: orchestration layer per `code-placement-and-ownership.md`.
 * No DOM, no IPC; pure store coordination + validation.
 */

import type { ChannelRole, InformationChannel } from '$lib/api/channels';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';

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

export interface SelectChannelForSlotArgs {
  facilityId: string;
  slotLabel: string;
  channelId: string;
  mode: 'select' | 'rebind';
  /** Required when `mode === 'rebind'`: the channel to detach before attaching. */
  previousChannelId?: string;
}

export interface RemoveFromSlotArgs {
  facilityId: string;
  slotLabel: string;
  channelId: string;
}

/**
 * Select or rebind a channel into a facility slot.
 *
 * On `mode: 'rebind'` this is a two-step atomic sequence: detach the
 * previous channel, then attach the new one. If the role check fails
 * neither mutation runs (validation is up-front, before any store
 * write). On `mode: 'select'` only the attach runs.
 *
 * The store's no-op suppression (`attachChannel` returns false when
 * the channel is already attached) makes Rebind→same-channel a
 * harmless no-op pair, which keeps the picker's Confirm-disabled
 * heuristic the only thing preventing the call in the UI layer.
 */
export function selectChannelForSlot(args: SelectChannelForSlotArgs): void {
  const { facilityId, slotLabel, channelId, mode, previousChannelId } = args;

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

  if (mode === 'rebind') {
    if (!previousChannelId) {
      throw new Error('Rebind requires previousChannelId');
    }
    facilitiesStore.detachChannel(facilityId, slotLabel, previousChannelId);
  }
  facilitiesStore.attachChannel(facilityId, slotLabel, channelId);
}

/**
 * Remove the given channel from the specified slot. No-op if the
 * channel was not bound there. The facility itself remains; only the
 * slot binding is cleared (the channel persists because its
 * hardware-config still selects it, per FR-018).
 */
export function removeFromSlot(args: RemoveFromSlotArgs): void {
  const { facilityId, slotLabel, channelId } = args;
  facilitiesStore.detachChannel(facilityId, slotLabel, channelId);
}
