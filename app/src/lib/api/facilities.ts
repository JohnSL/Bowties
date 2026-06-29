import { invoke } from '@tauri-apps/api/core';

/**
 * Per-slot binding list (Spec 018 / S4 — D8 cardinality contract).
 * Empty array = unbound; one or more entries = bound to those channels.
 * The cap is enforced backend-side per template `SlotDefinition.maxChannels`.
 */
export type SlotBinding = string[];

/// Facility wire form (matches `bowties_core::layout::facilities::Facility`).
export interface Facility {
  facilityId: string;
  templateId: string;
  name: string;
  slotBindings: Record<string, SlotBinding>;
}

/// Derived status of a facility (never persisted on the entity).
export type FacilityStatus = 'Incomplete' | 'Wired';

/// Fetch the facility inventory for the active layout.
export async function listFacilities(): Promise<Facility[]> {
  return invoke<Facility[]>('list_facilities');
}
