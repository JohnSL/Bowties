/**
 * `EventIdKey` — compiler-enforced identity type for LCC event IDs.
 *
 * ## Why this exists
 *
 * Event IDs have two string representations in Bowties:
 *
 *   - **Dotted display**   — `02.01.57.00.02.D9.04.D2` — used for UI display
 *     and (historically) some layout YAML files.
 *   - **Canonical**        — `0201570002D904D2`         — uppercase 16-char
 *     undotted; the canonical form for comparison, map keys, and storage,
 *     matching `lcc_rs::EventID::to_canonical()`.
 *
 * Mixing the two forms as `String` map keys is a documented source of bugs
 * (see ADR-0010 for the same lesson applied to `NodeKey`). This module
 * defines a branded string type that only accepts canonical form, so the
 * compiler catches accidental leaks of the display form into identity uses.
 *
 * ## Contract
 *
 *  - The only ways to obtain an `EventIdKey` are `toEventIdKey(hex)` and
 *    `eventIdKeyFromBytes(bytes)`. Both return canonical form.
 *  - `toEventIdKey` accepts dotted, canonical, or mixed input — it is the
 *    single normalization seam for values arriving from IPC, layout YAML,
 *    or unknown-shape strings.
 *  - `formatEventIdKey(key)` returns the dotted display form for UI only.
 *  - Direct `string` values MUST NOT be assigned to `EventIdKey` without
 *    going through `toEventIdKey`; the branded phantom property enforces
 *    this at compile time.
 *
 * ## Future extension seam
 *
 * When placeholder proxies get "wired up" (ADR-0009 follow-up), we may
 * introduce a `PlaceholderSlot` variant here (mirroring `NodeKey`'s
 * `Live` / `Placeholder` split from ADR-0010). Keeping identity behind
 * this branded type today means the compiler will enumerate every seam
 * that needs updating when that variant lands.
 */

import { canonicalEventIdHex, formatEventIdHex, parseEventIdHex } from '$lib/utils/serialize';

/**
 * Branded canonical uppercase 16-char undotted event ID hex.
 *
 * Cannot be constructed from a raw `string` — the phantom `__brand` field
 * prevents implicit conversion. Use `toEventIdKey` or `eventIdKeyFromBytes`.
 */
export type EventIdKey = string & { readonly __brand: 'EventIdKey' };

/**
 * Normalize an event-ID hex string (dotted, canonical, or mixed) into an
 * `EventIdKey`. Returns `null` if the input is not a valid 8-byte hex ID.
 *
 * Use at IPC / layout-YAML / user-input boundaries.
 */
export function toEventIdKey(hex: string): EventIdKey | null {
  const bytes = parseEventIdHex(hex);
  return bytes ? (canonicalEventIdHex(bytes) as EventIdKey) : null;
}

/**
 * Produce an `EventIdKey` from an 8-byte array (canonical uppercase).
 *
 * Prefer this when the identity is derived from bytes we already trust
 * (e.g. `event_id_bytes` on a catalog card).
 */
export function eventIdKeyFromBytes(bytes: number[] | readonly number[]): EventIdKey {
  return canonicalEventIdHex(bytes as number[]) as EventIdKey;
}

/**
 * Render an `EventIdKey` in dotted display form for the UI.
 * Never assign the result back into `EventIdKey` — this is a one-way trip.
 */
export function formatEventIdKey(key: EventIdKey): string {
  const bytes = parseEventIdHex(key);
  return bytes ? formatEventIdHex(bytes) : (key as string);
}

/**
 * Type-narrowing helper. True when `value` is already in canonical form.
 * Does NOT normalize — use `toEventIdKey` if you need to normalize.
 */
export function isCanonicalEventIdKey(value: string): value is EventIdKey {
  return /^[0-9A-F]{16}$/.test(value);
}
