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

function openEditor() {
  panelEditorActions.startPanelEditor('Fascia');
  render(PanelEditor, { props: { view: panelEditorStore, actions: panelEditorActions, detectedBlocks: DETECTED } });
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

  it('keeps move and add-all controls rendered but disabled in this release', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Add Main 1' }));
    expect(screen.getByRole('button', { name: 'Move Main 1 up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Main 1 down' })).toBeDisabled();
    expect(screen.getByLabelText('Add all blocks from')).toBeDisabled();
    expect(within(detected()).getByRole('button', { name: 'Add' })).toBeDisabled();
  });

  it('Next goes to Wiring, also with no blocks', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Next: Wiring' }));
    expect(panelEditorStore.step).toBe('wiring');
  });
});
