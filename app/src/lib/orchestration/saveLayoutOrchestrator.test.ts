import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LayoutFile, LayoutEditDelta } from '$lib/types/bowtie';
import type { BowtieCatalog } from '$lib/api/tauri';
import type { SaveLayoutResult, SaveWithBusWriteResult } from '$lib/api/layout';
import { saveLayoutOrchestrated, type SaveLayoutOrchestratedArgs } from './saveLayoutOrchestrator';

function makeLayout(): LayoutFile {
  return {
    schemaVersion: '1.0',
    bowties: {
      '02.01.57.00.02.D9.00.06': { name: 'Test Bowtie', tags: ['yard'] },
    },
    roleClassifications: {
      '02.01.57.00.00.01:seg:0/elem:0#1/elem:0': { role: 'Consumer' },
    },
    connectorSelections: {},
  };
}

function makeDeltas(): LayoutEditDelta[] {
  return [
    { type: 'renameBowtie', eventIdHex: '02.01.57.00.02.D9.00.06', newName: 'Test Bowtie' },
  ];
}

function makeSaveResult(overrides: Partial<SaveLayoutResult> = {}): SaveLayoutResult {
  return { manifestPath: '', nodeFilesWritten: 0, warnings: [], layout: makeLayout(), persistedNodeIds: [], nodeSnapshots: [], ...overrides };
}

function makeCatalog(): BowtieCatalog {
  return {
    bowties: [{
      event_id_hex: '02.01.57.00.02.D9.00.06',
      event_id_bytes: [0x02, 0x01, 0x57, 0x00, 0x02, 0xD9, 0x00, 0x06],
      producers: [],
      consumers: [],
      ambiguous_entries: [],
      name: 'Test Bowtie',
      tags: ['yard'],
      state: 'Active',
    }],
    built_at: '2026-01-01T00:00:00Z',
    source_node_count: 1,
    total_slots_scanned: 5,
  };
}

