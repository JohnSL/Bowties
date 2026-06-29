/**
 * Spec 018 / S1.2 — integration test for the dirty aggregate.
 *
 * Asserts ADR-0011's restored invariant: `effectiveNodeStore.isDirty` is the
 * single aggregate over every edit-bearing store, and `dirtyBreakdown` exposes
 * a per-bucket snapshot for the UnsavedChangesDialog and SaveControls
 * presenter to render counts from.
 *
 * Each test dirties exactly one store and asserts:
 *   1. `effectiveNodeStore.isDirty === true`
 *   2. `effectiveNodeStore.dirtyBreakdown` reports a non-zero count in the
 *      matching bucket and zero in every other bucket.
 *
 * The empty-baseline case verifies all buckets are zero and isDirty is false.
 *
 * Today (before S1.2) this file fails for `channelsStore` and
 * `connectorSelectionsStore` because those stores were never wired into the
 * aggregate, and fails for every dirtyBreakdown assertion because the getter
 * does not yet exist.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { NodeConfigTree, TreeConfigValue } from '$lib/types/nodeTree';
import type { DiscoveredNode } from '$lib/api/tauri';
import type { InformationChannel } from '$lib/api/channels';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import { editKeyForLeaf } from '$lib/utils/editKey';

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
vi.mock('$lib/api/connectorProfiles', () => ({
  getConnectorProfile: vi.fn(),
}));

const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');
const { nodeInfoStore } = await import('$lib/stores/nodeInfo');
const { configReadNodesStore, markNodeConfigRead, clearConfigReadStatus } =
  await import('$lib/stores/configReadStatus');
const { partialCaptureNodesStore } = await import(
  '$lib/stores/partialCaptureNodes.svelte'
);
const { layoutStore } = await import('$lib/stores/layout.svelte');
const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
const { bowtieMetadataStore } = await import('$lib/stores/bowtieMetadata.svelte');
const { offlineChangesStore } = await import('$lib/stores/offlineChanges.svelte');
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { connectorSelectionsStore } = await import(
  '$lib/stores/connectorSelections.svelte'
);
const { nodeRoster } = await import('$lib/stores/nodeRoster.svelte');
const { effectiveNodeStore } = await import('./effectiveNodeStore.svelte');

// ── Fixtures ─────────────────────────────────────────────────────────────────

const LIVE_A = '020157000001';
const LIVE_A_DOTTED = '02.01.57.00.00.01';
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

function setActiveLayoutWith(nodeIds: string[]): void {
  layoutStore.setActiveContext({
    layoutId: '/test/layout',
    rootPath: '/test/layout',
    mode: 'offline_file',
    pendingOfflineChangeCount: 0,
    layoutNodeIds: [...nodeIds],
  });
}

function intVal(value: number): TreeConfigValue {
  return { type: 'int', value };
}

const TEMPLATE: BehaviorTemplate = {
  templateId: 'block-indicator',
  displayName: 'Block Indicator',
  slots: [
    { label: 'Detector', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'Lamp', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
  ],
  mapping: [],
};

const CHANNEL_A: InformationChannel = {
  id: 'ch-1',
  name: 'BOD A1',
  role: 'block-occupancy',
  style: 'bod-block-detector-input',
  ownership: 'hardware-owned',
  binding: { kind: 'connectorInput', nodeKey: LIVE_A, connector: 'A', input: 1 },
};

const CONNECTOR_PROFILE = {
  nodeId: LIVE_A_DOTTED,
  carrierKey: 'rr-cirkits::tower-lcc',
  slots: [
    {
      slotId: 'connector-a',
      label: 'Connector A',
      order: 0,
      allowNoneInstalled: true,
      supportedDaughterboardIds: ['BOD4-CP'],
      affectedPaths: [],
      resolvedAffectedPaths: [],
      supportedDaughterboardConstraints: [],
    },
  ],
  supportedDaughterboards: [{ daughterboardId: 'BOD4-CP', displayName: 'BOD4-CP' }],
};

// Buckets the breakdown must report. Each test case asserts exactly one
// bucket > 0 (or specific values) and all others zero.
type BucketName =
  | 'config'
  | 'configNodes'
  | 'metadata'
  | 'channels'
  | 'facilities'
  | 'connectorSelections'
  | 'offlineDrafts'
  | 'offlineRevertedPersisted'
  | 'layoutStruct'
  | 'unsavedNewNodes'
  | 'unsavedRemovedNodes';

const ALL_BUCKETS: readonly BucketName[] = [
  'config',
  'configNodes',
  'metadata',
  'channels',
  'facilities',
  'connectorSelections',
  'offlineDrafts',
  'offlineRevertedPersisted',
  'layoutStruct',
  'unsavedNewNodes',
  'unsavedRemovedNodes',
] as const;

function emptyBreakdown(): Record<BucketName, number> {
  return Object.fromEntries(ALL_BUCKETS.map((b) => [b, 0])) as Record<
    BucketName,
    number
  >;
}

function expectOnlyBuckets(
  actual: Record<BucketName, number>,
  expectedNonZero: Partial<Record<BucketName, number>>,
): void {
  const expected = { ...emptyBreakdown(), ...expectedNonZero };
  for (const bucket of ALL_BUCKETS) {
    expect(actual[bucket], `bucket ${bucket}`).toBe(expected[bucket]);
  }
}

beforeEach(async () => {
  nodeTreeStore.reset();
  nodeInfoStore.set(new Map());
  clearConfigReadStatus();
  partialCaptureNodesStore.clear();
  layoutStore.reset();
  configChangesStore.clearAllDrafts();
  bowtieMetadataStore.clearAll();
  offlineChangesStore.clear();
  facilitiesStore.reset();
  channelsStore.reset();
  connectorSelectionsStore.reset();
  // nodeRoster clears persisted removals as part of its general reset path.
  nodeRoster.clearPersistedRemovals();
});

// ── Empty baseline ───────────────────────────────────────────────────────────

describe('dirty aggregate — empty baseline', () => {
  it('isDirty is false with all stores at rest', () => {
    expect(effectiveNodeStore.isDirty).toBe(false);
  });

  it('dirtyBreakdown reports zero in every bucket', () => {
    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, {});
  });
});

// ── Per-bucket cases ─────────────────────────────────────────────────────────

describe('dirty aggregate — config drafts', () => {
  it('flips isDirty and surfaces a config bucket count', () => {
    const key = editKeyForLeaf(LIVE_A_DOTTED, 253, 100);
    configChangesStore.set(key, intVal(9));

    expect(effectiveNodeStore.isDirty).toBe(true);
    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, {
      config: 1,
      configNodes: 1,
    });
  });

  it('counts drafts across multiple nodes', () => {
    configChangesStore.set(editKeyForLeaf(LIVE_A_DOTTED, 253, 100), intVal(9));
    configChangesStore.set(editKeyForLeaf(LIVE_A_DOTTED, 253, 200), intVal(7));
    configChangesStore.set(editKeyForLeaf('02.01.57.00.00.02', 253, 100), intVal(3));

    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, {
      config: 3,
      configNodes: 2,
    });
  });
});

describe('dirty aggregate — bowtie metadata', () => {
  it('flips isDirty and surfaces a metadata bucket count', () => {
    bowtieMetadataStore.deleteBowtie('01.01.01.01.01.01.01.01');

    expect(effectiveNodeStore.isDirty).toBe(true);
    const bd = effectiveNodeStore.dirtyBreakdown;
    expect(bd.metadata).toBeGreaterThan(0);
    // metadata should be the only non-zero bucket
    expectOnlyBuckets(bd, { metadata: bd.metadata });
  });
});

describe('dirty aggregate — channels store', () => {
  it('flips isDirty and surfaces a channels bucket count (currently missing — RED)', () => {
    channelsStore.addPendingChannels([CHANNEL_A]);

    expect(effectiveNodeStore.isDirty).toBe(true);
    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, { channels: 1 });
  });

  it('counts channel renames in the channels bucket', () => {
    channelsStore.hydrateBaseline([CHANNEL_A]);
    expect(effectiveNodeStore.isDirty).toBe(false);

    channelsStore.renameChannel(CHANNEL_A.id, 'Renamed');

    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, { channels: 1 });
  });
});

describe('dirty aggregate — facilities store', () => {
  it('flips isDirty and surfaces a facilities bucket count', () => {
    facilitiesStore.addFacility(TEMPLATE, 'Block 5');

    expect(effectiveNodeStore.isDirty).toBe(true);
    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, { facilities: 1 });
  });
});

describe('dirty aggregate — connector selections', () => {
  it('flips isDirty and surfaces a connectorSelections bucket count (currently missing — RED)', async () => {
    await connectorSelectionsStore.loadNode(LIVE_A_DOTTED, CONNECTOR_PROFILE);
    await connectorSelectionsStore.updateSlotSelection(
      LIVE_A_DOTTED,
      'connector-a',
      'BOD4-CP',
    );

    expect(effectiveNodeStore.isDirty).toBe(true);
    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, {
      connectorSelections: 1,
    });
  });
});

describe('dirty aggregate — offline drafts', () => {
  it('flips isDirty and surfaces an offlineDrafts bucket count', () => {
    setActiveLayoutWith([]);
    offlineChangesStore.upsertConfigChange({
      nodeId: LIVE_A_DOTTED,
      space: 251,
      offset: '0x00000000',
      baselineValue: '0',
      plannedValue: '5',
    });

    expect(effectiveNodeStore.isDirty).toBe(true);
    const bd = effectiveNodeStore.dirtyBreakdown;
    expect(bd.offlineDrafts).toBeGreaterThan(0);
    expectOnlyBuckets(bd, { offlineDrafts: bd.offlineDrafts });
  });
});

describe('dirty aggregate — unsaved new node', () => {
  it('flips isDirty and surfaces an unsavedNewNodes bucket count', () => {
    nodeTreeStore.setTree(LIVE_A, emptyTree(LIVE_A_DOTTED));
    nodeInfoStore.update((m) => new Map(m).set(LIVE_A, liveInfo(LIVE_A)));
    markNodeConfigRead(LIVE_A);
    setActiveLayoutWith([]);

    expect(effectiveNodeStore.isDirty).toBe(true);
    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, {
      unsavedNewNodes: 1,
    });
  });
});

describe('dirty aggregate — unsaved removed node', () => {
  it('flips isDirty and surfaces an unsavedRemovedNodes bucket count', () => {
    nodeTreeStore.setTree(PLACEHOLDER, emptyTree('00.00.00.00.00.00'));
    nodeInfoStore.update((m) => new Map(m).set(PLACEHOLDER, liveInfo('000000000000')));
    markNodeConfigRead(PLACEHOLDER);
    setActiveLayoutWith([PLACEHOLDER]);

    // Sanity: baseline is clean — placeholder is in the layout roster, nothing else dirty.
    expect(effectiveNodeStore.isDirty).toBe(false);

    nodeRoster.removePlaceholder(PLACEHOLDER);

    expect(effectiveNodeStore.isDirty).toBe(true);
    const bd = effectiveNodeStore.dirtyBreakdown;
    expectOnlyBuckets(bd, { unsavedRemovedNodes: 1 });
  });
});

// ── Multi-bucket combination ─────────────────────────────────────────────────

describe('dirty aggregate — mixed buckets', () => {
  it('each bucket reports independently when several stores are dirty', () => {
    // facilities + channels + config drafts
    facilitiesStore.addFacility(TEMPLATE, 'Block 5');
    channelsStore.addPendingChannels([CHANNEL_A]);
    configChangesStore.set(editKeyForLeaf(LIVE_A_DOTTED, 253, 100), intVal(9));

    expect(effectiveNodeStore.isDirty).toBe(true);
    expectOnlyBuckets(effectiveNodeStore.dirtyBreakdown, {
      facilities: 1,
      channels: 1,
      config: 1,
      configNodes: 1,
    });
  });
});
