import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import FacilityRemoveDialog from './FacilityRemoveDialog.svelte';

const absGroups = [{ board: 'Signal LCC 1', items: ['Mast 1', 'Logic 1', 'Track Circuit 1'] }];

function setup(extra: Record<string, unknown> = {}) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  render(FacilityRemoveDialog, {
    props: { facilityName: 'Cedar Hill — Eastward', groups: absGroups, onConfirm, onCancel, ...extra },
  });
  return { onConfirm, onCancel };
}

describe('FacilityRemoveDialog', () => {
  it('names the facility, the board and the slots it resets (ABS list)', () => {
    setup();
    const dialog = screen.getByRole('alertdialog', { name: /Remove Cedar Hill — Eastward\?/ });
    expect(within(dialog).getByText('On Signal LCC 1: resets Mast 1, Logic 1 and Track Circuit 1')).toBeInTheDocument();
    expect(within(dialog).getByText(/hand edits/i)).toBeInTheDocument();
    expect(within(dialog).getByText(/Nothing changes on the board until you Save/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Discard restores everything/)).toBeInTheDocument();
  });

  it('lists a panel claims list per board, including a row with no pin set', () => {
    setup({
      facilityName: 'Eagle Creek fascia',
      groups: [
        { board: 'Tower Signals', items: ['#9 H3-G', 'row 3 (no pin set)'] },
        { board: 'Yard Signals', items: ['#3 H1-R'] },
      ],
    });
    const dialog = screen.getByRole('alertdialog');
    expect(within(dialog).getByText('On Tower Signals: resets #9 H3-G and row 3 (no pin set)')).toBeInTheDocument();
    expect(within(dialog).getByText('On Yard Signals: resets #3 H1-R')).toBeInTheDocument();
  });

  it('Cancel emits cancel and Remove emits confirm without leaving the board', async () => {
    const { onConfirm, onCancel } = setup();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    await fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(onConfirm).toHaveBeenCalledWith(false);
  });

  it('when refused, shows the reason and offers to remove without resetting the board', async () => {
    const { onConfirm } = setup({ reason: 'The Signal LCC settings could not be read.' });
    expect(screen.getByRole('alert')).toHaveTextContent('The Signal LCC settings could not be read.');
    expect(screen.queryByRole('button', { name: 'Remove' })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Remove without resetting the board' }));
    expect(onConfirm).toHaveBeenCalledWith(true);
  });

  it('when the removal failed, shows a short error and still allows Remove again', () => {
    setup({ error: 'Something went wrong.' });
    expect(screen.getByRole('alert')).toHaveTextContent('Something went wrong.');
    expect(screen.getByRole('button', { name: 'Remove' })).toBeInTheDocument();
  });

  it('disables the actions while busy', () => {
    setup({ busy: true });
    expect(screen.getByRole('button', { name: 'Remove' })).toBeDisabled();
  });
});
