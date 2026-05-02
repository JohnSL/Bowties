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

  const slot = findSlotForPath(profile.slots, path);
  if (!slot) {
    return initialState;
  }

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
    if (!ruleMatchesPath(rule, normalizedPath)) {
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

function findSlotForPath(slots: ConnectorSlotView[], path: string[]): ConnectorSlotView | null {
  for (const slot of slots) {
    for (const affectedPath of slot.resolvedAffectedPaths ?? []) {
      if (isPathPrefix(affectedPath, path)) {
        return slot;
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

function ruleMatchesPath(rule: ConnectorConstraintRuleView, normalizedPath: string[]): boolean {
  return isPathPrefix(rule.resolvedPath, normalizedPath);
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