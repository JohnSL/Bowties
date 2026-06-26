/**
 * Channel state derivation — pure function.
 *
 * Given a channel's resolved event IDs and the event state store,
 * determines whether the channel is occupied, clear, unknown, or
 * has no resolvable configuration.
 */

export type OccupancyState = 'no-config' | 'unknown' | 'occupied' | 'clear';

/**
 * Derive the occupancy state for a single channel by comparing
 * the last-seen timestamps of its occupied and clear event IDs.
 *
 * - If neither event ID is resolvable → 'no-config' (Spec 017 / S2)
 *   The channel's config is missing or partial — distinct from "we know what
 *   to listen for but nothing has happened yet."
 * - If both event IDs are known but neither event has been seen → 'unknown'
 * - If only occupied seen → 'occupied'
 * - If only clear seen → 'clear'
 * - If both seen, most recent wins
 */
export function deriveChannelState(
  events: ReadonlyMap<string, number>,
  occupiedEventId: string | undefined,
  clearEventId: string | undefined,
): OccupancyState {
  if (!occupiedEventId && !clearEventId) return 'no-config';

  const occupiedTs = occupiedEventId ? events.get(occupiedEventId) : undefined;
  const clearTs = clearEventId ? events.get(clearEventId) : undefined;

  if (occupiedTs == null && clearTs == null) return 'unknown';
  if (occupiedTs != null && clearTs == null) return 'occupied';
  if (clearTs != null && occupiedTs == null) return 'clear';

  // Both seen — most recent wins
  return occupiedTs! > clearTs! ? 'occupied' : 'clear';
}
