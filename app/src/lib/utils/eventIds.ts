/**
 * Event ID utilities — generation of fresh unique event IDs for a node.
 */

import { collectEventIdLeaves, effectiveValue } from '$lib/types/nodeTree';
import type { NodeConfigTree } from '$lib/types/nodeTree';

/**
 * Generate a fresh unique event ID for a given node, avoiding all existing
 * event IDs already used in that node's tree.
 *
 * Algorithm:
 *  1. Parse nodeId (dotted-hex "XX.XX.XX.XX.XX.XX") → 6 nodeBytes
 *  2. Walk all eventId leaves via collectEventIdLeaves, extracting effective values
 *  3. Collect 16-bit counters (bytes[6]<<8 | bytes[7]) of IDs whose first 6 bytes match nodeBytes
 *  4. New counter = max + 1; if > 0xFFFF, scan backwards for first unused
 *  5. Return [...nodeBytes, counter >> 8, counter & 0xFF] as dotted-hex
 */
export function generateFreshEventIdForNode(nodeId: string, tree: NodeConfigTree): string {
  const nodeBytes = nodeId.split('.').map(h => parseInt(h, 16));

  const leaves = collectEventIdLeaves(tree);
  const usedCounters = new Set<number>();

  for (const leaf of leaves) {
    const val = effectiveValue(leaf);
    if (val?.type !== 'eventId') continue;
    const bytes = val.bytes;
    if (bytes.length >= 8 && nodeBytes.every((b, i) => b === bytes[i])) {
      const counter = (bytes[6] << 8) | bytes[7];
      if (counter !== 0) usedCounters.add(counter);
    }
  }

  let counter: number;
  if (usedCounters.size === 0) {
    counter = 1;
  } else {
    const max = Math.max(...usedCounters);
    if (max < 0xFFFF) {
      counter = max + 1;
    } else {
      // Scan backwards for the first unused value
      counter = 1;
      for (let c = 0xFFFE; c >= 1; c--) {
        if (!usedCounters.has(c)) {
          counter = c;
          break;
        }
      }
    }
  }

  const resultBytes = [...nodeBytes, counter >> 8, counter & 0xFF];
  return resultBytes.map(b => b.toString(16).toUpperCase().padStart(2, '0')).join('.');
}
