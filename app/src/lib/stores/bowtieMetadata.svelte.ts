/**
 * Svelte 5 reactive store for tracking pending (unsaved) bowtie metadata changes.
 *
 * Manages names, tags, role classifications, and bowtie creation/deletion.
 * Works alongside the Rust tree's `modified_value` tracking (which handles
 * event slot value changes) to provide a unified save/discard lifecycle (FR-018f).
 *
 * Spec: 009-editable-bowties, data-model.md §4, research.md R-004.
 */

import { SvelteMap } from 'svelte/reactivity';
import type { LayoutFile, BowtieMetadata, RoleClassification, BowtieMetadataEdit, BowtieEditKind, LayoutEditDelta } from '$lib/types/bowtie';
import { layoutStore } from '$lib/stores/layout.svelte';

// ─── Store class ──────────────────────────────────────────────────────────────

class BowtieMetadataStore {
  /**
   * Pending metadata edits keyed by a unique edit key.
   *
   * Uses SvelteMap (svelte/reactivity) so that .set() / .delete() / .clear()
   * mutations are natively tracked by Svelte 5's reactive scheduler, making
   * isDirty / editCount reliably reactive from any component $derived or $effect.
   */
  private _edits = new SvelteMap<string, BowtieMetadataEdit>();

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
  createBowtie(
    eventIdHex: string,
    name?: string,
    opts?: { createdByFacility?: string },
  ): void {
    const id = this._makeId();
    this._edits.set(`create:${eventIdHex}`, {
      id,
      kind: {
        type: 'create',
        eventIdHex,
        name,
        createdByFacility: opts?.createdByFacility,
      },
      timestamp: Date.now(),
    });
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
  }

  /** Rename a bowtie. No-op if the new name equals the current name (ADR-0012). */
  renameBowtie(eventIdHex: string, newName: string): void {
    const current = this._getEffectiveMetadata(eventIdHex);
    if (current?.name === newName) return; // no-op suppression
    const id = this._makeId();
    this._edits.set(`rename:${eventIdHex}`, {
      id,
      kind: { type: 'rename', eventIdHex, oldName: current?.name, newName },
      timestamp: Date.now(),
    });
  }

  /** Add a tag to a bowtie. */
  addTag(eventIdHex: string, tag: string): void {
    const id = this._makeId();
    this._edits.set(`addTag:${eventIdHex}:${tag}`, {
      id,
      kind: { type: 'addTag', eventIdHex, tag },
      timestamp: Date.now(),
    });
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
  }

  /** Classify an ambiguous event slot role. */
  classifyRole(key: string, role: 'Producer' | 'Consumer'): void {
    const id = this._makeId();
    this._edits.set(`classify:${key}`, {
      id,
      kind: { type: 'classifyRole', key, role },
      timestamp: Date.now(),
    });
  }

  /** Re-classify an existing role classification. */
  reclassifyRole(key: string, newRole: 'Producer' | 'Consumer'): void {
    this.classifyRole(key, newRole);
  }

