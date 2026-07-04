/**
 * Spec 018 / S5 (D1) tests for `effectiveLayoutStore.eligibleLampRowsForStyle`.
 *
 * Drives the derivation directly by stubbing `nodeRoster.allEntries` and
 * seeding `channelsStore` with lamp-row claims. The headline integration test
 * at `app/src/lib/components/Facilities/addChannel.integration.test.ts` spies
 * over this method, so the unit-level coverage lives here.
 *
 * The fixtures reflect the **real `build_children` wrapper shape**: a
 * single wrapper group at segment level whose children are the 1..N
 * `Lamp` instances. Earlier sibling-shape fixtures hid the wrapper-traversal
 * bug fixed by the Spec 018 quickchange; we now go through
 * `replicationInstances` and the fixtures match the backend.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { nodeRoster, type NodeRosterEntry } from '$lib/stores/nodeRoster.svelte';
import type {
  ConfigNode,
  GroupConfigNode,
  LeafConfigNode,
  NodeConfigTree,
  SegmentNode,
} from '$lib/types/nodeTree';
import type { InformationChannel } from '$lib/api/channels';

const SIGNAL_NODE_KEY = '05010101FF000010';

function lampInstance(
  ordinal: number,
  opts?: { description?: string },
): GroupConfigNode {
  const children: ConfigNode[] = [];
  if (opts?.description !== undefined) {
    const descLeaf: LeafConfigNode = {
      kind: 'leaf',
      name: 'Lamp Description',
      description: null,
      elementType: 'string',
      address: 1000 + ordinal,
      size: 32,
      space: 253,
      path: ['seg:0', 'elem:0', `elem:0#${ordinal}`, 'elem:0'],
      value: { type: 'string', value: opts.description },
      eventRole: null,
      constraints: null,
    };
    children.push(descLeaf);
  }
  return {
    kind: 'group',
    name: 'Lamp',
    description: null,
    instance: ordinal,
    instanceLabel: `Lamp ${ordinal}`,
    replicationOf: 'Lamp',
    replicationCount: 16,
    path: ['seg:0', 'elem:0', `elem:0#${ordinal}`],
    children,
    displayName: null,
  };
}

function lampWrapper(instances: GroupConfigNode[]): GroupConfigNode {
  return {
    kind: 'group',
    name: 'Lamp',
    description: null,
    instance: 0,
    instanceLabel: 'Lamp',
    replicationOf: 'Lamp',
    replicationCount: instances.length,
    path: ['seg:0', 'elem:0'],
    children: instances,
    displayName: null,
  };
}

function makeSignalLccTree(opts?: {
  lampCount?: number;
  segmentName?: string;
  /** Per-row Lamp Description value, indexed by 1-based ordinal. */
  descriptions?: Record<number, string>;
}): NodeConfigTree {
  const count = opts?.lampCount ?? 4;
  const segName = opts?.segmentName ?? 'Direct Lamp Control';
  const instances: GroupConfigNode[] = Array.from({ length: count }, (_, i) => {
    const ordinal = i + 1;
    const description = opts?.descriptions?.[ordinal];
    return lampInstance(ordinal, description !== undefined ? { description } : undefined);
  });
  const segment: SegmentNode = {
    name: segName,
    description: null,
    origin: 0,
    space: 253,
    children: [lampWrapper(instances)],
  };
  return {
    nodeId: '05.01.01.01.FF.10',
    identity: null,
    segments: [segment],
  };
}

function makeRosterEntry(opts: { nodeKey: string; tree?: NodeConfigTree; userName?: string; model?: string }): NodeRosterEntry {
  return {
    nodeKey: opts.nodeKey,
    kind: 'live',
    info: {
      node_id: { bytes: [5, 1, 1, 1, 0xFF, 0x10] },
      alias: 0,
      snip_data: opts.userName || opts.model
        ? {
            manufacturer: 'RR-CirKits',
            model: opts.model ?? 'Signal-LCC',
            hardware_version: '',
            software_version: '',
            user_name: opts.userName ?? '',
            user_description: '',
          }
        : null,
      snip_status: 'NotQueried',
      connection_status: 'connected',
      last_verified: null,
      last_seen: new Date().toISOString(),
      cdi: null,
      pip_flags: null,
      pip_status: 'NotQueried',
    } as unknown as NodeRosterEntry['info'],
    tree: opts.tree,
    readStatus: 'read',
  };
}

function lampChannel(rowOrdinal: number, nodeKey = SIGNAL_NODE_KEY): InformationChannel {
  return {
    id: `ch-lamp-${rowOrdinal}`,
    name: `Lamp ${rowOrdinal}`,
    role: 'lamp-indicator',
    style: 'single-led-direct-lamp',
    ownership: 'user-owned',
    binding: { kind: 'lampRow', nodeKey, rowOrdinal },
  };
}

let allEntriesSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  channelsStore.reset();
  // Replace nodeRoster.allEntries with a stub so the derivation reads from
  // our fixture instead of the (uninitialised) module store.
  allEntriesSpy = vi
    .spyOn(nodeRoster, 'allEntries' as unknown as never, 'get')
    .mockReturnValue([]);
});