describe('saveLayoutOrchestrated', () => {
  let saveFile: ReturnType<typeof vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>>>;
  let rebuildCatalog: ReturnType<typeof vi.fn<(layout: LayoutFile | null) => Promise<BowtieCatalog>>>;
  let setCatalog: ReturnType<typeof vi.fn>;
  let clearMetadata: ReturnType<typeof vi.fn>;
  let markClean: ReturnType<typeof vi.fn>;
  let hydrateLayout: ReturnType<typeof vi.fn>;
  let setActiveContext: ReturnType<typeof vi.fn>;
  let updatePartialCaptureNodes: ReturnType<typeof vi.fn>;
  let getPendingChangeCount: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    saveFile = vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>>(async () => makeSaveResult());
    rebuildCatalog = vi.fn<(layout: LayoutFile | null) => Promise<BowtieCatalog>>(async () => makeCatalog());
    setCatalog = vi.fn();
    clearMetadata = vi.fn();
    markClean = vi.fn();
    hydrateLayout = vi.fn();
    setActiveContext = vi.fn();
    updatePartialCaptureNodes = vi.fn();
    getPendingChangeCount = vi.fn(() => 0);
  });

  function baseArgs(): SaveLayoutOrchestratedArgs {
    return {
      saveFile, rebuildCatalog, setCatalog, clearMetadata, markClean, hydrateLayout,
      setActiveContext, updatePartialCaptureNodes, getPendingChangeCount,
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
    };
  }

  it('saves file, rebuilds catalog, hydrates layout, clears metadata, and marks clean in order', async () => {
    const callOrder: string[] = [];
    saveFile.mockImplementation(async () => { callOrder.push('save'); return makeSaveResult(); });
    rebuildCatalog.mockImplementation(async () => { callOrder.push('rebuild'); return makeCatalog(); });
    setCatalog.mockImplementation(() => { callOrder.push('setCatalog'); });
    hydrateLayout.mockImplementation(() => { callOrder.push('hydrateLayout'); });
    clearMetadata.mockImplementation(() => { callOrder.push('clearMetadata'); });
    markClean.mockImplementation(() => { callOrder.push('markClean'); });

    await saveLayoutOrchestrated(baseArgs());

    expect(callOrder).toEqual(['save', 'rebuild', 'setCatalog', 'hydrateLayout', 'clearMetadata', 'markClean']);
  });

  it('passes deltas to saveFile and persisted layout to rebuildCatalog', async () => {
    const deltas = makeDeltas();
    const persistedLayout = makeLayout();
    saveFile.mockResolvedValue(makeSaveResult({ layout: persistedLayout }));

    await saveLayoutOrchestrated({ ...baseArgs(), deltas });

    expect(saveFile).toHaveBeenCalledWith('/test/layout.bowties.yaml', deltas);
    expect(rebuildCatalog).toHaveBeenCalledWith(persistedLayout);
  });

  it('hydrates layout store from backend response (ADR-0002)', async () => {
    const persistedLayout = makeLayout();
    saveFile.mockResolvedValue(makeSaveResult({ layout: persistedLayout }));

    await saveLayoutOrchestrated(baseArgs());

    expect(hydrateLayout).toHaveBeenCalledWith(persistedLayout);
  });

  it('passes rebuilt catalog to setCatalog', async () => {
    const catalog = makeCatalog();
    rebuildCatalog.mockResolvedValue(catalog);

    await saveLayoutOrchestrated(baseArgs());

    expect(setCatalog).toHaveBeenCalledWith(catalog);
  });

  it('returns warnings from saveFile result', async () => {
    saveFile.mockResolvedValue(makeSaveResult({ warnings: ['node-1-partial'] }));

    const result = await saveLayoutOrchestrated(baseArgs());

    expect(result.warnings).toEqual(['node-1-partial']);
  });

  it('passes nodeSnapshots through from saveFile result (Bug 2b regression)', async () => {
    const snapshots = [{ nodeKey: '020157000001', capturedAt: '2026-06-06T00:00:00Z' }] as any;
    saveFile.mockResolvedValue(makeSaveResult({ nodeSnapshots: snapshots }));

    const result = await saveLayoutOrchestrated(baseArgs());

    expect(result.nodeSnapshots).toEqual(snapshots);
  });

  it('does not clear metadata or mark clean if saveFile throws', async () => {
    saveFile.mockRejectedValue(new Error('disk full'));

    await expect(saveLayoutOrchestrated(baseArgs())).rejects.toThrow('disk full');

    expect(clearMetadata).not.toHaveBeenCalled();
    expect(markClean).not.toHaveBeenCalled();
    expect(rebuildCatalog).not.toHaveBeenCalled();
    expect(hydrateLayout).not.toHaveBeenCalled();
  });

  it('does not clear metadata or mark clean if rebuildCatalog throws', async () => {
    rebuildCatalog.mockRejectedValue(new Error('catalog build failed'));

    await expect(saveLayoutOrchestrated(baseArgs())).rejects.toThrow('catalog build failed');

    expect(saveFile).toHaveBeenCalled();
    expect(clearMetadata).not.toHaveBeenCalled();
    expect(markClean).not.toHaveBeenCalled();
  });

  // ── S1 behaviour ─────────────────────────────────────────────────────────

  it('records pending changes before saveFile when provided', async () => {
    const callOrder: string[] = [];
    const recordPending = vi.fn(async () => { callOrder.push('flush'); return [] as string[]; });
    saveFile.mockImplementation(async () => { callOrder.push('save'); return makeSaveResult(); });

    await saveLayoutOrchestrated({ ...baseArgs(), recordPending });

    expect(callOrder).toEqual(['flush', 'save']);
  });

  it('does not require recordPending (backwards compat)', async () => {
    const args = baseArgs();
    await expect(saveLayoutOrchestrated(args)).resolves.toBeDefined();
  });

  it('does not call saveFile if recording the pending changes throws', async () => {
    const recordPending = vi.fn(async (): Promise<string[]> => { throw new Error('flush failed'); });

    await expect(saveLayoutOrchestrated({ ...baseArgs(), recordPending })).rejects.toThrow('flush failed');

    expect(saveFile).not.toHaveBeenCalled();
    expect(setActiveContext).not.toHaveBeenCalled();
  });

  it('calls updatePartialCaptureNodes with warnings from saveFile', async () => {
    saveFile.mockResolvedValue(makeSaveResult({ warnings: ['node-A', 'node-B'] }));

    await saveLayoutOrchestrated(baseArgs());

    expect(updatePartialCaptureNodes).toHaveBeenCalledWith(['node-A', 'node-B']);
  });

  it('calls setActiveContext with derived layoutId and path after save', async () => {
    await saveLayoutOrchestrated({
      ...baseArgs(),
      path: '/layouts/yard',
      getPendingChangeCount: () => 3,
    });

    expect(setActiveContext).toHaveBeenCalledWith(
      expect.objectContaining({
        layoutId: 'yard',
        rootPath: '/layouts/yard',
        mode: 'offline_file',
        pendingChangeCount: 3,
      }),
    );
  });

  it('setActiveContext receives a capturedAt ISO timestamp', async () => {
    await saveLayoutOrchestrated(baseArgs());

    const ctx = setActiveContext.mock.calls[0][0];
    expect(ctx.capturedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });

  it('does not call setActiveContext or updatePartialCaptureNodes if saveFile throws', async () => {
    saveFile.mockRejectedValue(new Error('disk full'));

    await expect(saveLayoutOrchestrated(baseArgs())).rejects.toThrow();

    expect(setActiveContext).not.toHaveBeenCalled();
    expect(updatePartialCaptureNodes).not.toHaveBeenCalled();
  });
});

