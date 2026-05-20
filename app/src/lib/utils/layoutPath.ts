/**
 * Layout path utilities — pure transformations for layout file paths.
 */

/**
 * Derives a human-readable layout title from a raw file path.
 *
 * Strips common layout file extensions and returns just the base name.
 * Returns null for null/undefined/empty input.
 *
 * @example
 * normalizeLayoutTitle('/layouts/yard.bowties.yaml')  // "yard"
 * normalizeLayoutTitle('my-layout.layout')            // "my-layout"
 */
export function normalizeLayoutTitle(raw: string | null | undefined): string | null {
  if (!raw) return null;
  const fileName = raw.replace(/\\/g, '/').split('/').pop() ?? raw;
  return fileName
    .replace(/\.layout$/i, '')
    .replace(/\.bowties\.ya?ml$/i, '')
    .replace(/\.ya?ml$/i, '')
    .replace(/\.layout\.d$/i, '');
}