  /**
   * T047: Re-key a planning bowtie from a placeholder event ID to the real
   * adopted event ID (when the first element is added to a name-only bowtie).
   */
  adoptEventId(placeholderHex: string, realEventIdHex: string): void {
    // Re-key the create edit
    const createEdit = this._edits.get(`create:${placeholderHex}`);
    if (createEdit?.kind.type === 'create') {
      // Add a delete edit for the placeholder so save sends the deletion.
      this._edits.set(`delete:${placeholderHex}`, {
        id: this._makeId(),
        kind: { type: 'delete', eventIdHex: placeholderHex },
        timestamp: Date.now(),
      });
      this._edits.delete(`create:${placeholderHex}`);
      this._edits.set(`create:${realEventIdHex}`, {
        ...createEdit,
        kind: { ...createEdit.kind, eventIdHex: realEventIdHex },
      });
    } else {
      // The planning bowtie was loaded from the layout file — no in-session
      // create edit exists. Add a delete for the old placeholder key and a
      // create for the real event ID (preserving the original name).
      const existingMeta = layoutStore.layout?.bowties[placeholderHex];
      this._edits.set(`delete:${placeholderHex}`, {
        id: this._makeId(),
        kind: { type: 'delete', eventIdHex: placeholderHex },
        timestamp: Date.now(),
      });
      this._edits.set(`create:${realEventIdHex}`, {
        id: this._makeId(),
        kind: {
          type: 'create',
          eventIdHex: realEventIdHex,
          name: existingMeta?.name,
          createdByFacility: existingMeta?.createdByFacility,
        },
        timestamp: Date.now(),
      });
    }
    // Re-key any rename edit
    const renameEdit = this._edits.get(`rename:${placeholderHex}`);
    if (renameEdit?.kind.type === 'rename') {
      this._edits.delete(`rename:${placeholderHex}`);
      this._edits.set(`rename:${realEventIdHex}`, {
        ...renameEdit,
        kind: { ...renameEdit.kind, eventIdHex: realEventIdHex },
      });
    }
    // Re-key addTag / removeTag edits
    for (const [key, edit] of [...this._edits.entries()]) {
      if (
        (edit.kind.type === 'addTag' || edit.kind.type === 'removeTag') &&
        edit.kind.eventIdHex === placeholderHex
      ) {
        const newKey = key.replace(placeholderHex, realEventIdHex);
        this._edits.delete(key);
        this._edits.set(newKey, {
          ...edit,
          kind: { ...edit.kind, eventIdHex: realEventIdHex },
        });
      }
    }
  }

  /**
   * Demote a bowtie from an active/incomplete state back to planning by
   * replacing its real event ID key with a fresh `planning-` placeholder.
   *
   * Used when the user removes the last element from a bowtie but chooses
   * to keep the bowtie as a planning entry. The node's event slot is left
   * unchanged — no hardware write is needed.
   */
  demoteToPlanningBowtie(eventIdHex: string): void {
    const placeholderHex = `planning-${Date.now()}`;
    const existingMeta = this._getEffectiveMetadata(eventIdHex);

    // Remove any in-session create/rename/tag edits for the old event ID
    this._edits.delete(`create:${eventIdHex}`);
    this._edits.delete(`rename:${eventIdHex}`);
    for (const key of [...this._edits.keys()]) {
      const edit = this._edits.get(key)!;
      if (
        (edit.kind.type === 'addTag' || edit.kind.type === 'removeTag') &&
        edit.kind.eventIdHex === eventIdHex
      ) {
        this._edits.delete(key);
      }
    }

    // Delete the real event ID from the layout
    this._edits.set(`delete:${eventIdHex}`, {
      id: this._makeId(),
      kind: { type: 'delete', eventIdHex },
      timestamp: Date.now(),
    });

    // Create a fresh planning entry preserving the bowtie's name and tags
    this._edits.set(`create:${placeholderHex}`, {
      id: this._makeId(),
      kind: {
        type: 'create',
        eventIdHex: placeholderHex,
        name: existingMeta?.name,
        createdByFacility: existingMeta?.createdByFacility,
      },
      timestamp: Date.now(),
    });
    for (const tag of existingMeta?.tags ?? []) {
      this._edits.set(`addTag:${placeholderHex}:${tag}`, {
        id: this._makeId(),
        kind: { type: 'addTag', eventIdHex: placeholderHex, tag },
        timestamp: Date.now(),
      });
    }
  }

  /** Clear all pending metadata edits (used by discard). */
  clearAll(): void {
    // SvelteMap.clear() is natively reactive — all subscribers re-evaluate.
    this._edits.clear();
  }

  resetForNewLayout(): void {
    this.clearAll();
  }

  // ── Queries ────────────────────────────────────────────────────────────────

  /**
   * Get the effective metadata for a bowtie, combining the loaded layout
   * with any pending edits.
   */
  getMetadata(eventIdHex: string): BowtieMetadata | undefined {
    return this._getEffectiveMetadata(eventIdHex);
  }