// ── S2: saveWithBusWrites path ────────────────────────────────────────────────

function noWrites(): SaveWithBusWriteResult['writeCounts'] {
  return { written: 0, failed: 0, readOnly: 0, waiting: 0 };
}

function makeBusWriteResult(overrides: Partial<SaveWithBusWriteResult> = {}): SaveWithBusWriteResult {
  return {
    layoutSaved: true,
    writeCounts: noWrites(),
    writeErrors: [],
    reconciled: false,
    catalogRebuilt: true,
    warnings: [],
    layout: makeLayout(),
    persistedNodeIds: [],
    nodeSnapshots: [],
    writtenNodeIds: [],
    ...overrides,
  };
}

describe('saveLayoutOrchestrated — saveWithBusWrites path', () => {
  let saveWithBusWrites: ReturnType<typeof vi.fn<(path: string, deltas: LayoutEditDelta[], ids: string[]) => Promise<SaveWithBusWriteResult>>>;
  let setCatalog: ReturnType<typeof vi.fn>;
  let clearMetadata: ReturnType<typeof vi.fn>;
  let markClean: ReturnType<typeof vi.fn>;
  let hydrateLayout: ReturnType<typeof vi.fn>;
  let setActiveContext: ReturnType<typeof vi.fn>;
  let updatePartialCaptureNodes: ReturnType<typeof vi.fn>;
  let getPendingChangeCount: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    saveWithBusWrites = vi.fn<(path: string, deltas: LayoutEditDelta[], ids: string[]) => Promise<SaveWithBusWriteResult>>(async () => makeBusWriteResult());
    setCatalog = vi.fn();
    clearMetadata = vi.fn();
    markClean = vi.fn();
    hydrateLayout = vi.fn();
    setActiveContext = vi.fn();
    updatePartialCaptureNodes = vi.fn();
    getPendingChangeCount = vi.fn(() => 0);
  });

  function busWriteArgs(): SaveLayoutOrchestratedArgs {
    return {
      saveWithBusWrites,
      setCatalog,
      clearMetadata,
      markClean,
      hydrateLayout,
      setActiveContext,
      updatePartialCaptureNodes,
      getPendingChangeCount,
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
    };
  }

  it('calls saveWithBusWrites with path and deltas', async () => {
    const deltas = makeDeltas();
    await saveLayoutOrchestrated({ ...busWriteArgs(), deltas });
    expect(saveWithBusWrites).toHaveBeenCalledWith('/test/layout.bowties.yaml', deltas, []);
  });

  it('hydrates layout store from saveWithBusWrites response (ADR-0002)', async () => {
    const persistedLayout = makeLayout();
    saveWithBusWrites.mockResolvedValue(makeBusWriteResult({ layout: persistedLayout }));

    await saveLayoutOrchestrated(busWriteArgs());

    expect(hydrateLayout).toHaveBeenCalledWith(persistedLayout);
  });

  it('passes nodeSnapshots through from saveWithBusWrites result (Bug 2b regression)', async () => {
    const snapshots = [{ nodeKey: '020157000001', capturedAt: '2026-06-06T00:00:00Z' }] as any;
    saveWithBusWrites.mockResolvedValue(makeBusWriteResult({ nodeSnapshots: snapshots }));

    const result = await saveLayoutOrchestrated(busWriteArgs());

    expect(result.nodeSnapshots).toEqual(snapshots);
  });

  it('does not call saveFile when saveWithBusWrites is provided', async () => {
    const saveFile = vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>>(async () => makeSaveResult());
    await saveLayoutOrchestrated({ ...busWriteArgs(), saveFile });
    expect(saveFile).not.toHaveBeenCalled();
    expect(saveWithBusWrites).toHaveBeenCalled();
  });

  it('does not call rebuildCatalog when saveWithBusWrites is provided', async () => {
    const rebuildCatalog = vi.fn<(layout: LayoutFile | null) => Promise<BowtieCatalog>>(async () => ({}) as BowtieCatalog);
    await saveLayoutOrchestrated({ ...busWriteArgs(), rebuildCatalog });
    expect(rebuildCatalog).not.toHaveBeenCalled();
  });

  it('calls clearMetadata and markClean after saveWithBusWrites', async () => {
    const callOrder: string[] = [];
    saveWithBusWrites.mockImplementation(async () => { callOrder.push('save'); return makeBusWriteResult(); });
    hydrateLayout.mockImplementation(() => callOrder.push('hydrateLayout'));
    clearMetadata.mockImplementation(() => callOrder.push('clearMetadata'));
    markClean.mockImplementation(() => callOrder.push('markClean'));

    await saveLayoutOrchestrated(busWriteArgs());

    expect(callOrder).toEqual(['save', 'hydrateLayout', 'clearMetadata', 'markClean']);
  });

  it('records pending changes before saveWithBusWrites when provided', async () => {
    const callOrder: string[] = [];
    const recordPending = vi.fn(async () => { callOrder.push('flush'); return [] as string[]; });
    saveWithBusWrites.mockImplementation(async () => { callOrder.push('save'); return makeBusWriteResult(); });

    await saveLayoutOrchestrated({ ...busWriteArgs(), recordPending });

    expect(callOrder).toEqual(['flush', 'save']);
  });

  it('does not call saveWithBusWrites if recording the pending changes throws', async () => {
    const recordPending = vi.fn(async (): Promise<string[]> => { throw new Error('flush failed'); });

    await expect(saveLayoutOrchestrated({ ...busWriteArgs(), recordPending })).rejects.toThrow('flush failed');

    expect(saveWithBusWrites).not.toHaveBeenCalled();
  });

  it('propagates warnings from saveWithBusWrites to result', async () => {
    saveWithBusWrites.mockResolvedValue(makeBusWriteResult({ warnings: ['node-A'] }));

    const result = await saveLayoutOrchestrated(busWriteArgs());

    expect(result.warnings).toEqual(['node-A']);
    expect(updatePartialCaptureNodes).toHaveBeenCalledWith(['node-A']);
  });

  it('calls setActiveContext with path and pending change count after saveWithBusWrites', async () => {
    await saveLayoutOrchestrated({
      ...busWriteArgs(),
      path: '/layouts/yard',
      getPendingChangeCount: () => 5,
    });

    expect(setActiveContext).toHaveBeenCalledWith(
      expect.objectContaining({
        layoutId: 'yard',
        rootPath: '/layouts/yard',
        mode: 'offline_file',
        pendingChangeCount: 5,
      }),
    );
  });

  it('reloads the Pending Change rows before reading the count for the active context', async () => {
    let rows = 2;
    await saveLayoutOrchestrated({
      ...busWriteArgs(),
      reloadPendingChanges: async () => { rows = 1; },
      getPendingChangeCount: () => rows,
    });

    expect(setActiveContext).toHaveBeenCalledWith(expect.objectContaining({ pendingChangeCount: 1 }));
  });

  it('does not call clearMetadata or markClean if saveWithBusWrites throws', async () => {
    saveWithBusWrites.mockRejectedValue(new Error('bus error'));

    await expect(saveLayoutOrchestrated(busWriteArgs())).rejects.toThrow('bus error');

    expect(clearMetadata).not.toHaveBeenCalled();
    expect(markClean).not.toHaveBeenCalled();
    expect(hydrateLayout).not.toHaveBeenCalled();
  });

  it('sends empty deltas for first-time save from live bus capture', async () => {
    await saveLayoutOrchestrated({ ...busWriteArgs(), deltas: [] });

    expect(saveWithBusWrites).toHaveBeenCalledWith('/test/layout.bowties.yaml', [], []);
    expect(clearMetadata).toHaveBeenCalled();
    expect(markClean).toHaveBeenCalled();
    expect(hydrateLayout).toHaveBeenCalled();
    expect(setActiveContext).toHaveBeenCalled();
  });
});

