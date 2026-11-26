import { beforeEach, describe, expect, it, vi } from 'vitest';
import { tick } from 'svelte';
import { createEvent, fireEvent, render, screen, within } from '@testing-library/svelte';
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
const onPanel = () => screen.getByRole('region', { name: 'Blocks on this panel' });

/** jsdom may not build PointerEvent coordinates, so set them on the event. */
function pointerAt(type: 'pointerDown' | 'pointerMove' | 'pointerUp', target: Element, clientY: number) {
  const event = createEvent[type](target);
  Object.defineProperty(event, 'clientY', { value: clientY });
  Object.defineProperty(event, 'button', { value: 0 });
  Object.defineProperty(event, 'pointerId', { value: 1 });
  return fireEvent(target, event);
}

const rowOf = (name: string) => within(onPanel()).getByText(name).closest('tr')!;
describe('Panel editor Blocks step', () => {
  beforeEach(() => {
    panelEditorStore.reset();
    panelWiringFactsMock.mockReset();
    panelWiringFactsMock.mockResolvedValue(wiringFacts());
  });

  it('presents detected blocks before the panel workspace', () => {
    openEditor();
    const panes = screen.getAllByRole('region');
    expect(panes.map((pane) => pane.getAttribute('aria-label'))).toEqual([
      'Detected blocks',
      'Blocks on this panel',
    ]);
    expect(within(panes[1]).getByText('Blocks on this panel')).toBeInTheDocument();
  });

  it('offers clear add actions and empty-panel guidance', () => {
    openEditor(SOURCES);
    expect.soft(within(detected()).queryByRole('button', { name: 'Add all →' })).toBeInTheDocument();
    const add = within(detected()).queryByRole('button', { name: 'Add Main 1 to panel' });
    expect.soft(add).toHaveTextContent('Add →');
    expect.soft(within(onPanel()).queryByText('No blocks on this panel yet')).toBeInTheDocument();
    expect.soft(within(onPanel()).queryByText('Choose a detected block and click Add →. Its position here sets the panel order.')).toBeInTheDocument();
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
    await fireEvent.click(screen.getByRole('button', { name: 'Add Main 1 to panel' }));

    expect(within(onPanel()).getByText('Main 1')).toBeInTheDocument();
    expect(within(onPanel()).getByText('Eagle Creek BOD · Mainline Eastbound')).toBeInTheDocument();
    expect(within(onPanel()).getByText('1 block')).toBeInTheDocument();
    expect(within(detected()).getByText('on panel')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add Main 1 to panel' })).not.toBeInTheDocument();
    expect(screen.queryByText('1 block on this panel')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /1\s*Blocks\s*1 block/ })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Remove Main 1 from panel' }));
    expect(screen.getByText('No blocks on this panel yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add Main 1 to panel' })).toBeInTheDocument();
  });

  it('lists panel blocks in a table like the signals list, with a grip and row actions', async () => {
    openEditor(SOURCES);
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add all →' }));

    const rows = within(onPanel()).getAllByRole('row').slice(1);
    expect(rows).toHaveLength(2);
    for (const [index, name] of ['Main 1', 'Eagle Creek Siding'].entries()) {
      const row = rows[index];
      expect(row.querySelector('[title="Drag to reorder"]')).toBeInTheDocument();
      expect(within(row).getByText(name)).toBeInTheDocument();
      expect(within(row).getByRole('button', { name: `Move ${name} up` })).toBeInTheDocument();
      expect(within(row).getByRole('button', { name: `Remove ${name} from panel` })).toHaveClass('icon-btn');
    }
    expect(within(rows[0]).getByText('Eagle Creek BOD · Mainline Eastbound')).toBeInTheDocument();
  });

  it('highlights the last added block, and a moved block stays highlighted with focus on an arrow', async () => {
    openEditor(SOURCES);
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add all →' }));
    expect(rowOf('Eagle Creek Siding')).toHaveClass('selected');

    await fireEvent.click(within(rowOf('Main 1')).getByText('Main 1'));
    expect(rowOf('Main 1')).toHaveClass('selected');
    expect(rowOf('Eagle Creek Siding')).not.toHaveClass('selected');

    await fireEvent.click(screen.getByRole('button', { name: 'Move Main 1 down' }));
    await tick();
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:b', 'block:a']);
    expect(rowOf('Main 1')).toHaveClass('selected');
    // Main 1 is now last, so its down arrow is disabled and focus goes to its up arrow
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Move Main 1 up' }));
  });

  it('drags a block by its grip to a new position', async () => {
    openEditor(SOURCES);
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add all →' }));
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add Helix Lower to panel' }));
    const rows = within(onPanel()).getAllByRole('row').slice(1);
    rows.forEach((row, i) => {
      row.getBoundingClientRect = () => ({ top: i * 40, height: 40, bottom: i * 40 + 40, left: 0, right: 0, width: 0, x: 0, y: i * 40, toJSON: () => ({}) });
    });
    const grip = rows[0].querySelector('[title="Drag to reorder"]')!;

    await pointerAt('pointerDown', grip, 20);
    await pointerAt('pointerMove', grip, 70);
    expect(rows[2]).toHaveAttribute('data-drop', 'before');
    await pointerAt('pointerUp', grip, 70);

    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:b', 'block:a', 'block:c']);
    expect(rowOf('Main 1')).toHaveClass('selected');
    expect(within(onPanel()).getAllByRole('row').every((row) => !row.hasAttribute('data-drop'))).toBe(true);
  });

  it('a click on the grip without moving does not reorder', async () => {
    openEditor(SOURCES);
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add all →' }));
    const grip = rowOf('Main 1').querySelector('[title="Drag to reorder"]')!;
    await pointerAt('pointerDown', grip, 20);
    await pointerAt('pointerUp', grip, 200);
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:a', 'block:b']);
  });
  it('disables move and add-all controls when no sources or other entries exist', async () => {
    openEditor();
    await fireEvent.click(screen.getByRole('button', { name: 'Add Main 1 to panel' }));
    expect(screen.getByRole('button', { name: 'Move Main 1 up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Main 1 down' })).toBeDisabled();
    expect(screen.getByLabelText('Add all blocks from')).toBeDisabled();
    expect(within(detected()).getByRole('button', { name: 'Add all →' })).toBeDisabled();
  });

  it('defaults to the first source, switches to a board, and adds without duplicates', async () => {
    openEditor(SOURCES);
    const source = screen.getByLabelText('Add all blocks from');
    expect(source).toHaveValue('facility');
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add all →' }));
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:a', 'block:b']);
    await fireEvent.change(source, { target: { value: 'board' } });
    expect(source).toHaveValue('board');
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add all →' }));
    expect(panelEditorStore.entries.map((entry) => entry.blockId)).toEqual(['block:a', 'block:b', 'block:c']);
    expect(within(onPanel()).getByText('3 blocks')).toBeInTheDocument();
  });

  it('reorders blocks with arrows and disables each boundary', async () => {
    openEditor(SOURCES);
    await fireEvent.click(within(detected()).getByRole('button', { name: 'Add all →' }));
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
    await fireEvent.click(screen.getByRole('button', { name: 'Add Main 1 to panel' }));
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
