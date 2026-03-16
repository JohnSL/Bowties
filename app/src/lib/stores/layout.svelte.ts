/**
 * Svelte 5 reactive store for layout file state (Feature 009).
 *
 * Manages the YAML layout file lifecycle: open, save, save-as, recent file
 * tracking, dirty state, and path tracking. Uses @tauri-apps/plugin-dialog
 * for native file dialogs.
 */

import { save, open } from '@tauri-apps/plugin-dialog';
import { loadLayout, saveLayout, getRecentLayout, setRecentLayout, buildBowtieCatalog } from '$lib/api/bowties';
import type { LayoutFile } from '$lib/types/bowtie';

// ─── Store class ─────────────────────────────────────────────────────────────

class LayoutStore {
  /** The currently loaded layout file data, or null if none loaded. */
  private _layout = $state<LayoutFile | null>(null);

  /** Absolute path to the currently loaded/saved layout file. */
  private _path = $state<string | null>(null);

  /** True if the layout has unsaved changes. */
  private _dirty = $state<boolean>(false);

  /** True if a file operation is in progress. */
  private _busy = $state<boolean>(false);

  // ── Reactive getters ──────────────────────────────────────────────────────

  get layout(): LayoutFile | null {
    return this._layout;
  }

  get path(): string | null {
    return this._path;
  }

  get isDirty(): boolean {
    return this._dirty;
  }

  get isLoaded(): boolean {
    return this._layout !== null;
  }

  get isBusy(): boolean {
    return this._busy;
  }

  /** Display name for the layout file (filename only, or 'Untitled'). */
  get displayName(): string {
    if (!this._path) return 'Untitled';
    const parts = this._path.replace(/\\/g, '/').split('/');
    return parts[parts.length - 1] || 'Untitled';
  }

  // ── Layout operations ─────────────────────────────────────────────────────

  /**
   * Open a layout file using a native file dialog.
   * Loads and validates the file, then triggers a catalog rebuild with merged metadata.
   */
  async openLayout(): Promise<void> {
    const selected = await open({
      title: 'Open Layout File',
      filters: [
        { name: 'Bowties Layout', extensions: ['bowties.yaml', 'yaml', 'yml'] },
      ],
    });

    if (!selected) return; // user cancelled

    await this.loadLayoutFromPath(selected);
  }

  /**
   * Load a layout file from a known path (e.g., from recent layout).
   */
  async loadLayoutFromPath(filePath: string): Promise<void> {
    this._busy = true;
    try {
      const layout = await loadLayout(filePath);
      this._layout = layout;
      this._path = filePath;
      this._dirty = false;

      // Remember as recent layout
      await setRecentLayout(filePath);

      // Trigger catalog rebuild with merged metadata
      await buildBowtieCatalog(layout);
    } finally {
      this._busy = false;
    }
  }

  /**
   * Save the current layout to its existing path.
   * If no path is set, falls back to saveLayoutAs.
   */
  async saveCurrentLayout(): Promise<void> {
    if (!this._layout) return;

    if (!this._path) {
      return this.saveLayoutAs();
    }

    this._busy = true;
    try {
      await saveLayout(this._path, this._layout);
      this._dirty = false;
    } finally {
      this._busy = false;
    }
  }

  /**
   * Save the current layout with a native "Save As" dialog.
   */
  async saveLayoutAs(): Promise<void> {
    if (!this._layout) return;

    const selected = await save({
      title: 'Save Layout File',
      defaultPath: this._path ?? 'layout.bowties.yaml',
      filters: [
        { name: 'Bowties Layout', extensions: ['bowties.yaml'] },
      ],
    });

    if (!selected) return; // user cancelled

    this._busy = true;
    try {
      await saveLayout(selected, this._layout);
      this._path = selected;
      this._dirty = false;

      // Remember as recent layout
      await setRecentLayout(selected);
    } finally {
      this._busy = false;
    }
  }

  // ── Mutation methods (called by other stores to update layout data) ────────

  /**
   * Update the in-memory layout data and mark as dirty.
   * Used by bowtie metadata store to sync edits into the layout.
   */
  updateLayout(layout: LayoutFile): void {
    this._layout = layout;
    this._dirty = true;
  }

  /**
   * Mark the layout as dirty (unsaved changes exist).
   */
  markDirty(): void {
    this._dirty = true;
  }

  /**
   * Mark the layout as clean (all changes saved).
   */
  markClean(): void {
    this._dirty = false;
  }

  /**
   * Create a new empty layout, discarding the current one.
   */
  newLayout(): void {
    this._layout = {
      schemaVersion: '1.0',
      bowties: {},
      roleClassifications: {},
    };
    this._path = null;
    this._dirty = false;
  }

  /**
   * Reset the layout store to its initial state (no layout loaded).
   */
  reset(): void {
    this._layout = null;
    this._path = null;
    this._dirty = false;
    this._busy = false;
  }

  // ── Recent layout auto-reopen ─────────────────────────────────────────────

  /**
   * Check for a recently opened layout and offer to reopen it.
   * Called on app startup after CDI reads complete. Returns true if a
   * layout was successfully loaded.
   */
  async checkAndReopenRecent(): Promise<boolean> {
    try {
      const recent = await getRecentLayout();
      if (!recent) return false;

      // Auto-load the recent layout (the backend validates the file still exists)
      await this.loadLayoutFromPath(recent.path);
      return true;
    } catch (e) {
      console.warn('[layout] Failed to reopen recent layout:', e);
      return false;
    }
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const layoutStore = new LayoutStore();
