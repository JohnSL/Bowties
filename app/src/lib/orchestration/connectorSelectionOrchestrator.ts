import { setModifiedValue } from '$lib/api/config';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import type {
  ConnectorConstraintScalar,
  ConnectorProfileView,
  ConnectorRepairRuleView,
  ConnectorSelectedDefaultView,
  ConnectorSelectionDocument,
  SlotSupportedDaughterboardView,
  StagedRepair,
} from '$lib/types/connectorProfile';
import {
  effectiveValue,
  isGroup,
  isLeaf,
  type ConfigNode,
  type LeafConfigNode,
  type NodeConfigTree,
  type TreeConfigValue,
} from '$lib/types/nodeTree';
import { evaluateConnectorConstraintsForPath } from '$lib/utils/connectorConstraints';
import { formatEventIdHex, parseEventIdHex } from '$lib/utils/serialize';

interface CompatibilityState {
  stagedRepairs: StagedRepair[];
  warnings: string[];
}

export async function applyConnectorSelectionChange(detail: {
  nodeId: string;
  slotId: string;
  selectedDaughterboardId: string | null;
}): Promise<ConnectorSelectionDocument | null> {
  const saved = await connectorSelectionsStore.updateSlotSelection(
    detail.nodeId,
    detail.slotId,
    detail.selectedDaughterboardId,
  );

  if (!saved) {
    return null;
  }

  await recomputeConnectorCompatibility(detail.nodeId);
  return saved;
}

export async function recomputeConnectorCompatibility(nodeId: string): Promise<void> {
  const tree = nodeTreeStore.getTree(nodeId) ?? null;
  const profile = connectorSelectionsStore.getProfile(nodeId) ?? tree?.connectorProfile ?? null;
  const document = connectorSelectionsStore.getDocument(nodeId);
  const previousRepairs = connectorSelectionsStore.getStagedRepairs(nodeId);

  if (!tree || !profile || !document) {
    connectorSelectionsStore.setCompatibilityPreview(nodeId, [], []);
    if (layoutStore.isOfflineMode) {
      offlineChangesStore.replaceConnectorGeneratedConfigChanges(nodeId, []);
      nodeTreeStore.restampOfflinePendingValues(offlineChangesStore.effectiveRows);
    } else {
      await clearOnlineRepairs(nodeId, tree, previousRepairs);
    }
    return;
  }

  const compatibilityState = computeConnectorCompatibilityState(tree, profile, document);
  connectorSelectionsStore.setCompatibilityPreview(
    nodeId,
    compatibilityState.stagedRepairs,
    compatibilityState.warnings,
  );

  if (layoutStore.isOfflineMode) {
    offlineChangesStore.replaceConnectorGeneratedConfigChanges(nodeId, compatibilityState.stagedRepairs);
    nodeTreeStore.restampOfflinePendingValues(offlineChangesStore.effectiveRows);
    return;
  }

  await stageOnlineRepairs(nodeId, tree, previousRepairs, compatibilityState.stagedRepairs);
}

export function computeConnectorCompatibilityState(
  tree: NodeConfigTree,
  profile: ConnectorProfileView,
  document: ConnectorSelectionDocument,
): CompatibilityState {
  const stagedRepairs = new Map<string, StagedRepair>();
  const warnings = new Set<string>();
  const leaves = collectLeaves(tree.segments.flatMap((segment) => segment.children));

  for (const selection of document.slotSelections) {
    const slotId = selection.slotId;
    const supportedDaughterboard = resolveSelectedDaughterboard(profile, document, slotId);

    for (const leaf of leaves) {
      const state = evaluateConnectorConstraintsForPath(profile, document, leaf.path);
      if (state.slotId !== slotId) {
        continue;
      }

      const currentValue = effectiveValue(leaf);
      if (!currentValue || isValueCompatibleWithState(leaf, currentValue, state)) {
        continue;
      }

      const repair = resolveRepairForLeaf(leaf, state, slotId, supportedDaughterboard);
      if (!repair) {
        warnings.add(`No compatible auto-repair was found for ${leaf.name} on ${selection.slotId}.`);
        continue;
      }

      const repairKey = `${repair.space ?? ''}:${repair.offset ?? ''}:${repair.targetPath}`;
      stagedRepairs.set(repairKey, repair);
    }
  }

  return {
    stagedRepairs: [...stagedRepairs.values()],
    warnings: [...warnings],
  };
}

