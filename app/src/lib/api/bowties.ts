/**
 * Tauri IPC wrappers for bowtie layout commands (Feature 009).
 *
 * These wrap the Rust backend commands added for editable bowties:
 * layout file load/save, recent layout tracking, and catalog building
 * with layout metadata.
 */

import { invoke } from '@tauri-apps/api/core';
import type { LayoutFile, RecentLayout } from '$lib/types/bowtie';
import type { BowtieCatalog } from '$lib/api/tauri';

/**
 * Load a YAML layout file from disk.
 * @param path Absolute filesystem path to the YAML layout file
 * @returns Parsed and validated LayoutFile
 */
export async function loadLayout(path: string): Promise<LayoutFile> {
  return invoke<LayoutFile>('load_layout', { path });
}

/**
 * Save bowtie metadata and role classifications to a YAML layout file.
 * Uses atomic write (temp → flush → rename).
 * @param path Absolute filesystem path to write
 * @param layout Layout data to persist
 */
export async function saveLayout(path: string, layout: LayoutFile): Promise<void> {
  return invoke<void>('save_layout', { path, layout });
}

/**
 * Retrieve the most recently opened layout file path.
 * @returns RecentLayout with path and timestamp, or null if none
 */
export async function getRecentLayout(): Promise<RecentLayout | null> {
  return invoke<RecentLayout | null>('get_recent_layout');
}

/**
 * Store the most recently opened layout file path.
 * @param path Absolute path to remember
 */
export async function setRecentLayout(path: string): Promise<void> {
  return invoke<void>('set_recent_layout', { path });
}

/**
 * Build or rebuild the bowtie catalog, optionally merging layout metadata.
 * @param layoutMetadata Optional layout file to merge with discovered state
 * @returns Built catalog with merged metadata
 */
export async function buildBowtieCatalog(layoutMetadata?: LayoutFile | null): Promise<BowtieCatalog> {
  return invoke<BowtieCatalog>('build_bowtie_catalog_command', { layoutMetadata: layoutMetadata ?? null });
}
