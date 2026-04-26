import type { ReadProgressState, NodeReadState } from '$lib/api/types';
import type { CdiNodeCandidate } from './cdiDialogOrchestrator';

export type ConfigReadPhase = 'reading' | 'complete' | 'cancelled';

export interface ConfigReadSessionPatch {
  cdiDownloadDialogVisible?: boolean;
  cdiMissingNodes?: CdiNodeCandidate[];
  discoveryModalVisible?: boolean;
  discoveryPhase?: ConfigReadPhase;
  errorMessage?: string;
  isCancelling?: boolean;
  nodeReadStates?: NodeReadState[];
  pendingConfigNodes?: CdiNodeCandidate[];
  readProgress?: ReadProgressState | null;
  readingRemaining?: boolean;
}

export function beginConfigReadSession(initialNodeReadStates: NodeReadState[] = []): ConfigReadSessionPatch {
  return {
    discoveryModalVisible: true,
    discoveryPhase: 'reading',
    errorMessage: '',
    isCancelling: false,
    nodeReadStates: initialNodeReadStates,
    readProgress: null,
    readingRemaining: true,
  };
}

export function finishConfigReadSession(): ConfigReadSessionPatch {
  return {
    discoveryModalVisible: false,
    isCancelling: false,
    nodeReadStates: [],
    readProgress: null,
    readingRemaining: false,
  };
}

export function failConfigReadSession(message: string): ConfigReadSessionPatch {
  return {
    ...finishConfigReadSession(),
    errorMessage: message,
  };
}

export function divertConfigReadToDownloadDialog(
  pendingConfigNodes: CdiNodeCandidate[],
  cdiMissingNodes: CdiNodeCandidate[],
): ConfigReadSessionPatch {
  return {
    cdiDownloadDialogVisible: true,
    cdiMissingNodes,
    discoveryModalVisible: false,
    nodeReadStates: [],
    pendingConfigNodes,
    readingRemaining: false,
  };
}

export function requestConfigReadCancellation(): ConfigReadSessionPatch {
  return {
    isCancelling: true,
  };
}

export function failConfigReadCancellation(message: string): ConfigReadSessionPatch {
  return {
    errorMessage: message,
    isCancelling: false,
  };
}

export function closeConfigReadProgressUi(): ConfigReadSessionPatch {
  return {
    discoveryModalVisible: false,
    isCancelling: false,
    nodeReadStates: [],
    readProgress: null,
  };
}

export function applyConfigReadProgressUpdate(
  currentNodeReadStates: NodeReadState[],
  payload: ReadProgressState,
): ConfigReadSessionPatch {
  if (payload.status.type === 'Cancelled' || payload.status.type === 'Complete') {
    return closeConfigReadProgressUi();
  }

  let nodeReadStates = currentNodeReadStates;
  if (currentNodeReadStates.length > 0) {
    const idx = payload.currentNodeIndex;
    nodeReadStates = currentNodeReadStates.map((state, index) => {
      if (index < idx) return { ...state, status: 'complete', percentage: 100 };
      if (index === idx) {
        if (payload.status.type === 'NodeComplete') {
          return { ...state, status: 'complete', percentage: 100 };
        }
        return { ...state, status: 'reading', percentage: payload.percentage };
      }
      return state;
    });
  }

  return {
    discoveryPhase: 'reading',
    nodeReadStates,
    readProgress: payload,
  };
}