import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import type { PanelWiringFacts } from '$lib/api/panels';
import { wiringFacts } from './panelEditorTestSupport';

const panelWiringFactsMock = vi.fn<(nodeKeys?: string[]) => Promise<PanelWiringFacts>>();
vi.mock('$lib/api/panels', () => ({ panelWiringFacts: panelWiringFactsMock }));
const createPanelFacilityMock = vi.fn();
vi.mock('$lib/orchestration/facilityOrchestrator', () => ({
  createPanelFacility: createPanelFacilityMock,
  syncDraftsForComposition: vi.fn(async () => undefined),
}));

const { default: PanelEditor } = await import('./PanelEditor.svelte');
const { panelEditorStore } = await import('$lib/stores/panelEditor.svelte');
const { panelEditorActions } = await import('$lib/orchestration/panelEditorOrchestrator');

const DETECTED = [
  { blockId: 'block:a', name: 'Main 1' },
  { blockId: 'block:b', name: 'Main 2' },
];

function openEditor(name = 'Eagle Creek fascia') {
  panelEditorActions.startPanelEditor(name);
  return render(PanelEditor, { props: { view: panelEditorStore, actions: panelEditorActions, detectedBlocks: DETECTED } });
}

describe('PanelEditor shell', () => {
  beforeEach(() => {
    panelEditorStore.reset();
    panelWiringFactsMock.mockReset();
    panelWiringFactsMock.mockResolvedValue(wiringFacts());
    createPanelFacilityMock.mockReset();
  });

  it('opens titled with the panel name on an empty Blocks step with guidance', () => {
    openEditor();
    expect(screen.getByRole('dialog', { name: 'Eagle Creek fascia' })).toBeInTheDocument();
    const stepper = screen.getByRole('list', { name: 'Panel steps' });
    expect(within(stepper).getByRole('button', { name: /1\s*Blocks\s*no blocks yet/ })).toHaveAttribute('aria-current', 'step');
    expect(within(stepper).getByRole('button', { name: /2\s*Wiring\s*—/ })).toBeInTheDocument();
    expect(screen.getByText('No blocks on this panel yet')).toBeInTheDocument();
    expect(screen.getByText('Nothing is created until Done on Wiring.')).toBeInTheDocument();
  });

  it('Cancel discards the editor and creates nothing', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(panelEditorStore.open).toBe(false);
    expect(createPanelFacilityMock).not.toHaveBeenCalled();
  });
});
