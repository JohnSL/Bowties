import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import type { PanelWiringFacts } from '$lib/api/panels';
import { wiringFacts } from './panelEditorTestSupport';

const panelWiringFactsMock = vi.fn<(nodeKeys?: string[]) => Promise<PanelWiringFacts>>();
vi.mock('$lib/api/panels', () => ({ panelWiringFacts: panelWiringFactsMock }));
vi.mock('$lib/orchestration/facilityOrchestrator', () => ({
  createPanelFacility: vi.fn(),
  syncDraftsForComposition: vi.fn(async () => undefined),
}));

const { default: PanelEditor } = await import('./PanelEditor.svelte');
const { panelEditorStore } = await import('$lib/stores/panelEditor.svelte');
const { panelEditorActions } = await import('$lib/orchestration/panelEditorOrchestrator');

const DETECTED = [
  { blockId: 'block:a', name: 'Main 1', board: 'Eagle Creek BOD', facility: 'Mainline Eastbound' },
  { blockId: 'block:b', name: 'Eagle Creek Siding', board: 'Eagle Creek BOD' },
  { blockId: 'block:c', name: 'Helix Lower', board: 'Helix BOD' },
];

const SOURCES = [
  { id: 'facility', label: 'Signals facility', blocks: [DETECTED[0], DETECTED[1]] },
  { id: 'board', label: 'Helix BOD', blocks: [DETECTED[1], DETECTED[2]] },
];

function openEditor(blockSources = [] as typeof SOURCES) {
  panelEditorActions.startPanelEditor('Fascia');
  render(PanelEditor, { props: { view: panelEditorStore, actions: panelEditorActions, detectedBlocks: DETECTED, blockSources } });
}

const detected = () => screen.getByRole('region', { name: 'Detected blocks' });
const onPanel = () => screen.getByRole('region', { name: 'On this panel' });

describe('Panel editor Blocks step', () => {
  beforeEach(() => {
    panelEditorStore.reset();
    panelWiringFactsMock.mockReset();
    panelWiringFactsMock.mockResolvedValue(wiringFacts());
  });

  it('lists detected blocks grouped by detector board, and filters with search', async () => {
    openEditor();
    expect(within(detected()).getByText('Eagle Creek BOD')).toBeInTheDocument();
    expect(within(detected()).getByText('Helix BOD')).toBeInTheDocument();
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search blocks' }), { target: { value: 'helix' } });
    expect(within(detected()).queryByText('Main 1')).not.toBeInTheDocument();
    expect(within(detected()).getByText('Helix Lower')).toBeInTheDocument();
  });

  it('adds a block once: it shows "on panel", cannot be added again, and × removes it', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Add Main 1' }));

    expect(within(onPanel()).getByText('Main 1')).toBeInTheDocument();
    expect(within(onPanel()).getByText('Eagle Creek BOD · Mainline Eastbound')).toBeInTheDocument();
    expect(within(onPanel()).getByText('1 block')).toBeInTheDocument();
    expect(within(detected()).getByText('on panel')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add Main 1' })).not.toBeInTheDocument();
    expect(screen.queryByText('1 block on this panel')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /1\s*Blocks\s*1 block/ })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Remove Main 1 from panel' }));
    expect(screen.getByText('No blocks on this panel yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add Main 1' })).toBeInTheDocument();
  });

  it('disables move and add-all controls when no sources or other entries exist', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Add Main 1' }));
    expect(screen.getByRole('button', { name: 'Move Main 1 up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Main 1 down' })).toBeDisabled();
    expect(screen.getByLabelText('Add all blocks from')).toBeDisabled();
    expect(within(detected()).getByRole('button', { name: 'Add' })).toBeDisabled();
  });

  it('defaults to the first source, switches to a board, and adds without duplicates', async () => {
    openEditor(SOURCES);
    const source = screen.getByLabelText('Add all blocks from');
    expect(source).toHaveValue('facility');
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add' }));
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:a', 'block:b']);
    await fireEvent.change(source, { target: { value: 'board' } });
    expect(source).toHaveValue('board');
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add' }));
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:a', 'block:b', 'block:c']);
    expect(within(onPanel()).getByText('3 blocks')).toBeInTheDocument();
  });

  it('reorders blocks with arrows and disables each boundary', async () => {
    openEditor(SOURCES);
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add' }));
    expect(screen.getByRole('button', { name: 'Move Main 1 up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Eagle Creek Siding down' })).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Move Eagle Creek Siding up' }));
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:b', 'block:a']);
    expect(screen.getByRole('button', { name: 'Move Eagle Creek Siding up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Main 1 down' })).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Move Eagle Creek Siding down' }));
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:a', 'block:b']);
  });

  it('navigates back to blocks retaining edits, then cancel discards them', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Add Main 1' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Next: Wiring' }));
    expect(panelEditorStore.step).toBe('wiring');
    await fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(within(onPanel()).getByText('Main 1')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(panelEditorStore.open).toBe(false);
    expect(panelEditorStore.entries).toHaveLength(0);
  });

  it('Next goes to Wiring, also with no blocks', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Next: Wiring' }));
    expect(panelEditorStore.step).toBe('wiring');
  });
});
