/**
 * Startup orchestrator — owns the layout picker lifecycle (Spec 013 / S6).
 *
 * Responsibilities:
 * - Load the known-layout registry into `knownLayoutsStore` on app startup.
 * - Drive picker-initiated layout opens (known entry / browse / new) through
 *   `openOfflineLayoutWithReplay`, register the opened layout in the registry,
 *   and refresh the in-memory list.
 * - Remove entries from the registry without touching layout files on disk.
 *
 * Like the other orchestrators (saveLayoutOrchestrator, offlineLayoutOrchestrator)
 * this module is pure: all API/store accesses are passed in as arguments so the
 * orchestrator can be tested without Tauri or real stores.
 */

import type { KnownLayoutEntry } from '$lib/api/startup';
import type { OpenLayoutResult } from '$lib/api/layout';
import type { NewLayoutResult } from '$lib/api/layout';

// ── Pluggable boundary types ────────────────────────────────────────────────

export interface StartupApi {
  getKnownLayouts: () => Promise<KnownLayoutEntry[]>;
  addKnownLayout: (entry: KnownLayoutEntry) => Promise<KnownLayoutEntry[]>;
  removeKnownLayout: (path: string) => Promise<KnownLayoutEntry[]>;
}

export interface LayoutLifecycleApi {
  /** Persist an empty layout file at the given path (used by "New Layout"). */
  createNewLayoutCapture: () => Promise<NewLayoutResult>;
  saveLayoutDirectory: (
    path: string,
    overwrite: boolean,
    deltas: never[],
  ) => Promise<unknown>;
  /** Open a layout file from a path and replay its snapshots — same as the
   *  flow used by "Open Recent" / menu-open-layout. */
  openLayout: (path: string) => Promise<OpenLayoutResult>;
}

export interface KnownLayoutsSink {
  setEntries: (entries: KnownLayoutEntry[]) => void;
  setBusy: (value: boolean) => void;
}

// ── Public operations ───────────────────────────────────────────────────────

export interface LoadKnownLayoutsArgs {
  api: Pick<StartupApi, 'getKnownLayouts'>;
  store: KnownLayoutsSink;
  onError?: (error: unknown) => void;
}

/** Hydrate the known-layouts store from the backend registry. */
export async function loadKnownLayouts(args: LoadKnownLayoutsArgs): Promise<void> {
  args.store.setBusy(true);
  try {
    const entries = await args.api.getKnownLayouts();
    args.store.setEntries(entries);
  } catch (err) {
    args.onError?.(err);
    // Surface an empty (loaded) list rather than leaving the store in an
    // unloaded state — the picker UI shows "no known layouts" instead of
    // spinning forever.
    args.store.setEntries([]);
  } finally {
    args.store.setBusy(false);
  }
}

// ──────────────────────────────────────────────────────────────────────────

export interface OpenLayoutFromRegistryArgs {
  path: string;
  /** Display name to record in the registry. Falls back to the basename of `path`. */
  name?: string;
  openLayout: (path: string) => Promise<OpenLayoutResult>;
  api: Pick<StartupApi, 'addKnownLayout'>;
  store: KnownLayoutsSink;
  /** Called after a successful open, with the open result (so the route can
   *  hydrate snapshots, partial-capture set, etc). */
  onOpened: (result: OpenLayoutResult) => Promise<void> | void;
}

/**
 * Open a layout by path through the same flow as menu-open-layout, then
 * upsert it into the known-layouts registry so its last-opened timestamp is
 * refreshed and the in-memory list is consistent with the backend.
 */
export async function openLayoutFromRegistry(
  args: OpenLayoutFromRegistryArgs,
): Promise<OpenLayoutResult> {
  const result = await args.openLayout(args.path);
  await args.onOpened(result);

  const entry: KnownLayoutEntry = {
    name: args.name?.trim() || deriveLayoutNameFromPath(args.path),
    path: args.path,
    lastOpened: new Date().toISOString(),
  };
  try {
    const entries = await args.api.addKnownLayout(entry);
    args.store.setEntries(entries);
  } catch {
    // Registry upsert failure must not block the user — they opened the
    // layout successfully. The next picker reload will resync.
  }
  return result;
}

// ──────────────────────────────────────────────────────────────────────────

export interface CreateNewLayoutArgs {
  /** Layout display name (also used as the file basename). */
  name: string;
  /** Filesystem path of the new `.layout` base file (the picker dialog
   *  builds this from a directory + filename). */
  path: string;
  api: Pick<StartupApi, 'addKnownLayout'>;
  lifecycle: LayoutLifecycleApi;
  store: KnownLayoutsSink;
  /** Same as `openLayoutFromRegistry.onOpened`. */
  onOpened: (result: OpenLayoutResult) => Promise<void> | void;
}

/**
 * Create a brand-new empty layout at the given path, then open it like any
 * other known layout. The flow is:
 *
 *   1. createNewLayoutCapture → empty in-memory capture state
 *   2. saveLayoutDirectory(path)  → writes the base file + companion dir
 *   3. openLayout(path)            → loads it back through the normal open path
 *   4. addKnownLayout              → registry entry
 */
export async function createNewLayout(
  args: CreateNewLayoutArgs,
): Promise<OpenLayoutResult> {
  const trimmed = args.name.trim();
  if (!trimmed) throw new Error('Layout name is required.');
  if (!args.path) throw new Error('Layout path is required.');

  await args.lifecycle.createNewLayoutCapture();
  await args.lifecycle.saveLayoutDirectory(args.path, true, []);

  return openLayoutFromRegistry({
    path: args.path,
    name: trimmed,
    openLayout: args.lifecycle.openLayout,
    api: args.api,
    store: args.store,
    onOpened: args.onOpened,
  });
}

// ──────────────────────────────────────────────────────────────────────────

export interface RemoveKnownLayoutArgs {
  path: string;
  api: Pick<StartupApi, 'removeKnownLayout'>;
  store: KnownLayoutsSink;
  onError?: (error: unknown) => void;
}

/** Remove a layout from the registry. Does NOT delete files on disk. */
export async function removeKnownLayout(args: RemoveKnownLayoutArgs): Promise<void> {
  try {
    const entries = await args.api.removeKnownLayout(args.path);
    args.store.setEntries(entries);
  } catch (err) {
    args.onError?.(err);
  }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

export function deriveLayoutNameFromPath(path: string): string {
  const normalized = path.replace(/\\/g, '/');
  const last = normalized.split('/').pop() ?? path;
  return last.replace(/\.layout$/i, '') || last;
}