describe('saveLayoutOrchestrated — recorded Pending Changes (E3b-1 B1)', () => {
  it('passes exactly the change IDs recorded by this Save to the connected Save, after recording them', async () => {
    const callOrder: string[] = [];
    const recordPending = vi.fn(async () => { callOrder.push('record'); return ['row-a', 'row-b']; });
    const saveWithBusWrites = vi.fn(async () => { callOrder.push('save'); return makeBusWriteResult(); });

    await saveLayoutOrchestrated({
      saveWithBusWrites,
      setCatalog: vi.fn(), clearMetadata: vi.fn(), markClean: vi.fn(), hydrateLayout: vi.fn(),
      setActiveContext: vi.fn(), updatePartialCaptureNodes: vi.fn(), getPendingChangeCount: vi.fn(() => 0),
      recordPending,
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
    });

    expect(callOrder).toEqual(['record', 'save']);
    expect(saveWithBusWrites).toHaveBeenCalledWith('/test/layout.bowties.yaml', makeDeltas(), ['row-a', 'row-b']);
  });
});

describe('saveLayoutOrchestrated — drafts and rows after a connected Save (E3b-1 B2)', () => {
  function onlineArgs(overrides: Partial<SaveLayoutOrchestratedArgs>): SaveLayoutOrchestratedArgs {
    return {
      saveWithBusWrites: vi.fn(async () => makeBusWriteResult()),
      setCatalog: vi.fn(), clearMetadata: vi.fn(), markClean: vi.fn(), hydrateLayout: vi.fn(),
      setActiveContext: vi.fn(), updatePartialCaptureNodes: vi.fn(), getPendingChangeCount: vi.fn(() => 0),
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
      ...overrides,
    };
  }

  it('keeps drafts and reloads nothing when the Save fails before recording', async () => {
    const clearPersistedDrafts = vi.fn();
    const reloadPendingChanges = vi.fn(async () => {});
    const saveWithBusWrites = vi.fn(async (): Promise<SaveWithBusWriteResult> => { throw new Error('disk full'); });

    await expect(saveLayoutOrchestrated(onlineArgs({ saveWithBusWrites, clearPersistedDrafts, reloadPendingChanges })))
      .rejects.toThrow('disk full');

    expect(clearPersistedDrafts).not.toHaveBeenCalled();
    expect(reloadPendingChanges).not.toHaveBeenCalled();
  });

  it('clears drafts, then reloads the rows so Phase 2 failed/conflict rows are visible', async () => {
    const callOrder: string[] = [];
    const clearPersistedDrafts = vi.fn(() => { callOrder.push('clearDrafts'); });
    const reloadPendingChanges = vi.fn(async () => { callOrder.push('reload'); });

    await saveLayoutOrchestrated(onlineArgs({ clearPersistedDrafts, reloadPendingChanges }));

    expect(callOrder).toEqual(['clearDrafts', 'reload']);
  });
});

