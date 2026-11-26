/**
 * Spec 021 / S2 — `effectiveLayoutStore.panelCardEntries`, `panelStatus` problems and
 * `channelUsedBy`: the per-entry card view, honest status and block consumers (ADR-0004).
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { nodeRoster, type NodeRosterEntry } from '$lib/stores/nodeRoster.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { editKeyForLeaf } from '$lib/utils/editKey';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';
import type { ConfigNode, GroupConfigNode, LeafConfigNode, NodeConfigTree } from '$lib/types/nodeTree';

/** Stubs the node roster the way the Lamp-row tests do; the getter is read-only on the store. */
const rosterSpy = () => vi.spyOn(nodeRoster, 'allEntries' as unknown as never, 'get') as unknown as { mockReturnValue(entries: unknown[]): unknown };

const NODE_KEY = '05010101FF10';
const NODE_ID = '05.01.01.01.FF.10';

function selectionLeaf(row: number, pin: number): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Lamp Selection',
    description: null,
    elementType: 'int',
    address: 2000 + row,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0', `elem:0#${row}`, 'elem:1'],
    value: { type: 'int', value: pin },
    eventRole: null,
    constraints: {
      mapEntries: [
        { value: 3, label: '#3 H1-R' },
        { value: 9, label: '#9 H3-G' },
        { value: 12, label: '#12 H4-R' },
      ],
    },
  } as unknown as LeafConfigNode;
}

function lampTree(pinsByRow: Record<number, number>): NodeConfigTree {
  const instances: GroupConfigNode[] = Object.entries(pinsByRow).map(([row, pin]) => ({
    kind: 'group',
    name: 'Lamp',
    description: null,
    instance: Number(row),
    instanceLabel: `Lamp ${row}`,
    replicationOf: 'Lamp',
    replicationCount: 16,
    path: ['seg:0', 'elem:0', `elem:0#${row}`],
    children: [selectionLeaf(Number(row), pin)] as ConfigNode[],
    displayName: null,
  }));
  const wrapper: GroupConfigNode = {
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
  return {
    nodeId: NODE_ID,
    identity: null,
    segments: [{ name: 'Direct Lamp Control', description: null, origin: 0, space: 253, children: [wrapper] }],
  };
}

function rosterEntry(tree?: NodeRosterEntry['tree']): NodeRosterEntry {
  return {
    nodeKey: NODE_KEY,
    kind: 'live',
    info: {
      node_id: { bytes: [5, 1, 1, 1, 0xff, 0x10] },
      alias: 0,
      snip_data: { manufacturer: 'RR-CirKits', model: 'Signal-LCC', hardware_version: '', software_version: '', user_name: 'Tower Signals', user_description: '' },
    } as unknown as NodeRosterEntry['info'],
    tree,
    readStatus: 'read',
  };
}

const block = (id: string, name: string): InformationChannel => ({
  id,
  name,
  role: 'block-occupancy',
  style: 'bod-block-detector-input',
  ownership: 'hardware-owned',
  binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input: 1 },
});

const lamp = (id: string, row: number): InformationChannel => ({
  id,
  name: `Lamp ${row}`,
  role: 'lamp-indicator',
  style: 'single-led-direct-lamp',
  ownership: 'user-owned',
  binding: { kind: 'lampRow', nodeKey: NODE_KEY, rowOrdinal: row },
});

const panel = (entries: Array<{ entryId: string; blockId: string; lampChannelId?: string }>, id = 'p-1', name = 'Fascia'): Facility => ({
  facilityId: id,
  templateId: 'panel',
  name,
  slotBindings: { lamps: [] },
  panel: { litWhen: 'occupied', entries },
});

let allEntries: ReturnType<typeof rosterSpy>;

beforeEach(() => {
  channelsStore.reset();
  facilitiesStore.reset();
  configChangesStore.clearAllDrafts();
  allEntries = rosterSpy();
  allEntries.mockReturnValue([]);
});

describe('effectiveLayoutStore.panelCardEntries (Spec 021 / S2)', () => {
  it('lists entries in panel order with the pin from the row Lamp Selection, never the row number', () => {
    allEntries.mockReturnValue([rosterEntry(lampTree({ 1: 12, 2: 9 }))]);
    channelsStore.hydrateBaseline([block('b1', 'Cedar Hill'), block('b2', 'Mill curve'), lamp('l1', 1), lamp('l2', 2)]);
    facilitiesStore.hydrateBaseline([
      panel([
        { entryId: 'e2', blockId: 'b2', lampChannelId: 'l2' },
        { entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' },
        { entryId: 'e3', blockId: 'b1' },
      ]),
    ]);

    expect(effectiveLayoutStore.panelCardEntries('p-1')).toEqual([
      { entryId: 'e2', blockId: 'b2', blockName: 'Mill curve', blockMissing: false,
        lamp: { channelId: 'l2', nodeKey: NODE_KEY, boardName: 'Tower Signals', boardMissing: false, pin: 9, pinLabel: '#9 H3-G' } },
      { entryId: 'e1', blockId: 'b1', blockName: 'Cedar Hill', blockMissing: false,
        lamp: { channelId: 'l1', nodeKey: NODE_KEY, boardName: 'Tower Signals', boardMissing: false, pin: 12, pinLabel: '#12 H4-R' } },
      { entryId: 'e3', blockId: 'b1', blockName: 'Cedar Hill', blockMissing: false, lamp: null },
    ]);
  });

  it('reads the effective pin: an unsaved draft of Lamp Selection wins over the saved value', () => {
    const tree = lampTree({ 1: 9 });
    allEntries.mockReturnValue([rosterEntry(tree)]);
    channelsStore.hydrateBaseline([block('b1', 'Cedar Hill'), lamp('l1', 1)]);
    facilitiesStore.hydrateBaseline([panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }])]);
    const leaf = selectionLeaf(1, 9);
    configChangesStore.set(editKeyForLeaf(NODE_ID, leaf.space, leaf.address), { type: 'int', value: 3 } as never);

    expect(effectiveLayoutStore.panelCardEntries('p-1')[0].lamp).toMatchObject({ pin: 3, pinLabel: '#3 H1-R' });
  });

  it('flags a block that is no longer detected and a lamp whose board is missing', () => {
    channelsStore.hydrateBaseline([lamp('l1', 1)]);
    facilitiesStore.hydrateBaseline([panel([{ entryId: 'e1', blockId: 'gone', lampChannelId: 'l1' }])]);

    const [entry] = effectiveLayoutStore.panelCardEntries('p-1');
    expect(entry.blockMissing).toBe(true);
    expect(entry.lamp).toMatchObject({ boardMissing: true, pin: null });
  });
});
