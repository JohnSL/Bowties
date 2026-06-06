/**
 * NodeKey — branded sum type for both live LCC nodes and placeholder
 * configuration-only boards (Spec 014, ADR-0008, ADR-0010).
 *
 *   NodeKey ::= LiveNodeKey | PlaceholderNodeKey
 *
 * Construction goes through `nodeKey(input)`. There is no `as NodeKey`
 * shortcut: the brand is a non-exported unique symbol, so raw strings
 * cannot be widened into the branded type without invoking the factory.
 * This is what makes the lookup-miss bug class structurally impossible
 * on the frontend side — every consumer ends up comparing branded keys
 * via `nodeKeyEquals`, and stringly-typed drift cannot enter the system
 * without a `nodeKey()` parse at the boundary.
 *
 * `NodeKeyInput = string | NodeKey` is the boundary shim that lets
 * legacy string-keyed callers and branded callers share signatures
 * during the migration. Wave 2 of Step 7 narrows public signatures to
 * branded `NodeKey` only and deletes the shim.
 */

import { normalizeNodeId } from './nodeId';

/** Internal prefix for placeholder NodeKeys. Mirrors the backend constant. */
const PLACEHOLDER_PREFIX = 'placeholder:';

declare const NodeKeyBrand: unique symbol;

export interface LiveNodeKey {
  readonly [NodeKeyBrand]: 'NodeKey';
  readonly kind: 'live';
  /** Canonical 12-hex uppercase form, no dots. */
  readonly id: string;
}

export interface PlaceholderNodeKey {
  readonly [NodeKeyBrand]: 'NodeKey';
  readonly kind: 'placeholder';
  /** The uuid (or opaque id) carried after the `placeholder:` prefix. */
  readonly id: string;
}

export type NodeKey = LiveNodeKey | PlaceholderNodeKey;

const LIVE_HEX_RE = /^[0-9A-F]{12}$/;

/**
 * The only public constructor for a `NodeKey`. Accepts the same inputs
 * the backend `NodeKey::parse` accepts:
 *
 *   - Live: dotted (`02.01.57.00.02.D9`) or canonical (`020157000002D9`).
 *     Case-insensitive on input; normalized to uppercase canonical.
 *   - Placeholder: `placeholder:<id>` where `<id>` is non-empty.
 *
 * Throws on invalid input. Callers must parse at the system boundary
 * (IPC, persisted snapshot, user input) and pass branded keys onward.
 */
export function nodeKey(input: string): NodeKey {
  if (typeof input !== 'string' || input.length === 0) {
    throw new Error('nodeKey: empty input');
  }
  if (input.startsWith(PLACEHOLDER_PREFIX)) {
    const id = input.slice(PLACEHOLDER_PREFIX.length);
    if (id.length === 0) {
      throw new Error('nodeKey: empty placeholder id');
    }
    return { kind: 'placeholder', id } as PlaceholderNodeKey;
  }
  const canonical = normalizeNodeId(input);
  if (!LIVE_HEX_RE.test(canonical)) {
    throw new Error(`nodeKey: invalid live form: ${JSON.stringify(input)}`);
  }
  return { kind: 'live', id: canonical } as LiveNodeKey;
}

/** Structural equality by `kind` + `id`. */
export function nodeKeyEquals(a: NodeKey, b: NodeKey): boolean {
  return a.kind === b.kind && a.id === b.id;
}

/**
 * Serialize a `NodeKey` to the wire form the backend expects
 * (`020157000002D9` for live, `placeholder:<id>` for placeholder).
 */
export function nodeKeyToString(key: NodeKey): string {
  return key.kind === 'live' ? key.id : `${PLACEHOLDER_PREFIX}${key.id}`;
}

/**
 * Format a `NodeKey` for UI display: dotted hex for live, the
 * `placeholder:<id>` wire form verbatim for placeholders.
 */
export function nodeKeyToDisplay(key: NodeKey): string {
  if (key.kind === 'placeholder') {
    return `${PLACEHOLDER_PREFIX}${key.id}`;
  }
  return (key.id.match(/.{1,2}/g) ?? []).join('.');
}

/**
 * Boundary-shim input type for legacy call sites that hold raw strings
 * (snapshot deserialization, user-selection state, IPC results). New
 * code should use branded `NodeKey` directly. The behavioral protection
 * lives in `toCanonicalNodeKey` / `isPlaceholderInput`, which normalize
 * both shapes; this alias exists only so existing API signatures keep
 * compiling while migration is in flight.
 *
 * Removal is tracked as a follow-up (Step 7 Wave 2 deferral) — narrowing
 * to `NodeKey` only surfaces ~460 caller-side type errors that need
 * case-by-case judgment about where the string-to-branded boundary
 * belongs.
 */
export type NodeKeyInput = string | NodeKey;

/**
 * Canonicalize a `NodeKeyInput` to the backend wire form
 * (`<canonical 12-hex>` for live, `placeholder:<id>` for placeholders).
 *
 * - String inputs: placeholder keys pass through unchanged (case-sensitive
 *   uuid); live forms are uppercased and dots stripped. Empty / null-ish
 *   inputs return `''` to preserve legacy permissiveness — invalid live
 *   forms produce the uppercased input, matching what callers see today.
 * - Branded inputs are serialised via `nodeKeyToString`.
 */
export function toCanonicalNodeKey(input: string | NodeKey | undefined | null): string {
  if (input === null || input === undefined) return '';
  if (typeof input !== 'string') return nodeKeyToString(input);
  if (input.length === 0) return '';
  if (input.startsWith(PLACEHOLDER_PREFIX)) return input;
  return normalizeNodeId(input);
}

/**
 * True when `input` represents a placeholder NodeKey. Accepts both
 * branded `NodeKey` values and raw strings (the latter checked against
 * the `placeholder:` prefix exactly, matching the backend predicate).
 */
export function isPlaceholderInput(input: string | NodeKey | undefined | null): boolean {
  if (input === null || input === undefined) return false;
  if (typeof input !== 'string') return input.kind === 'placeholder';
  return input.startsWith(PLACEHOLDER_PREFIX);
}
