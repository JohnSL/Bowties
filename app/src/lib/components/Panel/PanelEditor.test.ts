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
    expect(screen.queryByText('Nothing is created until Done on Wiring.')).not.toBeInTheDocument();
  });

  it('keeps the footer to buttons only and shows the Done failure as an alert in the body', async () => {
    openEditor();
    panelEditorStore.setFinishError('Pin 8 is taken.');
    const footer = document.querySelector('.fluent-dialog-footer') as HTMLElement;
    const body = document.querySelector('.fluent-dialog-body') as HTMLElement;
    expect(footer.textContent?.replace(/\s+/g, ' ').trim()).toBe('Cancel Next: Wiring');
    expect(within(footer).queryByRole('alert')).not.toBeInTheDocument();
    expect(await within(body).findByRole('alert')).toHaveTextContent('Pin 8 is taken.');
  });

  it('notes unwired blocks in the body on Wiring only, never on Blocks or in the footer', async () => {
    panelEditorActions.startPanelEditor('Fascia');
    for (const block of DETECTED) panelEditorActions.addBlock(block);
    render(PanelEditor, { props: { view: panelEditorStore, actions: panelEditorActions, detectedBlocks: DETECTED } });
    const note = '2 blocks without a lamp. Done saves the panel as Incomplete.';
    expect(screen.queryByText(note)).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Next: Wiring' }));
    const body = document.querySelector('.fluent-dialog-body') as HTMLElement;
    const footer = document.querySelector('.fluent-dialog-footer') as HTMLElement;
    expect(within(body).getByText(note)).toBeInTheDocument();
    expect(within(footer).queryByText(note)).not.toBeInTheDocument();
  });

  it('Cancel discards the editor and creates nothing', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(panelEditorStore.open).toBe(false);
    expect(createPanelFacilityMock).not.toHaveBeenCalled();
  });
});
