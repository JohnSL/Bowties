import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Facility, BlockSignalsRecord } from '$lib/api/facilities';
import type { BlockSignalsPreview, BlockSignalsPreviewRequest } from '$lib/api/signaling';

const previewBlockSignalsMock = vi.fn<(request: BlockSignalsPreviewRequest) => Promise<BlockSignalsPreview>>();
const syncDraftsMock = vi.fn(async () => undefined);

vi.mock('$lib/api/signaling', () => ({ previewBlockSignals: previewBlockSignalsMock }));
vi.mock('$lib/orchestration/facilityOrchestrator', () => ({ syncDraftsForComposition: syncDraftsMock }));

const { facilityDiagramsStore: store } = await import('$lib/stores/facilityDiagrams.svelte');
const { facilityDiagramOrchestrator: orchestrator } = await import('./facilityDiagramOrchestrator');
const { layoutScopedParticipants } = await import('./layoutLifecycleOrchestrator');

function record(signalName = 'East'): BlockSignalsRecord {
  return {
    direction: 'eastward',
    movements: 'oneDirection',
    entranceEnd: { kind: 'noPlant' },
    exitEnd: { kind: 'nothingBeyond' },
    blocks: [{ blockId: 'block:a', name: 'A', joinedToPrevious: false, turnouts: [] }],
    settings: { aspects: 3, turnoutNotLined: 'stop', approachLighting: 'alwaysLit' },
    signals: [{
      signalId: 'sig-1', name: signalName, protectedBlock: 'block:a', channelId: 'ch-1',
      resources: { nodeKey: '020157000001', mast: 1, trackCircuits: [] },
    }],
  };
}

const abs = (facilityId: string, signalName = 'East'): Facility => ({
  facilityId, templateId: 'automatic-block-signals', name: facilityId, slotBindings: {}, blockSignals: record(signalName),
});
const plain: Facility = { facilityId: 'other', templateId: 'x', name: 'other', slotBindings: {} };
const previewOf = (name: string) => ({ facilityName: name, signals: [] }) as unknown as BlockSignalsPreview;

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

describe('facilityDiagramOrchestrator', () => {
  beforeEach(() => {
    orchestrator.resetForNewLayout();
    previewBlockSignalsMock.mockReset();
    syncDraftsMock.mockClear();
  });

  it('fetches a preview for each Automatic Block Signals facility and ignores the rest', async () => {
    previewBlockSignalsMock.mockImplementation(async (r) => previewOf(r.facilityId));
    await orchestrator.sync([abs('a'), abs('b'), plain]);
    expect(previewBlockSignalsMock).toHaveBeenCalledTimes(2);
    expect(syncDraftsMock).toHaveBeenCalled();
    expect(store.get('a')).toMatchObject({ status: 'ready', preview: { facilityName: 'a' } });
    expect(store.get('b')?.status).toBe('ready');
    expect(store.get('other')).toBeUndefined();
  });

  it('marks the entry loading while the preview is pending', async () => {
    const d = deferred<BlockSignalsPreview>();
    previewBlockSignalsMock.mockReturnValue(d.promise);
    const done = orchestrator.sync([abs('a')]);
    expect(store.get('a')?.status).toBe('loading');
    d.resolve(previewOf('a'));
    await done;
    expect(store.get('a')?.status).toBe('ready');
  });

  it('ignores a stale response once a newer record has been requested', async () => {
    const first = deferred<BlockSignalsPreview>();
    const second = deferred<BlockSignalsPreview>();
    previewBlockSignalsMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const p1 = orchestrator.sync([abs('a', 'Old')]);
    const p2 = orchestrator.sync([abs('a', 'New')]);
    await vi.waitFor(() => expect(previewBlockSignalsMock).toHaveBeenCalledTimes(2));
    second.resolve(previewOf('new'));
    await p2;
    first.resolve(previewOf('old'));
    await p1;
    expect(store.get('a')).toMatchObject({ status: 'ready', preview: { facilityName: 'new' } });
  });

  it('records a failure with the error message', async () => {
    previewBlockSignalsMock.mockRejectedValue(new Error('boom'));
    await orchestrator.sync([abs('a')]);
    expect(store.get('a')).toMatchObject({ status: 'failed', error: 'boom' });
  });

  it('does not refetch an unchanged record', async () => {
    previewBlockSignalsMock.mockImplementation(async (r) => previewOf(r.facilityId));
    await orchestrator.sync([abs('a')]);
    await orchestrator.sync([abs('a')]);
    expect(previewBlockSignalsMock).toHaveBeenCalledTimes(1);
  });

  it('refetches when the record changes', async () => {
    previewBlockSignalsMock.mockImplementation(async (r) => previewOf(r.facilityId));
    await orchestrator.sync([abs('a', 'One')]);
    await orchestrator.sync([abs('a', 'Two')]);
    expect(previewBlockSignalsMock).toHaveBeenCalledTimes(2);
  });

  it('drops entries for facilities that no longer exist', async () => {
    previewBlockSignalsMock.mockImplementation(async (r) => previewOf(r.facilityId));
    await orchestrator.sync([abs('a'), abs('b')]);
    await orchestrator.sync([abs('a')]);
    expect(store.get('b')).toBeUndefined();
    expect(store.get('a')?.status).toBe('ready');
  });

  it('reset clears the store and discards an in-flight response', async () => {
    const d = deferred<BlockSignalsPreview>();
    previewBlockSignalsMock.mockReturnValue(d.promise);
    const done = orchestrator.sync([abs('a')]);
    orchestrator.resetForNewLayout();
    expect(store.get('a')).toBeUndefined();
    d.resolve(previewOf('a'));
    await done;
    expect(store.get('a')).toBeUndefined();
  });

  it('is registered with the layout lifecycle', () => {
    expect(layoutScopedParticipants).toContain(orchestrator);
  });
});
