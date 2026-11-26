/** Panel card on the Railroad tab (Spec 021 / S1, S2): head, sub line, lamp tiles with live state, honest status. */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, within } from '@testing-library/svelte';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { eventStateStore } from '$lib/stores/eventState.svelte';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';
import { nodeRoster, type NodeRosterEntry } from '$lib/stores/nodeRoster.svelte';
import PanelCard from './PanelCard.svelte';

/** Stubs the node roster the way the Lamp-row tests do; the getter is read-only on the store. */
const rosterSpy = () => vi.spyOn(nodeRoster, 'allEntries' as unknown as never, 'get') as unknown as { mockReturnValue(entries: unknown[]): void };

const BOARD = '05010101FF10';

const block = (id: string, name: string): InformationChannel => ({
  id, name, role: 'block-occupancy', style: 'bod-block-detector-input', ownership: 'hardware-owned',
  binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input: 1 },
});
const lamp = (id: string, row: number): InformationChannel => ({
  id, name: `Lamp ${row}`, role: 'lamp-indicator', style: 'single-led-direct-lamp', ownership: 'user-owned',
  binding: { kind: 'lampRow', nodeKey: BOARD, rowOrdinal: row },
});

/** A Signal LCC whose Lamp rows 1 and 2 have Lamp Selection 9 and 3. */
function boardEntry(): NodeRosterEntry {
  const row = (n: number, pin: number) => ({
    kind: 'group', name: 'Lamp', description: null, instance: n, instanceLabel: `Lamp ${n}`, replicationOf: 'Lamp',
    replicationCount: 2, path: ['seg:0', 'elem:0', `elem:0#${n}`], displayName: null,
    children: [{
      kind: 'leaf', name: 'Lamp Selection', description: null, elementType: 'int', address: 10 + n, size: 1, space: 253,
      path: ['seg:0', 'elem:0', `elem:0#${n}`, 'elem:0'], value: { type: 'int', value: pin }, eventRole: null,
      constraints: { mapEntries: [{ value: 3, label: '#3 H1-R' }, { value: 9, label: '#9 H3-G' }] },
    }],
  });
  return {
    nodeKey: BOARD, kind: 'live',
    info: { snip_data: { manufacturer: 'RR-CirKits', model: 'Signal-LCC', user_name: 'Tower Signals', user_description: '' } },
    tree: {
      nodeId: '05.01.01.01.FF.10', identity: null,
      segments: [{
        name: 'Direct Lamp Control', description: null, origin: 0, space: 253,
        children: [{
          kind: 'group', name: 'Lamp', description: null, instance: 0, instanceLabel: 'Lamp', replicationOf: 'Lamp',
          replicationCount: 2, path: ['seg:0', 'elem:0'], displayName: null, children: [row(1, 9), row(2, 3)],
        }],
      }],
    },
    readStatus: 'read',
  } as unknown as NodeRosterEntry;
}

function panel(entries: Array<{ entryId: string; blockId: string; lampChannelId?: string }>): Facility {
  return { facilityId: 'p-1', templateId: 'panel', name: 'Eagle Creek fascia', slotBindings: { lamps: [] }, panel: { litWhen: 'occupied', entries } };
}

const resolved = new Map<string, Record<string, string>>([
  ['l1', { lit: 'L1_ON', unlit: 'L1_OFF' }],
  ['b1', { occupied: 'B1_OCC', clear: 'B1_CLR' }],
]);

function show(facility: Facility) {
  facilitiesStore.hydrateBaseline([facility]);
  render(PanelCard, { props: { facility, resolvedEventIds: resolved } });
}

function seedWired() {
  channelsStore.hydrateBaseline([block('b1', 'Cedar Hill'), block('b2', 'Mill curve'), lamp('l1', 1), lamp('l2', 2)]);
}

beforeEach(() => {
  facilitiesStore.reset();
  channelsStore.reset();
  eventStateStore.clear();
  pendingChangesStore.clear();
  rosterSpy().mockReturnValue([boardEntry()]);
});

