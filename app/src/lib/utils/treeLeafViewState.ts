import type { PendingChangeRow } from '$lib/api/sync';
import { effectiveValue, type LeafConfigNode, type TreeConfigValue, type TreeMapEntry } from '$lib/types/nodeTree';
import {
  filterAllowedMapEntries,
  type ConnectorConstraintState,
} from '$lib/utils/connectorConstraints';
import { decideConnectorLeafValue } from '$lib/utils/connectorLeafDecision';
import {
  parseOfflineStoredValueForLeaf,
  treeConfigValueToOfflineString,
} from '$lib/utils/treeConfigValuePersistence';

export interface LeafOfflineValueState {
  offlinePlannedValue: TreeConfigValue | null;
  displayValue: TreeConfigValue | null;
}

export interface LeafSelectViewState {
  selectedValue: number;
  managedMapEntries: TreeMapEntry[];
  currentValueCompatibilityMessage: string | null;
  currentSelectFallbackLabel: string | null;
}

export function resolveConnectorCompatibilityMessage(args: {
  leaf: LeafConfigNode;
  displayValue: TreeConfigValue | null;
  connectorConstraintState: ConnectorConstraintState | null | undefined;
}): string | null {
  if (!args.connectorConstraintState?.slotId) {
    return null;
  }

  const decision = decideConnectorLeafValue({
    leaf: args.leaf,
    currentValue: args.displayValue,
    constraintState: args.connectorConstraintState,
  });

  return decision.kind === 'unsupported'
    ? 'Current value is incompatible with selected daughterboard'
    : null;
}

export function resolveLeafOfflineValueState(args: {
  leaf: LeafConfigNode;
  effectiveOfflineRow: PendingChangeRow | null;
}): LeafOfflineValueState {
  const offlinePlannedValue = args.effectiveOfflineRow
    ? parseOfflineStoredValueForLeaf(args.leaf, args.effectiveOfflineRow.plannedValue)
    : null;

  return {
    offlinePlannedValue,
    displayValue: offlinePlannedValue ?? effectiveValue(args.leaf),
  };
}

/**
 * The reason a field's waiting Pending Change row was not written to the board
 * (status failed or conflict), or null when the row is plain pending (shown as
 * "saved in layout, pending apply") or absent. E3b D3: the row is the only
 * record of a write result.
 */
export function resolveLeafNotWrittenMessage(row: PendingChangeRow | null): string | null {
  if (!row || (row.status !== 'failed' && row.status !== 'conflict')) {
    return null;
  }
  return row.error?.trim() || 'Not written to the board';
}

export function resolveLeafSelectViewState(args: {
  leaf: LeafConfigNode;
  displayValue: TreeConfigValue | null;
  connectorConstraintState: ConnectorConstraintState | null | undefined;
}): LeafSelectViewState {
  const managedMapEntries = filterAllowedMapEntries(
    args.leaf.constraints?.mapEntries,
    args.connectorConstraintState,
  );
  const selectedValue = args.displayValue?.type === 'int'
    ? args.displayValue.value
    : args.leaf.value?.type === 'int'
      ? args.leaf.value.value
      : 0;
  const currentUnfilteredMapEntry = args.leaf.constraints?.mapEntries?.find(
    (entry) => entry.value === selectedValue,
  ) ?? null;
  const currentValueCompatibilityMessage = resolveConnectorCompatibilityMessage(args);

  if (managedMapEntries.some((entry) => entry.value === selectedValue)) {
    return {
      selectedValue,
      managedMapEntries,
      currentValueCompatibilityMessage,
      currentSelectFallbackLabel: null,
    };
  }

  return {
    selectedValue,
    managedMapEntries,
    currentValueCompatibilityMessage,
    currentSelectFallbackLabel: currentUnfilteredMapEntry?.label ?? `(Reserved: ${selectedValue})`,
  };
}