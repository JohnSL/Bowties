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
 * - `{ role: 'signal-aspect', state: 'stop' | 'approach' | 'clear' | 'dark' }` —
 *   consumer-side 2-LED bicolor signal head; state derives from observed
 *   red/green Lamp On / Lamp Off PCERs.
 */
export type ChannelState =
  | { kind: 'no-config' }
  | { kind: 'unknown' }
  | { role: 'block-occupancy'; state: 'occupied' | 'clear' }
  | { role: 'lamp-indicator'; state: 'lit' | 'unlit' }
  | { role: 'signal-aspect'; state: 'stop' | 'approach' | 'clear' | 'dark' };

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
  if (s.role === 'signal-aspect') return `signal-${s.state}`;
  return s.state;
}

/**
 * Map a `ChannelRole` onto the role discriminator the derivation function
 * accepts. Today `block-occupancy`, `lamp-indicator`, and `signal-aspect`
 * are the runtime-state-bearing roles.
 */
export function roleForChannelState(
  role: ChannelRole,
): 'block-occupancy' | 'lamp-indicator' | 'signal-aspect' {
  if (role === 'lamp-indicator') return 'lamp-indicator';
  if (role === 'signal-aspect') return 'signal-aspect';
  return 'block-occupancy';
}

/**
 * Derive the current `ChannelState` for a signal-aspect channel from the
 * most-recently observed red/green LED On/Off events.
 *
 * Logic:
 * - If none of the 4 IDs are defined → `{ kind: 'no-config' }`
 * - If none of the 4 events have been observed → `{ kind: 'unknown' }`
 * - Determine red LED state: redOn more recent than redOff → red is on
 * - Determine green LED state: greenOn more recent than greenOff → green is on
 * - Map combination: (on,off)→stop, (on,on)→approach, (off,on)→clear, (off,off)→dark
 * - Edge case: if only some events seen, treat unseen LEDs as "off" (dark default).
 */
export function deriveSignalAspectState(
  events: ReadonlyMap<string, number>,
  redOnId: string | undefined,
  redOffId: string | undefined,
  greenOnId: string | undefined,
  greenOffId: string | undefined,
): ChannelState {
  if (!redOnId && !redOffId && !greenOnId && !greenOffId) return NO_CONFIG;

  const redOnTs = redOnId ? events.get(redOnId) : undefined;
  const redOffTs = redOffId ? events.get(redOffId) : undefined;
  const greenOnTs = greenOnId ? events.get(greenOnId) : undefined;
  const greenOffTs = greenOffId ? events.get(greenOffId) : undefined;

  if (redOnTs == null && redOffTs == null && greenOnTs == null && greenOffTs == null) {
    return UNKNOWN;
  }

  // Determine red LED state: on if redOn more recent than redOff (or only redOn seen).
  // Default to off if neither red event observed.
  const redIsOn = redOnTs != null && (redOffTs == null || redOnTs > redOffTs);

  // Determine green LED state: on if greenOn more recent than greenOff (or only greenOn seen).
  // Default to off if neither green event observed.
  const greenIsOn = greenOnTs != null && (greenOffTs == null || greenOnTs > greenOffTs);

  // Map LED combination to signal aspect
  if (redIsOn && !greenIsOn) return { role: 'signal-aspect', state: 'stop' };
  if (redIsOn && greenIsOn) return { role: 'signal-aspect', state: 'approach' };
  if (!redIsOn && greenIsOn) return { role: 'signal-aspect', state: 'clear' };
  return { role: 'signal-aspect', state: 'dark' };
}

/**
 * Wrap a rule-predicted aspect into a signal-aspect `ChannelState` literal.
 * Used by the FacilityCard's prediction-first output indicator path (Spec
 * 020 / S7) so a compiled facility's output shows a known aspect pre-Save,
 * driven by the same rule evaluation the Logic block already renders.
 * Falls back path: observation via `deriveSignalAspectState`.
 */
export function signalAspectStateFromPredictedAspect(
  aspect: 'stop' | 'approach' | 'clear' | 'dark',
): ChannelState {
  return { role: 'signal-aspect', state: aspect };
}

/**
 * Per-lamp LED state for a 2-LED bicolor signal.
 * Used by the facility comprehension view to show individual lamp On/Off state.
 */
export interface LedLampState {
  label: string;
  isOn: boolean;
  color: 'red' | 'green';
}

/**
 * Derive individual LED lamp states for a 2-LED bicolor signal channel.
 * Returns an array of per-lamp states (red LED, green LED) using the same
 * most-recent-wins logic as `deriveSignalAspectState`.
 */
export function deriveLedLampStates(
  events: ReadonlyMap<string, number>,
  redOnId: string | undefined,
  redOffId: string | undefined,
  greenOnId: string | undefined,
  greenOffId: string | undefined,
): LedLampState[] {
  const redOnTs = redOnId ? events.get(redOnId) : undefined;
  const redOffTs = redOffId ? events.get(redOffId) : undefined;
  const greenOnTs = greenOnId ? events.get(greenOnId) : undefined;
  const greenOffTs = greenOffId ? events.get(greenOffId) : undefined;

  const redIsOn = redOnTs != null && (redOffTs == null || redOnTs > redOffTs);
  const greenIsOn = greenOnTs != null && (greenOffTs == null || greenOnTs > greenOffTs);

  return [
    { label: 'Red', isOn: redIsOn, color: 'red' },
    { label: 'Green', isOn: greenIsOn, color: 'green' },
  ];
}

/**
 * Derive the two-LED (red, green) breakdown for a rule-predicted aspect.
 * Direct inverse of `deriveSignalAspectState`'s aspect ↔ (red, green)
 * truth table:
 *   stop     → red on,  green off
 *   approach → red on,  green on
 *   clear    → red off, green on
 *   dark     → red off, green off
 * Used by the FacilityCard's prediction-first output indicator path (Spec
 * 020 / S7). Fallback path: observation via `deriveLedLampStates`.
 */
export function ledLampStatesFromPredictedAspect(
  aspect: 'stop' | 'approach' | 'clear' | 'dark',
): LedLampState[] {
  const redOn = aspect === 'stop' || aspect === 'approach';
  const greenOn = aspect === 'approach' || aspect === 'clear';
  return [
    { label: 'Red', isOn: redOn, color: 'red' },
    { label: 'Green', isOn: greenOn, color: 'green' },
  ];
}
