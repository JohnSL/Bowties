import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import SaveControls from '$lib/components/ElementCardDeck/SaveControls.svelte';
import * as layoutFacade from '$lib/layout';
import { effectiveNodeStore } from '$lib/layout';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { layoutEditParticipants } from '$lib/orchestration/layoutEditParticipants';
import { discardLayoutEdits } from '$lib/orchestration/layoutDiscardOrchestrator';

const invoke = vi.hoisted(() => vi.fn(async (_command: string): Promise<void> => undefined));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

it('Discard leaves placeholder additions and removals dirty until S7', async () => {
  const nodeKey = 'placeholder:11111111-2222-4333-8444-555555555555';
  layoutStore.hydrateOfflineLayout(
    { schemaVersion: '1.0', bowties: {}, roleClassifications: {} },
    { layoutId: 'layout', rootPath: 'layout', mode: 'offline_file', pendingChangeCount: 0, layoutNodeIds: [nodeKey] },
  );
  nodeRoster.removePlaceholder(nodeKey);
  nodeRoster.addPlaceholder({
    nodeKey: 'placeholder:aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
    profileStem: 'stem',
    info: {} as never,
    tree: { segments: [] } as never,
  });
  expect(effectiveNodeStore.dirtyBreakdown.unsavedRemovedNodes).toBe(1);

  render(SaveControls);
  await fireEvent.click(screen.getByRole('button', { name: 'Discard pending changes' }));
  await fireEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Revert' }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('clear_layout_drafts'));
  // S7 will give placeholders a saved baseline and make these discardable.
  expect(effectiveNodeStore.dirtyBreakdown.unsavedRemovedNodes).toBe(1);
  expect(effectiveNodeStore.dirtyBreakdown.unsavedNewNodes).toBe(1);
});


it('confirmed Discard restores the saved edits before clearing backend drafts through the layout facade', async () => {
  nodeRoster.clearLayoutScope();
  const savedPanel = { facilityId: 'p-1', templateId: 'panel', name: 'Saved panel', slotBindings: { lamps: [] } };
  facilitiesStore.hydrateBaseline([savedPanel]);
  facilitiesStore.renameFacility('p-1', 'Draft panel');
  layoutStore.hydrateOfflineLayout(
    { schemaVersion: '1.0', bowties: {}, roleClassifications: {} },
    { layoutId: 'layout', rootPath: 'layout', mode: 'offline_file', pendingChangeCount: 0, layoutNodeIds: [] },
  );
  layoutStore.markDirty();
  bowtieMetadataStore.deleteBowtie('0102030405060708');
  pendingChangesStore.upsertConfigChange({ nodeId: '020157000001', space: 253, offset: '0x00000000', baselineValue: '0', plannedValue: '1' });

  // Observe the state at the IPC boundary: the local restore must precede it.
  let snapshotAtClear: unknown;
  invoke.mockImplementation(async (command: string) => {
    if (command === 'clear_layout_drafts') {
      snapshotAtClear = {
        pending: pendingChangesStore.draftCount,
        metadata: bowtieMetadataStore.editCount,
        facility: facilitiesStore.facilities[0],
        layoutDirty: layoutStore.isDirty,
      };
    }
  });
  render(SaveControls);
  await fireEvent.click(screen.getByRole('button', { name: 'Discard pending changes' }));
  await fireEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Revert' }));
  await waitFor(() => expect(snapshotAtClear).toEqual({ pending: 0, metadata: 0, facility: savedPanel, layoutDirty: false }));
  expect(typeof (layoutFacade as Record<string, unknown>).discardLayoutEdits).toBe('function');
});


it('Discard awaits pending revert, executes each restore phase in order, and awaits backend clearing', async () => {
  const saved = {
    schemaVersion: '1.0', bowties: {}, roleClassifications: {},
    nodeModeSelections: { '020157000001': { 'connector-a': 'saved' } },
  };
  layoutStore.hydrateOfflineLayout(saved, {
    layoutId: 'sequence', rootPath: 'sequence', mode: 'offline_file', pendingChangeCount: 0, layoutNodeIds: [],
  });
  layoutStore.updateLayout({ ...saved, nodeModeSelections: { '020157000001': { 'connector-a': 'draft' } } });
  const draftLayout = layoutStore.layout;
  let releaseClear!: () => void;
  const clearGate = new Promise<void>((resolve) => { releaseClear = resolve; });
  invoke.mockReset();
  invoke.mockImplementation((command: string) => {
    if (command !== 'clear_layout_drafts') throw new Error(`Unexpected IPC ${command}`);
    return clearGate;
  });

  // Pass-through observers preserve the real store operations and their ordering.
  const phases = [
    ['pending', vi.spyOn(pendingChangesStore, 'revertAllPending')],
    ['config', vi.spyOn(layoutEditParticipants.config, 'discard')],
    ['metadata', vi.spyOn(layoutEditParticipants.metadata, 'discard')],
    ['connectors', vi.spyOn(layoutEditParticipants.connectors, 'discard')],
    ['channels', vi.spyOn(layoutEditParticipants.channels, 'discard')],
    ['facilities', vi.spyOn(layoutEditParticipants.facilities, 'discard')],
    ['layout', vi.spyOn(layoutEditParticipants.layout, 'discard')],
    ['hydrate', vi.spyOn(connectorSelectionsStore, 'hydrateFromLayout')],
    ['clear', invoke],
  ] as const;
  let completed = false;
  let completion: Promise<void> | undefined;
  try {
    completion = discardLayoutEdits();
    void completion.then(() => { completed = true; });
    expect(phases[0][1]).toHaveBeenCalledOnce();
    // The real pending revert returns a promise; no later phase may run synchronously.
    for (const [, observer] of phases.slice(1)) expect(observer).not.toHaveBeenCalled();
    await Promise.resolve();

    for (const [, observer] of phases) expect(observer).toHaveBeenCalledOnce();
    expect(phases.map(([name, observer]) => ({ name, order: observer.mock.invocationCallOrder[0] }))
      .sort((a, b) => a.order - b.order).map(({ name }) => name))
      .toEqual(['pending', 'config', 'metadata', 'connectors', 'channels', 'facilities', 'layout', 'hydrate', 'clear']);
    expect(layoutStore.layout).not.toBe(draftLayout);
    expect(layoutStore.layout).toEqual(saved);
    expect(connectorSelectionsStore.hydrateFromLayout).toHaveBeenCalledWith(layoutStore.layout);
    expect(invoke).toHaveBeenCalledWith('clear_layout_drafts');
    await Promise.resolve();
    expect(completed).toBe(false);
    releaseClear();
    await completion;
    expect(completed).toBe(true);
  } finally {
    releaseClear();
    if (completion) await completion;
    for (const [, observer] of phases.slice(0, -1)) observer.mockRestore();
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
  }
});
