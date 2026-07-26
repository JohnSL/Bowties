/**
 * Facility reference utilities — find upstream facilities bound to a target facility.
 * Spec 020 / S6 — deletion warning detection.
 */

import type { FacilityRecord } from '$lib/api/facilities';

/**
 * Find all upstream facilities whose downstream-signal input is bound to any
 * of the target facility's output channels.
 *
 * Used in S6 deletion to warn about affected upstream signals before reclaiming
 * resources. Returns an empty array if no upstream facilities reference the target.
 *
 * Channel ownership is derived from `slotBindings['output']` on each facility;
 * no separate channel inventory lookup is needed.
 *
 * @param facilityId - The target facility ID (the one being deleted)
 * @param facilities - Complete facility inventory
 * @returns Array of facilities whose downstream-signal input is bound to a channel owned by facilityId
 */
export function findUpstreamReferrers(
  facilityId: string,
  facilities: FacilityRecord[],
): FacilityRecord[] {
  // Find all channels owned (output slot) by the target facility.
  const targetChannelIds = new Set<string>();
  const targetFacility = facilities.find((f) => f.facilityId === facilityId);
  if (!targetFacility) {
    return [];
  }

  // Collect channels in the target facility's "output" slot.
  const outputChannels = targetFacility.slotBindings['output'] ?? [];
  for (const channelId of outputChannels) {
    targetChannelIds.add(channelId);
  }

  if (targetChannelIds.size === 0) {
    return [];
  }

  // Scan all other facilities for downstream-signal bindings pointing at our channels.
  const referrers: FacilityRecord[] = [];
  for (const facility of facilities) {
    // Skip the target facility itself (self-reference).
    if (facility.facilityId === facilityId) {
      continue;
    }

    // Check if this facility's downstream-signal slot is bound to any of the target's channels.
    const downstreamChannels = facility.slotBindings['downstream-signal'] ?? [];
    for (const channelId of downstreamChannels) {
      if (targetChannelIds.has(channelId)) {
        referrers.push(facility);
        break; // Don't add the same facility twice.
      }
    }
  }

  return referrers;
}
