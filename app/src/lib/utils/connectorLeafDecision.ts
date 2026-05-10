import type { ConnectorConstraintScalar } from '$lib/types/connectorProfile';
import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import type { ConnectorConstraintState } from '$lib/utils/connectorConstraints';
import { formatEventIdHex, parseEventIdHex } from '$lib/utils/serialize';

export type ConnectorLeafDecision =
  | { kind: 'compatible' }
  | { kind: 'autoCorrect'; nextValue: TreeConfigValue }
  | { kind: 'unsupported'; reason: string };

export function decideConnectorLeafValue(args: {
  leaf: LeafConfigNode;
  currentValue: TreeConfigValue | null;
  constraintState: ConnectorConstraintState;
}): ConnectorLeafDecision {
  const { leaf, currentValue, constraintState } = args;

  if (!currentValue || !constraintState.slotId || isValueCompatibleWithState(leaf, currentValue, constraintState)) {
    return { kind: 'compatible' };
  }

  const nextValue = deriveAllowedSubsetValue(leaf, constraintState.allowedValues ?? []);
  if (!nextValue || serializeTreeConfigValue(nextValue) === serializeTreeConfigValue(currentValue)) {
    return {
      kind: 'unsupported',
      reason: 'No compatible allowed value could be derived for this leaf.',
    };
  }

  return {
    kind: 'autoCorrect',
    nextValue,
  };
}

function isValueCompatibleWithState(
  leaf: LeafConfigNode,
  value: TreeConfigValue,
  state: ConnectorConstraintState,
): boolean {
  const scalars = valueToConstraintScalars(leaf, value);
  const denied = new Set(state.deniedValues ?? []);
  if (scalars.some((scalar) => denied.has(scalar))) {
    return false;
  }

  if (!state.allowedValues?.length) {
    return true;
  }

  const allowed = new Set(state.allowedValues);
  return scalars.some((scalar) => allowed.has(scalar));
}

function valueToConstraintScalars(
  leaf: LeafConfigNode,
  value: TreeConfigValue,
): ConnectorConstraintScalar[] {
  switch (value.type) {
    case 'int': {
      const scalars: ConnectorConstraintScalar[] = [value.value];
      const label = leaf.constraints?.mapEntries?.find((entry) => entry.value === value.value)?.label;
      if (label) {
        scalars.push(label);
      }
      return scalars;
    }
    case 'float':
      return [value.value];
    case 'string':
      return [value.value];
    case 'eventId':
      return [value.hex];
  }
}

function deriveAllowedSubsetValue(
  leaf: LeafConfigNode,
  allowedValues: ConnectorConstraintScalar[],
): TreeConfigValue | null {
  for (const allowedValue of allowedValues) {
    const mappedValue = scalarToTreeConfigValue(allowedValue, leaf);
    if (mappedValue) {
      return mappedValue;
    }
  }

  return null;
}

function scalarToTreeConfigValue(
  scalar: ConnectorConstraintScalar,
  leaf: LeafConfigNode,
): TreeConfigValue | null {
  if (leaf.elementType === 'int') {
    if (typeof scalar === 'number') {
      return { type: 'int', value: scalar };
    }

    const mappedEntry = leaf.constraints?.mapEntries?.find((entry) => entry.label === scalar);
    if (mappedEntry) {
      return { type: 'int', value: mappedEntry.value };
    }
  }

  if (leaf.elementType === 'float' && typeof scalar === 'number') {
    return { type: 'float', value: scalar };
  }

  if (leaf.elementType === 'string' && typeof scalar === 'string') {
    return { type: 'string', value: scalar };
  }

  if (leaf.elementType === 'eventId' && typeof scalar === 'string') {
    const bytes = parseEventIdHex(scalar);
    if (bytes) {
      return { type: 'eventId', bytes, hex: formatEventIdHex(bytes) };
    }
  }

  return null;
}

function serializeTreeConfigValue(value: TreeConfigValue): string {
  switch (value.type) {
    case 'int':
    case 'float':
      return String(value.value);
    case 'string':
      return value.value;
    case 'eventId':
      return value.hex;
  }
}