describe('PanelCard', () => {
  it('shows an empty panel as Empty with "No blocks yet." and no Edit hint', () => {
    show(panel([]));
    expect(screen.getByText('Eagle Creek fascia')).toBeTruthy();
    expect(screen.getByText('Panel')).toBeTruthy();
    expect(screen.getByText('Empty')).toBeTruthy();
    expect(screen.getByText('No blocks yet.')).toBeTruthy();
    expect(screen.queryByText(/Edit to add/)).toBeNull();
  });

  it('shows Incomplete with how many blocks need a lamp, and "needs a lamp" on the tile', () => {
    seedWired();
    show(panel([
      { entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' },
      { entryId: 'e2', blockId: 'b2' },
    ]));
    expect(screen.getByText('Incomplete · 1 needs a lamp')).toBeTruthy();
    expect(screen.getByText('2 blocks · lamps on Tower Signals')).toBeTruthy();
    const tiles = screen.getAllByTestId('panel-tile');
    expect(within(tiles[1]).getByText(/^needs a lamp/)).toBeTruthy();
    expect(within(tiles[1]).getByLabelText('No reading')).toBeTruthy();
  });

  it('lists tiles in panel order with the pin label from Lamp Selection and the block reading; no buttons', () => {
    seedWired();
    show(panel([
      { entryId: 'e2', blockId: 'b2', lampChannelId: 'l2' },
      { entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' },
    ]));
    expect(screen.getByText('Wired')).toBeTruthy();
    expect(screen.getByText('2 blocks · lamps on Tower Signals')).toBeTruthy();
    const tiles = screen.getAllByTestId('panel-tile');
    expect(tiles).toHaveLength(2);
    expect(tiles[0]).toHaveTextContent('Mill curve');
    expect(tiles[0]).toHaveTextContent('#3 H1-R · no reading');
    expect(tiles[1]).toHaveTextContent('Cedar Hill');
    expect(tiles[1]).toHaveTextContent('#9 H3-G · no reading');
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('colours the tile dot from the block occupancy, the same indicator as the Channels panel, with no second dot', () => {
    seedWired();
    eventStateStore.record('L1_ON', 5);
    eventStateStore.record('B1_OCC', 6);
    show(panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }]));
    const tile = screen.getByTestId('panel-tile');
    expect(tile).toHaveTextContent('#9 H3-G · occupied');
    const dots = within(tile).getAllByTestId('occupancy-indicator');
    expect(dots).toHaveLength(1);
    expect(dots[0]).toHaveAttribute('aria-label', 'Block occupied');
    expect(dots[0]).toHaveClass('occupied');
  });

  it('shows the block reading as unknown until an event is seen', () => {
    seedWired();
    show(panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }]));
    expect(within(screen.getByTestId('panel-tile')).getByLabelText('No reading')).toBeTruthy();
  });

  it('labels a block that is no longer detected and counts the panel Incomplete', () => {
    channelsStore.hydrateBaseline([lamp('l1', 1)]);
    show(panel([{ entryId: 'e1', blockId: 'gone', lampChannelId: 'l1' }]));
    expect(screen.getByText('Incomplete')).toBeTruthy();
    expect(screen.getByTestId('panel-tile')).toHaveTextContent('Block no longer detected');
  });

  it('says a lamp row with no Lamp Selection needs a pin and counts the panel Incomplete', () => {
    seedWired();
    const entry = boardEntry();
    const lampRows = (entry.tree as unknown as { segments: Array<{ children: Array<{ children: Array<{ children: Array<{ value: unknown }> }> }> }> }).segments[0].children[0].children;
    lampRows[0].children[0].value = { type: 'int', value: 0 };
    rosterSpy().mockReturnValue([entry]);
    show(panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }]));
    expect(screen.getByText('Incomplete')).toBeTruthy();
    expect(screen.getByTestId('panel-tile')).toHaveTextContent('needs a pin');
  });

  it('labels a lamp whose board is missing and counts the panel Incomplete', () => {
    seedWired();
    rosterSpy().mockReturnValue([]);
    show(panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }]));
    expect(screen.getByText('Incomplete')).toBeTruthy();
    expect(screen.getByTestId('panel-tile')).toHaveTextContent('Board missing');
  });

  it('shows Waiting and each board with unwritten changes, never Wired', () => {
    seedWired();
    const row = (changeId: string, nodeId: string, status: 'failed' | 'pending', error?: string) => ({
      changeId, kind: 'config' as const, nodeId, space: 253, offset: `0x${changeId}`,
      baselineValue: '0', plannedValue: '1', status, ...(error ? { error } : {}),
      origin: { facilityId: 'p-1' },
    });
    pendingChangesStore.setRows([row('1', '05.01.01.01.FF.10', 'failed', 'Timed out'), row('2', '05.01.01.01.FF.10', 'pending')]);
    show(panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }]));
    expect(screen.queryByText('Wired')).toBeNull();
    expect(screen.getByText('Waiting')).toBeTruthy();
    expect(screen.getByText(/1 change waiting on Tower Signals: Timed out/)).toBeTruthy();
    expect(screen.getByText(/1 change not yet written to Tower Signals/)).toBeTruthy();
  });
});
