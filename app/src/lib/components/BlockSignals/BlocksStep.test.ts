import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import BlocksStep from './BlocksStep.svelte';
import { fakeView, spyActions } from './editorTestSupport';
import type { SelectedDetectedBlock } from '$lib/api/signaling';

const CEDAR = { blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [{ turnoutId: 't1', name: 'Switch 1' }] };
const MILL = { blockId: 'block:mill', name: 'Mill curve', joinedToPrevious: false, turnouts: [] };
const DEPOT = { blockId: 'block:depot', name: 'Depot', joinedToPrevious: false, turnouts: [] };

function show(extra = {}, detected: SelectedDetectedBlock[] = []) {
  const actions = spyActions();
  const view = fakeView({ blocks: [CEDAR, MILL, DEPOT], ...extra });
  render(BlocksStep, { props: { view, actions, detectedBlocks: detected } });
  return actions;
}

const click = (name: string | RegExp) => fireEvent.click(screen.getByRole('button', { name }));

describe('BlocksStep list and row actions', () => {
  it('lists each block and its turnout in travel order, with the name field and diagram', () => {
    show();
    const rows = within(screen.getByRole('table')).getAllByRole('row').slice(1);
    expect(rows.map((row) => row.querySelector('strong')?.textContent)).toEqual(['Cedar Hill', 'Switch 1', 'Mill curve', 'Depot']);
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('Cedar Hill');
    expect(screen.getByRole('group', { name: /Track diagram derived from the list/ })).toBeInTheDocument();
  });

  it('emits the new facility name as it is typed', async () => {
    const actions = show();
    await fireEvent.input(screen.getByRole('textbox', { name: 'Name' }), { target: { value: 'Cedar Hill East' } });
    expect(actions.setFacilityName).toHaveBeenCalledWith('Cedar Hill East');
  });

  it('moves and removes blocks, disabling up on the first row and down on the last', async () => {
    const actions = show();
    expect(screen.getByRole('button', { name: 'Move Cedar Hill up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Depot down' })).toBeDisabled();
    await click('Move Mill curve up');
    expect(actions.moveBlock).toHaveBeenCalledWith('block:mill', -1);
    await click('Move Mill curve down');
    expect(actions.moveBlock).toHaveBeenCalledWith('block:mill', 1);
    await click('Remove Mill curve from these signals');
    expect(actions.removeBlock).toHaveBeenCalledWith('block:mill');
  });

  it('offers to remove the signal between a block and the one above (join), but not on the first block', async () => {
    const actions = show();
    expect(screen.queryByRole('button', { name: 'More for Cedar Hill' })).not.toBeInTheDocument();
    await click('More for Mill curve');
    await fireEvent.click(screen.getByRole('menuitem', { name: /Remove the signal between Cedar Hill and Mill curve/ }));
    expect(actions.setJoinedToPrevious).toHaveBeenCalledWith('block:mill', true);
  });

  it('for a joined block, says so and offers to add the signal back (split)', async () => {
    const actions = show({ blocks: [CEDAR, { ...MILL, joinedToPrevious: true }, DEPOT] });
    expect(screen.getByText('No signal between it and Cedar Hill')).toBeInTheDocument();
    await click('More for Mill curve');
    await fireEvent.click(screen.getByRole('menuitem', { name: /Add a signal between Cedar Hill and Mill curve/ }));
    expect(actions.setJoinedToPrevious).toHaveBeenCalledWith('block:mill', false);
  });

  it('clicking a ghost off signal in the diagram restores the boundary', async () => {
    const actions = show({ blocks: [CEDAR, { ...MILL, joinedToPrevious: true }, DEPOT] });
    await click(/Signals between .* are off/);
    expect(actions.setJoinedToPrevious).toHaveBeenCalledWith('block:mill', false);
  });
});

describe('BlocksStep turnouts', () => {
  it('moves and removes a turnout, with move buttons following canMoveTurnout', async () => {
    const actions = show({ canMoveTurnout: (_id: string, delta: -1 | 1) => delta === 1 });
    expect(screen.getByRole('button', { name: 'Move Switch 1 up' })).toBeDisabled();
    await click('Move Switch 1 down');
    expect(actions.moveTurnout).toHaveBeenCalledWith('t1', 1);
    await click('Remove Switch 1 from these signals');
    expect(actions.removeTurnout).toHaveBeenCalledWith('t1');
  });

  it('selecting a turnout row shows an editable name that emits renameTurnout', async () => {
    const actions = show();
    expect(screen.queryByRole('textbox', { name: 'Turnout name' })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByText('Switch 1'));
    await fireEvent.change(screen.getByRole('textbox', { name: 'Turnout name' }), { target: { value: 'Yard lead' } });
    expect(actions.renameTurnout).toHaveBeenCalledWith('t1', 'Yard lead');
  });

  it('selecting a turnout in the diagram shows its shape options; choosing one emits setTurnoutShape', async () => {
    const actions = show();
    expect(screen.getByText('Select a turnout to set how it is drawn')).toBeInTheDocument();
    await click(/Switch 1: in Cedar Hill/);
    const shapes = screen.getByRole('group', { name: 'Drawing shape for Switch 1' });
    expect(within(shapes).getAllByRole('radio')).toHaveLength(4);
    await fireEvent.click(within(shapes).getByRole('radio', { name: 'Points at the east end · leg below the main' }));
    expect(actions.setTurnoutShape).toHaveBeenCalledWith('t1', { face: 'east', side: 'below' });
  });

  it('adds a turnout in the first block, then after the new (selected) turnout', async () => {
    const actions = show();
    vi.mocked(actions.addTurnout).mockResolvedValue('t2');
    await click('Add turnout in block');
    expect(actions.addTurnout).toHaveBeenLastCalledWith('block:cedar', undefined);
    await click('Add turnout in block');
    expect(actions.addTurnout).toHaveBeenLastCalledWith('block:cedar', 't2');
  });

  it('adds a turnout into the selected block', async () => {
    const actions = show();
    await fireEvent.click(screen.getAllByText('Depot').find((el) => el.tagName === 'STRONG')!);
    await click('Add turnout in block');
    expect(actions.addTurnout).toHaveBeenCalledWith('block:depot', undefined);
  });
});

