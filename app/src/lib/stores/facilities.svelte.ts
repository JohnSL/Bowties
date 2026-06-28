import { listFacilities, type Facility, type SlotBinding } from '$lib/api/facilities';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { LayoutEditDelta } from '$lib/types/bowtie';
import { generateUuidV4 } from '$lib/utils/uuid';

/**
 * Facility CRUD store with the ADR-0012 draft-layer contract.
 *
 * Mirrors `channelsStore`'s structure: a baseline loaded from disk plus
 * pending creations / renames / deletions held in memory. Mutations
 * never write through — `collectDeltas()` produces `LayoutEditDelta[]`
 * for the save orchestrator to apply atomically with the rest of the
 * layout.
 *
 * Slot bindings are materialised on `addFacility` from the supplied
 * template, so the in-memory facility view is renderable immediately
 * without a save round-trip (per S1 acceptance).
 */
class FacilitiesStore {
  /** Facilities loaded from disk (baseline). */
  private _baseline = $state<Facility[]>([]);
  /** Facilities created in-memory since last save (drafts). */
  private _pendingCreations = $state<Facility[]>([]);
  /** Pending renames: facilityId → new name. */
  private _pendingRenames = $state<Map<string, string>>(new Map());
  /** IDs of baseline facilities pending deletion. */
  private _pendingDeletions = $state<Set<string>>(new Set());

  /** Effective view: baseline (minus deletions) + pending creations, with pending renames applied. */
  get facilities(): Facility[] {
    const base = this._pendingDeletions.size > 0
      ? this._baseline.filter((f) => !this._pendingDeletions.has(f.facilityId))
      : this._baseline;
    const raw = [...base, ...this._pendingCreations];
    if (this._pendingRenames.size === 0) return raw;
    return raw.map((f) => {
      const newName = this._pendingRenames.get(f.facilityId);
      return newName !== undefined ? { ...f, name: newName } : f;
    });
  }

  get isEmpty(): boolean {
    return this.facilities.length === 0;
  }

  // ── ADR-0012: Draft lifecycle ───────────────────────────────────────────

  get isDirty(): boolean {
    return (
      this._pendingCreations.length > 0
      || this._pendingRenames.size > 0
      || this._pendingDeletions.size > 0
    );
  }

  get editCount(): number {
    return this._pendingCreations.length + this._pendingRenames.size + this._pendingDeletions.size;
  }

  get pendingCreations(): Facility[] {
    return this._pendingCreations;
  }
  get pendingRenames(): Map<string, string> {
    return this._pendingRenames;
  }
  get pendingDeletions(): Set<string> {
    return this._pendingDeletions;
  }

  /** Revert all pending edits. */
  discard(): void {
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
  }

  /** Replace the baseline after a successful save; clear all drafts. */
  hydrateBaseline(facilities: Facility[]): void {
    this._baseline = facilities;
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
  }

  /** Collect deltas for the save orchestrator. */
  collectDeltas(): LayoutEditDelta[] {
    const deltas: LayoutEditDelta[] = [];
    for (const facility of this._pendingCreations) {
      // Apply any pending rename to the created facility before emitting,
      // so the on-disk record matches the in-memory effective view.
      const renamed = this._pendingRenames.get(facility.facilityId);
      const payload = renamed !== undefined ? { ...facility, name: renamed } : facility;
      deltas.push({ type: 'addFacility', facility: payload });
    }
    for (const [id, newName] of this._pendingRenames) {
      // Renames against baseline facilities (pending-creation renames are
      // already folded into the addFacility deltas above).
      if (this._pendingCreations.some((f) => f.facilityId === id)) continue;
      deltas.push({ type: 'renameFacility', facilityId: id, newName });
    }
    for (const id of this._pendingDeletions) {
      deltas.push({ type: 'deleteFacility', facilityId: id });
    }
    return deltas;
  }

  // ── Operations ──────────────────────────────────────────────────────────

  /** Load facilities from the backend baseline. */
  async loadFacilities(): Promise<void> {
    this._baseline = await listFacilities();
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
  }

  /**
   * Add a new facility for the given template + name.
   * Materialises empty slot bindings from the template's slot definitions
   * so the facility is renderable immediately.
   * Returns the newly created facility.
   */
  addFacility(template: BehaviorTemplate, name: string): Facility {
    const slotBindings: Record<string, SlotBinding> = {};
    for (const slot of template.slots) {
      slotBindings[slot.label] = null;
    }
    const facility: Facility = {
      facilityId: generateUuidV4(),
      templateId: template.templateId,
      name: name.trim(),
      slotBindings,
    };
    this._pendingCreations = [...this._pendingCreations, facility];
    return facility;
  }

  /**
   * Rename a facility. Empty/whitespace-only names are rejected.
   * No-op suppression (ADR-0012 2026-06-25 extension): if the new name
   * equals the current effective name, no draft is recorded. If the
   * user reverts to the baseline name, any prior pending rename is
   * cleared. Returns true if the change was accepted (or suppressed
   * as a no-op against the effective view), false on rejection.
   */
  renameFacility(facilityId: string, newName: string): boolean {
    const trimmed = newName.trim();
    if (trimmed.length === 0) return false;

    // Pending creation: update the in-place creation record (no rename delta needed).
    const creationIdx = this._pendingCreations.findIndex((f) => f.facilityId === facilityId);
    if (creationIdx >= 0) {
      if (this._pendingCreations[creationIdx].name === trimmed) return false;
      const next = this._pendingCreations.slice();
      next[creationIdx] = { ...next[creationIdx], name: trimmed };
      this._pendingCreations = next;
      return true;
    }

    // Baseline facility: no-op suppression against the current effective view.
    const effective = this.facilities.find((f) => f.facilityId === facilityId);
    if (effective && effective.name === trimmed) return false;

    const baseline = this._baseline.find((f) => f.facilityId === facilityId);
    if (baseline && baseline.name === trimmed) {
      // User reverted to the baseline name → drop any pending rename.
      if (this._pendingRenames.has(facilityId)) {
        const next = new Map(this._pendingRenames);
        next.delete(facilityId);
        this._pendingRenames = next;
      }
      return true;
    }

    this._pendingRenames = new Map(this._pendingRenames).set(facilityId, trimmed);
    return true;
  }

  /**
   * Mark a facility for deletion. Pending creations are removed
   * immediately (never persisted); baseline facilities are tracked
   * in `_pendingDeletions` for save flush.
   */
  deleteFacility(facilityId: string): void {
    if (this._pendingCreations.some((f) => f.facilityId === facilityId)) {
      this._pendingCreations = this._pendingCreations.filter((f) => f.facilityId !== facilityId);
    }
    if (this._baseline.some((f) => f.facilityId === facilityId)) {
      this._pendingDeletions = new Set([...this._pendingDeletions, facilityId]);
    }
    if (this._pendingRenames.has(facilityId)) {
      const next = new Map(this._pendingRenames);
      next.delete(facilityId);
      this._pendingRenames = next;
    }
  }

  /** Clear all facility state. Called on layout close. */
  reset(): void {
    this._baseline = [];
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
  }
}

export const facilitiesStore = new FacilitiesStore();
