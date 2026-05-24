import { invoke } from '@tauri-apps/api/core';

/**
 * A single entry in the app's known-layout registry (Spec 013 / S5).
 *
 * Mirrors the Rust `KnownLayoutEntry` shape (serialised camelCase).
 * The layout picker (S6) displays `name`, `path`, and `lastOpened`.
 */
export interface KnownLayoutEntry {
  /** Display name shown in the layout picker. */
  name: string;
  /** Absolute path to the `.layout` base file. */
  path: string;
  /** ISO 8601 timestamp of the most recent open. */
  lastOpened: string;
}

/**
 * Read the known-layout registry from `$APPDATA/bowties/known-layouts.json`.
 *
 * Entries whose `path` no longer exists on disk are filtered out
 * before being returned, so the picker never shows a layout the
 * user can't actually open.
 */
export async function getKnownLayouts(): Promise<KnownLayoutEntry[]> {
  return invoke<KnownLayoutEntry[]>('get_known_layouts');
}

/**
 * Add or refresh a known-layout entry. Entries are matched by
 * `path` — re-adding a path replaces the existing entry's name and
 * `lastOpened` timestamp without duplicating it.
 *
 * @returns The post-write registry (with stale entries filtered).
 */
export async function addKnownLayout(
  entry: KnownLayoutEntry,
): Promise<KnownLayoutEntry[]> {
  return invoke<KnownLayoutEntry[]>('add_known_layout', { entry });
}

/**
 * Remove a known-layout entry by path. The `.layout` file and its
 * companion directory on disk are not touched — only the registry
 * forgets the path.
 *
 * @returns The post-write registry (with stale entries filtered).
 */
export async function removeKnownLayout(
  path: string,
): Promise<KnownLayoutEntry[]> {
  return invoke<KnownLayoutEntry[]>('remove_known_layout', { path });
}
