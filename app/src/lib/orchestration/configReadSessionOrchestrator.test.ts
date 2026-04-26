import { describe, expect, it } from 'vitest';
import type { NodeReadState, ReadProgressState } from '$lib/api/types';
import {
  applyConfigReadProgressUpdate,
  beginConfigReadSession,
  closeConfigReadProgressUi,
  divertConfigReadToDownloadDialog,
  failConfigReadCancellation,
  failConfigReadSession,
  finishConfigReadSession,
  requestConfigReadCancellation,
} from './configReadSessionOrchestrator';

function makeNodeReadStates(): NodeReadState[] {
  return [
    { nodeId: '02.01.57.00.00.01', name: 'East Panel', percentage: 0, status: 'waiting' },
    { nodeId: '02.01.57.00.00.02', name: 'West Panel', percentage: 0, status: 'waiting' },
  ];
}

function makeReadProgress(status: ReadProgressState['status']): ReadProgressState {
  return {
    currentNodeId: '02.01.57.00.00.02',
    currentNodeIndex: 1,
    currentNodeName: 'West Panel',
    elementsFailed: 0,
    elementsRead: 5,
    percentage: 42,
    status,
    totalElements: 12,
    totalNodes: 2,
  };
}

describe('configReadSessionOrchestrator', () => {
  it('builds begin and end session patches', () => {
    expect(beginConfigReadSession(makeNodeReadStates())).toEqual({
      discoveryModalVisible: true,
      discoveryPhase: 'reading',
      errorMessage: '',
      isCancelling: false,
      nodeReadStates: makeNodeReadStates(),
      readProgress: null,
      readingRemaining: true,
    });

    expect(finishConfigReadSession()).toEqual({
      discoveryModalVisible: false,
      isCancelling: false,
      nodeReadStates: [],
      readProgress: null,
      readingRemaining: false,
    });

    expect(failConfigReadSession('Read failed')).toEqual({
      discoveryModalVisible: false,
      errorMessage: 'Read failed',
      isCancelling: false,
      nodeReadStates: [],
      readProgress: null,
      readingRemaining: false,
    });
  });

  it('builds missing-cdi diversion and cancel patches', () => {
    expect(divertConfigReadToDownloadDialog(
      [{ nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' }],
      [{ nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' }],
    )).toEqual({
      cdiDownloadDialogVisible: true,
      cdiMissingNodes: [{ nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' }],
      discoveryModalVisible: false,
      nodeReadStates: [],
      pendingConfigNodes: [{ nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' }],
      readingRemaining: false,
    });

    expect(requestConfigReadCancellation()).toEqual({ isCancelling: true });
    expect(failConfigReadCancellation('Cancel failed')).toEqual({
      errorMessage: 'Cancel failed',
      isCancelling: false,
    });
  });

  it('updates progress state for active reads and closes UI on terminal progress events', () => {
    expect(applyConfigReadProgressUpdate(
      makeNodeReadStates(),
      makeReadProgress({ type: 'ReadingNode', node_name: 'West Panel' }),
    )).toEqual({
      discoveryPhase: 'reading',
      nodeReadStates: [
        { nodeId: '02.01.57.00.00.01', name: 'East Panel', percentage: 100, status: 'complete' },
        { nodeId: '02.01.57.00.00.02', name: 'West Panel', percentage: 42, status: 'reading' },
      ],
      readProgress: makeReadProgress({ type: 'ReadingNode', node_name: 'West Panel' }),
    });

    expect(applyConfigReadProgressUpdate(
      makeNodeReadStates(),
      makeReadProgress({ type: 'Cancelled' }),
    )).toEqual(closeConfigReadProgressUi());

    expect(applyConfigReadProgressUpdate(
      makeNodeReadStates(),
      makeReadProgress({ type: 'Complete', fail_count: 0, success_count: 2 }),
    )).toEqual(closeConfigReadProgressUi());
  });
});