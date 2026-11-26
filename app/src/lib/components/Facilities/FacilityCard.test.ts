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

const SUPPORTED_TEMPLATE: BehaviorTemplate = {
  templateId: 'automatic-block-signals',
  displayName: 'Automatic Block Signals',
  slots: [{ label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 1, maxChannels: null }],
  mapping: [],
};

beforeEach(async () => {
  channelsStore.reset();
  facilitiesStore.reset();
  behaviorTemplatesStore.reset();
  listBehaviorTemplatesMock.mockResolvedValue([SUPPORTED_TEMPLATE]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
});

function facility(id: string, slots: Record<string, string[]>): Facility {
  return { facilityId: id, templateId: 'automatic-block-signals', name: 'Mainline', slotBindings: { signals: slots.output ?? [] } };
}

describe('FacilityCard status pill (Spec 018 / S6 — D5)', () => {
  it('reads Incomplete from the facade when a required slot is empty', () => {
    const f = facility('f-1', { input: ['ch-bod-1'], output: [] });
    facilitiesStore.hydrateBaseline([f]);
    const spy = vi.spyOn(effectiveLayoutStore, 'facilityStatus');
    render(FacilityCard, { props: { facility: f, template: SUPPORTED_TEMPLATE } });
    expect(screen.getByText(/^Incomplete$/)).toBeInTheDocument();
    expect(spy).toHaveBeenCalledWith('f-1');
    spy.mockRestore();
  });

  it('reads Wired from the facade when every required slot is filled', () => {
    const f = facility('f-1', { input: ['ch-bod-1'], output: ['ch-lamp-2'] });
    facilitiesStore.hydrateBaseline([f]);
    const spy = vi.spyOn(effectiveLayoutStore, 'facilityStatus');
    render(FacilityCard, { props: { facility: f, template: SUPPORTED_TEMPLATE } });
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
    render(FacilityCard, { props: { facility: f, template: SUPPORTED_TEMPLATE } });
    expect(screen.getByText(/^Wired$/)).toBeInTheDocument();
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
  const absFacility: Facility = { facilityId: 'f-abs', templateId: 'automatic-block-signals', name: 'ABS East', slotBindings: { signals: [signal.id] } };

  it('says the track is being drawn while the diagram is loading or absent', () => {
    facilitiesStore.hydrateBaseline([absFacility]);
    const { unmount } = render(FacilityCard, { props: { facility: absFacility, template: ABS } });
    expect(screen.getByText('Drawing the track…')).toBeInTheDocument();
    unmount();

    render(FacilityCard, { props: { facility: absFacility, template: ABS, diagram: { status: 'loading', key: 'k' } } });
    expect(screen.getByText('Drawing the track…')).toBeInTheDocument();
    expect(screen.queryByText(/Red .* Green/)).toBeNull();
  });

  it('says the diagram could not be drawn and shows the error when the preview failed', () => {
    facilitiesStore.hydrateBaseline([absFacility]);
    render(FacilityCard, { props: { facility: absFacility, template: ABS, diagram: { status: 'failed', key: 'k', error: 'Node offline' } } });
    expect(screen.getByText(/The track diagram couldn't be drawn\./)).toHaveTextContent('Node offline');
    expect(screen.queryByText('Drawing the track…')).toBeNull();
  });

  it('offers Edit on an Automatic Block Signals card and emits the edit intent', async () => {
    const f: Facility = { facilityId: 'f-abs', templateId: 'automatic-block-signals', name: 'ABS East', slotBindings: { signals: [] } };
    facilitiesStore.hydrateBaseline([f]);
    const onEdit = vi.fn();

    render(FacilityCard, { props: { facility: f, template: ABS, onEdit } });
    await fireEvent.click(screen.getByRole('button', { name: 'Edit facility' }));

    expect(onEdit).toHaveBeenCalledWith('f-abs');
  });

  it('offers no Add channel picker when the signals slot is empty', () => {
    const f: Facility = { facilityId: 'f-abs', templateId: 'automatic-block-signals', name: 'ABS East', slotBindings: { signals: [] } };
    facilitiesStore.hydrateBaseline([f]);

    render(FacilityCard, { props: { facility: f, template: ABS } });

    expect(screen.queryByTestId('add-channel-button')).toBeNull();
  });
});

describe('FacilityCard write outcome (Spec 020 / E5-T4)', () => {
  it('shows each board waiting with its reason and never the Wired pill', async () => {
    const { pendingChangesStore } = await import('$lib/stores/pendingChanges.svelte');
    const f = facility('f-1', { input: ['ch-bod-1'], output: ['ch-lamp-2'] });
    facilitiesStore.hydrateBaseline([f]);
    const row = (changeId: string, nodeId: string, status: 'failed' | 'pending', error?: string) => ({
      changeId, kind: 'config' as const, nodeId, space: 253, offset: `0x${changeId}`,
      baselineValue: '0', plannedValue: '1', status, ...(error ? { error } : {}),
      origin: { facilityId: 'f-1' },
    });
    pendingChangesStore.setRows([
      row('1', '05.02.01.02.03.00', 'failed', 'Timed out'),
      row('2', '05.02.01.02.03.00', 'pending'),
      row('3', '05.02.01.02.03.01', 'pending'),
    ]);

    render(FacilityCard, { props: { facility: f, template: SUPPORTED_TEMPLATE } });

    expect(screen.queryByText(/^Wired$/)).toBeNull();
    expect(screen.getByText(/^Waiting$/)).toBeInTheDocument();
    expect(screen.getByText(/1 change waiting on .*: Timed out/)).toBeInTheDocument();
    expect(screen.getAllByText(/1 change not yet written to /)).toHaveLength(2);
    pendingChangesStore.clear();
  });
});
