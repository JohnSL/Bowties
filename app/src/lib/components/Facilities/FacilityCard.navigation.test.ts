import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';
import type { NodeConfigTree } from '$lib/types/nodeTree';

vi.mock('$lib/api/behaviorTemplates', () => ({ listBehaviorTemplates: async () => [] }));
vi.mock('$lib/api/facilities', () => ({ listFacilities: async () => [] }));
vi.mock('$lib/api/channels', () => ({ listChannels: async () => [] }));
const { focusConfigFieldMock } = vi.hoisted(() => ({ focusConfigFieldMock: vi.fn() }));
vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: { focusConfigField: focusConfigFieldMock },
}));

const { channelsStore } = await import('$lib/stores/channels.svelte');
const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { effectiveLayoutStore } = await import('$lib/layout/effectiveLayoutStore.svelte');
const FacilityCard = (await import('./FacilityCard.svelte')).default;

const template: BehaviorTemplate = {
  templateId: 'block-indicator',
  displayName: 'Block Indicator',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'output', displayLabel: 'indicator', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
  ],
  mapping: [],
};
const facility: Facility = {
  facilityId: 'f-1',
  templateId: 'block-indicator',
  name: 'Block 5',
  slotBindings: { input: ['ch-1'], output: [] },
};
const channel: InformationChannel = {
  id: 'ch-1',
  name: 'Block 3 Occupancy',
  role: 'block-occupancy',
  style: 'bod-block-detector-input',
  ownership: 'hardware-owned',
  binding: { kind: 'connectorInput', nodeKey: '020157000001', connector: 'connector-a', input: 2 },
};
const tree: NodeConfigTree = {
  nodeId: '020157000001',
  identity: null,
  connectorProfile: {
    nodeId: '020157000001',
    carrierKey: 'carrier-1',
    slots: [{
      slotId: 'connector-a', label: 'Connector A', order: 0, allowNoneInstalled: true,
      supportedDaughterboardIds: ['bod-8'], affectedPaths: [],
      resolvedAffectedPaths: [['seg:0', 'elem:0#1'], ['seg:0', 'elem:0#2']],
    }],
  },
  segments: [],
};

describe('FacilityCard configuration navigation', () => {
  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
    focusConfigFieldMock.mockClear();
    channelsStore.hydrateBaseline([channel]);
    facilitiesStore.hydrateBaseline([facility]);
    vi.spyOn(effectiveLayoutStore, 'facilityStatus').mockReturnValue('Incomplete');
  });

  it('navigates from a Block Indicator slot to its bound channel configuration', async () => {
    render(FacilityCard, {
      facility,
      template,
      nodeTree: (nodeKey: string) => nodeKey === tree.nodeId ? tree : undefined,
    });

    await fireEvent.click(screen.getByTestId('slot-config-nav'));

    expect(focusConfigFieldMock).toHaveBeenCalledWith('020157000001', ['seg:0', 'elem:0#2']);
  });
});