function resolveSelectedDaughterboard(
  profile: ConnectorProfileView,
  document: ConnectorSelectionDocument,
  slotId: string,
): SlotSupportedDaughterboardView | null {
  const slot = profile.slots.find((candidate) => candidate.slotId === slotId);
  const selection = document.slotSelections.find((candidate) => candidate.slotId === slotId);
  if (!slot || !selection?.selectedDaughterboardId || selection.status !== 'selected') {
    return null;
  }

  return slot.supportedDaughterboardConstraints?.find(
    (candidate) => candidate.daughterboardId === selection.selectedDaughterboardId,
  ) ?? null;
}

function resolveRepairForLeaf(
  leaf: LeafConfigNode,
  state: ReturnType<typeof evaluateConnectorConstraintsForPath>,
  slotId: string,
  supportedDaughterboard: SlotSupportedDaughterboardView | null,
): StagedRepair | null {
  const currentValue = effectiveValue(leaf);
  if (!currentValue) {
    return null;
  }

  const authoredRule = findMatchingRepairRule(leaf, supportedDaughterboard?.repairRules ?? []);
  const selectedDefault = findMatchingSelectedDefault(leaf, supportedDaughterboard?.defaultsWhenSelected ?? []);

  const resolved = authoredRule
    ? resolveAuthoredRepair(leaf, state, authoredRule)
    : resolveFallbackRepair(leaf, state, selectedDefault);
  if (!resolved) {
    return null;
  }

  const currentSerialized = serializeTreeConfigValue(currentValue);
  const nextSerialized = serializeTreeConfigValue(resolved.value);
  if (currentSerialized === nextSerialized) {
    return null;
  }

  return {
    targetPath: leaf.path.join('/'),
    space: leaf.space,
    offset: toOffsetHex(leaf.address),
    baselineValue: serializeTreeConfigValue(leaf.value ?? currentValue),
    plannedValue: nextSerialized,
    reason: resolved.reason,
    originSlotId: slotId,
  };
}

function findMatchingRepairRule(
  leaf: LeafConfigNode,
  rules: ConnectorRepairRuleView[],
): ConnectorRepairRuleView | null {
  const normalizedLeafPath = stripInstanceSteps(leaf.path);
  return rules.find((rule) => isPathPrefix(rule.resolvedPath, normalizedLeafPath)) ?? null;
}

function findMatchingSelectedDefault(
  leaf: LeafConfigNode,
  defaults: ConnectorSelectedDefaultView[],
): ConnectorSelectedDefaultView | null {
  const normalizedLeafPath = stripInstanceSteps(leaf.path);
  return defaults.find((entry) => isPathPrefix(entry.resolvedPath, normalizedLeafPath)) ?? null;
}

function resolveAuthoredRepair(
  leaf: LeafConfigNode,
  state: ReturnType<typeof evaluateConnectorConstraintsForPath>,
  rule: ConnectorRepairRuleView,
): { value: TreeConfigValue; reason: string } | null {
  switch (rule.replacementStrategy) {
    case 'setExplicit': {
      const explicitValue = toTreeConfigValue(rule.replacementValue, leaf);
      if (explicitValue && isValueCompatibleWithState(leaf, explicitValue, state)) {
        return {
          value: explicitValue,
          reason: 'Auto-staged profile-authored replacement',
        };
      }
      return null;
    }
    case 'resetDefault': {
      const defaultValue = deriveFieldDefaultValue(leaf);
      if (defaultValue && isValueCompatibleWithState(leaf, defaultValue, state)) {
        return {
          value: defaultValue,
          reason: 'Auto-staged reset to field default',
        };
      }
      return null;
    }
    case 'clearEmpty': {
      const clearedValue = deriveEmptyValue(leaf);
      if (clearedValue && isValueCompatibleWithState(leaf, clearedValue, state)) {
        return {
          value: clearedValue,
          reason: 'Auto-staged clear to empty value',
        };
      }
      return null;
    }
  }
}