describe('saveLayoutOrchestrated — trees refresh before drafts clear (E3b-1 T11)', () => {
  it('reloads the written nodes trees, then clears drafts, so the field never falls back to the old value', async () => {
    const callOrder: string[] = [];
    const reloadNodeTrees = vi.fn(async (ids: string[]) => { callOrder.push(`reload:${ids.join(',')}`); });
    const clearPersistedDrafts = vi.fn(() => { callOrder.push('clearDrafts'); });

    await saveLayoutOrchestrated({
      saveWithBusWrites: vi.fn(async () => makeBusWriteResult({ writtenNodeIds: ['050201020300'] })),
      setCatalog: vi.fn(), clearMetadata: vi.fn(), markClean: vi.fn(), hydrateLayout: vi.fn(),
      setActiveContext: vi.fn(), updatePartialCaptureNodes: vi.fn(), getPendingChangeCount: vi.fn(() => 0),
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
      reloadNodeTrees,
      clearPersistedDrafts,
    });

    expect(callOrder).toEqual(['reload:050201020300', 'clearDrafts']);
  });
});

describe('saveLayoutOrchestrated — Save result reports leftovers (E3b-1 B3)', () => {
  it('returns the write counts and reasons from the connected Save', async () => {
    const writeCounts = { written: 1, failed: 1, readOnly: 0, waiting: 4 };
    const saveWithBusWrites = vi.fn(async () => makeBusWriteResult({
      writeCounts,
      writeErrors: ['Node 05.02: the board kept 5'],
    }));

    const result = await saveLayoutOrchestrated({
      saveWithBusWrites,
      setCatalog: vi.fn(), clearMetadata: vi.fn(), markClean: vi.fn(), hydrateLayout: vi.fn(),
      setActiveContext: vi.fn(), updatePartialCaptureNodes: vi.fn(), getPendingChangeCount: vi.fn(() => 0),
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
    });

    expect(result.writeOutcome).toEqual({ counts: writeCounts, errors: ['Node 05.02: the board kept 5'] });
  });
});

