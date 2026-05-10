import { normalizeNodeId } from '$lib/utils/nodeId';

class ConnectorSlotFocusStore {
  private _focusedByNode = $state<Map<string, string>>(new Map());

  getFocusedSlot(nodeId: string): string | null {
    return this._focusedByNode.get(normalizeNodeId(nodeId)) ?? null;
  }

  setFocusedSlot(nodeId: string, slotId: string): void {
    const nodeKey = normalizeNodeId(nodeId);
    const nextMap = new Map(this._focusedByNode);
    nextMap.set(nodeKey, slotId);
    this._focusedByNode = nextMap;
  }

  reset(): void {
    this._focusedByNode = new Map();
  }
}

export const connectorSlotFocusStore = new ConnectorSlotFocusStore();
