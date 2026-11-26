/**
 * Shared pin-seat rule: a free pin seats the claimant, the claimant's own pin
 * clears it, and another claimant's pin is either swapped (that claimant takes
 * the claimant's old pin, possibly none) or moved (that claimant ends with no
 * pin). The claimant that lost its pin to the claim is reported as `displaced`
 * so callers can word their own messages.
 */
export interface SeatPinOptions<P> {
  taken: 'swap' | 'move';
  /** Pin identity; defaults to `===`. */
  samePin?: (a: P, b: P) => boolean;
}

export interface SeatPinResult<K extends string, P> {
  assignments: Record<K, P | null>;
  displaced: K | null;
}

export function seatPin<K extends string, P>(
  assignments: Record<K, P | null>,
  claimant: K,
  pin: P,
  options: SeatPinOptions<P>,
): SeatPinResult<K, P> {
  const same = options.samePin ?? ((a: P, b: P) => a === b);
  const own = assignments[claimant];
  const next = { ...assignments };

  if (own !== null && same(own, pin)) {
    next[claimant] = null;
    return { assignments: next, displaced: null };
  }

  const holder = (Object.keys(assignments) as K[]).find((k) => {
    const held = assignments[k];
    return k !== claimant && held !== null && same(held, pin);
  }) ?? null;

  if (holder !== null) next[holder] = options.taken === 'swap' ? own : null;
  next[claimant] = pin;
  return { assignments: next, displaced: holder };
}
