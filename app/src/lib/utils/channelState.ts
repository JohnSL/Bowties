/**
 * Channel state derivation — pure function.
 *
 * Spec 018 / S5 D3: `ChannelState` is a discriminated union over the
 * (no-config | unknown | {role; state}) space so that nonsensical pairings
 * (e.g. `{ role: 'lamp-indicator', state: 'occupied' }`) are structurally
 * unrepresentable.
 */

import type { ChannelRole } from '$lib/api/channels';

/**
 * Tag-discriminated union over the runtime state of a channel.
 *
 * - `no-config` — neither expected event id resolves.
 * - `unknown` — both ids known, no PCER observed yet.
 * - `{ role: 'block-occupancy', state: 'occupied' | 'clear' }` —
 *   producer-side block-detector channel.
 * - `{ role: 'lamp-indicator', state: 'lit' | 'unlit' }` —
 *   consumer-side direct-lamp channel; state derives from observed
 *   Lamp On / Lamp Off PCERs on the bus.
 */
export type ChannelState =
  | { kind: 'no-config' }
  | { kind: 'unknown' }
  | { role: 'block-occupancy'; state: 'occupied' | 'clear' }
  | { role: 'lamp-indicator'; state: 'lit' | 'unlit' };

const NO_CONFIG: ChannelState = { kind: 'no-config' };
const UNKNOWN: ChannelState = { kind: 'unknown' };

/**
 * Derive the current `ChannelState` for one channel.
 *
 * - If neither event id resolves → `{ kind: 'no-config' }`
 * - If both ids known but neither has been seen → `{ kind: 'unknown' }`
 * - Otherwise the most-recent observation wins; the resulting state literal
 *   is dispatched by `role` (occupied/clear for `block-occupancy`,
 *   lit/unlit for `lamp-indicator`).
 *
 * For `block-occupancy` callers the two ids are (occupied, clear).
 * For `lamp-indicator` callers the two ids are (lit, unlit).
 */
export function deriveChannelState(
  events: ReadonlyMap<string, number>,
  occupiedOrLitEventId: string | undefined,
  clearOrUnlitEventId: string | undefined,
  role: 'block-occupancy' | 'lamp-indicator',
): ChannelState {
  if (!occupiedOrLitEventId && !clearOrUnlitEventId) return NO_CONFIG;

  const litTs = occupiedOrLitEventId ? events.get(occupiedOrLitEventId) : undefined;
  const unlitTs = clearOrUnlitEventId ? events.get(clearOrUnlitEventId) : undefined;

  if (litTs == null && unlitTs == null) return UNKNOWN;

  // Tie + only-unlit + only-lit collapse to a single boolean: lit wins iff
  // its timestamp is strictly greater (matches the pre-S5 occupied/clear
  // tie-break behaviour exactly).
  const isLit = litTs != null && (unlitTs == null || litTs > unlitTs);

  if (role === 'lamp-indicator') {
    return { role: 'lamp-indicator', state: isLit ? 'lit' : 'unlit' };
  }
  return { role: 'block-occupancy', state: isLit ? 'occupied' : 'clear' };
}

/** Human-readable label for the state cell ("Occupied" / "Lit" / "Unknown" / "No config"). */
export function channelStateLabel(s: ChannelState): string {
  if ('kind' in s) return s.kind === 'no-config' ? 'No config' : 'Unknown';
  return s.state.charAt(0).toUpperCase() + s.state.slice(1);
}

/** CSS class name representing the state, for state-dot / row styling. */
export function channelStateClass(s: ChannelState): string {
  if ('kind' in s) return s.kind;
  return s.state;
}

/**
 * Map a `ChannelRole` onto the role discriminator the derivation function
 * accepts. Today `block-occupancy` and `lamp-indicator` are the only two
 * runtime-state-bearing roles.
 */
export function roleForChannelState(role: ChannelRole): 'block-occupancy' | 'lamp-indicator' {
  return role === 'lamp-indicator' ? 'lamp-indicator' : 'block-occupancy';
}
