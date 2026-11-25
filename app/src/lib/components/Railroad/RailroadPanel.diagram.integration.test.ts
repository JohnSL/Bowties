/**
 * Spec 020 / signal line first slice - a saved Automatic Block Signals facility
 * shows its read-only track diagram on the Railroad panel, with live occupancy.
 * Real stores and orchestrators; IPC mocked only.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { eventStateStore } from '$lib/stores/eventState.svelte';
import { facilityDiagramsStore } from '$lib/stores/facilityDiagrams.svelte';
import { cancelBlockSignalsEditor } from '$lib/orchestration/blockSignalsEditorOrchestrator';
import {
  mockIpc,
  renderRailroadPanel,
  seedSavedBlockSignals,
  signalIpc,
} from './railroadPanelTestSupport';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const resolvedEventIds = new Map<string, Record<string, string>>([
  ['block:cedar', { occupied: '05.01.01.01.FF.00.00.01', clear: '05.01.01.01.FF.00.00.02' }],
]);

describe('Saved facility track diagram on the Railroad panel', () => {
  beforeEach(async () => {
    channelsStore.reset();
    facilitiesStore.reset();
    behaviorTemplatesStore.reset();
    configChangesStore.clearAllDrafts();
    facilityDiagramsStore.reset();
    eventStateStore.clear();
    cancelBlockSignalsEditor();
    invoke.mockReset();
    seedSavedBlockSignals();
    mockIpc(invoke, signalIpc());
    await renderRailroadPanel(resolvedEventIds);
  });

  it('draws the saved line read-only with names and a live occupancy dot', async () => {
    const card = screen.getByTestId('facility-card');
    const diagram = await within(card).findByLabelText('Track and signals for eastward travel');

    expect(within(diagram).getByText('Cedar Hill East')).toBeInTheDocument();
    expect(within(diagram).getByText('Cedar Hill')).toBeInTheDocument();
    expect(card.textContent).not.toMatch(/Red .* Green/);
    expect(diagram.querySelector('[role="button"], [tabindex], button, a')).toBeNull();
    expect(diagram.querySelector('circle.occupancy.occupied')).toBeNull();

    eventStateStore.record('05.01.01.01.FF.00.00.01', Date.now());
    await waitFor(() => expect(diagram.querySelector('circle.occupancy.occupied')).not.toBeNull());
  });

  it('redraws after the saved record is edited', async () => {
    const card = screen.getByTestId('facility-card');
    await within(card).findByLabelText('Track and signals for eastward travel');
    expect(within(card).queryByText('Mill curve')).toBeNull();

    // the record the editor's Done saves: a second, separately signalled block
    const facility = facilitiesStore.facilities[0];
    facilitiesStore.setBlockSignalsRecord(facility.facilityId, {
      ...facility.blockSignals!,
      blocks: [
        ...facility.blockSignals!.blocks,
        { blockId: 'block:mill', name: 'Mill curve', joinedToPrevious: false, turnouts: [] },
      ],
    });

    expect(await within(card).findByText('Mill curve')).toBeInTheDocument();
  });
});
