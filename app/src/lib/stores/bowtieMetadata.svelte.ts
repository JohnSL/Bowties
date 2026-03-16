/**
 * Svelte 5 reactive store for tracking pending (unsaved) bowtie metadata changes.
 *
 * Manages names, tags, role classifications, and bowtie creation/deletion.
 * Works alongside the Rust tree's `modified_value` tracking (which handles
 * event slot value changes) to provide a unified save/discard lifecycle (FR-018f).
 *
 * Spec: 009-editable-bowties, data-model.md §4, research.md R-004.
 */

import type { LayoutFile, BowtieMetadata, RoleClassification, BowtieMetadataEdit, BowtieEditKind } from '$lib/types/bowtie';
import { layoutStore } from '$lib/stores/layout.svelte';

// ─── Store class ──────────────────────────────────────────────────────────────

class BowtieMetadataStore {
  /** Pending metadata edits keyed by a unique edit key. */
  private _edits = $state<Map<string, BowtieMetadataEdit>>(new Map());

  /** Counter for generating unique edit IDs. */
  private _nextId = 0;

  // ── Reactive getters ───────────────────────────────────────────────────────

  /** True if any bowtie metadata edits are pending. */
  get isDirty(): boolean {
    return this._edits.size > 0;
  }

  /** True if there are pending metadata edits. */
  get hasPendingEdits(): boolean {
    return this._edits.size > 0;
  }

  /** Number of pending edits. */
  get editCount(): number {
    return this._edits.size;
  }

  // ── Mutations ──────────────────────────────────────────────────────────────

  /** Record a bowtie creation. */
  createBowtie(eventIdHex: string, name?: string): void {
    const id = this._makeId();
    this._edits.set(`create:${eventIdHex}`, {
      id,
      kind: { type: 'create', eventIdHex, name },
      timestamp: Date.now(),
    });
    this._applyToLayout();
  }

  /** Record a bowtie deletion. */
  deleteBowtie(eventIdHex: string): void {
    const id = this._makeId();
    // Remove any pending create for this bowtie
    this._edits.delete(`create:${eventIdHex}`);
    this._edits.set(`delete:${eventIdHex}`, {
      id,
      kind: { type: 'delete', eventIdHex },
      timestamp: Date.now(),
    });
    this._applyToLayout();
  }

  /** Rename a bowtie. */
  renameBowtie(eventIdHex: string, newName: string): void {
    const id = this._makeId();
    const current = this._getEffectiveMetadata(eventIdHex);
    this._edits.set(`rename:${eventIdHex}`, {
      id,
      kind: { type: 'rename', eventIdHex, oldName: current?.name, newName },
      timestamp: Date.now(),
    });
    this._applyToLayout();
  }

  /** Add a tag to a bowtie. */
  addTag(eventIdHex: string, tag: string): void {
    const id = this._makeId();
    this._edits.set(`addTag:${eventIdHex}:${tag}`, {
      id,
      kind: { type: 'addTag', eventIdHex, tag },
      timestamp: Date.now(),
    });
    this._applyToLayout();
  }

  /** Remove a tag from a bowtie. */
  removeTag(eventIdHex: string, tag: string): void {
    const id = this._makeId();
    // Remove any pending addTag for this same tag
    this._edits.delete(`addTag:${eventIdHex}:${tag}`);
    this._edits.set(`removeTag:${eventIdHex}:${tag}`, {
      id,
      kind: { type: 'removeTag', eventIdHex, tag },
      timestamp: Date.now(),
    });
    this._applyToLayout();
  }

  /** Classify an ambiguous event slot role. */
  classifyRole(key: string, role: 'Producer' | 'Consumer'): void {
    const id = this._makeId();
    this._edits.set(`classify:${key}`, {
      id,
      kind: { type: 'classifyRole', key, role },
      timestamp: Date.now(),
    });
    this._applyToLayout();
  }

  /** Re-classify an existing role classification. */
  reclassifyRole(key: string, newRole: 'Producer' | 'Consumer'): void {
    this.classifyRole(key, newRole);
  }

  /** Clear all pending metadata edits (used by discard). */
  clearAll(): void {
    this._edits.clear();
  }

  // ── Queries ────────────────────────────────────────────────────────────────

  /**
   * Get the effective metadata for a bowtie, combining the loaded layout
   * with any pending edits.
   */
  getMetadata(eventIdHex: string): BowtieMetadata | undefined {
    return this._getEffectiveMetadata(eventIdHex);
  }

  /** Get the effective role classification for a key. */
  getRoleClassification(key: string): RoleClassification | undefined {
    // Check pending edits first
    const edit = this._edits.get(`classify:${key}`);
    if (edit && edit.kind.type === 'classifyRole') {
      return { role: edit.kind.role };
    }
    // Fall back to loaded layout
    return layoutStore.layout?.roleClassifications[key];
  }

