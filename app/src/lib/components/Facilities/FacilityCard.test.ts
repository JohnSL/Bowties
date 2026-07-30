/**
 * Spec 018 / S6 (D5) — `FacilityCard.svelte` renders the status pill via the
 * `effectiveLayoutStore.facilityStatus` facade (single-owner derivation per
 * ADR-0004).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';
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

const resolveNodeNameMock = vi.fn<(nodeId: string) => string>((nodeId) => nodeId);
vi.mock('$lib/layout', async (importOriginal) => {
  const actual = await importOriginal<typeof import('$lib/layout')>();
  return { ...actual, resolveNodeName: (...args: Parameters<typeof actual.resolveNodeName>) => resolveNodeNameMock(...args) };
});

const { focusConfigFieldMock } = vi.hoisted(() => ({
  focusConfigFieldMock: vi.fn(),
}));
vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: {
    focusConfigField: focusConfigFieldMock,
  },
}));

const { effectiveLayoutStore } = await import('$lib/layout/effectiveLayoutStore.svelte');
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
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
  compilationTarget: 'composed' as const,
  rules: [],
};

const ABS_3_ASPECT: BehaviorTemplate = {
  templateId: 'abs-3-aspect-signal',
  displayName: 'ABS 3-Aspect Signal',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1, shared: true },
    { label: 'output', displayLabel: 'signal', kind: 'consumer', requiredRole: 'signal-aspect', minChannels: 1, maxChannels: 1 },
    { label: 'downstream-signal', displayLabel: 'downstream', kind: 'producer', requiredRole: 'signal-aspect', minChannels: 0, maxChannels: 1, shared: true },
  ],
  mapping: [],
  compilationTarget: 'compiled' as const,
  rules: [
    { label: 'Stop', priority: 1, condition: { inputSlot: 'input', producerState: 'occupied' }, aspect: 'stop' },
    { label: 'Clear', priority: 3, condition: { inputSlot: 'input', producerState: 'clear' }, aspect: 'clear' },
  ],
};

beforeEach(async () => {
  facilitiesStore.reset();
  channelsStore.reset();
  behaviorTemplatesStore.reset();
  focusConfigFieldMock.mockClear();
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
});

describe('FacilityCard comprehension view (Spec 020 / S4)', () => {
  it('shows comprehension view directly for compiled-template facilities', () => {
    const f: Facility = { facilityId: 'f-abs', templateId: 'abs-3-aspect-signal', name: 'Signal 5',
      slotBindings: { input: ['ch-1'], output: ['ch-2'], 'downstream-signal': [] } };
    facilitiesStore.hydrateBaseline([f]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Wired');
    render(FacilityCard, { props: { facility: f, template: ABS_3_ASPECT } });
    expect(screen.getByTestId('comprehension-view')).toBeInTheDocument();
  });

  it('does not show comprehension view for composed-template facilities', () => {
    const f: Facility = { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
      slotBindings: { input: ['ch-1'], output: ['ch-2'] } };
    facilitiesStore.hydrateBaseline([f]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Wired');
    render(FacilityCard, { props: { facility: f, template: BLOCK_INDICATOR } });
    expect(screen.queryByTestId('comprehension-view')).not.toBeInTheDocument();
  });

  it('shows slot-grid for composed-template facilities', () => {
    const f: Facility = { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
      slotBindings: { input: ['ch-1'], output: ['ch-2'] } };
    facilitiesStore.hydrateBaseline([f]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Wired');
    render(FacilityCard, { props: { facility: f, template: BLOCK_INDICATOR } });
    expect(screen.getAllByTestId('facility-slot').length).toBeGreaterThan(0);
  });

  it('downstream-signal empty state shows "Add channel" button that invokes onSelectChannel', async () => {
    const f: Facility = { facilityId: 'f-abs', templateId: 'abs-3-aspect-signal', name: 'Signal 5',
      slotBindings: { input: ['ch-1'], output: ['ch-2'], 'downstream-signal': [] } };
    facilitiesStore.hydrateBaseline([f]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Wired');
    const selectHandler = vi.fn();
    render(FacilityCard, { props: { facility: f, template: ABS_3_ASPECT, onSelectChannel: selectHandler } });

    // Comprehension view is shown directly — no expand click needed.
    // Scope to the downstream-signal slot card specifically.
    const dsSlot = screen.getByTestId('comprehension-view').querySelector('[data-slot="downstream-signal"]')!;
    const addBtn = within(dsSlot as HTMLElement).getByTestId('add-channel-button');
    expect(addBtn).toBeInTheDocument();

    await fireEvent.click(addBtn);
    expect(selectHandler).toHaveBeenCalledWith('f-abs', 'downstream-signal');
  });

  it('displays resolved node name for logic target instead of raw key', () => {
    const f: Facility = {
      facilityId: 'f-abs', templateId: 'abs-3-aspect-signal', name: 'Signal 5',
      slotBindings: { input: ['ch-1'], output: ['ch-2'], 'downstream-signal': [] },
      logicAllocation: { facilityId: 'f-abs', targetNodeKey: '05010101FF000001', conditionalLines: { start: 0, count: 2 } },
    };
    facilitiesStore.hydrateBaseline([f]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Wired');
    resolveNodeNameMock.mockReturnValue('Tower LCC — Main');
    render(FacilityCard, { props: { facility: f, template: ABS_3_ASPECT } });
    expect(screen.getByText('Tower LCC — Main')).toBeInTheDocument();
    expect(screen.queryByText('05010101FF000001')).not.toBeInTheDocument();
  });
});

function connectorInputChannel(): InformationChannel {
  return {
    id: 'ch-1',
    name: 'Block 3 Occupancy',
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: '020157000001', connector: 'connector-a', input: 2 },
  };
}

function connectorInputTree(): NodeConfigTree {
  return {
    nodeId: '020157000001',
    identity: null,
    connectorProfile: {
      nodeId: '020157000001',
      carrierKey: 'carrier-1',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['bod-8'],
          affectedPaths: [],
          resolvedAffectedPaths: [['seg:0', 'elem:0#1'], ['seg:0', 'elem:0#2']],
        },
      ],
    },
    segments: [],
  };
}

describe('FacilityCard — channel-to-config navigation', () => {
  it('compiled view: clicking the meta line navigates via configFocusStore using the nodeTree resolver', async () => {
    const tree = connectorInputTree();
    channelsStore.hydrateBaseline([connectorInputChannel()]);
    const f: Facility = { facilityId: 'f-abs', templateId: 'abs-3-aspect-signal', name: 'Signal 5',
      slotBindings: { input: ['ch-1'], output: [], 'downstream-signal': [] } };
    facilitiesStore.hydrateBaseline([f]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Incomplete');

    render(FacilityCard, {
      props: {
        facility: f,
        template: ABS_3_ASPECT,
        nodeTree: (nodeKey: string) => (nodeKey === tree.nodeId ? tree : undefined),
      },
    });

    await fireEvent.click(screen.getByTestId('slot-config-nav'));

    expect(focusConfigFieldMock).toHaveBeenCalledWith('020157000001', ['seg:0', 'elem:0#2']);
  });

  it('non-compiled view: clicking the meta line navigates via configFocusStore using the nodeTree resolver', async () => {
    const tree = connectorInputTree();
    channelsStore.hydrateBaseline([connectorInputChannel()]);
    const f: Facility = { facilityId: 'f-1', templateId: 'block-indicator', name: 'Block 5',
      slotBindings: { input: ['ch-1'], output: [] } };
    facilitiesStore.hydrateBaseline([f]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Incomplete');

    render(FacilityCard, {
      props: {
        facility: f,
        template: BLOCK_INDICATOR,
        nodeTree: (nodeKey: string) => (nodeKey === tree.nodeId ? tree : undefined),
      },
    });

    await fireEvent.click(screen.getByTestId('slot-config-nav'));

    expect(focusConfigFieldMock).toHaveBeenCalledWith('020157000001', ['seg:0', 'elem:0#2']);
  });
});
