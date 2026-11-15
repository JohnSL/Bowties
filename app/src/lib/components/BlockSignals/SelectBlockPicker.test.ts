import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import SelectBlockPicker from './SelectBlockPicker.svelte';

describe('SelectBlockPicker', () => {
  it('renders candidates alphabetically by display name without mutating their supplied order', () => {
    const candidateBlocks = [
      { blockId: 'zulu', name: 'zulu' },
      { blockId: 'alpha', name: 'Alpha' },
      { blockId: 'bravo', name: 'bravo' },
    ];
    const suppliedReferences = [...candidateBlocks];

    render(SelectBlockPicker, {
      props: { candidateBlocks, onConfirm: vi.fn(), onCancel: vi.fn() },
    });

    expect(screen.getAllByRole('radio').map((radio) => radio.getAttribute('value'))).toEqual([
      'alpha',
      'bravo',
      'zulu',
    ]);
    expect(candidateBlocks).toEqual([
      { blockId: 'zulu', name: 'zulu' },
      { blockId: 'alpha', name: 'Alpha' },
      { blockId: 'bravo', name: 'bravo' },
    ]);
    candidateBlocks.forEach((block, index) => {
      expect(block).toBe(suppliedReferences[index]);
    });
  });

  it('filters a long block list, clears a hidden selection, and cancels without emitting a block', async () => {
    const onConfirm = vi.fn<(blockId: string) => void>();
    const onCancel = vi.fn<() => void>();
    const candidateBlocks = Array.from({ length: 40 }, (_, index) => ({
      blockId: `block-${index + 1}`,
      name: `Platform ${String(index + 1).padStart(2, '0')}`,
    }));

    render(SelectBlockPicker, {
      props: { candidateBlocks, onConfirm, onCancel },
    });

    const dialog = screen.getByRole('dialog', { name: 'Add block' });
    const list = within(dialog).getByRole('radiogroup', { name: 'Detected blocks' });
    expect(within(list).getAllByRole('radio')).toHaveLength(40);
    expect(getComputedStyle(list).maxHeight).not.toBe('none');

    const search = within(dialog).getByRole('searchbox', { name: 'Filter blocks' });
    await fireEvent.input(search, { target: { value: 'Platform 40' } });
    await fireEvent.click(within(list).getByRole('radio', { name: 'Platform 40' }));
    expect(within(dialog).getByRole('button', { name: 'Confirm' })).toBeEnabled();

    await fireEvent.input(search, { target: { value: 'Platform 01' } });
    expect(within(list).getByRole('radio', { name: 'Platform 01' })).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Confirm' })).toBeDisabled();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
