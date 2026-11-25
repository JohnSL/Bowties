import type { BlockSignalsPreview } from '$lib/api/signaling';

/** The Railroad-tab diagram state of one saved Automatic Block Signals facility. */
export interface FacilityDiagramEntry {
  status: 'loading' | 'ready' | 'failed';
  /** Identifies the saved record this entry was requested for. */
  key: string;
  preview?: BlockSignalsPreview;
  error?: string;
}

/**
 * Backend previews of saved Automatic Block Signals facilities, by facility ID.
 * Derived state only: nothing here is persisted; the facility diagram
 * orchestrator fills and prunes it.
 */
class FacilityDiagramsStore {
  private _entries = $state<Record<string, FacilityDiagramEntry>>({});

  get(facilityId: string): FacilityDiagramEntry | undefined {
    return this._entries[facilityId];
  }

  set(facilityId: string, entry: FacilityDiagramEntry): void {
    this._entries = { ...this._entries, [facilityId]: entry };
  }

  /** Keep only the entries whose facility ID is in `facilityIds`. */
  retain(facilityIds: ReadonlySet<string>): void {
    this._entries = Object.fromEntries(Object.entries(this._entries).filter(([id]) => facilityIds.has(id)));
  }

  reset(): void {
    this._entries = {};
  }

  resetForNewLayout(): void { this.reset(); }
}

export const facilityDiagramsStore = new FacilityDiagramsStore();
