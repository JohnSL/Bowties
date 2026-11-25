import type { BlockSignalsRecord } from '$lib/api/facilities';
import type { BlockSignalsPreviewRequest } from '$lib/api/signaling';

/**
 * The preview request for a saved Automatic Block Signals facility: the one
 * mapping from a stored record to the backend preview input, shared by the
 * editor (when opened on a saved facility) and the Railroad tab's diagram.
 * A saved facility has no per-signal overrides or released owners.
 */
export function previewRequestFromRecord(
  facilityId: string,
  facilityName: string,
  record: BlockSignalsRecord,
): BlockSignalsPreviewRequest {
  return {
    facilityId,
    facilityName,
    direction: record.direction,
    movements: record.movements,
    blocks: record.blocks.map((block) => ({ ...block, turnouts: block.turnouts.map((t) => ({ ...t })) })),
    entranceEnd: record.entranceEnd,
    exitEnd: record.exitEnd,
    settings: { ...record.settings },
    signalOverrides: [],
    signalNames: Object.fromEntries(record.signals.map((s) => [s.signalId, s.name])),
    release: [],
  };
}
