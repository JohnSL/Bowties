import { beforeEach, describe, expect, it } from 'vitest';
import type { BlockSignalsRecord } from '$lib/api/facilities';
import { blockSignalsEditorStore as store } from '$lib/stores/blockSignalsEditor.svelte';
import { previewRequestFromRecord } from './blockSignalsPreviewRequest';

const record: BlockSignalsRecord = {
  direction: 'eastward',
  movements: 'oneDirection',
  entranceEnd: { kind: 'noPlant' },
  exitEnd: { kind: 'nothingBeyond' },
  blocks: [{ blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [{ turnoutId: 't1', name: 'T1' }] }],
  settings: { aspects: 3, turnoutNotLined: 'stop', approachLighting: 'alwaysLit' },
  signals: [{
    signalId: 'sig-1', name: 'Cedar Hill East', protectedBlock: 'block:cedar', channelId: 'ch-1',
    resources: { nodeKey: '020157000001', mast: 1, trackCircuits: [] },
  }],
};

describe('previewRequestFromRecord', () => {
  beforeEach(() => store.reset());

  it('builds the same request the editor builds when opened on the saved facility', () => {
    store.startEdit({ facilityId: 'abs-1', name: 'Cedar Hill', record, wiring: {} });
    const request = previewRequestFromRecord('abs-1', 'Cedar Hill', record);
    expect(request).toEqual(store.previewRequest());
    expect(request.signalNames).toEqual({ 'sig-1': 'Cedar Hill East' });
    expect(request.signalOverrides).toEqual([]);
    expect(request.release).toEqual([]);
  });
});