describe('saveLayoutOrchestrated — empty deltas offline path', () => {
  it('accepts empty deltas for offline save', async () => {
    const saveFile = vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>>(async () => makeSaveResult());
    const rebuildCatalog = vi.fn<(layout: LayoutFile | null) => Promise<BowtieCatalog>>(async () => makeCatalog());
    const setCatalog = vi.fn();
    const clearMetadata = vi.fn();
    const markClean = vi.fn();
    const hydrateLayout = vi.fn();
    const setActiveContext = vi.fn();
    const updatePartialCaptureNodes = vi.fn();
    const getPendingChangeCount = vi.fn(() => 0);

    await saveLayoutOrchestrated({
      saveFile, rebuildCatalog, setCatalog, clearMetadata, markClean, hydrateLayout,
      setActiveContext, updatePartialCaptureNodes, getPendingChangeCount,
      path: '/test/layout.bowties.yaml',
      deltas: [],
    });

    expect(saveFile).toHaveBeenCalledWith('/test/layout.bowties.yaml', []);
    expect(markClean).toHaveBeenCalled();
  });
});

// ── S2c-T2: clearPersistedDrafts (ADR-0004) ──────────────────────────────────

describe('saveLayoutOrchestrated — clearPersistedDrafts (S2c)', () => {
  let saveFile: ReturnType<typeof vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>>>;
  let rebuildCatalog: ReturnType<typeof vi.fn<(layout: LayoutFile | null) => Promise<BowtieCatalog>>>;
  let setCatalog: ReturnType<typeof vi.fn>;
  let clearMetadata: ReturnType<typeof vi.fn>;
  let markClean: ReturnType<typeof vi.fn>;
  let hydrateLayout: ReturnType<typeof vi.fn>;
  let setActiveContext: ReturnType<typeof vi.fn>;
  let updatePartialCaptureNodes: ReturnType<typeof vi.fn>;
  let getPendingChangeCount: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    saveFile = vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>>(async () => makeSaveResult());
    rebuildCatalog = vi.fn<(layout: LayoutFile | null) => Promise<BowtieCatalog>>(async () => makeCatalog());
    setCatalog = vi.fn();
    clearMetadata = vi.fn();
    markClean = vi.fn();
    hydrateLayout = vi.fn();
    setActiveContext = vi.fn();
    updatePartialCaptureNodes = vi.fn();
    getPendingChangeCount = vi.fn(() => 0);
  });

  function baseArgsWithClear(clearPersistedDrafts: () => void): SaveLayoutOrchestratedArgs {
    return {
      saveFile, rebuildCatalog, setCatalog, clearMetadata, markClean, hydrateLayout,
      setActiveContext, updatePartialCaptureNodes, getPendingChangeCount,
      clearPersistedDrafts,
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
    };
  }

  it('calls clearPersistedDrafts after a successful offline save', async () => {
    const clearPersistedDrafts = vi.fn();
    await saveLayoutOrchestrated(baseArgsWithClear(clearPersistedDrafts));
    expect(clearPersistedDrafts).toHaveBeenCalledTimes(1);
  });

  it('clears drafts AFTER catalog is set so the read model never sees a blank state', async () => {
    const callOrder: string[] = [];
    saveFile.mockImplementation(async () => { callOrder.push('save'); return makeSaveResult(); });
    rebuildCatalog.mockImplementation(async () => { callOrder.push('rebuild'); return makeCatalog(); });
    setCatalog.mockImplementation(() => { callOrder.push('setCatalog'); });
    const clearPersistedDrafts = vi.fn(() => { callOrder.push('clearDrafts'); });

    await saveLayoutOrchestrated(baseArgsWithClear(clearPersistedDrafts));

    const setCatalogIdx = callOrder.indexOf('setCatalog');
    const clearDraftsIdx = callOrder.indexOf('clearDrafts');
    expect(setCatalogIdx).toBeGreaterThanOrEqual(0);
    expect(clearDraftsIdx).toBeGreaterThan(setCatalogIdx);
  });

  it('does not call clearPersistedDrafts if saveFile throws', async () => {
    saveFile.mockRejectedValue(new Error('disk full'));
    const clearPersistedDrafts = vi.fn();

    await expect(saveLayoutOrchestrated(baseArgsWithClear(clearPersistedDrafts))).rejects.toThrow();
    expect(clearPersistedDrafts).not.toHaveBeenCalled();
  });

  it('does not call clearPersistedDrafts if rebuildCatalog throws', async () => {
    rebuildCatalog.mockRejectedValue(new Error('catalog build failed'));
    const clearPersistedDrafts = vi.fn();

    await expect(saveLayoutOrchestrated(baseArgsWithClear(clearPersistedDrafts))).rejects.toThrow();
    expect(clearPersistedDrafts).not.toHaveBeenCalled();
  });

  it('is optional (backwards compat — no callback, no throw)', async () => {
    await expect(saveLayoutOrchestrated({
      saveFile, rebuildCatalog, setCatalog, clearMetadata, markClean, hydrateLayout,
      setActiveContext, updatePartialCaptureNodes, getPendingChangeCount,
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
    })).resolves.toBeDefined();
  });

  it('is also called on the saveWithBusWrites (online) path', async () => {
    const saveWithBusWrites = vi.fn<(path: string, deltas: LayoutEditDelta[], ids: string[]) => Promise<SaveWithBusWriteResult>>(async () => ({
      layoutSaved: true,
      writeCounts: noWrites(),
      writeErrors: [],
      reconciled: false,
      catalogRebuilt: true,
      warnings: [],
      layout: makeLayout(),
      persistedNodeIds: [],
      nodeSnapshots: [],
      writtenNodeIds: [],
    }));
    const clearPersistedDrafts = vi.fn();

    await saveLayoutOrchestrated({
      saveWithBusWrites, setCatalog, clearMetadata, markClean, hydrateLayout,
      setActiveContext, updatePartialCaptureNodes, getPendingChangeCount,
      clearPersistedDrafts,
      path: '/test/layout.bowties.yaml',
      deltas: makeDeltas(),
    });

    expect(clearPersistedDrafts).toHaveBeenCalledTimes(1);
  });
});

