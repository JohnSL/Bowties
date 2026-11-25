import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import BlockSignalsRemoveDialog from './BlockSignalsRemoveDialog.svelte';

const slots = [
  { kind: 'logic' as const, index: 1 },
  { kind: 'mast' as const, index: 1 },
];

function setup(extra: Record<string, unknown> = {}) {
  const onConfirm = vi.fn();
  const onConfirmLeaveBoard = vi.fn();
  const onCancel = vi.fn();
  render(BlockSignalsRemoveDialog, {
    props: {
      facilityName: 'Cedar Hill — Eastward',
      boards: [{ name: 'Signal LCC 1', slots }],
      onConfirm,
      onConfirmLeaveBoard,
      onCancel,
      ...extra,
    },
  });
  return { onConfirm, onConfirmLeaveBoard, onCancel };
}

describe('BlockSignalsRemoveDialog', () => {
  it('names the facility, the board and the slots it resets', () => {
    setup();
    const dialog = screen.getByRole('alertdialog', { name: /Remove Cedar Hill — Eastward\?/ });
    expect(within(dialog).getByText(/Signal LCC 1/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Mast 1 and Logic 1/)).toBeInTheDocument();
    expect(within(dialog).getByText(/hand edits/i)).toBeInTheDocument();
    expect(within(dialog).getByText(/Discard restores everything/)).toBeInTheDocument();
  });

  it('lists Track Circuits after the masts and Logic', () => {
    setup({ boards: [{ name: 'Signal LCC 1', slots: [...slots, { kind: 'trackCircuit' as const, index: 1 }] }] });
    const dialog = screen.getByRole('alertdialog');
    expect(within(dialog).getByText(/Mast 1, Logic 1 and Track Circuit 1/)).toBeInTheDocument();
  });

  it('names each board with the slots it resets when the signals span two boards', () => {
    setup({
      boards: [
        { name: 'West Signal LCC', slots: [...slots, { kind: 'logic' as const, index: 2 }, { kind: 'trackCircuit' as const, index: 1 }] },
        { name: 'East Signal LCC', slots },
      ],
    });
    const dialog = screen.getByRole('alertdialog');
    expect(within(dialog).getByText(
      /resets Mast 1, Logic 1, Logic 2 and Track Circuit 1 on West Signal LCC, and Mast 1 and Logic 1 on East Signal LCC so/,
    )).toBeInTheDocument();
  });

  it('Cancel emits cancel and Remove emits confirm', async () => {
    const { onConfirm, onCancel } = setup();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    await fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it('when refused, shows the reason and offers to remove without resetting the board', async () => {
    const { onConfirm, onConfirmLeaveBoard } = setup({ reason: 'The Signal LCC settings could not be read.' });
    expect(screen.getByRole('alert')).toHaveTextContent('The Signal LCC settings could not be read.');
    expect(screen.queryByRole('button', { name: 'Remove' })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Remove without resetting the board' }));
    expect(onConfirmLeaveBoard).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('when the removal failed, shows a short error and still allows Remove again', () => {
    setup({ error: 'Something went wrong.' });
    expect(screen.getByRole('alert')).toHaveTextContent('Something went wrong.');
    expect(screen.getByRole('button', { name: 'Remove' })).toBeInTheDocument();
  });
});
