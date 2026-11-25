/**
 * Pure layout for the Automatic Block Signals track drawing, shared by the
 * Blocks page (blocks, turnouts and boundaries) and the Aspects page (a
 * selectable signal per signal block).
 *
 * Every x is computed in list order, left to right. When the list runs
 * westward the drawing is mirrored, so the first block is on the right; use
 * {@link mirrorX} / {@link mirrorRect} / {@link mirrorAnchor} when rendering.
 */
import type {
  ListedBlock,
  Movements,
  SignalPreview,
  TravelDirection,
} from '$lib/api/signaling';

/** Drawing-only turnout shape, relative to the list (it turns with the list). */
export interface TurnoutShape {
  /** Which end of the turnout the points are at, relative to the list's left-to-right order. */
  face: 'west' | 'east';
  side: 'above' | 'below';
}

export const DEFAULT_TURNOUT_SHAPE: TurnoutShape = { face: 'west', side: 'below' };

export type TrackSelection =
  | { kind: 'block'; blockId: string }
  | { kind: 'turnout'; turnoutId: string }
  | null;

export interface TrackInput {
  mode: 'blocks' | 'signals';
  blocks: ListedBlock[];
  direction: TravelDirection;
  movements: Movements;
  signals: SignalPreview[];
  shapes: Record<string, TurnoutShape>;
  /** Blocks mode: the selected list row. */
  selection?: TrackSelection;
  /** Signals mode: the selected signal. */
  selectedSignalId?: string | null;
  /** Signals mode: number of items to resolve per signal. */
  issueCounts?: Record<string, number>;
  /** Text shown at each end, or null for nothing (Blocks shows "+ Plant"). */
  endLabels: { start: string | null; end: string | null };
}

export interface DrawnSignal {
  /** Present for a real signal; absent for a ghost. */
  signalId?: string;
  name: string;
  /** Joint x the signal stands at. */
  jx: number;
  /** +1 when the signal faces list order, -1 against it. */
  k: 1 | -1;
  /** +1 below the track (eastward signals), -1 above (westward). */
  sy: 1 | -1;
}

export interface DrawnTurnout {
  turnoutId: string;
  name: string;
  blockName: string;
  x: number;
  /** +1 when the leg leaves towards list order, -1 against it. */
  dir: 1 | -1;
  sy: 1 | -1;
  selected: boolean;
  /** For a siding pair, the partner's x; drawn once, from the west-facing turnout. */
  sidingToX: number | null;
  paired: boolean;
}

export interface TrackModel {
  width: number;
  height: number;
  railY: number;
  flip: boolean;
  gap: number;
  joints: number[];
  shade: { x0: number; x1: number } | null;
  blocks: Array<{ blockId: string; name: string; cx: number }>;
  turnouts: DrawnTurnout[];
  signals: Array<DrawnSignal & { signalId: string; selected: boolean; issueCount: number; protects: string }>;
  ghosts: Array<{ blockId: string; between: string; signals: DrawnSignal[] }>;
  ends: Array<{ end: 'start' | 'end'; cx: number; label: string | null }>;
  arrows: { west: boolean; east: boolean };
  /** Room under signal names for a "to resolve" line on the Aspects page. */
  extra: number;
}

const PAD = 104;
const GAP = 5;

function opposite(direction: TravelDirection): TravelDirection {
  return direction === 'eastward' ? 'westward' : 'eastward';
}

/** The default name core gives a signal, used for ghosts that have no signal. */
export function defaultSignalName(blockName: string, governs: TravelDirection): string {
  return `${blockName} ${governs === 'eastward' ? 'East' : 'West'}`;
}

/** Signal blocks as lists of block indexes, in list order. */
export function signalBlocks(blocks: ListedBlock[]): number[][] {
  const groups: number[][] = [];
  blocks.forEach((block, index) => {
    if (index > 0 && block.joinedToPrevious) groups[groups.length - 1].push(index);
    else groups.push([index]);
  });
  return groups;
}