// ── S8.11: unified inMemorySnapshotKeys promotion ────────────────────────────

describe('saveLayoutOrchestrated — S8.11 inMemorySnapshotKeys promotion', () => {
  function setup() {
    const saveFile = vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>>(
      async () => makeSaveResult({ persistedNodeIds: ['020157000001', '020157000099'] }),
    );
    const rebuildCatalog = vi.fn<(layout: LayoutFile | null) => Promise<BowtieCatalog>>(
      async () => makeCatalog(),
    );
    const setCatalog = vi.fn();
    const clearMetadata = vi.fn();
    const markClean = vi.fn();
    const hydrateLayout = vi.fn();
    const setActiveContext = vi.fn();
    const updatePartialCaptureNodes = vi.fn();
    const getPendingChangeCount = vi.fn(() => 0);
    return {
      saveFile, rebuildCatalog, setCatalog, clearMetadata, markClean, hydrateLayout,
      setActiveContext, updatePartialCaptureNodes, getPendingChangeCount,
    };
  }

  it('appends addNode deltas for each inMemorySnapshotKeys entry', async () => {
    const env = setup();
    await saveLayoutOrchestrated({
      ...env,
      path: '/p/layout.bowties.yaml',
      deltas: makeDeltas(),
      inMemorySnapshotKeys: ['020157000099', '020157000042'],
    });

    expect(env.saveFile).toHaveBeenCalledTimes(1);
    const sentDeltas = env.saveFile.mock.calls[0][1];
    const addNodeDeltas = sentDeltas.filter((d): d is { type: 'addNode'; nodeKey: string } =>
      d.type === 'addNode');
    expect(addNodeDeltas.map((d) => d.nodeKey)).toEqual(['020157000099', '020157000042']);
    // Original deltas preserved
    expect(sentDeltas.filter((d) => d.type !== 'addNode')).toEqual(makeDeltas());
  });

  it('does not append addNode deltas when inMemorySnapshotKeys is empty or omitted', async () => {
    const env = setup();
    await saveLayoutOrchestrated({
      ...env,
      path: '/p/layout.bowties.yaml',
      deltas: makeDeltas(),
      inMemorySnapshotKeys: [],
    });
    const sentDeltas = env.saveFile.mock.calls[0][1];
    expect(sentDeltas.some((d) => d.type === 'addNode')).toBe(false);

    await saveLayoutOrchestrated({
      ...env,
      path: '/p/layout.bowties.yaml',
      deltas: makeDeltas(),
    });
    const sentDeltas2 = env.saveFile.mock.calls[1][1];
    expect(sentDeltas2.some((d) => d.type === 'addNode')).toBe(false);
  });

  it('propagates persistedNodeIds from the save response into the active layout context', async () => {
    const env = setup();
    await saveLayoutOrchestrated({
      ...env,
      path: '/p/layout.bowties.yaml',
      deltas: makeDeltas(),
      inMemorySnapshotKeys: ['020157000099'],
    });

    expect(env.setActiveContext).toHaveBeenCalledTimes(1);
    const ctx = env.setActiveContext.mock.calls[0][0];
    expect(ctx.layoutNodeIds).toEqual(['020157000001', '020157000099']);
  });

  it('appends unified addNode deltas for both real nodes and placeholders', async () => {
    const env = setup();
    await saveLayoutOrchestrated({
      ...env,
      path: '/p/layout.bowties.yaml',
      deltas: [],
      inMemorySnapshotKeys: ['020157000099', 'placeholder:abcd'],
    });
    const sent = env.saveFile.mock.calls[0][1];
    const addNodes = sent.filter((d): d is { type: 'addNode'; nodeKey: string } =>
      d.type === 'addNode');
    expect(addNodes.map((d) => d.nodeKey)).toEqual(['020157000099', 'placeholder:abcd']);
  });

  it('calls clearPersistedPlaceholders only with placeholder keys after a successful save', async () => {
    const env = setup();
    const clearPersistedPlaceholders = vi.fn();
    await saveLayoutOrchestrated({
      ...env,
      path: '/p/layout.bowties.yaml',
      deltas: [],
      inMemorySnapshotKeys: ['020157000099', 'placeholder:1111', 'placeholder:2222'],
      clearPersistedPlaceholders,
    });
    expect(clearPersistedPlaceholders).toHaveBeenCalledTimes(1);
    expect(clearPersistedPlaceholders).toHaveBeenCalledWith(['placeholder:1111', 'placeholder:2222']);
  });

  it('does not call clearPersistedPlaceholders when saveFile throws', async () => {
    const env = setup();
    env.saveFile.mockRejectedValue(new Error('disk full'));
    const clearPersistedPlaceholders = vi.fn();
    await expect(saveLayoutOrchestrated({
      ...env,
      path: '/p/layout.bowties.yaml',
      deltas: [],
      inMemorySnapshotKeys: ['placeholder:1111'],
      clearPersistedPlaceholders,
    })).rejects.toThrow();
    expect(clearPersistedPlaceholders).not.toHaveBeenCalled();
  });

  it('also works on the saveWithBusWrites (online) path', async () => {
    const saveWithBusWrites = vi.fn<(path: string, deltas: LayoutEditDelta[], ids: string[]) => Promise<SaveWithBusWriteResult>>(
      async () => makeBusWriteResult(),
    );
    const clearPersistedPlaceholders = vi.fn();
    await saveLayoutOrchestrated({
      saveWithBusWrites,
      setCatalog: vi.fn(),
      clearMetadata: vi.fn(),
      markClean: vi.fn(),
      hydrateLayout: vi.fn(),
      setActiveContext: vi.fn(),
      updatePartialCaptureNodes: vi.fn(),
      getPendingChangeCount: vi.fn(() => 0),
      clearPersistedPlaceholders,
      path: '/p/layout.bowties.yaml',
      deltas: [],
      inMemorySnapshotKeys: ['placeholder:9999'],
    });
    const sent = saveWithBusWrites.mock.calls[0][1];
    expect(sent.some((d) => d.type === 'addNode')).toBe(true);
    expect(clearPersistedPlaceholders).toHaveBeenCalledWith(['placeholder:9999']);
  });
});
