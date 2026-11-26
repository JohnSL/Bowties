/**
 * Spec 020 / E6-T2 - Maintenance edits to a saved Automatic Block Signals
 * facility go through the normal unsaved-changes plumbing (integration).
 *
 * A saved three-block facility is edited through the real editor store and
 * orchestrators, then shown through the real `SaveControls` toolbar and
 * `UnsavedChangesDialog`. Only the IPC boundary (the `$lib/api/*` modules) is
 * mocked.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import { previewWith, SIGNAL } from '$lib/components/BlockSignals/editorTestSupport';

const listBehaviorTemplatesMock = vi.fn<() => Promise<BehaviorTemplate[]>>(async () => []);
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('$lib/api/behaviorTemplates', () => ({ listBehaviorTemplates: listBehaviorTemplatesMock }));
vi.mock('$lib/api/facilities', () => ({ listFacilities: async () => [] as Facility[], composeFacilityRemoval: vi.fn() }));
vi.mock('$lib/api/channels', () => ({ listChannels: async () => [] }));
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: vi.fn(async () => undefined),
  clearLayoutDrafts: vi.fn(async () => undefined),
}));
vi.mock('$lib/api/signaling', () => ({
  composeBlockSignals: vi.fn(),
  previewBlockSignals: vi.fn(),
}));
vi.mock('$lib/api/sync', () => ({
  listPendingChanges: vi.fn(async () => []),
  revertPendingChange: vi.fn(),
  setPendingChange: vi.fn(async () => 'row-id'),
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
const { pendingChangesStore } = await import('$lib/stores/pendingChanges.svelte');
const { blockSignalsEditorStore } = await import('$lib/stores/blockSignalsEditor.svelte');
const { effectiveNodeStore } = await import('$lib/layout');
const { composeBlockSignals, previewBlockSignals } = await import('$lib/api/signaling');
const { composeFacilityRemoval } = await import('$lib/api/facilities');
const editor = await import('$lib/orchestration/blockSignalsEditorOrchestrator');
const { removeFacility } = await import('$lib/orchestration/facilityOrchestrator');
const SaveControls = (await import('$lib/components/ElementCardDeck/SaveControls.svelte')).default;
const UnsavedChangesDialog = (await import('$lib/components/UnsavedChangesDialog.svelte')).default;
const FacilitiesSection = (await import('./FacilitiesSection.svelte')).default;

const ABS_TEMPLATE: BehaviorTemplate = {
  templateId: 'automatic-block-signals',
  displayName: 'Automatic Block Signals',
  slots: [
    { label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 0, maxChannels: null },
  ],
  mapping: [],
};

const HOST = '050101018800';
const FACILITY_ID = 'abs-1';
const block = (id: string, name: string) => ({ blockId: id, name, joinedToPrevious: false, turnouts: [] });
const SAVED_BLOCKS = [block('b1', 'Cedar'), block('b2', 'Mill'), block('b3', 'Quarry')];
const SAVED_RECORD = {
  direction: 'eastward',
  movements: 'oneDirection',
  entranceEnd: { kind: 'noPlant' },
  exitEnd: { kind: 'nothingBeyond' },
  blocks: SAVED_BLOCKS,
  settings: { aspects: 3, turnoutNotLined: 'stop', approachLighting: 'alwaysLit' },
  signals: [{
    signalId: 's1', name: 'Cedar East', protectedBlock: 'b1', channelId: 'ch-sig-1',
    resources: { nodeKey: HOST, mast: 1, trackCircuits: [] },
  }],
};
const blockIds = () => facilitiesStore.facilities[0].blockSignals?.blocks.map((b) => b.blockId);

beforeEach(async () => {
  channelsStore.reset();
  facilitiesStore.reset();
  behaviorTemplatesStore.reset();
  configChangesStore.clearAllDrafts();
  pendingChangesStore.clear();
  editor.cancelBlockSignalsEditor();
  listBehaviorTemplatesMock.mockResolvedValue([ABS_TEMPLATE]);
  await behaviorTemplatesStore.loadBehaviorTemplates();
  vi.mocked(previewBlockSignals).mockReset();
  vi.mocked(previewBlockSignals).mockResolvedValue(previewWith([{ ...SIGNAL, signalId: 's1' }]));
  vi.mocked(composeBlockSignals).mockReset();
  vi.mocked(composeBlockSignals).mockResolvedValue({
    edits: [{ nodeKey: HOST, space: 253, address: 10, path: 'a', value: { type: 'int', value: 1 } }],
    signalResources: [{ signalId: 's1', resources: SAVED_RECORD.signals[0].resources }],
    logicPlacement: [],
    issues: [],
  } as never);
  channelsStore.hydrateBaseline([
    {
      id: 'ch-sig-1', name: 'Cedar East', role: 'signal-indication', style: 'two-driver-rg-signal',
      ownership: 'user-owned',
      binding: { kind: 'signalLccRgOutputs', nodeKey: HOST, redOutput: 1, greenOutput: 2 },
    },
    {
      id: 'ch-bod-1', name: 'BOD Cedar', role: 'block-occupancy', style: 'bod-block-detector-input',
      ownership: 'hardware-owned',
      binding: { kind: 'connectorInput', nodeKey: HOST, connector: 'A', input: 1 },
    },
  ] as never);
  facilitiesStore.hydrateBaseline([{
    facilityId: FACILITY_ID,
    templateId: 'automatic-block-signals',
    name: 'Cedar Hill',
    slotBindings: { signals: ['ch-sig-1'] },
    blockSignals: SAVED_RECORD,
  } as never]);
});

describe('Maintenance edits to a saved facility (E6-T2)', () => {
  const edits: Array<[string, () => Promise<void>, string[]]> = [
    ['reordering the blocks', () => editor.moveSignaledBlock('b3', -1), ['b1', 'b3', 'b2']],
    ['removing the middle block', () => editor.removeSignaledBlock('b2'), ['b1', 'b3']],
  ];

  async function editAndFinish(change: () => Promise<void>) {
    await editor.startEditBlockSignalsEditor(FACILITY_ID);
    await change();
    await editor.navigateBlockSignalsEditor('wiring');
    expect(blockSignalsEditorStore.canFinish).toBe(true);
    await editor.finishBlockSignalsEditor();
    expect(blockSignalsEditorStore.open).toBe(false);
  }

  it.each(edits)('%s shows as unsaved in the toolbar, the close prompt and the composed configuration', async (_name, change, expectedOrder) => {
    render(SaveControls, { props: { toolbar: true } });
    expect(effectiveNodeStore.isDirty).toBe(false);

    await editAndFinish(change);

    expect(blockIds()).toEqual(expectedOrder);
    expect(effectiveNodeStore.isDirty).toBe(true);
    const toolbar = await screen.findByRole('group', { name: 'Configuration save controls' });
    expect(within(toolbar).getByText(/1 configuration change \+ other edits/)).toBeInTheDocument();
    expect(within(toolbar).getByRole('button', { name: 'Save' })).toBeEnabled();

    // the composed configuration is an ordinary draft
    expect(configChangesStore.draftEntries()).toHaveLength(1);
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ facilities: 1, configChanges: 1 });

    render(UnsavedChangesDialog, {
      props: {
        message: 'Close this layout?',
        breakdown: effectiveNodeStore.dirtyBreakdown,
        confirmLabel: 'Close without saving',
        onConfirm: () => {},
        onCancel: () => {},
      },
    });
    const prompt = screen.getByRole('alertdialog');
    expect(within(prompt).getByText('1 configuration change')).toBeInTheDocument();
    expect(within(prompt).getByText('Facility edits')).toBeInTheDocument();
  });

  it('Discard restores the saved block order and signals and clears the staged drafts', async () => {
    render(SaveControls, { props: { toolbar: true } });
    await editAndFinish(() => editor.moveSignaledBlock('b3', -1));
    expect(blockIds()).toEqual(['b1', 'b3', 'b2']);

    const toolbar = await screen.findByRole('group', { name: 'Configuration save controls' });
    await fireEvent.click(within(toolbar).getByRole('button', { name: 'Discard pending changes' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Revert' }));

    await waitFor(() => expect(blockIds()).toEqual(['b1', 'b2', 'b3']));
    expect(facilitiesStore.facilities[0].blockSignals).toEqual(SAVED_RECORD);
    expect(configChangesStore.draftEntries()).toEqual([]);
    expect(effectiveNodeStore.isDirty).toBe(false);
    expect(effectiveNodeStore.dirtyBreakdown).toMatchObject({ facilities: 0, configChanges: 0, config: 0, channels: 0 });
  });
});

describe('The only block of a saved facility (E6-T2)', () => {
  it('cannot be removed through to a finished facility with no blocks', async () => {
    render(FacilitiesSection, { props: {} });
    await fireEvent.click(within(await screen.findByTestId('facility-card')).getByRole('button', { name: 'Edit facility' }));
    const dialog = await screen.findByRole('dialog', { name: /edit automatic block signals/i });

    for (const name of ['Cedar', 'Mill', 'Quarry']) {
      await fireEvent.click(await within(dialog).findByRole('button', { name: `Remove ${name} from these signals` }));
    }

    // With no blocks the editor offers no way forward: no Done, later steps unavailable.
    await waitFor(() => expect(within(dialog).getByText(/No blocks yet/)).toBeInTheDocument());
    expect(within(dialog).getByRole('button', { name: 'Next: Aspects' })).toBeDisabled();
    const stepper = within(dialog).getByRole('list', { name: 'Automatic Block Signals steps' });
    for (const step of [/Aspects/, /Appearance/, /Wiring/]) {
      expect(within(stepper).getByRole('button', { name: step })).toBeDisabled();
    }
    expect(within(dialog).queryByRole('button', { name: 'Done' })).toBeNull();
    expect(blockSignalsEditorStore.canFinish).toBe(false);

    // Even if Done were invoked directly, nothing is staged.
    await editor.finishBlockSignalsEditor();
    expect(blockSignalsEditorStore.open).toBe(true);
    expect(blockIds()).toEqual(['b1', 'b2', 'b3']);
    expect(effectiveNodeStore.isDirty).toBe(false);
  });

  it('Remove facility drops the facility and its signal channels but keeps the block detection channel', async () => {
    vi.mocked(composeFacilityRemoval).mockResolvedValue(
      { edits: [], signalResources: [], logicPlacement: [], issues: [] } as never,
    );

    const result = await removeFacility({ facilityId: FACILITY_ID });

    expect(result).toEqual({ ok: true });
    expect(facilitiesStore.facilities).toHaveLength(0);
    expect(channelsStore.channels.map((c) => c.id)).toEqual(['ch-bod-1']);
  });
});
