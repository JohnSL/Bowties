import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';

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
    const bytes = raw.split('.').map((part) => Number.parseInt(part, 16));
    if (bytes.length !== 8 || bytes.some((part) => Number.isNaN(part) || part < 0 || part > 255)) {
      return null;
    }
    return { type: 'eventId', bytes, hex: raw };
  }
  return null;
}