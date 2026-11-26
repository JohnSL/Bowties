import { beforeEach, describe, expect, it, vi } from 'vitest';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { facilityCascadeOrchestrator as cascade } from './facilityCascadeOrchestrator.svelte';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const lamp = (id: string): InformationChannel => ({
  id, name: id, role: 'lamp-indicator', style: 'single-led-direct-lamp', ownership: 'user-owned',
  binding: { kind: 'lampRow', nodeKey: '050101010700', rowOrdinal: 1 },
});
const panel = (id: string, lamps: string[]): Facility => ({
  facilityId: id, templateId: 'panel', name: id, slotBindings: { lamps },
  panel: { litWhen: 'occupied', entries: lamps.map((lampChannelId) => ({ entryId: lampChannelId, blockId: 'block', lampChannelId })) },
});

beforeEach(() => {
  cascade.stopCascade(); channelsStore.reset(); facilitiesStore.reset();
  bowtieMetadataStore.clearAll(); configChangesStore.clearAllDrafts();
});

describe('facility channel cascade after Block Indicator retirement', () => {
  it('does nothing when the channel inventory is unchanged', () => {
    channelsStore.hydrateBaseline([lamp('one')]); facilitiesStore.hydrateBaseline([panel('p', ['one'])]);
    cascade.reconcile(new Set(['one'])); cascade.reconcile(new Set(['one']));
    expect(facilitiesStore.isDirty).toBe(false);
  });

  it('detaches every lost membership across facilities without event or metadata edits', () => {
    channelsStore.hydrateBaseline([lamp('one'), lamp('two')]);
    facilitiesStore.hydrateBaseline([panel('p', ['one']), panel('q', ['two'])]);
    bowtieMetadataStore.createBowtie('0102030405060708', 'Old bowtie name');
    const metadata = bowtieMetadataStore.collectDeltas();
    cascade.reconcile(new Set(['one', 'two'])); cascade.reconcile(new Set());
    expect(facilitiesStore.facilities.map((f) => f.slotBindings.lamps)).toEqual([[], []]);
    expect(facilitiesStore.collectDeltas()).toHaveLength(2);
    expect(configChangesStore.draftEntries()).toEqual([]);
    expect(bowtieMetadataStore.collectDeltas()).toEqual(metadata);
  });

  it('does not duplicate an already detached membership when a channel is removed', () => {
    channelsStore.hydrateBaseline([lamp('one')]); facilitiesStore.hydrateBaseline([panel('p', ['one'])]);
    cascade.reconcile(new Set(['one']));
    facilitiesStore.detachChannel('p', 'lamps', 'one'); channelsStore.removeUserOwnedChannel('one');
    const deltas = facilitiesStore.collectDeltas();
    cascade.reconcile(new Set());
    expect(facilitiesStore.collectDeltas()).toEqual(deltas);
  });

  it('stages dangling membership repairs, preserves known channels and is idempotent', () => {
    channelsStore.hydrateBaseline([lamp('known')]);
    facilitiesStore.hydrateBaseline([panel('p', ['known', 'ghost']), panel('q', ['other-ghost'])]);
    cascade.reconcileDanglingChannelRefsOnLoad();
    expect(facilitiesStore.facilities.map((f) => f.slotBindings.lamps)).toEqual([['known'], []]);
    expect(facilitiesStore.collectDeltas()).toHaveLength(2);
    const repaired = facilitiesStore.collectDeltas();
    cascade.reconcileDanglingChannelRefsOnLoad();
    expect(facilitiesStore.collectDeltas()).toEqual(repaired);
    expect(configChangesStore.draftEntries()).toEqual([]);
  });

  it('does nothing on load when all memberships resolve', () => {
    channelsStore.hydrateBaseline([lamp('one')]); facilitiesStore.hydrateBaseline([panel('p', ['one'])]);
    cascade.reconcileDanglingChannelRefsOnLoad();
    expect(facilitiesStore.isDirty).toBe(false);
  });

  it('handles shared ABS signal membership without retiring the facility', () => {
    facilitiesStore.hydrateBaseline([{ facilityId: 'abs', templateId: 'automatic-block-signals', name: 'ABS', slotBindings: { signals: ['signal'] } }]);
    cascade.reconcile(new Set(['signal'])); cascade.reconcile(new Set());
    expect(facilitiesStore.facilities[0].slotBindings.signals).toEqual([]);
    expect(facilitiesStore.facilities[0].templateId).toBe('automatic-block-signals');
    expect(configChangesStore.draftEntries()).toEqual([]);
  });
});