function resolveFallbackRepair(
  leaf: LeafConfigNode,
  state: ReturnType<typeof evaluateConnectorConstraintsForPath>,
  selectedDefault: ConnectorSelectedDefaultView | null,
): { value: TreeConfigValue; reason: string } | null {
  const candidates: Array<{ value: TreeConfigValue | null; reason: string }> = [
    {
      value: selectedDefault ? toTreeConfigValue(selectedDefault.value, leaf) : null,
      reason: 'Auto-staged selected-hardware default',
    },
    {
      value: deriveFieldDefaultValue(leaf),
      reason: 'Auto-staged field default',
    },
    {
      value: deriveAllowedSubsetValue(leaf, state.allowedValues ?? []),
      reason: 'Auto-staged first compatible allowed value',
    },
    {
      value: deriveEmptyValue(leaf),
      reason: 'Auto-staged empty compatible value',
    },
  ];

  for (const candidate of candidates) {
    if (candidate.value && isValueCompatibleWithState(leaf, candidate.value, state)) {
      return {
        value: candidate.value,
        reason: candidate.reason,
      };
    }
  }

  return null;
}

async function clearOnlineRepairs(
  nodeId: string,
  tree: NodeConfigTree | null,
  repairs: StagedRepair[],
): Promise<void> {
  if (!tree) {
    return;
  }

  for (const repair of repairs) {
    if (repair.space == null || !repair.offset) {
      continue;
    }

    const leaf = nodeTreeStore.getLeafByLocation(nodeId, repair.space, parseOffsetHex(repair.offset));
    if (!leaf?.value) {
      continue;
    }

    const cleared = await setModifiedValue(nodeId, leaf.address, leaf.space, leaf.value);
    if (cleared) {
      nodeTreeStore.setLeafModifiedValue(nodeId, leaf.path, null);
    }
  }
}

async function stageOnlineRepairs(
  nodeId: string,
  tree: NodeConfigTree,
  previousRepairs: StagedRepair[],
  nextRepairs: StagedRepair[],
): Promise<void> {
  const nextKeys = new Set(nextRepairs.map((repair) => `${repair.space ?? ''}:${repair.offset ?? ''}`));

  for (const repair of previousRepairs) {
    const repairKey = `${repair.space ?? ''}:${repair.offset ?? ''}`;
    if (nextKeys.has(repairKey)) {
      continue;
    }
  }

  await clearOnlineRepairs(nodeId, tree, previousRepairs.filter((repair) => !nextKeys.has(`${repair.space ?? ''}:${repair.offset ?? ''}`)));

  for (const repair of nextRepairs) {
    if (repair.space == null || !repair.offset) {
      continue;
    }

    const leaf = nodeTreeStore.getLeafByLocation(nodeId, repair.space, parseOffsetHex(repair.offset));
    if (!leaf) {
      continue;
    }

    const nextValue = parseSerializedValueForLeaf(repair.plannedValue, leaf);
    if (!nextValue) {
      continue;
    }

    const staged = await setModifiedValue(nodeId, leaf.address, leaf.space, nextValue);
    if (staged) {
      nodeTreeStore.setLeafModifiedValue(nodeId, leaf.path, nextValue);
    }
  }
}

