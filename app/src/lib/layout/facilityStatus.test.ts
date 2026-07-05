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
