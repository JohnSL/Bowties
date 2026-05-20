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
  return { manifestPath: '', nodeFilesWritten: 0, warnings: [], layout: makeLayout(), ...overrides };
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

  it('calls flushPending before saveFile when provided', async () => {
    const callOrder: string[] = [];
    const flushPending = vi.fn(async () => { callOrder.push('flush'); });
    saveFile.mockImplementation(async () => { callOrder.push('save'); return makeSaveResult(); });

    await saveLayoutOrchestrated({ ...baseArgs(), flushPending });

    expect(callOrder).toEqual(['flush', 'save']);
  });

  it('does not require flushPending (backwards compat)', async () => {
    const args = baseArgs();
    await expect(saveLayoutOrchestrated(args)).resolves.toBeDefined();
  });

  it('does not call saveFile if flushPending throws', async () => {
    const flushPending = vi.fn(async () => { throw new Error('flush failed'); });

    await expect(saveLayoutOrchestrated({ ...baseArgs(), flushPending })).rejects.toThrow('flush failed');

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
      path: '/layouts/yard.bowties.yaml',
      getPendingChangeCount: () => 3,
    });

    expect(setActiveContext).toHaveBeenCalledWith(
      expect.objectContaining({
        layoutId: 'yard',
        rootPath: '/layouts/yard.bowties.yaml',
        mode: 'offline_file',
        pendingOfflineChangeCount: 3,
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

function makeBusWriteResult(overrides: Partial<SaveWithBusWriteResult> = {}): SaveWithBusWriteResult {
  return {
    layoutSaved: true,
    busWrites: null,
    reconciled: false,
    catalogRebuilt: true,
    warnings: [],
    layout: makeLayout(),
    ...overrides,
  };
}

describe('saveLayoutOrchestrated — saveWithBusWrites path', () => {
  let saveWithBusWrites: ReturnType<typeof vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveWithBusWriteResult>>>;
  let setCatalog: ReturnType<typeof vi.fn>;
  let clearMetadata: ReturnType<typeof vi.fn>;
  let markClean: ReturnType<typeof vi.fn>;
  let hydrateLayout: ReturnType<typeof vi.fn>;
  let setActiveContext: ReturnType<typeof vi.fn>;
  let updatePartialCaptureNodes: ReturnType<typeof vi.fn>;
  let getPendingChangeCount: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    saveWithBusWrites = vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveWithBusWriteResult>>(async () => makeBusWriteResult());
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
    expect(saveWithBusWrites).toHaveBeenCalledWith('/test/layout.bowties.yaml', deltas);
  });

  it('hydrates layout store from saveWithBusWrites response (ADR-0002)', async () => {
    const persistedLayout = makeLayout();
    saveWithBusWrites.mockResolvedValue(makeBusWriteResult({ layout: persistedLayout }));

    await saveLayoutOrchestrated(busWriteArgs());

    expect(hydrateLayout).toHaveBeenCalledWith(persistedLayout);
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

  it('calls flushPending before saveWithBusWrites when provided', async () => {
    const callOrder: string[] = [];
    const flushPending = vi.fn(async () => { callOrder.push('flush'); });
    saveWithBusWrites.mockImplementation(async () => { callOrder.push('save'); return makeBusWriteResult(); });

    await saveLayoutOrchestrated({ ...busWriteArgs(), flushPending });

    expect(callOrder).toEqual(['flush', 'save']);
  });

  it('does not call saveWithBusWrites if flushPending throws', async () => {
    const flushPending = vi.fn(async () => { throw new Error('flush failed'); });

    await expect(saveLayoutOrchestrated({ ...busWriteArgs(), flushPending })).rejects.toThrow('flush failed');

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
      path: '/layouts/yard.bowties.yaml',
      getPendingChangeCount: () => 5,
    });

    expect(setActiveContext).toHaveBeenCalledWith(
      expect.objectContaining({
        layoutId: 'yard',
        rootPath: '/layouts/yard.bowties.yaml',
        mode: 'offline_file',
        pendingOfflineChangeCount: 5,
      }),
    );
  });

  it('does not call clearMetadata or markClean if saveWithBusWrites throws', async () => {
    saveWithBusWrites.mockRejectedValue(new Error('bus error'));

    await expect(saveLayoutOrchestrated(busWriteArgs())).rejects.toThrow('bus error');

    expect(clearMetadata).not.toHaveBeenCalled();
    expect(markClean).not.toHaveBeenCalled();
    expect(hydrateLayout).not.toHaveBeenCalled();
  });

  it('returns busWriteResult from saveWithBusWrites in orchestrated result', async () => {
    const busResult = makeBusWriteResult({
      busWrites: { total: 3, succeeded: 2, failed: 1, readOnlyRejected: 0 },
      reconciled: true,
    });
    saveWithBusWrites.mockResolvedValue(busResult);

    const result = await saveLayoutOrchestrated(busWriteArgs());

    expect(result.busWriteResult).toMatchObject({ total: 3, succeeded: 2, failed: 1 });
  });

  it('sends empty deltas for first-time save from live bus capture', async () => {
    await saveLayoutOrchestrated({ ...busWriteArgs(), deltas: [] });

    expect(saveWithBusWrites).toHaveBeenCalledWith('/test/layout.bowties.yaml', []);
    expect(clearMetadata).toHaveBeenCalled();
    expect(markClean).toHaveBeenCalled();
    expect(hydrateLayout).toHaveBeenCalled();
    expect(setActiveContext).toHaveBeenCalled();
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
    const saveWithBusWrites = vi.fn<(path: string, deltas: LayoutEditDelta[]) => Promise<SaveWithBusWriteResult>>(async () => ({
      layoutSaved: true,
      busWrites: null,
      reconciled: false,
      catalogRebuilt: true,
      warnings: [],
      layout: makeLayout(),
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