function collectLeaves(children: ConfigNode[]): LeafConfigNode[] {
  const leaves: LeafConfigNode[] = [];
  for (const child of children) {
    if (isLeaf(child)) {
      leaves.push(child);
      continue;
    }

    if (isGroup(child)) {
      leaves.push(...collectLeaves(child.children));
    }
  }
  return leaves;
}

function stripInstanceSteps(path: string[]): string[] {
  return path.map((step) => {
    if (!step.startsWith('elem:')) {
      return step;
    }

    const hashIndex = step.indexOf('#');
    return hashIndex >= 0 ? step.slice(0, hashIndex) : step;
  });
}

function isPathPrefix(prefix: string[], fullPath: string[]): boolean {
  if (prefix.length > fullPath.length) {
    return false;
  }

  return prefix.every((step, index) => fullPath[index] === step);
}

function isValueCompatibleWithState(
  leaf: LeafConfigNode,
  value: TreeConfigValue,
  state: ReturnType<typeof evaluateConnectorConstraintsForPath>,
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

function deriveFieldDefaultValue(leaf: LeafConfigNode): TreeConfigValue | null {
  const rawDefault = leaf.constraints?.defaultValue;
  if (!rawDefault) {
    return null;
  }

  return parseSerializedValueForLeaf(rawDefault, leaf);
}

function deriveEmptyValue(leaf: LeafConfigNode): TreeConfigValue | null {
  switch (leaf.elementType) {
    case 'int':
      return { type: 'int', value: 0 };
    case 'float':
      return { type: 'float', value: 0 };
    case 'string':
      return { type: 'string', value: '' };
    case 'eventId': {
      const bytes = new Array(8).fill(0);
      return { type: 'eventId', bytes, hex: formatEventIdHex(bytes) };
    }
    default:
      return null;
  }
}

function toTreeConfigValue(rawValue: unknown, leaf: LeafConfigNode): TreeConfigValue | null {
  if (rawValue == null) {
    return null;
  }

  if (leaf.elementType === 'int') {
    if (typeof rawValue === 'number') {
      return { type: 'int', value: Math.trunc(rawValue) };
    }
    if (typeof rawValue === 'string' && /^-?\d+$/.test(rawValue.trim())) {
      return { type: 'int', value: Number.parseInt(rawValue, 10) };
    }
  }

  if (leaf.elementType === 'float') {
    if (typeof rawValue === 'number') {
      return { type: 'float', value: rawValue };
    }
    if (typeof rawValue === 'string' && /^-?\d+(?:\.\d+)?$/.test(rawValue.trim())) {
      return { type: 'float', value: Number.parseFloat(rawValue) };
    }
  }

  if (leaf.elementType === 'string') {
    return { type: 'string', value: String(rawValue) };
  }

  if (leaf.elementType === 'eventId') {
    if (Array.isArray(rawValue) && rawValue.length === 8 && rawValue.every((byte) => typeof byte === 'number')) {
      const bytes = rawValue.map((byte) => Number(byte));
      return { type: 'eventId', bytes, hex: formatEventIdHex(bytes) };
    }
    if (typeof rawValue === 'string') {
      const bytes = parseEventIdHex(rawValue);
      if (bytes) {
        return { type: 'eventId', bytes, hex: formatEventIdHex(bytes) };
      }
    }
  }

  return null;
}

function serializeTreeConfigValue(value: TreeConfigValue): string {
  switch (value.type) {
    case 'int':
      return `${value.value}`;
    case 'float':
      return `${value.value}`;
    case 'string':
      return value.value;
    case 'eventId':
      return value.hex;
  }
}

function parseSerializedValueForLeaf(
  serializedValue: string,
  leaf: LeafConfigNode,
): TreeConfigValue | null {
  return toTreeConfigValue(serializedValue, leaf);
}

function toOffsetHex(address: number): string {
  return `0x${address.toString(16).toUpperCase().padStart(8, '0')}`;
}

function parseOffsetHex(offset: string): number {
  return Number.parseInt(offset.replace(/^0x/i, ''), 16);
}