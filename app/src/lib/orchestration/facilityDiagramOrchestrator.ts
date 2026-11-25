import type { Facility } from '$lib/api/facilities';
import { previewBlockSignals } from '$lib/api/signaling';
import { syncDraftsForComposition } from '$lib/orchestration/facilityOrchestrator';
import { facilityDiagramsStore as store } from '$lib/stores/facilityDiagrams.svelte';
import { previewRequestFromRecord } from '$lib/utils/blockSignalsPreviewRequest';

/**
 * Owns fetching the backend preview that feeds each saved Automatic Block
 * Signals facility's Railroad-tab diagram. A facility is fetched when its
 * saved record first appears or changes; only the newest response per
 * facility is accepted, and a layout reset discards any in flight.
 */
class FacilityDiagramOrchestrator {
  private epoch = 0;
  private generations = new Map<string, number>();

  /** Bring the store in line with `facilities`; resolves when every started fetch has settled. */
  async sync(facilities: readonly Facility[]): Promise<void> {
    const fetches: Promise<void>[] = [];
    const present = new Set<string>();
    for (const facility of facilities) {
      const record = facility.blockSignals;
      if (!record) continue;
      present.add(facility.facilityId);
      const key = JSON.stringify([facility.facilityId, facility.name, record]);
      if (store.get(facility.facilityId)?.key === key) continue;
      store.set(facility.facilityId, { status: 'loading', key });
      fetches.push(this.fetch(facility.facilityId, facility.name, key, record));
    }
    store.retain(present);
    for (const id of [...this.generations.keys()]) if (!present.has(id)) this.generations.delete(id);
    await Promise.all(fetches);
  }

  resetForNewLayout(): void {
    this.epoch += 1;
    this.generations.clear();
    store.reset();
  }

  private async fetch(
    facilityId: string,
    name: string,
    key: string,
    record: NonNullable<Facility['blockSignals']>,
  ): Promise<void> {
    const epoch = this.epoch;
    const generation = (this.generations.get(facilityId) ?? 0) + 1;
    this.generations.set(facilityId, generation);
    const current = () => epoch === this.epoch && this.generations.get(facilityId) === generation;
    try {
      await syncDraftsForComposition();
      const preview = await previewBlockSignals(previewRequestFromRecord(facilityId, name, record));
      if (current()) store.set(facilityId, { status: 'ready', key, preview });
    } catch (error) {
      if (current()) {
        store.set(facilityId, { status: 'failed', key, error: error instanceof Error ? error.message : String(error) });
      }
    }
  }
}

export const facilityDiagramOrchestrator = new FacilityDiagramOrchestrator();
