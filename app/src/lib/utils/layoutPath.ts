/**
 * Layout path utilities — pure transformations for layout folder paths.
 */

/**
 * Derives a human-readable layout title from a raw folder path.
 *
 * Returns the last path segment (the folder name). Returns null for
 * null/undefined/empty input.
 *
 * @example
 * normalizeLayoutTitle('/layouts/yard')        // "yard"
 * normalizeLayoutTitle('C:\\Layouts\\MyRR')    // "MyRR"
 */
export function normalizeLayoutTitle(raw: string | null | undefined): string | null {
  if (!raw) return null;
  const normalized = raw.replace(/\\/g, '/').replace(/\/+$/, '');
  return normalized.split('/').pop() ?? raw;
}
