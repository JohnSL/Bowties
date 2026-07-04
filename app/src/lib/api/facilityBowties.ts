/**
 * Spec 018 / S6 (D2) — TS wrapper around the `compose_facility_bowties` IPC.
 *
 * Returns the ordered list of composition ops the frontend orchestrator
 * dispatches: each op writes the producer's event ID onto the consumer's
 * CDI leaf (via `configEditor.applyEdit`) and registers a matching bowtie
 * metadata row (via `bowtieMetadataStore.createBowtie`).
 *
 * See the `bowties-core::facility_bowties` module for the composition rules.
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * A single composition op — one leaf write + one bowtie registration.
 *
 * `eventIdBytes` is the producer's existing event ID adopted verbatim
 * (D6 — LCC producer-identifies / consumer-subscribes). `consumerLeafPath`
 * is the CDI path shape produced by `resolve_lamp_row_path_prefix` (name
 * segments, not index paths).
 */
export interface CompositionOp {
  consumerNodeKey: string;
  consumerLeafPath: string[];
  consumerLeafSpace: number;
  consumerLeafAddress: number;
  eventIdBytes: number[];
  bowtieName: string;
  createdByFacility: string;
}

/**
 * Ask the backend to compose the bowtie ops for the given Wired facility.
 *
 * Rejects (via the Tauri error channel) when the facility is unknown, is
 * still Incomplete, or a bound channel's CDI does not resolve. Callers
 * gate the call on `effectiveLayoutStore.facilityStatus === 'Wired'` so
 * the not-wired error path is defensive rather than expected.
 */
export async function composeFacilityBowties(facilityId: string): Promise<CompositionOp[]> {
  return invoke<CompositionOp[]>('compose_facility_bowties', { facilityId });
}
