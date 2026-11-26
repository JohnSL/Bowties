import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { effectiveNodeStore } from '$lib/layout';
import RailroadPanel from './RailroadPanel.svelte';
import SaveControls from '../ElementCardDeck/SaveControls.svelte';
import UnsavedChangesDialog from '../UnsavedChangesDialog.svelte';
import { blocks } from './railroadPanelTestSupport';
import { openOfflineLayoutWithReplay } from '$lib/orchestration/offlineLayoutOrchestrator';
import { saveLayoutOrchestrated } from '$lib/orchestration/saveLayoutOrchestrator';
import { openLayoutDirectory, saveLayoutDirectory } from '$lib/api/layout';
import { buildBowtieCatalog } from '$lib/api/bowties';
import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const panel = {
  facilityId: 'migrated', templateId: 'panel', name: 'Indicators without a lamp', slotBindings: { lamps: [] },
  panel: { litWhen: 'occupied', entries: [{ entryId: 'old', blockId: 'block:cedar' }] },
};
const listing = {
  facilities: [panel], logicPlacement: [],
  panelMigration: { indicators: 1, panels: 1, warnings: [] },
};

describe('migration persistence reaches Railroad / Save / close consumers', () => {
  beforeEach(() => {
    facilitiesStore.reset(); channelsStore.reset(); behaviorTemplatesStore.reset();
    pendingChangesStore.clear(); configChangesStore.clearAllDrafts(); bowtieMetadataStore.clearAll();
    connectorSelectionsStore.reset(); layoutStore.reset();
    layoutStore.setActiveContext({ layoutId: 'legacy', rootPath: 'D:\\layouts\\legacy', mode: 'offline_file', pendingChangeCount: 0, layoutNodeIds: [] });
    invoke.mockReset();
    invoke.mockImplementation(async (command: string) => {
      if (command === 'list_facilities') return listing;
      if (command === 'list_channels') return blocks;
      if (command === 'list_behavior_templates') return [{ templateId: 'panel', displayName: 'Panel', slots: [], mapping: [] }];
      if (command === 'list_pending_changes') return [];
      if (command === 'clear_layout_drafts') return;
      if (command === 'build_bowtie_catalog_command') return { bowties: [], built_at: '', source_node_count: 0, total_slots_scanned: 0 };
      if (command === 'open_layout_directory') return {
        layoutId: 'legacy', capturedAt: 't', layout: { schemaVersion: '1.0', bowties: {}, roleClassifications: {} },
        offlineMode: true, nodeCount: 0, partialNodes: [], pendingChangeCount: 0, nodeSnapshots: [],
        recoveryOccurred: false, loadWarnings: [], panelMigration: listing.panelMigration,
      };
      throw new Error(`Unexpected IPC ${command}`);
    });
  });

  it('opens converted cards as unsaved without Sync work, Discard keeps them and equivalent reload stays acknowledged; reopening re-dirties', async () => {
    await channelsStore.loadChannels();
    await facilitiesStore.loadFacilities();
    await behaviorTemplatesStore.loadBehaviorTemplates();
    render(RailroadPanel, { props: { nodeName: (key: string) => key } });
    render(SaveControls, { props: { toolbar: true } });
    const card = await screen.findByTestId('panel-card');
    expect(card).toHaveTextContent('Indicators without a lamp');
    expect(card).toHaveTextContent('Incomplete');
    expect(screen.getByRole('group', { name: 'Configuration save controls' })).toBeInTheDocument();
    expect(effectiveNodeStore.dirtyBreakdown.facilities).toBe(1);
    expect(effectiveNodeStore.dirtyBreakdown.configChanges).toBe(0);
    expect(pendingChangesStore.pendingApplyCount).toBe(0);
    expect(facilitiesStore.collectDeltas()).toEqual([]);
    expect(configChangesStore.draftEntries()).toEqual([]);

    const close = render(UnsavedChangesDialog, { props: {
      message: 'Close this layout?', breakdown: effectiveNodeStore.dirtyBreakdown,
      confirmLabel: 'Discard and close', onConfirm: () => {}, onCancel: () => {},
    } });
    const dialog = screen.getByRole('alertdialog');
    expect(within(dialog).getByText('Facility edits')).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    close.unmount();

    await fireEvent.click(screen.getByRole('button', { name: 'Discard pending changes' }));
    const discard = await screen.findByRole('alertdialog');
    await fireEvent.click(within(discard).getByRole('button', { name: 'Revert' }));
    await waitFor(() => expect(effectiveNodeStore.isDirty).toBe(false));
    expect(screen.getByTestId('panel-card')).toHaveTextContent('Indicators without a lamp');
    await facilitiesStore.loadFacilities();
    expect(effectiveNodeStore.isDirty).toBe(false);
    await openOfflineLayoutWithReplay({
      path: 'D:\\layouts\\legacy', openLayout: openLayoutDirectory,
      hydrateOfflineSnapshots: async () => {}, resetSidebar: () => {},
    });
    await facilitiesStore.loadFacilities();
    expect(effectiveNodeStore.isDirty).toBe(true);
  });

  it('failed Save retains conversion; successful Save acknowledges it even if an equivalent listing still carries the outcome', async () => {
    await facilitiesStore.loadFacilities();
    const save = () => saveLayoutOrchestrated({
      path: 'D:\\layouts\\legacy', deltas: facilitiesStore.collectDeltas(),
      saveFile: (path, deltas) => saveLayoutDirectory(path, true, deltas),
      rebuildCatalog: buildBowtieCatalog, setCatalog: (catalog) => bowtieCatalogStore.setCatalog(catalog),
      clearMetadata: () => bowtieMetadataStore.clearAll(), markClean: () => layoutStore.markClean(),
      hydrateLayout: (layout) => layoutStore.hydrateFromBackend(layout),
      setActiveContext: (context) => layoutStore.setActiveContext(context),
      updatePartialCaptureNodes: () => {}, getPendingChangeCount: () => pendingChangesStore.pendingApplyCount,
    });
    invoke.mockImplementationOnce(async () => { throw new Error('Disk full'); });
    await expect(save()).rejects.toThrow('Disk full');
    expect(effectiveNodeStore.isDirty).toBe(true);
    invoke.mockImplementationOnce(async () => ({
      manifestPath: 'D:\\layouts\\legacy\\manifest.yaml', nodeFilesWritten: 0, warnings: [],
      layout: { schemaVersion: '1.0', bowties: {}, roleClassifications: {} }, persistedNodeIds: [], nodeSnapshots: [],
    }));
    await save();
    expect(effectiveNodeStore.isDirty).toBe(false);
    await facilitiesStore.loadFacilities();
    expect(effectiveNodeStore.isDirty).toBe(false);
  });
});
