/** Panel card on the Railroad tab (Spec 021 / S1): name, type chip, derived status, block count. */
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import type { Facility } from '$lib/api/facilities';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import PanelCard from './PanelCard.svelte';

function panel(entries: Array<{ entryId: string; blockId: string; lampChannelId?: string }>): Facility {
  return {
    facilityId: 'p-1',
    templateId: 'panel',
    name: 'Eagle Creek fascia',
    slotBindings: { lamps: [] },
    panel: { litWhen: 'occupied', entries },
  };
}

function show(facility: Facility) {
  facilitiesStore.hydrateBaseline([facility]);
  render(PanelCard, { props: { facility } });
}

beforeEach(() => facilitiesStore.reset());

describe('PanelCard', () => {
  it('shows an empty panel as Empty with "No blocks yet."', () => {
    show(panel([]));
    expect(screen.getByText('Eagle Creek fascia')).toBeTruthy();
    expect(screen.getByText('Panel')).toBeTruthy();
    expect(screen.getByText('Empty')).toBeTruthy();
    expect(screen.getByText('No blocks yet.')).toBeTruthy();
  });

  it('shows Incomplete with how many blocks need a lamp', () => {
    show(panel([
      { entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' },
      { entryId: 'e2', blockId: 'b2' },
    ]));
    expect(screen.getByText('Incomplete')).toBeTruthy();
    expect(screen.getByText('2 blocks')).toBeTruthy();
    expect(screen.getByText('1 needs a lamp')).toBeTruthy();
  });

  it('shows Wired with a singular block count and no Edit, Delete or channel controls in S1', () => {
    show(panel([{ entryId: 'e1', blockId: 'b1', lampChannelId: 'l1' }]));
    expect(screen.getByText('Wired')).toBeTruthy();
    expect(screen.getByText('1 block')).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
  });
});
