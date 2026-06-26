import type {
  ConnectorConstraintRuleView,
  ConnectorConstraintScalar,
  ConnectorProfileView,
  ConnectorSelectionDocument,
  ConnectorSlotView,
} from '$lib/types/connectorProfile';
import type { TreeMapEntry } from '$lib/types/nodeTree';

export interface ConnectorConstraintState {
  slotId: string | null;
  hidden: boolean;
  disabled: boolean;
  readOnly: boolean;
  allowedValues: ConnectorConstraintScalar[] | null;
  deniedValues: ConnectorConstraintScalar[];
  explanations: string[];
}

export function evaluateConnectorConstraintsForPath(
  profile: ConnectorProfileView | null,
  document: ConnectorSelectionDocument | null,
  path: string[],
): ConnectorConstraintState {
  const initialState: ConnectorConstraintState = {
    slotId: null,
    hidden: false,
    disabled: false,
    readOnly: false,
    allowedValues: null,
    deniedValues: [],
    explanations: [],
  };

  if (!profile) {
    return initialState;
  }

  const slotMatch = findSlotForPath(profile.slots, path);
  if (!slotMatch) {
    return initialState;
  }

  const { slot, affectedPathOrdinal } = slotMatch;

  const selection = document?.slotSelections.find((candidate) => candidate.slotId === slot.slotId);
  const selectedDaughterboardId = selection?.selectedDaughterboardId;
  const state: ConnectorConstraintState = {
    ...initialState,
    slotId: slot.slotId,
  };

  if (!selectedDaughterboardId || selection?.status !== 'selected') {
    applyEmptyBehavior(slot, state);
    return finalizeState(state);
  }

  const daughterboard = profile.supportedDaughterboards?.find(
    (candidate) => candidate.daughterboardId === selectedDaughterboardId,
  );
  const validityRules = slot.supportedDaughterboardConstraints
    ?.find((candidate) => candidate.daughterboardId === selectedDaughterboardId)
    ?.validityRules
    ?? daughterboard?.validityRules;

  if (!validityRules?.length) {
    return state;
  }

  const normalizedPath = stripInstanceSteps(path);

  for (const rule of validityRules) {
    if (!ruleMatchesPath(rule, normalizedPath, affectedPathOrdinal, path)) {
      continue;
    }

    applyRule(rule, state);
  }

  return finalizeState(state);
}

export function filterAllowedMapEntries(
  entries: TreeMapEntry[] | null | undefined,
  state: ConnectorConstraintState | null | undefined,
): TreeMapEntry[] {
  if (!entries?.length || !state?.allowedValues?.length) {
    return entries ?? [];
  }

  const allowed = new Set(state.allowedValues);
  return entries.filter((entry) => allowed.has(entry.value) || allowed.has(entry.label));
}

interface SlotPathMatch {
  slot: ConnectorSlotView;
  affectedPathOrdinal: number;
}

function findSlotForPath(slots: ConnectorSlotView[], path: string[]): SlotPathMatch | null {
  for (const slot of slots) {
    for (const [index, affectedPath] of (slot.resolvedAffectedPaths ?? []).entries()) {
      if (isPathPrefix(affectedPath, path)) {
        return {
          slot,
          affectedPathOrdinal: index + 1,
        };
      }
    }
  }

  return null;
}

