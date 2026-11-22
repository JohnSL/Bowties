/**
 * Spec 018 / S6 (D5) — `FacilityCard.svelte` renders the status pill via the
 * `effectiveLayoutStore.facilityStatus` facade (single-owner derivation per
 * ADR-0004).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { InformationChannel } from '$lib/api/channels';
import type { Facility } from '$lib/api/facilities';
import type { NodeConfigTree } from '$lib/types/nodeTree';

const listBehaviorTemplatesMock = vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []);
vi.mock('$lib/api/behaviorTemplates', () => ({
  listBehaviorTemplates: listBehaviorTemplatesMock,
}));
vi.mock('$lib/api/facilities', () => ({
  listFacilities: async () => [] as Facility[],
}));
vi.mock('$lib/api/channels', () => ({
  listChannels: async () => [],
}));

const { effectiveLayoutStore } = await import('$lib/layout/effectiveLayoutStore.svelte');
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const FacilityCard = (await import('./FacilityCard.svelte')).default;

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

beforeEach(async () => {
  channelsStore.reset();
  facilitiesStore.reset();
  behaviorTemplatesStore.reset();
  listBehaviorTemplatesMock.mockResolvedValue([BLOCK_INDICATOR]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
});

function facility(id: string, slots: Record<string, string[]>): Facility {
  return { facilityId: id, templateId: 'block-indicator', name: 'Block 5', slotBindings: slots };
}

describe('FacilityCard status pill (Spec 018 / S6 — D5)', () => {
  it('reads Incomplete from the facade when a required slot is empty', () => {
    const f = facility('f-1', { input: ['ch-bod-1'], output: [] });
    facilitiesStore.hydrateBaseline([f]);
    const spy = vi.spyOn(effectiveLayoutStore, 'facilityStatus');
    render(FacilityCard, { props: { facility: f, template: BLOCK_INDICATOR } });
    expect(screen.getByText(/^Incomplete$/)).toBeInTheDocument();
    expect(spy).toHaveBeenCalledWith('f-1');
    spy.mockRestore();
  });

  it('reads Wired from the facade when every required slot is filled', () => {
    const f = facility('f-1', { input: ['ch-bod-1'], output: ['ch-lamp-2'] });
    facilitiesStore.hydrateBaseline([f]);
    const spy = vi.spyOn(effectiveLayoutStore, 'facilityStatus');
    render(FacilityCard, { props: { facility: f, template: BLOCK_INDICATOR } });
    expect(screen.getByText(/^Wired$/)).toBeInTheDocument();
    expect(spy).toHaveBeenCalledWith('f-1');
    spy.mockRestore();
  });

  it('follows the facade even when a stubbed override returns different values', () => {
    const f = facility('f-1', { input: [], output: [] });
    facilitiesStore.hydrateBaseline([f]);
    // Force the facade to claim the facility is Wired even though its slots
    // are empty; the card must reflect what the facade says.
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Wired');
    render(FacilityCard, { props: { facility: f, template: BLOCK_INDICATOR } });
    expect(screen.getByText(/^Wired$/)).toBeInTheDocument();
  });

  it('renders a Signal LCC channel with its node and paired output labels', () => {
    const signalChannel: InformationChannel = {
      id: 'ch-signal-1',
      name: 'Eastbound signal',
      role: 'signal-indication',
      style: 'signal-lcc-rg',
      ownership: 'hardware-owned',
      binding: {
        kind: 'signalLccRgOutputs',
        nodeKey: '0501010114a1',
        greenOutput: 1,
        redOutput: 3,
      },
    };
    const f = facility('f-signal', { input: [], output: [signalChannel.id] });
    channelsStore.hydrateBaseline([signalChannel]);
    facilitiesStore.hydrateBaseline([f]);

    render(FacilityCard, { props: { facility: f, template: BLOCK_INDICATOR } });

    expect(screen.getByText(/Signal LCC RG Outputs · Node 0501010114a1 · Outputs 1 \/ 3/)).toBeInTheDocument();
  });
});

describe('FacilityCard Automatic Block Signals (Spec 020 / E1a)', () => {
  const ABS: BehaviorTemplate = {
    templateId: 'automatic-block-signals',
    displayName: 'Automatic Block Signals',
    slots: [{ label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 1, maxChannels: null }],
    mapping: [],
  };
  const signal: InformationChannel = {
    id: 'sig-1',
    name: 'Mill curve East',
    role: 'signal-indication',
    style: 'two-driver-rg-signal',
    ownership: 'user-owned',
    binding: { kind: 'signalLccRgOutputs', nodeKey: '020157000001', greenOutput: 1, redOutput: 3 },
  };
  const lampTree = {
    nodeId: '020157000001',
    identity: null,
    segments: [{
      name: 'Rule to Aspect', description: null, origin: 0, space: 253,
      children: [{
        kind: 'leaf', name: 'Lamp Selection', description: null, elementType: 'int',
        address: 0, size: 1, space: 253, path: ['seg:0', 'elem:0'], value: null, eventRole: null,
        constraints: { min: null, max: null, defaultValue: null, mapEntries: [{ value: 1, label: '#1 H1-G' }, { value: 3, label: '#3 H1-R' }] },
      }],
    }],
  } as unknown as NodeConfigTree;

  it('shows each signal read-only with its lamp names and no add or remove controls', () => {
    const f: Facility = { facilityId: 'f-abs', templateId: 'automatic-block-signals', name: 'ABS East', slotBindings: { signals: [signal.id] } };
    channelsStore.hydrateBaseline([signal]);
    facilitiesStore.hydrateBaseline([f]);

    render(FacilityCard, { props: { facility: f, template: ABS, nodeTree: () => lampTree } });

    expect(screen.getByText('Mill curve East')).toBeInTheDocument();
    expect(screen.getByText(/#3 H1-R/)).toBeInTheDocument();
    expect(screen.getByText(/#1 H1-G/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /add channel|select channel|remove|detach/i })).toBeNull();
    expect(screen.queryByTestId('add-channel-button')).toBeNull();
  });

  it('offers Edit on an Automatic Block Signals card and emits the edit intent', async () => {
    const f: Facility = { facilityId: 'f-abs', templateId: 'automatic-block-signals', name: 'ABS East', slotBindings: { signals: [] } };
    facilitiesStore.hydrateBaseline([f]);
    const onEdit = vi.fn();

    render(FacilityCard, { props: { facility: f, template: ABS, onEdit } });
    await fireEvent.click(screen.getByRole('button', { name: 'Edit facility' }));

    expect(onEdit).toHaveBeenCalledWith('f-abs');
  });

  it('offers no Edit on a Block Indicator card', () => {
    const f = facility('f-1', { input: [], output: [] });
    facilitiesStore.hydrateBaseline([f]);

    render(FacilityCard, { props: { facility: f, template: BLOCK_INDICATOR, onEdit: vi.fn() } });

    expect(screen.queryByRole('button', { name: 'Edit facility' })).toBeNull();
  });

  it('offers no Add channel picker when the signals slot is empty', () => {
    const f: Facility = { facilityId: 'f-abs', templateId: 'automatic-block-signals', name: 'ABS East', slotBindings: { signals: [] } };
    facilitiesStore.hydrateBaseline([f]);

    render(FacilityCard, { props: { facility: f, template: ABS } });

    expect(screen.queryByTestId('add-channel-button')).toBeNull();
  });
});
