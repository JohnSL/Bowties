import { setModifiedValue } from '$lib/api/config';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import { normalizeNodeId } from '$lib/utils/nodeId';
import { editKeyForLeaf } from '$lib/utils/editKey';
import type {
  ConnectorConstraintScalar,
  ConnectorProfileView,
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
import { decideConnectorLeafValue } from '$lib/utils/connectorLeafDecision';
import { formatEventIdHex, parseEventIdHex } from '$lib/utils/serialize';

interface CompatibilityState {
  stagedRepairs: StagedRepair[];
  warnings: string[];
}

const connectorSelectionQueue = new Map<string, Promise<void>>();

function runConnectorSelectionExclusive<T>(nodeId: string, operation: () => Promise<T>): Promise<T> {
  const nodeKey = normalizeNodeId(nodeId);
  const prior = connectorSelectionQueue.get(nodeKey) ?? Promise.resolve();

  const next = prior.catch(() => undefined).then(operation);
  let tracked: Promise<void>;
  tracked = next
    .then(() => undefined)
    .catch(() => undefined)
    .finally(() => {
      if (connectorSelectionQueue.get(nodeKey) === tracked) {
        connectorSelectionQueue.delete(nodeKey);
      }
    });

  connectorSelectionQueue.set(nodeKey, tracked);
  return next;
}

export async function applyConnectorSelectionChange(detail: {
  nodeId: string;
  slotId: string;
  selectedDaughterboardId: string | null;
}): Promise<ConnectorSelectionDocument | null> {
  return runConnectorSelectionExclusive(detail.nodeId, async () => {
    const saved = await connectorSelectionsStore.updateSlotSelection(
      detail.nodeId,
      detail.slotId,
      detail.selectedDaughterboardId,
    );

    if (!saved) {
      return null;
    }

    // Spec 014 / S6: a Configuration Mode variant change re-runs
    // `annotate_tree` server-side, so re-fetch the node tree to pick up
    // the re-shaped relevance + event-role annotations.
    await nodeTreeStore.refreshTree(detail.nodeId);

    await recomputeConnectorCompatibility(detail.nodeId);
    return saved;
  });
}

export async function recomputeConnectorCompatibility(nodeId: string): Promise<void> {
  const tree = nodeTreeStore.getTree(nodeId) ?? null;
  const profile = connectorSelectionsStore.getProfile(nodeId) ?? tree?.connectorProfile ?? null;
  const document = connectorSelectionsStore.getDocument(nodeId);

  if (!tree || !profile || !document) {
    connectorSelectionsStore.setCompatibilityWarnings(nodeId, []);
    return;
  }

  // In offline mode, clear stale offlineChangesStore draft rows for
  // connector-governed leaves before computing compatibility. Without this,
  // cancellation drafts from a previous connector change suppress persisted
  // pending values and prevent repairs on re-selection.
  if (layoutStore.isOfflineMode) {
    clearOfflineConnectorDrafts(nodeId, tree, profile, document);
  }

  const resolveCurrentValue = layoutStore.isOfflineMode
    ? (leaf: LeafConfigNode) => offlineChangesStore.resolveEffectiveCurrentValue(nodeId, leaf)
    : (leaf: LeafConfigNode) => resolveOnlineCurrentValue(nodeId, leaf);

  const compatibilityState = computeConnectorCompatibilityState(
    tree,
    profile,
    document,
    resolveCurrentValue,
  );
  connectorSelectionsStore.setCompatibilityWarnings(nodeId, compatibilityState.warnings);

  if (layoutStore.isOfflineMode) {
    offlineChangesStore.applyConnectorCompatibilityConfigChanges(nodeId, compatibilityState.stagedRepairs);
    applyOfflineCompatibilityDrafts(nodeId, tree, compatibilityState.stagedRepairs);
    return;
  }

  await applyOnlineCompatibilityEdits(nodeId, tree, compatibilityState.stagedRepairs);
}

export function computeConnectorCompatibilityState(
  tree: NodeConfigTree,
  profile: ConnectorProfileView,
  document: ConnectorSelectionDocument,
  resolveCurrentValue: (leaf: LeafConfigNode) => TreeConfigValue | null = effectiveValue,
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

      const currentValue = resolveCurrentValue(leaf);
      const decision = decideConnectorLeafValue({
        leaf,
        currentValue,
        constraintState: state,
      });

      if (decision.kind === 'compatible') {
        continue;
      }

      if (decision.kind === 'unsupported') {
        warnings.add(`${decision.reason} (${leaf.name} on ${selection.slotId}).`);
        continue;
      }

      const repair = resolveRepairForLeaf(leaf, currentValue, decision.nextValue, slotId, supportedDaughterboard);

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
  currentValue: TreeConfigValue | null,
  nextValue: TreeConfigValue,
  slotId: string,
  _supportedDaughterboard: SlotSupportedDaughterboardView | null,
): StagedRepair {
  if (!currentValue) {
    throw new Error('resolveRepairForLeaf requires a current value');
  }

  const currentSerialized = serializeTreeConfigValue(currentValue);
  const nextSerialized = serializeTreeConfigValue(nextValue);

  return {
    targetPath: leaf.path.join('/'),
    space: leaf.space,
    offset: toOffsetHex(leaf.address),
    baselineValue: serializeTreeConfigValue(leaf.value ?? currentValue),
    plannedValue: nextSerialized,
    reason: 'Auto-staged first compatible allowed value',
    originSlotId: slotId,
  };
}

async function applyOnlineCompatibilityEdits(
  nodeId: string,
  tree: NodeConfigTree,
  nextRepairs: StagedRepair[],
): Promise<void> {
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

    const key = editKeyForLeaf(nodeId, leaf.space, leaf.address);
    configChangesStore.set(key, nextValue);

    try {
      const staged = await setModifiedValue(nodeId, leaf.address, leaf.space, nextValue);
      if (!staged) {
        configChangesStore.revert(key);
      }
    } catch (error) {
      configChangesStore.revert(key);
      throw error;
    }
  }
}

