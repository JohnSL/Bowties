/**
 * Spec 018 / S6 (D5) — `effectiveLayoutStore.facilityStatus(facilityId)`.
 *
 * ADR-0004 single-owner facade for the FacilityCard status pill.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';

const listBehaviorTemplatesMock = vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []);
vi.mock('$lib/api/behaviorTemplates', () => ({
  listBehaviorTemplates: listBehaviorTemplatesMock,
}));
vi.mock('$lib/api/facilities', () => ({
  listFacilities: async () => [] as Facility[],
}));

const { effectiveLayoutStore } = await import('$lib/layout/effectiveLayoutStore.svelte');
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { nodeRoster } = await import('$lib/stores/nodeRoster.svelte');

/** Stubs the node roster the way the Lamp-row tests do; the getter is read-only on the store. */
const rosterSpy = () => vi.spyOn(nodeRoster, 'allEntries' as unknown as never, 'get') as unknown as { mockReturnValue(entries: unknown[]): void };


const BLOCK_INDICATOR: BehaviorTemplate = {
  templateId: 'block-indicator',
  displayName: 'Block Indicator',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'output', displayLabel: 'indicator', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
  ],
  mapping: [
    { producerState: 'occupied', consumerCommand: 'lit' },
    { producerState: 'clear', consumerCommand: 'unlit' },
  ],
};

const OPTIONAL_SLOT_TEMPLATE: BehaviorTemplate = {
  templateId: 'block-indicator-with-optional',
  displayName: 'Block Indicator (optional)',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    // minChannels: 0 → forward-compat with future optional slots.
    { label: 'aux', displayLabel: 'aux', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 0, maxChannels: 4 },
  ],
  mapping: [],
};

beforeEach(async () => {
  facilitiesStore.reset();
  channelsStore.reset();
  behaviorTemplatesStore.reset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR, OPTIONAL_SLOT_TEMPLATE]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
});

describe('effectiveLayoutStore.facilityStatus (Spec 018 / S6 — D5)', () => {
  it('returns Incomplete for an unknown facility id', () => {
    expect(effectiveLayoutStore.facilityStatus('nope')).toBe('Incomplete');
  });

  it('returns Incomplete when every slot is empty', () => {
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-1',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: [], output: [] },
      },
    ]);
    expect(effectiveLayoutStore.facilityStatus('f-1')).toBe('Incomplete');
  });

  it('returns Incomplete when only one slot is filled', () => {
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-1',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-1'], output: [] },
      },
    ]);
    expect(effectiveLayoutStore.facilityStatus('f-1')).toBe('Incomplete');
  });

  it('returns Wired when every required slot is at minChannels', () => {
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-1',
        templateId: 'block-indicator',
        name: 'Block 5',
        slotBindings: { input: ['ch-1'], output: ['ch-2'] },
      },
    ]);
    expect(effectiveLayoutStore.facilityStatus('f-1')).toBe('Wired');
  });

  it('returns Wired even when a minChannels: 0 slot is empty (forward-compat)', () => {
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-1',
        templateId: 'block-indicator-with-optional',
        name: 'Block 5',
        slotBindings: { input: ['ch-1'], aux: [] },
      },
    ]);
    expect(effectiveLayoutStore.facilityStatus('f-1')).toBe('Wired');
  });

  it('returns Incomplete when the facility\'s template is unknown', () => {
    facilitiesStore.hydrateBaseline([
      {
        facilityId: 'f-1',
        templateId: 'nonexistent-template',
        name: 'Block 5',
        slotBindings: {},
      },
    ]);
    expect(effectiveLayoutStore.facilityStatus('f-1')).toBe('Incomplete');
  });
});

describe('effectiveLayoutStore.panelStatus (Spec 021 / S1)', () => {
  const panel = (entries: Array<{ entryId: string; blockId: string; lampChannelId?: string }>): Facility => ({
    facilityId: 'p-1',
    templateId: 'panel',
    name: 'Fascia',
    slotBindings: { lamps: [] },
    panel: { litWhen: 'occupied', entries },
  });

  it('is Empty with no entries', () => {
    facilitiesStore.hydrateBaseline([panel([])]);
    expect(effectiveLayoutStore.panelStatus('p-1')).toEqual({ status: 'Empty', needsLamp: 0 });
  });

  const lampBoardTree = () => ({
    nodeId: 'N2', identity: null,
    segments: [{
      name: 'Direct Lamp Control', description: null, origin: 0, space: 253,
      children: [{
        kind: 'group', name: 'Lamp', instance: 0, replicationOf: 'Lamp', replicationCount: 1, path: [], children: [{
          kind: 'group', name: 'Lamp', instance: 1, replicationOf: 'Lamp', replicationCount: 1, path: [], children: [{
            kind: 'leaf', name: 'Lamp Selection', elementType: 'int', address: 11, size: 1, space: 253, path: [],
            value: { type: 'int', value: 9 }, constraints: null,
          }],
        }],
      }],
    }],
  });

  const seed = (lampBoardLoaded = true) => {
    channelsStore.hydrateBaseline([
      { id: 'b1', name: 'Cedar', role: 'block-occupancy', style: 'bod-block-detector-input', ownership: 'hardware-owned', binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input: 1 } },
      { id: 'b2', name: 'Mill', role: 'block-occupancy', style: 'bod-block-detector-input', ownership: 'hardware-owned', binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input: 2 } },
      { id: 'l1', name: 'Lamp 1', role: 'lamp-indicator', style: 'single-led-direct-lamp', ownership: 'user-owned', binding: { kind: 'lampRow', nodeKey: 'N2', rowOrdinal: 1 } },
    ] as never);
    rosterSpy().mockReturnValue(
      lampBoardLoaded ? [{ nodeKey: 'N2', kind: 'live', info: null, tree: lampBoardTree(), readStatus: 'read' }] as never : [],
    );
  };

  it('is Incomplete and counts the entries without a lamp', () => {
    seed();
    facilitiesStore.hydrateBaseline([
      panel([
        { entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' },
        { entryId: 'e2', blockId: 'b2' },
        { entryId: 'e3', blockId: 'b2' },
      ]),
    ]);
    expect(effectiveLayoutStore.panelStatus('p-1')).toEqual({ status: 'Incomplete', needsLamp: 2 });
  });

  it('is Wired when every entry has a lamp, a detected block and a loaded board', () => {
    seed();
    facilitiesStore.hydrateBaseline([panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }])]);
    expect(effectiveLayoutStore.panelStatus('p-1')).toEqual({ status: 'Wired', needsLamp: 0 });
  });

  it('is Incomplete when an entry block is no longer detected, though no lamp is missing', () => {
    seed();
    facilitiesStore.hydrateBaseline([panel([{ entryId: 'e1', blockId: 'gone', lampChannelId: 'l1' }])]);
    expect(effectiveLayoutStore.panelStatus('p-1')).toEqual({ status: 'Incomplete', needsLamp: 0 });
  });

  it('is Incomplete when the board of a lamp is missing, though no lamp is missing', () => {
    seed(false);
    facilitiesStore.hydrateBaseline([panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }])]);
    expect(effectiveLayoutStore.panelStatus('p-1')).toEqual({ status: 'Incomplete', needsLamp: 0 });
  });
});