describe('effectiveLayoutStore.eligibleLampRowsForStyle (Spec 018 / S5 D1)', () => {
  it('returns empty for an unknown style id', () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({ nodeKey: SIGNAL_NODE_KEY, tree: makeSignalLccTree() }),
    ]);

    expect(effectiveLayoutStore.eligibleLampRowsForStyle('not-a-real-style')).toEqual([]);
  });

  it('lists every Direct Lamp Control row on a connected node grouped by node when no claims exist', () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 4 }),
        userName: 'Signal-LCC-1',
      }),
    ]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp');

    expect(groups).toHaveLength(1);
    expect(groups[0].nodeKey).toBe(SIGNAL_NODE_KEY);
    expect(groups[0].nodeName).toBe('Signal-LCC-1');
    expect(groups[0].rows).toHaveLength(4);
    expect(groups[0].rows.map((r) => r.rowOrdinal)).toEqual([1, 2, 3, 4]);
    for (const row of groups[0].rows) {
      expect(row.nodeKey).toBe(SIGNAL_NODE_KEY);
      expect(row.nodeName).toBe('Signal-LCC-1');
      expect(row.rowLabel).toMatch(/^Lamp \d+$/);
    }
  });

  it('regression: walks the real build_children wrapper shape (16 rows under one Signal-LCC)', () => {
    // The bug fixed by Spec 018 quickchange: the old hand-rolled traversal
    // only inspected segment-level siblings, so a real Signal-LCC CDI with
    // <group replication="16"><name>Lamp</name>… surfaced as a single
    // ordinal-1 row (the wrapper). After the fix, `replicationInstances`
    // descends the wrapper and yields all 16 instance ordinals.
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 16 }),
        userName: 'Signal-LCC-1',
      }),
    ]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp');

    expect(groups).toHaveLength(1);
    expect(groups[0].rows).toHaveLength(16);
    expect(groups[0].rows.map((r) => r.rowOrdinal)).toEqual(
      Array.from({ length: 16 }, (_, i) => i + 1),
    );
  });

  it("uses the row's Lamp Description value (via getInstanceDisplayName) for the label when present", () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({
          lampCount: 4,
          descriptions: { 1: 'Up Main Block 5', 3: 'Crossover West' },
        }),
        userName: 'Signal-LCC-1',
      }),
    ]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp');

    expect(groups).toHaveLength(1);
    const labels = groups[0].rows.map((r) => r.rowLabel);
    // Format matches the Config tab's `getInstanceDisplayName`:
    // `"${description} (${instance})"` when description present, else
    // `instanceLabel` ("Lamp N").
    expect(labels).toEqual([
      'Up Main Block 5 (1)',
      'Lamp 2',
      'Crossover West (3)',
      'Lamp 4',
    ]);
  });

  it('excludes rows already claimed by a lampRow-binding channel', () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 4 }),
        userName: 'Signal-LCC-1',
      }),
    ]);
    channelsStore.setChannels([lampChannel(2), lampChannel(4)]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp');

    expect(groups).toHaveLength(1);
    expect(groups[0].rows.map((r) => r.rowOrdinal)).toEqual([1, 3]);
  });

  it('puts a row back when excludeChannelId names the channel currently claiming it', () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 4 }),
        userName: 'Signal-LCC-1',
      }),
    ]);
    channelsStore.setChannels([lampChannel(2)]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp', {
      excludeChannelId: 'ch-lamp-2',
    });

    expect(groups).toHaveLength(1);
    expect(groups[0].rows.map((r) => r.rowOrdinal)).toEqual([1, 2, 3, 4]);
  });

  it('skips nodes whose CDI tree has no Direct Lamp Control segment', () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: '05010101FF0000FF',
        tree: makeSignalLccTree({ segmentName: 'Port I/O-1' }),
        userName: 'TowerLCC-1',
      }),
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 2 }),
        userName: 'Signal-LCC-1',
      }),
    ]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp');

    expect(groups).toHaveLength(1);
    expect(groups[0].nodeKey).toBe(SIGNAL_NODE_KEY);
    expect(groups[0].rows).toHaveLength(2);
  });

  it('skips nodes with no tree available', () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({ nodeKey: SIGNAL_NODE_KEY }),
    ]);

    expect(effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp')).toEqual([]);
  });

  it('falls back to model name when user_name is empty', () => {
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 1 }),
        model: 'Signal-LCC',
      }),
    ]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp');

    expect(groups).toHaveLength(1);
    expect(groups[0].nodeName).toBe('Signal-LCC');
    expect(groups[0].rows[0].nodeName).toBe('Signal-LCC');
  });

  it('emits one group per connected Signal-LCC node', () => {
    const SECOND_NODE_KEY = '05010101FF000020';
    allEntriesSpy.mockReturnValue([
      makeRosterEntry({
        nodeKey: SIGNAL_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 4 }),
        userName: 'Signal-LCC-1',
      }),
      makeRosterEntry({
        nodeKey: SECOND_NODE_KEY,
        tree: makeSignalLccTree({ lampCount: 2 }),
        userName: 'Signal-LCC-2',
      }),
    ]);

    const groups = effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp');

    expect(groups).toHaveLength(2);
    expect(groups.map((g) => g.nodeName)).toEqual(['Signal-LCC-1', 'Signal-LCC-2']);
    expect(groups[0].rows).toHaveLength(4);
    expect(groups[1].rows).toHaveLength(2);
  });
});