describe('BlocksStep end chips', () => {
  it('opens the end menu from the exit chip and emits the chosen exit end', async () => {
    const actions = show();
    await click(/beyond Depot/);
    const dialog = screen.getByRole('dialog', { name: 'What lies beyond Depot' });
    await fireEvent.click(within(dialog).getByRole('radio', { name: /Nothing beyond · treat as Stop/ }));
    expect(actions.setExitEnd).toHaveBeenCalledWith({ kind: 'treatAsStop' });
    expect(within(dialog).getByRole('button', { name: 'Choose a Plant…' })).toBeDisabled();
  });

  it('the entrance chip menu offers only No Plant, checked when it is current', async () => {
    show();
    await click(/before Cedar Hill/);
    const dialog = screen.getByRole('dialog', { name: 'What lies before Cedar Hill' });
    expect(within(dialog).getAllByRole('radio')).toHaveLength(1);
    expect(within(dialog).getByRole('radio', { name: /No Plant/ })).toBeChecked();
  });

  it('choosing No Plant from a Plant entrance emits setEntranceEnd', async () => {
    const actions = show({ entranceEnd: { kind: 'plant', plantId: 'plant:1' } as never });
    await click(/before Cedar Hill/);
    const dialog = screen.getByRole('dialog', { name: 'What lies before Cedar Hill' });
    await fireEvent.click(within(dialog).getByRole('radio', { name: /No Plant/ }));
    expect(actions.setEntranceEnd).toHaveBeenCalledWith({ kind: 'noPlant' });
  });

  it('closes the menu when the same chip is activated again', async () => {
    show();
    await click(/beyond Depot/);
    await click(/beyond Depot/);
    expect(screen.queryByRole('dialog', { name: /What lies beyond/ })).not.toBeInTheDocument();
  });
});

describe('BlocksStep scope and picker', () => {
  it('emits direction choices, with Both directions shown disabled', async () => {
    const actions = show();
    expect(screen.getByRole('radio', { name: /Both directions/ })).toBeDisabled();
    expect(screen.getByRole('radio', { name: /One direction/ })).toBeChecked();
    await fireEvent.click(screen.getByRole('radio', { name: 'Westward' }));
    expect(actions.setDirection).toHaveBeenCalledWith('westward');
  });

  it('shows the coordination choices only for both directions, with APB disabled', () => {
    show({ movements: 'bothDirections' });
    expect(screen.getByRole('radio', { name: /Coordinate traffic through the single-track section/ })).toBeDisabled();
    expect(screen.getByText('List order')).toBeInTheDocument();
  });

  it('shows an empty state with no blocks and cannot add a turnout', () => {
    show({ blocks: [] });
    expect(screen.getByText(/No blocks yet/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add turnout in block' })).toBeDisabled();
  });

  it('disables Add block when every detected block is already listed', () => {
    show({}, [{ blockId: 'block:cedar', name: 'Cedar Hill' } as SelectedDetectedBlock]);
    expect(screen.getByRole('button', { name: 'Add block…' })).toBeDisabled();
  });

  it('adds a chosen detected block through the picker', async () => {
    const block = { blockId: 'block:new', name: 'New siding' } as SelectedDetectedBlock;
    const actions = show({}, [block]);
    await click('Add block…');
    const dialog = screen.getByRole('dialog');
    await fireEvent.click(within(dialog).getByRole('radio', { name: 'New siding' }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Confirm' }));
    expect(actions.addBlock).toHaveBeenCalledWith(block);
  });

  it('shows preview issues for missing detection facts and duplicate blocks', () => {
    show({
      preview: {
        ...fakeView().preview!,
        issues: [
          { kind: 'missingDetectionFacts', blockId: 'block:mill', missingStates: ['occupied', 'clear'] },
          { kind: 'duplicateBlock', blockId: 'block:depot' },
        ] as never,
      },
    });
    expect(screen.getAllByRole('alert').map((a) => a.textContent)).toEqual([
      'Mill curve is missing detected occupied and clear facts.',
      'Depot is listed more than once.',
    ]);
  });
});
