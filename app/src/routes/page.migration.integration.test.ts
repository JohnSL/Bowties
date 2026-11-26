import { beforeEach, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { SvelteToast, toast } from '@zerodevx/svelte-toast';
import Page from './+page.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

const { invoke, closeHandler } = vi.hoisted(() => ({
  invoke: vi.fn(), closeHandler: { current: null as null | ((event: { preventDefault: () => void }) => Promise<void>) },
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: class {},
  getCurrentWebviewWindow: () => ({
    setTitle: vi.fn(async () => {}), close: vi.fn(),
    onCloseRequested: vi.fn(async (handler) => { closeHandler.current = handler; return () => {}; }),
  }),
}));

beforeEach(() => {
  Element.prototype.animate = vi.fn(() => ({
    finished: Promise.resolve(), cancel() {}, finish() {}, addEventListener() {}, removeEventListener() {},
  })) as unknown as typeof Element.prototype.animate;
  facilitiesStore.reset(); channelsStore.reset(); behaviorTemplatesStore.reset(); layoutStore.reset();
  configChangesStore.clearAllDrafts(); pendingChangesStore.clear(); bowtieMetadataStore.clearAll();
  toast.pop(); invoke.mockReset();
  layoutStore.setActiveContext({ layoutId: 'legacy', rootPath: 'D:\\layouts\\legacy', mode: 'offline_file', pendingChangeCount: 0, layoutNodeIds: [] });
  layoutStore.hydrateFromBackend({
    schemaVersion: '1.0', bowties: { '0102030405060708': { name: 'Old Main occupied', tags: [] } },
    roleClassifications: {}, nodeModeSelections: {},
  });
  bowtieCatalogStore.setCatalog({ bowties: [], built_at: '', source_node_count: 1, total_slots_scanned: 0 });
  nodeInfoStore.set(new Map([['050101010700', {
    node_id: [5, 1, 1, 1, 7, 0], alias: 1, snip_data: null, snip_status: 'Unknown',
    connection_status: 'Unknown', last_verified: null, last_seen: 't', cdi: null, pip_flags: null, pip_status: 'Unknown',
  }]]));
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_connection_status') return { connected: false, config: null };
    if (command === 'list_facilities') return {
      facilities: [{
        facilityId: 'old', templateId: 'panel', name: 'Indicators without a lamp', slotBindings: { lamps: [] },
        panel: { litWhen: 'occupied', entries: [{ entryId: 'old', blockId: 'block' }] },
      }], logicPlacement: [],
      panelMigration: { indicators: 4, panels: 2, warnings: ["Block Indicator 'Broken': dropped malformed record."] },
    };
    if (command === 'list_behavior_templates') return [{ templateId: 'panel', displayName: 'Panel', slots: [], mapping: [] }];
    if (command === 'build_bowtie_catalog_command') return { bowties: [], built_at: '', source_node_count: 0, total_slots_scanned: 0 };
    if (['list_channels', 'list_pending_changes', 'get_known_layouts', 'list_connection_configs'].includes(command)) return [];
    if (command === 'get_recent_layout') return null;
    return null;
  });
});

it('automatically notifies conversion counts and explicit warnings, renders Panel cards, and prompts on window close without Sync writes', async () => {
  render(SvelteToast);
  render(Page);
  expect(await screen.findByText(/4 Block Indicators became 2 Panels/)).toBeInTheDocument();
  expect(screen.getByText(/Broken.*dropped malformed record/)).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('tab', { name: 'Railroad' }));
  expect(await screen.findByTestId('panel-card')).toHaveTextContent('Indicators without a lamp');
  expect(screen.getByRole('group', { name: 'Configuration save controls' })).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('tab', { name: 'Bowties' }));
  expect(await screen.findByText('Old Main occupied')).toBeInTheDocument();
  await fireEvent.click(screen.getByRole('tab', { name: 'Railroad' }));
  await waitFor(() => expect(closeHandler.current).not.toBeNull());
  await closeHandler.current!({ preventDefault: () => {} });
  const dialog = await screen.findByRole('alertdialog');
  expect(within(dialog).getByText('Facility edits')).toBeInTheDocument();
  await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
  expect(pendingChangesStore.pendingApplyCount).toBe(0);
  expect(configChangesStore.draftEntries()).toEqual([]);
  expect(invoke.mock.calls.some(([cmd]) => /compose|write_memory|apply_pending/.test(cmd))).toBe(false);
});