/**
 * Write offline connector repairs to the config-draft layer so they appear
 * as visible change badges with "from → to" annotations.
 *
 * The offlineChangesStore owns persistence staging; this writes the same
 * repair value to configChangesStore so the display pipeline shows the
 * change relative to the persisted offline pending layer.
 */
function applyOfflineCompatibilityDrafts(
  nodeId: string,
  tree: NodeConfigTree,
  repairs: StagedRepair[],
): void {
  for (const repair of repairs) {
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

    const key = editKeyForLeaf(nodeId, leaf.space, leaf.address);
    configChangesStore.set(key, nextValue);
  }
}

/**
 * Resolve the current value for a leaf in online mode, preferring any
 * in-flight config draft over the committed baseline.
 *
 * Without this, multi-step connector changes evaluate compatibility against
 * the committed value and miss incompatible drafts from prior repairs.
 */
function resolveOnlineCurrentValue(nodeId: string, leaf: LeafConfigNode): TreeConfigValue | null {
  const key = editKeyForLeaf(nodeId, leaf.space, leaf.address);
  const visible = configChangesStore.visibleValue(key);
  return visible ?? effectiveValue(leaf);
}

/**
 * Clear offlineChangesStore draft rows for connector-governed leaves.
 *
 * Called before computing compatibility so that stale cancellation drafts
 * from a previous connector change don't suppress persisted pending values.
 * The repair computation then sees the persisted values and can stage fresh
 * repairs as needed.
 */
function clearOfflineConnectorDrafts(
  nodeId: string,
  tree: NodeConfigTree,
  profile: ConnectorProfileView,
  document: ConnectorSelectionDocument,
): void {
  const leaves = collectLeaves(tree.segments.flatMap((segment) => segment.children));
  const locations: { space: number; offset: string }[] = [];
  for (const leaf of leaves) {
    const state = evaluateConnectorConstraintsForPath(profile, document, leaf.path);
    if (!state.slotId) continue;
    locations.push({ space: leaf.space, offset: toOffsetHex(leaf.address) });
  }
  offlineChangesStore.clearDraftConfigChanges(nodeId, locations);
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