  /**
   * True when a `delete:<eventIdHex>` edit is pending for this bowtie.
   *
   * Display surfaces consult this through `effectiveLayoutStore` so that
   * deleted bowties disappear from the panel immediately (ADR-0004, Bug 3),
   * before the next save persists the deletion.
   */
  hasPendingDeletion(eventIdHex: string): boolean {
    return this._edits.has(`delete:${eventIdHex}`);
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

  /**
   * Collect pending edits as backend-ready `LayoutEditDelta[]` (ADR-0002).
   *
   * Converts the internal BowtieEditKind discriminated union into the
   * format expected by the Rust `LayoutEditDelta` enum (camelCase tagged).
   */
  collectDeltas(): LayoutEditDelta[] {
    const deltas: LayoutEditDelta[] = [];
    for (const edit of this._edits.values()) {
      switch (edit.kind.type) {
        case 'create':
          deltas.push({
            type: 'createBowtie',
            eventIdHex: edit.kind.eventIdHex,
            name: edit.kind.name,
            createdByFacility: edit.kind.createdByFacility ?? null,
          });
          break;
        case 'delete':
          deltas.push({ type: 'deleteBowtie', eventIdHex: edit.kind.eventIdHex });
          break;
        case 'rename':
          deltas.push({ type: 'renameBowtie', eventIdHex: edit.kind.eventIdHex, newName: edit.kind.newName });
          break;
        case 'addTag':
          deltas.push({ type: 'addTag', eventIdHex: edit.kind.eventIdHex, tag: edit.kind.tag });
          break;
        case 'removeTag':
          deltas.push({ type: 'removeTag', eventIdHex: edit.kind.eventIdHex, tag: edit.kind.tag });
          break;
        case 'classifyRole':
          deltas.push({ type: 'classifyRole', key: edit.kind.key, role: edit.kind.role });
          break;
      }
    }
    return deltas;
  }

  /**
   * Return which metadata fields have pending (unsaved) edits for a given event ID.
   * Driven by the _edits map directly, not by diffing the layout.
   */
  getDirtyFields(eventIdHex: string): Set<string> {
    const fields = new Set<string>();
    if (this._edits.has(`create:${eventIdHex}`)) {
      fields.add('name');
    }
    if (this._edits.has(`rename:${eventIdHex}`)) {
      fields.add('name');
    }
    for (const edit of this._edits.values()) {
      if ((edit.kind.type === 'addTag' || edit.kind.type === 'removeTag') &&
          edit.kind.eventIdHex === eventIdHex) {
        fields.add('tags');
      }
    }
    return fields;
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
      meta = {
        name: createEdit.kind.name,
        tags: [],
        createdByFacility: createEdit.kind.createdByFacility,
      };
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
   * Spec 018 / S6 (D1) — return the event-id hex strings of every bowtie whose
   * effective metadata carries `createdByFacility === facilityId`. Merges the
   * loaded layout with pending create/delete edits so the reader is a single
   * source of truth for teardown + the composed-cards catalog surface.
   */
  bowtiesForFacility(facilityId: string): string[] {
    const seen = new Set<string>();
    const results: string[] = [];

    // Layout baseline.
    const layout = layoutStore.layout;
    if (layout) {
      for (const [hex, meta] of Object.entries(layout.bowties)) {
        if (meta.createdByFacility === facilityId && !this.hasPendingDeletion(hex)) {
          seen.add(hex);
          results.push(hex);
        }
      }
    }

    // Pending create edits.
    for (const edit of this._edits.values()) {
      if (edit.kind.type !== 'create') continue;
      if (edit.kind.createdByFacility !== facilityId) continue;
      const hex = edit.kind.eventIdHex;
      if (seen.has(hex)) continue;
      if (this.hasPendingDeletion(hex)) continue;
      seen.add(hex);
      results.push(hex);
    }

    return results;
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

/** Global singleton — import this in components and the save handler. */
export const bowtieMetadataStore = new BowtieMetadataStore();