  /** Collect all unique tags from the effective layout + edits. */
  getAllTags(): string[] {
    const tags = new Set<string>();
    const layout = layoutStore.layout;
    if (layout) {
      for (const meta of Object.values(layout.bowties)) {
        for (const tag of meta.tags) {
          tags.add(tag);
        }
      }
    }
    return Array.from(tags);
  }

  /** Get all pending edits as an array. */
  get allEdits(): BowtieMetadataEdit[] {
    return Array.from(this._edits.values());
  }

  /** Get all event IDs that have pending create edits. */
  get allEventIds(): string[] {
    const ids: string[] = [];
    for (const [key, edit] of this._edits) {
      if (edit.kind.type === 'create') {
        ids.push(edit.kind.eventIdHex);
      }
    }
    return ids;
  }

  // ── Private helpers ────────────────────────────────────────────────────────

  private _makeId(): string {
    return `bme-${++this._nextId}`;
  }

  /**
   * Get a bowtie's effective metadata by applying pending edits on top
   * of the loaded layout data.
   */
  private _getEffectiveMetadata(eventIdHex: string): BowtieMetadata | undefined {
    const layout = layoutStore.layout;
    let meta: BowtieMetadata = layout?.bowties[eventIdHex]
      ? { ...layout.bowties[eventIdHex], tags: [...layout.bowties[eventIdHex].tags] }
      : { tags: [] };

    // Check if there's a pending create or delete
    if (this._edits.has(`delete:${eventIdHex}`)) {
      return undefined;
    }

    const createEdit = this._edits.get(`create:${eventIdHex}`);
    if (createEdit && createEdit.kind.type === 'create') {
      meta = { name: createEdit.kind.name, tags: [] };
    }

    // Apply rename
    const renameEdit = this._edits.get(`rename:${eventIdHex}`);
    if (renameEdit && renameEdit.kind.type === 'rename') {
      meta.name = renameEdit.kind.newName;
    }

    // Apply tag additions and removals
    for (const [key, edit] of this._edits) {
      if (edit.kind.type === 'addTag' && edit.kind.eventIdHex === eventIdHex) {
        if (!meta.tags.includes(edit.kind.tag)) {
          meta.tags.push(edit.kind.tag);
        }
      }
      if (edit.kind.type === 'removeTag' && edit.kind.eventIdHex === eventIdHex) {
        meta.tags = meta.tags.filter(t => t !== edit.kind.tag);
      }
    }

    // Return undefined if no data exists at all (not in layout, no pending create)
    if (!createEdit && !layout?.bowties[eventIdHex] && !renameEdit) {
      return undefined;
    }

    return meta;
  }

  /**
   * Apply all pending metadata edits to the layout store's in-memory layout.
   * This keeps the layout store in sync for immediate UI updates and for
   * eventual save.
   */
  private _applyToLayout(): void {
    const layout = layoutStore.layout;
    if (!layout) {
      // Auto-create a new layout when first edit is made
      layoutStore.newLayout();
    }

    const current = layoutStore.layout;
    if (!current) return;

    const updated: LayoutFile = {
      schemaVersion: current.schemaVersion,
      bowties: { ...current.bowties },
      roleClassifications: { ...current.roleClassifications },
    };

    // Apply all edits in order of insertion
    for (const edit of this._edits.values()) {
      switch (edit.kind.type) {
        case 'create': {
          const existing = updated.bowties[edit.kind.eventIdHex];
          if (!existing) {
            updated.bowties[edit.kind.eventIdHex] = {
              name: edit.kind.name,
              tags: [],
            };
          }
          break;
        }
        case 'delete':
          delete updated.bowties[edit.kind.eventIdHex];
          break;
        case 'rename': {
          const entry = updated.bowties[edit.kind.eventIdHex];
          if (entry) {
            updated.bowties[edit.kind.eventIdHex] = { ...entry, name: edit.kind.newName };
          }
          break;
        }
        case 'addTag': {
          const entry = updated.bowties[edit.kind.eventIdHex];
          if (entry && !entry.tags.includes(edit.kind.tag)) {
            updated.bowties[edit.kind.eventIdHex] = {
              ...entry,
              tags: [...entry.tags, edit.kind.tag],
            };
          }
          break;
        }
        case 'removeTag': {
          const entry = updated.bowties[edit.kind.eventIdHex];
          if (entry) {
            updated.bowties[edit.kind.eventIdHex] = {
              ...entry,
              tags: entry.tags.filter(t => t !== edit.kind.tag),
            };
          }
          break;
        }
        case 'classifyRole':
          updated.roleClassifications[edit.kind.key] = { role: edit.kind.role };
          break;
      }
    }

    layoutStore.updateLayout(updated);
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

/** Global singleton — import this in components and the save handler. */
export const bowtieMetadataStore = new BowtieMetadataStore();
