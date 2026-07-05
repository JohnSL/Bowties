/**
 * Spec 018 / S6 (D5) — `FacilityCard.svelte` renders the status pill via the
 * `effectiveLayoutStore.facilityStatus` facade (single-owner derivation per
 * ADR-0004).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
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
});
