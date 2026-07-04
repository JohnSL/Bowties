/**
 * Spec 018 / S6 (D3) — Hardware Channel Cascade orchestrator.
 *
 * Subscribes to `channelsStore.channels` and cascades the loss of a
 * hardware-owned channel into every facility slot referencing it. On
 * each disappearance:
 *
 *   1. Every facility slot binding pointing at the lost channel is
 *      detached via `facilitiesStore.detachChannel`.
 *   2. If a facility transitioned Wired → Incomplete as a result of
 *      those detaches, its composed bowties are torn down via
 *      `facilityOrchestrator.tearDownFacilityBowties`.
 *
 * All effects are staged in the draft layer (per ADR-0012 extension —
 * cascade side effects appear next to their trigger, atomic on Save,
 * revertable on Discard). No IPC calls fire from here directly; the
 * detach + teardown side effects flow through the normal store draft
 * pipeline. User-owned channel losses via `removeFromSlot` do not
 * re-trigger this cascade because the detach they cause is already
 * inside the same draft transaction (diff-based mechanism handles
 * this naturally).
 */

import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import * as facilityOrchestrator from '$lib/orchestration/facilityOrchestrator';

class FacilityCascadeOrchestrator {
  /** IDs of channels observed at the last synchronization point. */
  private _lastSeenIds = new Set<string>();
  /** Disposer returned by `$effect.root`, or `null` when stopped. */
  private _dispose: (() => void) | null = null;

  /**
   * Start the cascade subscription. Idempotent: repeated calls without an
   * intervening `stopCascade()` are no-ops so the layout-open path can
   * safely re-invoke it. Mount this exactly once per layout open (see
   * `+page.svelte`).
   */
  startCascade(): void {
    if (this._dispose !== null) return;
    // Seed the baseline from the current store snapshot.
    this._lastSeenIds = new Set(channelsStore.channels.map((c) => c.id));
    this._dispose = $effect.root(() => {
      $effect(() => {
        const currentIds = new Set(channelsStore.channels.map((c) => c.id));
        this._reconcile(currentIds);
      });
    });
  }

  /**
   * Tear down the cascade subscription (called by
   * `layoutLifecycleOrchestrator.resetForNewLayout`).
   */
  stopCascade(): void {
    this._dispose?.();
    this._dispose = null;
    this._lastSeenIds = new Set();
  }

  resetForNewLayout(): void {
    this.stopCascade();
  }

  /**
   * Test seam: force a diff pass against a caller-supplied id set. Used by
   * the T14 unit tests instead of driving the `$effect.root` reactive path
   * (which requires a browser + tick discipline that the vitest harness
   * doesn't set up here).
   */
  reconcile(currentIds: ReadonlySet<string>): void {
    this._reconcile(currentIds);
  }

  /**
   * Load-time repair (2026-07-03): stage `detachChannelFromSlot` drafts
   * for every facility slot binding that points at a channel absent from
   * the current channel inventory. The pre-018 layout store hydration
   * IPC (`list_facilities`) does not normalise the on-disk facilities
   * doc against `channels.yaml`, so a layout that survived the earlier
   * split-write channel-save bug carried orphan bindings into the
   * frontend baseline. Without this pass the ghost id counts against
   * the slot cap, keeps the facility "Wired" against a phantom
   * consumer, and never appears as an unsaved change. Staging the
   * detach as a normal draft resolves all three symptoms in one place
   * and lets the fix ride the standard save flow (ADR-0012).
   *
   * Idempotent: subsequent calls after the drafts are staged see the
   * cleaned effective view and no-op.
   */
  reconcileDanglingChannelRefsOnLoad(): void {
    const known = new Set(channelsStore.channels.map((c) => c.id));
    const dangling = new Set<string>();
    for (const facility of facilitiesStore.facilities) {
      for (const ids of Object.values(facility.slotBindings)) {
        for (const id of ids) {
          if (!known.has(id)) dangling.add(id);
        }
      }
    }
    if (dangling.size === 0) return;
    for (const id of dangling) {
      console.warn(
        '[facility] load-time repair: staged detach for unknown channel',
        id,
      );
    }
    this._cascadeDetach(dangling);
  }

  private _reconcile(currentIds: ReadonlySet<string>): void {
    const lost = new Set<string>();
    for (const id of this._lastSeenIds) {
      if (!currentIds.has(id)) lost.add(id);
    }
    // Advance the seen-set BEFORE running side effects so a store write
    // triggered by a side effect (e.g. a facility detach that removes a
    // user-owned channel) doesn't re-enter this diff.
    this._lastSeenIds = new Set(currentIds);
    if (lost.size === 0) return;
    this._cascadeDetach(lost);
  }

  /**
   * Detach every facility slot binding referencing an id in `lostIds` and
   * tear down the composed bowties of any facility that transitioned
   * Wired → Incomplete as a result. Shared by the reactive `_reconcile`
   * path (runtime channel loss) and `reconcileDanglingChannelRefsOnLoad`
   * (load-time repair of orphan references).
   */
  private _cascadeDetach(lostIds: ReadonlySet<string>): void {
    // Collect (facilityId, slotLabel, channelId) triples to detach.
    const detachTargets: Array<{ facilityId: string; slotLabel: string; channelId: string }> = [];
    const affectedFacilities = new Set<string>();
    for (const facility of facilitiesStore.facilities) {
      for (const [slotLabel, ids] of Object.entries(facility.slotBindings)) {
        for (const id of ids) {
          if (lostIds.has(id)) {
            detachTargets.push({ facilityId: facility.facilityId, slotLabel, channelId: id });
            affectedFacilities.add(facility.facilityId);
          }
        }
      }
    }
    if (detachTargets.length === 0) return;

    // Snapshot which facilities were Wired BEFORE the detaches so we can
    // decide who needs teardown afterwards.
    const wiredBefore = new Set(
      [...affectedFacilities].filter((id) => effectiveLayoutStore.facilityStatus(id) === 'Wired'),
    );

    for (const t of detachTargets) {
      facilitiesStore.detachChannel(t.facilityId, t.slotLabel, t.channelId);
    }

    // For every facility that transitioned Wired → Incomplete, tear down
    // its composed bowties. Fire-and-forget: teardown is idempotent and its
    // side effects are all draft-layer.
    for (const facilityId of affectedFacilities) {
      if (!wiredBefore.has(facilityId)) continue;
      if (effectiveLayoutStore.facilityStatus(facilityId) === 'Wired') continue;
      void facilityOrchestrator.tearDownFacilityBowties(facilityId);
    }
  }
}

export const facilityCascadeOrchestrator = new FacilityCascadeOrchestrator();
