/**
 * Svelte 5 reactive store for layout file state (Feature 009).
 *
 * Manages the YAML layout file lifecycle: open, save, save-as, recent file
 * tracking, dirty state, and path tracking. Uses @tauri-apps/plugin-dialog
 * for native file dialogs.
 */

import { open } from '@tauri-apps/plugin-dialog';
import { loadLayout, saveLayout, getRecentLayout, setRecentLayout, buildBowtieCatalog } from '$lib/api/bowties';
import type { LayoutFile } from '$lib/types/bowtie';
import { normalizeNodeId } from '$lib/utils/nodeId';

export type ActiveLayoutMode = 'legacy_file' | 'offline_file';

export interface ActiveLayoutContext {
  layoutId: string;
  rootPath: string;
  mode: ActiveLayoutMode;
  capturedAt?: string;
  pendingOfflineChangeCount: number;
  /**
   * Canonical (uppercase, no-dots) node IDs persisted in this layout's
   * companion `nodes/` directory (S8). The layout is the durable source
   * of truth for which nodes belong to it; discovered nodes whose IDs
   * are not in this list are unsaved drafts until promoted via save.
   *
   * Defaults to `[]` for layouts opened before this field existed and for
   * brand-new in-memory layouts that have never been saved.
   */
  layoutNodeIds?: string[];
}

// ─── Store class ─────────────────────────────────────────────────────────────

class LayoutStore {
  /** The currently loaded layout file data, or null if none loaded. */
  private _layout = $state<LayoutFile | null>(null);

  /** Last-saved (or last-loaded) snapshot — used to revert unsaved metadata edits. */
  private _savedLayout: LayoutFile | null = null;

  /** Absolute path to the currently loaded/saved layout file. */
  private _path = $state<string | null>(null);

  /** True if the LayoutFile struct has unsaved metadata edits. */
  private _dirty = $state<boolean>(false);

  /** True if a file operation is in progress. */
  private _busy = $state<boolean>(false);

  /** Current active layout context (legacy file or offline directory layout). */
  private _activeContext = $state<ActiveLayoutContext | null>(null);

  /** True when an offline directory layout is active. */
  private _offlineMode = $state<boolean>(false);

  /** True when the LCC bus is connected. */
  private _connected = $state<boolean>(false);

  // ── Reactive getters ──────────────────────────────────────────────────────

  get layout(): LayoutFile | null {
    return this._layout;
  }

  get path(): string | null {
    return this._path;
  }

  /**
   * True when the LayoutFile struct has unsaved metadata edits (bowties,
   * role classifications, connector selections). The aggregate "any
   * in-memory change" signal lives on `effectiveNodeStore.isDirty`
   * (ADR-0011) — this getter intentionally does NOT include drafts,
   * offline changes, or fully-captured discovered-but-unsaved nodes.
   */
  get isDirty(): boolean {
    return this._dirty;
  }

  get isLoaded(): boolean {
    return this._layout !== null;
  }

  get isBusy(): boolean {
    return this._busy;
  }

  get activeContext(): ActiveLayoutContext | null {
    return this._activeContext;
  }

  get isOfflineMode(): boolean {
    return this._offlineMode && !this._connected;
  }

  /** True when an offline directory layout file is open (regardless of connection). */
  get hasLayoutFile(): boolean {
    return this._offlineMode;
  }

