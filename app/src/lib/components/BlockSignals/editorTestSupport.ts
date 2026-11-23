/** Test doubles for the Automatic Block Signals editor components: a plain view and spy intents. */
import { vi } from 'vitest';
import type { BlockSignalsPreview, SignalPreview } from '$lib/api/signaling';
import type { BlockSignalsEditorView, BlockSignalsStep, BlockSignalsStepState, BlockSignalsStepStatus } from '$lib/stores/blockSignalsEditor.svelte';
import type { BlockSignalsEditorActions } from '$lib/orchestration/blockSignalsEditorOrchestrator';

export const SIGNAL: SignalPreview = {
  signalId: 'sig-1',
  name: 'Cedar Hill East',
  governs: 'eastward',
  run: 'withList',
  entranceBlockId: 'block:cedar',
  protectedBlockIds: ['block:cedar'],
  turnoutIds: [],
  settings: { aspects: 3, approachLighting: 'alwaysLit' },
  overridden: false,
  next: { kind: 'nothingBeyond' },
  rules: [
    { condition: { kind: 'blockOccupied' }, indication: 'stop' },
    { condition: { kind: 'blockClear' }, indication: 'clear' },
  ],
  observation: 'unavailable',
};

export function previewWith(signals: SignalPreview[] = [SIGNAL]): BlockSignalsPreview {
  return {
    facilityName: 'Cedar Hill',
    direction: 'eastward',
    movements: 'oneDirection',
    blocks: [],
    entranceEnd: { kind: 'noPlant' },
    exitEnd: { kind: 'nothingBeyond' },
    signals,
    capability: { kind: 'incompletePreview', stage: 'unavailable', save: 'unavailable', apply: 'unavailable' },
    issues: [],
  };
}

/** Step states for a view: every step but Review `complete` unless overridden. */
export function stepsWith(overrides: Partial<Record<BlockSignalsStep, BlockSignalsStepStatus>> = {}): BlockSignalsStepState[] {
  const ids: BlockSignalsStep[] = ['blocks', 'aspects', 'appearance', 'wiring', 'review'];
  return ids.map((id) => id === 'review'
    ? { id, status: 'unavailable', unavailableReason: 'Not in this release' }
    : { id, status: overrides[id] ?? 'complete' });
}

export function fakeView(overrides: Partial<BlockSignalsEditorView> = {}): BlockSignalsEditorView {
  return {
    mode: 'create',
    step: 'blocks' as BlockSignalsStep,
    facilityName: 'Cedar Hill',
    direction: 'eastward',
    movements: 'oneDirection',
    blocks: [{ blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] }],
    entranceEnd: { kind: 'noPlant' },
    exitEnd: { kind: 'nothingBeyond' },
    settings: { aspects: 3, turnoutNotLined: 'stop', approachLighting: 'alwaysLit' },
    signalOverrides: {},
    signalNames: {},
    turnoutShapes: {},
    preview: previewWith(),
    error: null,
    steps: stepsWith(),
    outputHosts: [],
    wiringComplete: false,
    outOfScopeSettings: [],
    issues: [],
    finishError: null,
    canFinish: false,
    issuesForSignal: () => [],
    issuesForNode: () => [],
    wiringFor: () => null,
    canMoveTurnout: () => false,
    ...overrides,
  };
}

export function spyActions(): BlockSignalsEditorActions {
  return new Proxy({}, { get: (target: Record<string, unknown>, key: string) => (target[key] ??= vi.fn()) }) as BlockSignalsEditorActions;
}