function isPathPrefix(prefix: string[], fullPath: string[]): boolean {
  if (prefix.length > fullPath.length) {
    return false;
  }

  return prefix.every((step, index) => fullPath[index] === step);
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

function ruleMatchesPath(
  rule: ConnectorConstraintRuleView,
  normalizedPath: string[],
  affectedPathOrdinal: number,
  originalPath: string[],
): boolean {
  if (rule.lineOrdinals?.length && !rule.lineOrdinals.includes(affectedPathOrdinal)) {
    return false;
  }

  if (!isPathPrefix(rule.resolvedPath, normalizedPath)) {
    return false;
  }

  if (rule.replicationOrdinals?.length) {
    const replicationIndex = extractReplicationOrdinal(rule.resolvedPath, originalPath);
    if (replicationIndex !== null && !rule.replicationOrdinals.includes(replicationIndex)) {
      return false;
    }
  }

  return true;
}

/**
 * Extract the 1-based replication ordinal from the original (non-stripped)
 * path at the position corresponding to the last step of the rule's
 * resolvedPath. The resolvedPath targets the base element (e.g. `elem:5`)
 * while the original path carries the instance suffix (e.g. `elem:5#2`).
 * Returns null if the path is too short or has no instance suffix.
 */
function extractReplicationOrdinal(
  resolvedPath: string[],
  originalPath: string[],
): number | null {
  // The replication instance lives on the step in the original path that
  // corresponds to the last group-level step in the resolvedPath (the one
  // before the leaf field). For `Event#2/Upon this action` the resolvedPath
  // is e.g. ['seg:2', 'elem:0', 'elem:5', 'elem:0'] — the replicated group
  // is at index 2 ('elem:5'), and the leaf is at index 3.
  // We look for the deepest step in the original path that has a `#N`
  // suffix matching a resolvedPath step (after stripping the suffix).
  for (let i = resolvedPath.length - 1; i >= 0; i--) {
    if (i >= originalPath.length) continue;
    const originalStep = originalPath[i];
    const hashIndex = originalStep.indexOf('#');
    if (hashIndex < 0) continue;
    const base = originalStep.slice(0, hashIndex);
    if (base === resolvedPath[i]) {
      const ordinal = parseInt(originalStep.slice(hashIndex + 1), 10);
      return Number.isNaN(ordinal) ? null : ordinal;
    }
  }
  return null;
}

function applyRule(rule: ConnectorConstraintRuleView, state: ConnectorConstraintState): void {
  switch (rule.effect) {
    case 'hide':
      state.hidden = true;
      break;
    case 'disable':
      state.disabled = true;
      break;
    case 'readOnly':
      state.disabled = true;
      state.readOnly = true;
      break;
    case 'allowValues':
      state.allowedValues = intersectAllowedValues(state.allowedValues, rule.allowedValues ?? []);
      break;
    case 'denyValues':
      state.deniedValues = mergeDeniedValues(state.deniedValues, rule.deniedValues ?? []);
      break;
    case 'show':
      state.hidden = false;
      break;
  }

  if (rule.explanation) {
    state.explanations.push(rule.explanation);
  }
}

function applyEmptyBehavior(slot: ConnectorSlotView, state: ConnectorConstraintState): void {
  const behavior = slot.baseBehaviorWhenEmpty;
  if (!behavior) {
    return;
  }

  switch (behavior.effect) {
    case 'hide':
      state.hidden = true;
      break;
    case 'disable':
      state.disabled = true;
      break;
    case 'allowValues':
      state.allowedValues = intersectAllowedValues(state.allowedValues, behavior.allowedValues ?? []);
      break;
  }

  if (behavior.allowedValues?.length) {
    state.allowedValues = intersectAllowedValues(state.allowedValues, behavior.allowedValues);
  }
}

function intersectAllowedValues(
  current: ConnectorConstraintScalar[] | null,
  next: ConnectorConstraintScalar[],
): ConnectorConstraintScalar[] {
  if (!current) {
    return [...next];
  }

  const allowed = new Set(next);
  return current.filter((value) => allowed.has(value));
}

function mergeDeniedValues(
  current: ConnectorConstraintScalar[],
  next: ConnectorConstraintScalar[],
): ConnectorConstraintScalar[] {
  const merged = new Set(current);
  for (const value of next) {
    merged.add(value);
  }
  return [...merged];
}

function finalizeState(state: ConnectorConstraintState): ConnectorConstraintState {
  if (!state.allowedValues?.length || !state.deniedValues.length) {
    return state;
  }

  const denied = new Set(state.deniedValues);
  return {
    ...state,
    allowedValues: state.allowedValues.filter((value) => !denied.has(value)),
  };
}