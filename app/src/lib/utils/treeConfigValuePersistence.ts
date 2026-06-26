import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import { canonicalEventIdHex, parseEventIdHex } from '$lib/utils/serialize';

export function treeConfigValueToOfflineString(value: TreeConfigValue): string {
  switch (value.type) {
    case 'string':
      return value.value;
    case 'int':
      return String(value.value);
    case 'float':
      return String(value.value);
    case 'eventId':
      return value.hex;
  }
}

export function parseOfflineStoredValueForLeaf(
  leaf: Pick<LeafConfigNode, 'elementType'>,
  raw: string,
): TreeConfigValue | null {
  if (leaf.elementType === 'string') {
    return { type: 'string', value: raw };
  }
  if (leaf.elementType === 'int') {
    const parsed = Number.parseInt(raw, 10);
    return Number.isNaN(parsed) ? null : { type: 'int', value: parsed };
  }
  if (leaf.elementType === 'float') {
    const parsed = Number.parseFloat(raw);
    return Number.isNaN(parsed) ? null : { type: 'float', value: parsed };
  }
  if (leaf.elementType === 'eventId') {
    // Accept both canonical contiguous and legacy dotted formats
    const bytes = parseEventIdHex(raw);
    if (!bytes) return null;
    return { type: 'eventId', bytes, hex: canonicalEventIdHex(bytes) };
  }
  return null;
}