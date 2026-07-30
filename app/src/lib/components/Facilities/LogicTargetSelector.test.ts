import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import LogicTargetSelector from './LogicTargetSelector.svelte';

function candidate(nodeKey: string, displayName: string, used = 2, total = 32) {
  return { nodeKey, displayName, capacity: { usedLines: used, totalLines: total, usedTrackCircuits: 0, totalTrackCircuits: 8 } };
}

describe('LogicTargetSelector', () => {
  let onConfirm: ReturnType<typeof vi.fn>;
  let onCancel: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onConfirm = vi.fn();
    onCancel = vi.fn();
  });

  it('renders candidates as a radio list', () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A'), candidate('N2', 'Tower B')],
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    expect(radios).toHaveLength(2);
    expect(screen.getByText('Tower A')).toBeInTheDocument();
    expect(screen.getByText('Tower B')).toBeInTheDocument();
  });

  it('shows capacity info per candidate', () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A', 5, 32)],
        onConfirm,
        onCancel,
      },
    });
    expect(screen.getByText('27/32 lines available')).toBeInTheDocument();
  });

  it('Confirm is disabled until a candidate is selected', async () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A'), candidate('N2', 'Tower B')],
        onConfirm,
        onCancel,
      },
    });
    const confirmBtn = screen.getByRole('button', { name: /confirm/i });
    expect(confirmBtn).toBeDisabled();

    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.change(radios[0]);
    expect(confirmBtn).not.toBeDisabled();
  });

  it('clicking a row does not call onConfirm', async () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A')],
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.change(radios[0]);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('Confirm calls onConfirm with the selected node key', async () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A'), candidate('N2', 'Tower B')],
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.change(radios[1]);

    await fireEvent.click(screen.getByRole('button', { name: /confirm/i }));
    expect(onConfirm).toHaveBeenCalledWith('N2');
  });

  it('Cancel calls onCancel', async () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A')],
        onConfirm,
        onCancel,
      },
    });
    await fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalled();
  });

  it('shows empty message when no candidates', () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [],
        onConfirm,
        onCancel,
      },
    });
    expect(screen.getByText('No candidate nodes available.')).toBeInTheDocument();
    expect(screen.queryByRole('radiogroup')).not.toBeInTheDocument();
  });

  it('seeds selection from selectedNodeKey when valid', () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A'), candidate('N2', 'Tower B')],
        selectedNodeKey: 'N2',
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    expect(radios[1]).toBeChecked();
    expect(screen.getByRole('button', { name: /confirm/i })).not.toBeDisabled();
  });

  it('ignores selectedNodeKey when not in candidates', () => {
    render(LogicTargetSelector, {
      props: {
        candidates: [candidate('N1', 'Tower A')],
        selectedNodeKey: 'N99',
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    expect(radios[0]).not.toBeChecked();
    expect(screen.getByRole('button', { name: /confirm/i })).toBeDisabled();
  });
});
