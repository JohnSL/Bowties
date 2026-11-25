/**
 * Structural equality for JSON-shaped values (key order insensitive).
 * Shared by stores that compare an edited record against its saved baseline.
 */
export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a as object);
  const kb = Object.keys(b as object);
  if (ka.length !== kb.length) return false;
  return ka.every((k) =>
    Object.prototype.hasOwnProperty.call(b, k)
    && deepEqual((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]));
}
