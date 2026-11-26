import { expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import ReorderButtons from './ReorderButtons.svelte';

it('offers accessible icon buttons that move only in enabled directions', async () => {
  const onMove = vi.fn();
  const { rerender } = render(ReorderButtons, {
    props: { name: 'Cedar Hill', canMoveUp: false, canMoveDown: true, onMove },
  });
  const up = screen.getByRole('button', { name: 'Move Cedar Hill up' });
  const down = screen.getByRole('button', { name: 'Move Cedar Hill down' });
  expect(up).toHaveAttribute('title', 'Move Cedar Hill up');
  expect(down).toHaveAttribute('title', 'Move Cedar Hill down');
  expect(up.querySelector('svg[aria-hidden="true"]')).toBeInTheDocument();
  expect(down.querySelector('svg[aria-hidden="true"]')).toBeInTheDocument();
  expect(up).toBeDisabled();
  expect(down).toBeEnabled();
  await fireEvent.click(up);
  await fireEvent.click(down);
  expect(onMove).toHaveBeenCalledTimes(1);
  expect(onMove).toHaveBeenLastCalledWith(1);

  await rerender({ name: 'Cedar Hill', canMoveUp: true, canMoveDown: false, onMove });
  expect(up).toBeEnabled();
  expect(down).toBeDisabled();
  await fireEvent.click(down);
  await fireEvent.click(up);
  expect(onMove).toHaveBeenCalledTimes(2);
  expect(onMove).toHaveBeenLastCalledWith(-1);
  expect(screen.queryByRole('button', { name: /Remove/ })).not.toBeInTheDocument();
});

it('adds a matching remove icon button when asked', async () => {
  const onRemove = vi.fn();
  render(ReorderButtons, {
    props: { name: 'Cedar Hill', canMoveUp: true, canMoveDown: true, onMove: vi.fn(), removeLabel: 'Remove Cedar Hill from panel', onRemove },
  });
  const remove = screen.getByRole('button', { name: 'Remove Cedar Hill from panel' });
  expect(remove).toHaveClass('icon-btn');
  expect(remove.querySelector('svg[aria-hidden="true"]')).toBeInTheDocument();
  await fireEvent.click(remove);
  expect(onRemove).toHaveBeenCalledOnce();
});
