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
 *
 * Spec 018 / S4 (D8): slot bindings are plural (`string[]`). Empty
 * array = unbound; one or more entries = bound. Block Indicator caps
 * both slots at one channel, but the wire form and store shape are
 * forward-compatible with future multi-channel slots (ABS aspect-slot
 * repeaters). Attach / detach mutations live in
 * `_pendingSlotBindings`, a per-(facility, slot) override bucket that
 * snapshots the desired post-edit `Vec` and is diffed against the
 * baseline at `collectDeltas()` time.
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
  /**
   * Pending slot-binding overrides: facilityId → (slotLabel → post-edit Vec).
   * Pending creations are mutated in place; only baseline facilities use
   * this bucket. Diffed against baseline at `collectDeltas()` time to
   * produce attach / detach deltas (set diff, order-insensitive).
   */
  private _pendingSlotBindings = $state<Map<string, Map<string, string[]>>>(new Map());

  /** Effective view: baseline (minus deletions) + pending creations, with pending renames + slot edits applied. */
  get facilities(): Facility[] {
    const base = this._pendingDeletions.size > 0
      ? this._baseline.filter((f) => !this._pendingDeletions.has(f.facilityId))
      : this._baseline;
    const raw = [...base, ...this._pendingCreations];
    return raw.map((f) => {
      const newName = this._pendingRenames.get(f.facilityId);
      const slotOverrides = this._pendingSlotBindings.get(f.facilityId);
      if (newName === undefined && !slotOverrides) return f;
      const slotBindings: Record<string, SlotBinding> = { ...f.slotBindings };
      if (slotOverrides) {
        for (const [label, vec] of slotOverrides) {
          slotBindings[label] = [...vec];
        }
      }
      return {
        ...f,
        ...(newName !== undefined ? { name: newName } : {}),
        slotBindings,
      };
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
      || this._pendingSlotBindings.size > 0
    );
  }

  get editCount(): number {
    let slotEdits = 0;
    for (const slots of this._pendingSlotBindings.values()) slotEdits += slots.size;
    return (
      this._pendingCreations.length
      + this._pendingRenames.size
      + this._pendingDeletions.size
      + slotEdits
    );
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
  get pendingSlotBindings(): Map<string, Map<string, string[]>> {
    return this._pendingSlotBindings;
  }

  /** Revert all pending edits. */
  discard(): void {
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
    this._pendingSlotBindings = new Map();
  }

  /** Replace the baseline after a successful save; clear all drafts. */
  hydrateBaseline(facilities: Facility[]): void {
    this._baseline = facilities;
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
    this._pendingSlotBindings = new Map();
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
    // Slot bindings: diff each pending Vec against the baseline (or against
    // pending-creation seed) as a set, emitting one Attach/Detach per change.
    for (const [facilityId, slots] of this._pendingSlotBindings) {
      // For pending creations, the seed (empty Vec) is whatever was
      // materialised by addFacility; for baseline facilities, the seed
      // is the on-disk Vec.
      const seedSource =
        this._pendingCreations.find((f) => f.facilityId === facilityId)
        ?? this._baseline.find((f) => f.facilityId === facilityId);
      if (!seedSource) continue;
      for (const [slotLabel, vec] of slots) {
        const seed = seedSource.slotBindings[slotLabel] ?? [];
        const seedSet = new Set(seed);
        const nextSet = new Set(vec);
        // Detach: in seed but not in next.
        for (const id of seed) {
          if (!nextSet.has(id)) {
            deltas.push({ type: 'detachChannelFromSlot', facilityId, slotLabel, channelId: id });
          }
        }
        // Attach: in next but not in seed.
        for (const id of vec) {
          if (!seedSet.has(id)) {
            deltas.push({ type: 'attachChannelToSlot', facilityId, slotLabel, channelId: id });
          }
        }
      }
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
    this._pendingSlotBindings = new Map();
  }

  /**
   * Add a new facility for the given template + name.
   * Materialises empty slot bindings (Spec 018 / S4: `[]` per D8) from the
   * template's slot definitions so the facility is renderable immediately.
   * Returns the newly created facility.
   */
  addFacility(template: BehaviorTemplate, name: string): Facility {
    const slotBindings: Record<string, SlotBinding> = {};
    for (const slot of template.slots) {
      slotBindings[slot.label] = [];
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
    if (this._pendingSlotBindings.has(facilityId)) {
      const next = new Map(this._pendingSlotBindings);
      next.delete(facilityId);
      this._pendingSlotBindings = next;
    }
  }

  /**
   * Attach a channel to a facility slot (Spec 018 / S4 — D8).
   *
   * For pending creations the channel is appended directly into the
   * facility's `slotBindings` Vec and no override bucket is recorded.
   * For baseline facilities the post-edit Vec is staged in
   * `_pendingSlotBindings`; `collectDeltas` emits one
   * `attachChannelToSlot` per net new channel id.
   *
   * Returns `false` on no-op (the channel is already attached in the
   * effective view), `true` otherwise. The store does not enforce the
   * template `maxChannels` cap — that lives in `facilityOrchestrator`
   * (rejection up-front) and in `apply_facility_deltas` (rejection on
   * save). Frontend enforcement is by candidate-picker filter.
   */
  attachChannel(facilityId: string, slotLabel: string, channelId: string): boolean {
    return this._mutateSlot(facilityId, slotLabel, (vec) => {
      if (vec.includes(channelId)) return null;
      return [...vec, channelId];
    });
  }

  /**
   * Detach a channel from a facility slot (Spec 018 / S4 — D8).
   * Returns `false` on no-op (channel already absent), `true` otherwise.
   */
  detachChannel(facilityId: string, slotLabel: string, channelId: string): boolean {
    return this._mutateSlot(facilityId, slotLabel, (vec) => {
      if (!vec.includes(channelId)) return null;
      return vec.filter((id) => id !== channelId);
    });
  }

  /**
   * Internal helper: apply a pure transform to the effective Vec for
   * a (facility, slot). The transform returns `null` to signal no-op,
   * or the new Vec to record / collapse-to-baseline.
   *
   * Pending-creation facilities mutate the creation record in place;
   * baseline facilities stage their post-edit Vec in
   * `_pendingSlotBindings`, collapsing back to a no-op when the result
   * equals the baseline (set-equality, order-insensitive).
   */
  private _mutateSlot(
    facilityId: string,
    slotLabel: string,
    transform: (currentVec: readonly string[]) => string[] | null,
  ): boolean {
    // Pending creation: mutate the creation record in place.
    const creationIdx = this._pendingCreations.findIndex((f) => f.facilityId === facilityId);
    if (creationIdx >= 0) {
      const creation = this._pendingCreations[creationIdx];
      const current = creation.slotBindings[slotLabel] ?? [];
      const next = transform(current);
      if (next === null) return false;
      const updated = {
        ...creation,
        slotBindings: { ...creation.slotBindings, [slotLabel]: next },
      };
      const arr = this._pendingCreations.slice();
      arr[creationIdx] = updated;
      this._pendingCreations = arr;
      return true;
    }

    // Baseline facility: stage the post-edit Vec.
    const baseline = this._baseline.find((f) => f.facilityId === facilityId);
    if (!baseline) return false;
    const baselineVec = baseline.slotBindings[slotLabel] ?? [];
    const overrides = this._pendingSlotBindings.get(facilityId);
    const currentVec = overrides?.get(slotLabel) ?? baselineVec;
    const next = transform(currentVec);
    if (next === null) return false;

    const nextOverrides = new Map(
      [...this._pendingSlotBindings].map(([k, v]) => [k, new Map(v)]),
    );
    if (sameSet(next, baselineVec)) {
      // User reverted to the baseline; drop the override.
      const slotMap = nextOverrides.get(facilityId);
      if (slotMap) {
        slotMap.delete(slotLabel);
        if (slotMap.size === 0) nextOverrides.delete(facilityId);
      }
    } else {
      const slotMap = nextOverrides.get(facilityId) ?? new Map<string, string[]>();
      slotMap.set(slotLabel, next);
      nextOverrides.set(facilityId, slotMap);
    }
    this._pendingSlotBindings = nextOverrides;
    return true;
  }

  /** Clear all facility state. Called on layout close. */
  reset(): void {
    this._baseline = [];
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
    this._pendingSlotBindings = new Map();
  }
}

function sameSet(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false;
  const set = new Set(a);
  for (const x of b) if (!set.has(x)) return false;
  return true;
}

export const facilitiesStore = new FacilitiesStore();