  /** True when the LCC bus is connected. */
  get isConnected(): boolean {
    return this._connected;
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
      title: 'Open Layout Folder',
      directory: true,
      multiple: false,
    });

    if (!selected || typeof selected !== 'string') return; // user cancelled

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
      this._savedLayout = JSON.parse(JSON.stringify(layout));
      this._path = filePath;
      this._dirty = false;
      this._offlineMode = false;
      this._activeContext = {
        layoutId: filePath,
        rootPath: filePath,
        mode: 'legacy_file',
        pendingOfflineChangeCount: 0,
      };

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
   * Returns true if the save was performed, false if cancelled or no-op.
   */
  async saveCurrentLayout(): Promise<boolean> {
    if (!this._layout) return false;

    if (!this._path) {
      return this.saveLayoutAs();
    }

    this._busy = true;
    try {
      await saveLayout(this._path, this._layout);
      this._savedLayout = JSON.parse(JSON.stringify(this._layout));
      this._dirty = false;
      return true;
    } finally {
      this._busy = false;
    }
  }

  /**
   * Save the current layout with a native "Save As" dialog.
   * Auto-creates an empty layout when none is loaded (fixes Bug 3).
   * Returns true if the save was performed, false if the user cancelled.
   */
  async saveLayoutAs(): Promise<boolean> {
    if (!this._layout) this.newLayout();

    const selected = await open({
      title: 'Choose Layout Folder',
      directory: true,
      multiple: false,
    });

    if (!selected || typeof selected !== 'string') return false; // user cancelled

    this._busy = true;
    try {
      await saveLayout(selected, this._layout!);
      this._path = selected;
      this._savedLayout = JSON.parse(JSON.stringify(this._layout));
      this._dirty = false;
      this._offlineMode = false;
      this._activeContext = {
        layoutId: selected,
        rootPath: selected,
        mode: 'legacy_file',
        pendingOfflineChangeCount: 0,
      };

      // Remember as recent layout
      await setRecentLayout(selected);
      return true;
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
   * Hydrate the layout store from the backend's persisted copy (ADR-0002).
   *
   * Called only after a successful save or open — sets both the live layout
   * and the saved snapshot so dirty tracking resets correctly.
   */
  hydrateFromBackend(layout: LayoutFile): void {
    this._layout = layout;
    this._savedLayout = JSON.parse(JSON.stringify(layout));
  }

  /** Lookup configuration-mode selections for a node from the active layout metadata. */
  getNodeModeSelections(nodeKey: string): Record<string, string> | null {
    if (!this._layout) {
      return null;
    }
    return this._layout.nodeModeSelections?.[normalizeNodeId(nodeKey)] ?? null;
  }

  /**
   * Hydrate the layout store from an opened offline layout directory.
   * Keeps the loaded layout metadata available to preview/edit flows while
   * the orchestrator owns the broader open lifecycle.
   */
  hydrateOfflineLayout(layout: LayoutFile, context: ActiveLayoutContext): void {
    this._layout = JSON.parse(JSON.stringify(layout));
    this._savedLayout = JSON.parse(JSON.stringify(layout));
    this._path = context.rootPath;
    this._dirty = false;
    this._activeContext = context;
    this._offlineMode = context.mode === 'offline_file';
  }

  /**
   * Revert in-memory layout to the last saved (or loaded) snapshot.
   * Called as part of the unified Discard flow so metadata edits that were
   * already baked into the layout by _applyToLayout() are rolled back.
   */
  revertToSaved(): void {
    if (this._savedLayout) {
      this._layout = JSON.parse(JSON.stringify(this._savedLayout));
    }
    this._dirty = false;
  }

  /**
   * Mark the layout as dirty (unsaved changes exist).
   */
  markDirty(): void {
    this._dirty = true;
  }

  /**
   * Mark the layout struct as clean. Aggregate cleanliness (drafts +
   * metadata + offline + unsaved-new) is owned by the facade.
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
    this._savedLayout = JSON.parse(JSON.stringify(this._layout));
    this._path = null;
    this._dirty = false;
    this._activeContext = null;
    this._offlineMode = false;
  }

  /**
   * Reset the layout store to its initial state (no layout loaded).
   */
  reset(): void {
    this._layout = null;
    this._savedLayout = null;
    this._path = null;
    this._dirty = false;
    this._busy = false;
    this._activeContext = null;
    this._offlineMode = false;
  }

  /**
   * Update the bus connection status.
   * When connected, isOfflineMode returns false even if a layout file is open,
   * so edits go to hardware instead of offline changes.
   */
  setConnected(connected: boolean): void {
    this._connected = connected;
  }

  setActiveContext(context: ActiveLayoutContext | null): void {
    this._activeContext = context;
    this._offlineMode = context?.mode === 'offline_file';
  }

  private recomputeDirtyFromSaved(): void {
    if (!this._layout || !this._savedLayout) {
      this._dirty = !!this._layout;
      return;
    }

    this._dirty = JSON.stringify(this._layout) !== JSON.stringify(this._savedLayout);
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
