/**
 * The one place a TrackDiagram input is assembled from a line's facts, shared
 * by the editor steps (Blocks, Aspects, Wiring) and by read-only summaries of
 * a saved facility. A saved facility's preview already has these fields, so it
 * can be passed as the line with no turnout shapes.
 */
import type {
  EntranceEnd,
  ExitEnd,
  ListedBlock,
  Movements,
  SignalPreview,
  TravelDirection,
} from '$lib/api/signaling';
import { endChipLabel } from './aspectText';
import type { TrackInput, TurnoutShape } from './trackGeometry';

export interface TrackLine {
  blocks: ListedBlock[];
  direction: TravelDirection;
  movements: Movements;
  entranceEnd: EntranceEnd;
  exitEnd: ExitEnd;
  signals: SignalPreview[];
  /** Drawing-only turnout shapes; omit for a saved facility (drawn with defaults). */
  shapes?: Record<string, TurnoutShape>;
}

export type TrackInputExtras = Pick<TrackInput, 'selection' | 'selectedSignalId' | 'issueCounts'>;

export function trackInputFor(line: TrackLine, mode: TrackInput['mode'], extras: TrackInputExtras = {}): TrackInput {
  return {
    mode,
    blocks: line.blocks,
    direction: line.direction,
    movements: line.movements,
    signals: line.signals,
    shapes: line.shapes ?? {},
    ...extras,
    endLabels: {
      start: endChipLabel('start', line.entranceEnd, line.exitEnd),
      end: endChipLabel('end', line.entranceEnd, line.exitEnd),
    },
  };
}
