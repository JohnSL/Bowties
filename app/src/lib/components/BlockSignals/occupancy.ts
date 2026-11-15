/**
 * Live block occupancy as the Automatic Block Signals editor shows it.
 * `unavailable` covers every case without a live reading: no occupied/clear
 * event received yet, offline, or a channel without configured events. It is
 * never shown as if it were live.
 */
import type { ChannelState } from '$lib/utils/channelState';

export type BlockOccupancy = 'occupied' | 'clear' | 'unavailable';

/** Block id → occupancy; a block that isn't in the map is unavailable. */
export type BlockOccupancyMap = ReadonlyMap<string, BlockOccupancy>;

export const OCCUPANCY_LABEL: Record<BlockOccupancy, string> = {
  occupied: 'Occupied',
  clear: 'Clear',
  unavailable: 'Occupancy unavailable',
};

export function occupancyFromChannelState(state: ChannelState): BlockOccupancy {
  if ('kind' in state || state.role !== 'block-occupancy') return 'unavailable';
  return state.state;
}

export function occupancyOf(map: BlockOccupancyMap | undefined, blockId: string): BlockOccupancy {
  return map?.get(blockId) ?? 'unavailable';
}

/** Status shown beside the track diagram: live once any listed block has a reading. */
export function occupancyStatus(map: BlockOccupancyMap | undefined, blockIds: string[]): string {
  return blockIds.some((id) => occupancyOf(map, id) !== 'unavailable') ? 'Live occupancy' : 'Occupancy unavailable';
}
