import { invoke } from '@tauri-apps/api/core';

/// One slot's binding: null = empty, string = channel ID.
export type SlotBinding = string | null;

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
