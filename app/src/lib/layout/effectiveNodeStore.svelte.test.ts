/**
 * Tests for effectiveNodeStore — ADR-0011 (Phase 9 of the Spec 014
 * regression-fix plan).
 *
 * The per-node facade projects the layered stores into a single
 * persistability predicate so Save, the orange in-memory-changes dot,
 * the unsaved-count, and the unsaved-new badge stop drifting.
 *
 * Six contracts pin the surface:
 *
 *   1. nodeOrigin            — three-way classification across live / layout / placeholder.
 *   2. isFullyCaptured       — tree present AND not in partial-capture set (pins ADR-0007).
 *   3. isConfigRead          — thin getter through configReadNodesStore, canonicalised.
 *   4. isPersistableInLayout — fully-captured AND (config-read OR placeholder).
 *       Regression contract for R5: a node with a tree but absent from
 *       configReadNodesStore returns false.
 *   5. unsavedInMemoryNodeIds — live persistable additions not yet in the
 *       layout roster.
 *   6. isDirty               — any persistable addition OR any draft /
 *       metadata / offline-change / layout-struct edit.
 *       Regression contract for R6: an unread real node alone does not
 *       flip isDirty.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import type { DiscoveredNode } from '$lib/api/tauri';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn(), open: vi.fn() }));
vi.mock('$lib/api/layout', () => ({
  addPlaceholderBoardIpc: vi.fn(),
  getNodeTree: vi.fn(),
  listBundledProfiles: vi.fn(),
}));

const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');
const { nodeInfoStore } = await import('$lib/stores/nodeInfo');
const { configReadNodesStore, markNodeConfigRead, clearConfigReadStatus } =
  await import('$lib/stores/configReadStatus');
const { partialCaptureNodesStore } = await import('$lib/stores/partialCaptureNodes.svelte');
const { layoutStore } = await import('$lib/stores/layout.svelte');
const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
const { bowtieMetadataStore } = await import('$lib/stores/bowtieMetadata.svelte');
const { offlineChangesStore } = await import('$lib/stores/offlineChanges.svelte');
const { effectiveNodeStore } = await import('./effectiveNodeStore.svelte');

// ── Fixtures ─────────────────────────────────────────────────────────────────

const LIVE_A = '020157000001';
const LIVE_A_DOTTED = '02.01.57.00.00.01';
const LIVE_B = '020157000002';
const PLACEHOLDER = 'placeholder:11111111-2222-4333-8444-555555555555';

function emptyTree(nodeId: string): NodeConfigTree {
  return {
    nodeId,
    identity: {
      manufacturer: null,
      model: null,
      hardwareVersion: null,
      softwareVersion: null,
    },
    segments: [],
  };
}

function liveInfo(canonical: string): DiscoveredNode {
  const node_id = (canonical.match(/.{1,2}/g) ?? []).map((h) => parseInt(h, 16));
  return {
    node_id,
    alias: 0,
    snip_data: null,
    snip_status: 'NotRequested',
    connection_status: 'Connected',
    last_verified: null,
    last_seen: null,
    cdi: null,
    pip_flags: null,
    pip_status: 'NotRequested',
  };
}

function seedLive(canonical: string): void {
  nodeInfoStore.update((m) => {
    const next = new Map(m);
    next.set(canonical, liveInfo(canonical));
    return next;
  });
}

function seedPlaceholder(key: string): void {
  nodeInfoStore.update((m) => {
    const next = new Map(m);
    next.set(key, liveInfo('000000000000'));
    return next;
  });
}

function setActiveLayoutWith(nodeIds: string[]): void {
  layoutStore.setActiveContext({
    layoutId: '/test/layout',
    rootPath: '/test/layout',
    mode: 'offline_file',
    pendingOfflineChangeCount: 0,
    layoutNodeIds: [...nodeIds],
  });
}

beforeEach(() => {
  nodeTreeStore.reset();
  nodeInfoStore.set(new Map());
  clearConfigReadStatus();
  partialCaptureNodesStore.clear();
  layoutStore.reset();
  configChangesStore.clearAllDrafts();
  bowtieMetadataStore.clearAll();
  offlineChangesStore.clear();
});

// ── 1. nodeOrigin ────────────────────────────────────────────────────────────

describe('effectiveNodeStore.nodeOrigin', () => {
  it('returns placeholder for a placeholder key regardless of layout membership', () => {
    seedPlaceholder(PLACEHOLDER);
    expect(effectiveNodeStore.nodeOrigin(PLACEHOLDER)).toBe('placeholder');

    setActiveLayoutWith([PLACEHOLDER]);
    expect(effectiveNodeStore.nodeOrigin(PLACEHOLDER)).toBe('placeholder');
  });

  it('returns live-only when the node is discovered but not in the layout', () => {
    seedLive(LIVE_A);
    setActiveLayoutWith([]);
    expect(effectiveNodeStore.nodeOrigin(LIVE_A)).toBe('live-only');
  });

  it('returns layout-only when the node is in the saved layout but not discovered', () => {
    setActiveLayoutWith([LIVE_A]);
    expect(effectiveNodeStore.nodeOrigin(LIVE_A)).toBe('layout-only');
  });

  it('returns both when the node is in both the layout and the live roster', () => {
    seedLive(LIVE_A);
    setActiveLayoutWith([LIVE_A]);
    expect(effectiveNodeStore.nodeOrigin(LIVE_A)).toBe('both');
  });

  it('canonicalises dotted live input', () => {
    seedLive(LIVE_A);
    setActiveLayoutWith([LIVE_A]);
    expect(effectiveNodeStore.nodeOrigin(LIVE_A_DOTTED)).toBe('both');
  });
});

// ── 2. isFullyCaptured ───────────────────────────────────────────────────────

describe('effectiveNodeStore.isFullyCaptured', () => {
  it('is true when a tree exists and the node is not partial-capture', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    expect(effectiveNodeStore.isFullyCaptured(LIVE_A)).toBe(true);
  });

  it('is false when there is no tree', () => {
    expect(effectiveNodeStore.isFullyCaptured(LIVE_A)).toBe(false);
  });

  it('is false when the node is in the partial-capture set', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    partialCaptureNodesStore.replace([LIVE_A]);
    expect(effectiveNodeStore.isFullyCaptured(LIVE_A)).toBe(false);
  });

  it('accepts dotted input', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    expect(effectiveNodeStore.isFullyCaptured(LIVE_A_DOTTED)).toBe(true);
  });
});

// ── 3. isConfigRead ──────────────────────────────────────────────────────────

describe('effectiveNodeStore.isConfigRead', () => {
  it('reflects the configReadNodesStore set, canonicalised', () => {
    expect(effectiveNodeStore.isConfigRead(LIVE_A)).toBe(false);
    markNodeConfigRead(LIVE_A);
    expect(effectiveNodeStore.isConfigRead(LIVE_A)).toBe(true);
    expect(effectiveNodeStore.isConfigRead(LIVE_A_DOTTED)).toBe(true);
  });
});

// ── 4. isPersistableInLayout — R5 regression contract ────────────────────────

describe('effectiveNodeStore.isPersistableInLayout', () => {
  it('is false for a live node that has a tree but has not been config-read (R5)', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    expect(effectiveNodeStore.isPersistableInLayout(LIVE_A)).toBe(false);
  });

  it('is true for a live node that is fully captured and config-read', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    markNodeConfigRead(LIVE_A);
    expect(effectiveNodeStore.isPersistableInLayout(LIVE_A)).toBe(true);
  });

  it('is true for a placeholder that is fully captured even without config-read', () => {
    nodeTreeStore.setTree(PLACEHOLDER, emptyTree(PLACEHOLDER));
    expect(effectiveNodeStore.isPersistableInLayout(PLACEHOLDER)).toBe(true);
  });

  it('is false when partial-capture warnings flag the node', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    markNodeConfigRead(LIVE_A);
    partialCaptureNodesStore.replace([LIVE_A]);
    expect(effectiveNodeStore.isPersistableInLayout(LIVE_A)).toBe(false);
  });
});

// ── 5. unsavedInMemoryNodeIds ────────────────────────────────────────────────

describe('effectiveNodeStore.unsavedInMemoryNodeIds', () => {
  it('returns persistable live keys absent from the layout roster', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    markNodeConfigRead(LIVE_A);
    setActiveLayoutWith([]);
    expect(effectiveNodeStore.unsavedInMemoryNodeIds).toEqual([LIVE_A]);
  });

  it('excludes live keys already in the layout roster', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    markNodeConfigRead(LIVE_A);
    setActiveLayoutWith([LIVE_A]);
    expect(effectiveNodeStore.unsavedInMemoryNodeIds).toEqual([]);
  });

  it('excludes live keys that are not config-read (R5)', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    setActiveLayoutWith([]);
    expect(effectiveNodeStore.unsavedInMemoryNodeIds).toEqual([]);
  });

  it('returns [] when no layout context is active (pre-S8 semantics)', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    markNodeConfigRead(LIVE_A);
    // no setActiveLayoutWith call → activeContext is null
    expect(effectiveNodeStore.unsavedInMemoryNodeIds).toEqual([]);
  });

  it('includes an unsaved placeholder absent from the layout roster', () => {
    // Mirror what `nodeRoster.addPlaceholder` does: seed tree + info + read-status.
    nodeTreeStore.setTree(PLACEHOLDER, emptyTree('00.00.00.00.00.00'));
    seedPlaceholder(PLACEHOLDER);
    markNodeConfigRead(PLACEHOLDER);
    setActiveLayoutWith([]);
    expect(effectiveNodeStore.unsavedInMemoryNodeIds).toEqual([PLACEHOLDER]);
  });

  it('excludes a placeholder already in the layout roster (post-save)', () => {
    nodeTreeStore.setTree(PLACEHOLDER, emptyTree('00.00.00.00.00.00'));
    seedPlaceholder(PLACEHOLDER);
    markNodeConfigRead(PLACEHOLDER);
    setActiveLayoutWith([PLACEHOLDER]);
    expect(effectiveNodeStore.unsavedInMemoryNodeIds).toEqual([]);
  });
});

// ── 6. isDirty — R6 regression contract ──────────────────────────────────────

describe('effectiveNodeStore.isDirty', () => {
  it('is false when an unread real node is discovered (R6)', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    seedLive(LIVE_A);
    setActiveLayoutWith([]);
    expect(effectiveNodeStore.isDirty).toBe(false);
  });

  it('is true when a persistable live addition is pending save', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    markNodeConfigRead(LIVE_A);
    setActiveLayoutWith([]);
    expect(effectiveNodeStore.isDirty).toBe(true);
  });

  it('is true when offline changes are pending', () => {
    setActiveLayoutWith([]);
    offlineChangesStore.upsertConfigChange({
      nodeId: LIVE_A_DOTTED,
      space: 251,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '5',
    });
    expect(effectiveNodeStore.isDirty).toBe(true);
  });

  it('is true when bowtie metadata is dirty', () => {
    setActiveLayoutWith([]);
    bowtieMetadataStore.deleteBowtie('01.01.01.01.01.01.01.01');
    expect(effectiveNodeStore.isDirty).toBe(true);
  });

  it('is true when an unsaved placeholder is the only change', () => {
    nodeTreeStore.setTree(PLACEHOLDER, emptyTree('00.00.00.00.00.00'));
    seedPlaceholder(PLACEHOLDER);
    markNodeConfigRead(PLACEHOLDER);
    setActiveLayoutWith([]);
    expect(effectiveNodeStore.isDirty).toBe(true);
  });
});