export function buildTrackModel(input: TrackInput): TrackModel {
  const { blocks, direction, mode } = input;
  const blocksMode = mode === 'blocks';
  const both = input.movements === 'bothDirections';
  const flip = direction === 'westward';
  const geo = {
    east: direction === 'eastward' || both,
    west: direction === 'westward' || both,
  };
  const extra = blocksMode ? 0 : 12;
  const railY = geo.west ? 72 + extra : 42;
  // a signal box with a "N to resolve" line is 10px taller than the 40px box
  const issueRoom = !blocksMode && Object.values(input.issueCounts ?? {}).some((n) => n > 0) ? 6 : 0;
  const height = geo.east ? railY + 52 + extra + issueRoom : railY + 28;

  // block widths grow with their turnouts; turnouts are spaced evenly, clear of the centre
  const joints: number[] = [];
  const turnoutX = new Map<string, number>();
  let x = PAD;
  blocks.forEach((block, index) => {
    joints[index] = x;
    const n = block.turnouts.length;
    const width = Math.max(180, 90 + 100 * n);
    const slots = Array.from({ length: n + 1 }, (_, k) => x + ((k + 1) * width) / (n + 2));
    slots.splice(Math.floor((n + 1) / 2), 1);
    block.turnouts.forEach((turnout, k) => turnoutX.set(turnout.turnoutId, slots[k]));
    x += width;
  });
  joints[blocks.length] = x;
  const width = x + PAD;

  const groups = signalBlocks(blocks);
  const selection = blocksMode ? input.selection ?? null : null;
  const selectedTurnoutId = selection?.kind === 'turnout' ? selection.turnoutId : null;
  const selectedBlockIndex = selection?.kind === 'block'
    ? blocks.findIndex((block) => block.blockId === selection.blockId)
    : selectedTurnoutId
      ? blocks.findIndex((block) => block.turnouts.some((t) => t.turnoutId === selectedTurnoutId))
      : -1;
  let shade: TrackModel['shade'] = null;
  if (selectedBlockIndex >= 0) {
    const group = groups.find((g) => g.includes(selectedBlockIndex))!;
    shade = { x0: joints[group[0]] + GAP, x1: joints[group[group.length - 1] + 1] - GAP };
  }

  // turnouts, with siding pairs joined
  const ordered = blocks.flatMap((block) =>
    block.turnouts.map((turnout) => ({ turnout, block, shape: input.shapes[turnout.turnoutId] ?? DEFAULT_TURNOUT_SHAPE })),
  );
  const partner = new Map<string, string>();
  for (let k = 0; k < ordered.length - 1; k++) {
    const t = ordered[k], u = ordered[k + 1];
    if (
      t.shape.face === 'west' && u.shape.face === 'east' && t.shape.side === u.shape.side
      && turnoutX.get(u.turnout.turnoutId)! - turnoutX.get(t.turnout.turnoutId)! >= 70
      && !partner.has(t.turnout.turnoutId)
    ) {
      partner.set(t.turnout.turnoutId, u.turnout.turnoutId);
      partner.set(u.turnout.turnoutId, t.turnout.turnoutId);
    }
  }
  const turnouts: DrawnTurnout[] = ordered.map(({ turnout, block, shape }) => {
    const mate = partner.get(turnout.turnoutId);
    return {
      turnoutId: turnout.turnoutId,
      name: turnout.name,
      blockName: block.name,
      x: turnoutX.get(turnout.turnoutId)!,
      dir: shape.face === 'west' ? 1 : -1,
      sy: shape.side === 'below' ? 1 : -1,
      selected: turnout.turnoutId === selectedTurnoutId,
      paired: mate !== undefined,
      sidingToX: mate !== undefined && shape.face === 'west' ? turnoutX.get(mate)! : null,
    };
  });

  // derived signals: with-list signals at the list-start end of their signal block, against-list at the other
  const indexOf = new Map(blocks.map((block, index) => [block.blockId, index]));
  const nameOf = new Map(blocks.map((block) => [block.blockId, block.name]));
  const signals = input.signals.flatMap((signal) => {
    const index = indexOf.get(signal.entranceBlockId);
    if (index === undefined) return [];
    const withList = signal.run === 'withList';
    return [{
      signalId: signal.signalId,
      name: signal.name,
      jx: withList ? joints[index] : joints[index + 1],
      k: (withList ? 1 : -1) as 1 | -1,
      sy: (signal.governs === 'eastward' ? 1 : -1) as 1 | -1,
      selected: signal.signalId === input.selectedSignalId,
      issueCount: input.issueCounts?.[signal.signalId] ?? 0,
      protects: signal.protectedBlockIds.map((id) => nameOf.get(id) ?? id).join(', '),
    }];
  });

  // ghost signals where a boundary has its signals turned off
  const ghosts = blocksMode
    ? blocks.flatMap((block, index) => {
      if (index === 0 || !block.joinedToPrevious) return [];
      const ghostSignals: DrawnSignal[] = [{
        name: defaultSignalName(block.name, direction), jx: joints[index], k: 1, sy: direction === 'eastward' ? 1 : -1,
      }];
      if (both) {
        const back = opposite(direction);
        ghostSignals.push({
          name: defaultSignalName(blocks[index - 1].name, back), jx: joints[index], k: -1, sy: back === 'eastward' ? 1 : -1,
        });
      }
      return [{ blockId: block.blockId, between: `${blocks[index - 1].name} and ${block.name}`, signals: ghostSignals }];
    })
    : [];

  return {
    width,
    height,
    railY,
    flip,
    gap: GAP,
    joints,
    shade,
    blocks: blocks.map((block, index) => ({
      blockId: block.blockId,
      name: block.name,
      cx: (joints[index] + joints[index + 1]) / 2,
    })),
    turnouts,
    signals,
    ghosts,
    ends: [
      { end: 'start', cx: (10 + joints[0] - GAP) / 2, label: input.endLabels.start },
      { end: 'end', cx: (joints[blocks.length] + GAP + width - 10) / 2, label: input.endLabels.end },
    ],
    arrows: geo,
    extra,
  };
}

/** Mirror an x coordinate for a westward list. */
export function mirrorX(model: TrackModel, x: number): number {
  return model.flip ? model.width - x : x;
}

/** Mirror a rectangle's left edge for a westward list. */
export function mirrorRect(model: TrackModel, x: number, width: number): number {
  return model.flip ? model.width - x - width : x;
}

/** Swap start/end text anchors for a westward list. */
export function mirrorAnchor(model: TrackModel, anchor: 'start' | 'middle' | 'end'): 'start' | 'middle' | 'end' {
  if (!model.flip || anchor === 'middle') return anchor;
  return anchor === 'start' ? 'end' : 'start';
}
