/**
 * Shared pin-seat rule: a free pin seats the claimant, the claimant's own pin
 * clears it, and another claimant's pin is swapped (that claimant takes the
 * claimant's old pin, possibly none). Callers that must not steal a pin refuse
 * before calling.
 */
export interface SeatPinOptions<P> {
  /** Pin identity; defaults to `===`. */
  samePin?: (a: P, b: P) => boolean;
}

export interface SeatPinResult<K extends string, P> {
  assignments: Record<K, P | null>;
}

export function seatPin<K extends string, P>(
  assignments: Record<K, P | null>,
  claimant: K,
  pin: P,
  options: SeatPinOptions<P> = {},
): SeatPinResult<K, P> {
  const same = options.samePin ?? ((a: P, b: P) => a === b);
  const own = assignments[claimant];
  const next = { ...assignments };

  if (own !== null && same(own, pin)) {
    next[claimant] = null;
    return { assignments: next };
  }

  const holder = (Object.keys(assignments) as K[]).find((k) => {
    const held = assignments[k];
    return k !== claimant && held !== null && same(held, pin);
  }) ?? null;

  if (holder !== null) next[holder] = own;
  next[claimant] = pin;
  return { assignments: next };
}
