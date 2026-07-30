import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import SingleSelectListHarness from './SingleSelectList.test.harness.svelte';

describe('SingleSelectList', () => {
  it('renders selectable items with radio inputs', () => {
    render(SingleSelectListHarness, {
      props: {
        items: [
          { key: 'a' },
          { key: 'b' },
          { key: 'c' },
        ],
        ariaLabel: 'Test list',
        name: 'test',
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    expect(radios).toHaveLength(3);
  });

  it('shows empty message when no selectable items', () => {
    render(SingleSelectListHarness, {
      props: {
        items: [],
        ariaLabel: 'Test list',
        name: 'test',
        emptyMessage: 'Nothing here',
      },
    });
    expect(screen.getByText('Nothing here')).toBeInTheDocument();
    expect(screen.queryByRole('radiogroup')).not.toBeInTheDocument();
  });

  it('shows default empty message', () => {
    render(SingleSelectListHarness, {
      props: {
        items: [],
        ariaLabel: 'Test list',
        name: 'test',
      },
    });
    expect(screen.getByText('No items available.')).toBeInTheDocument();
  });

  it('renders group headers as non-selectable items', () => {
    render(SingleSelectListHarness, {
      props: {
        items: [
          { key: 'group-1', isHeader: true },
          { key: 'a' },
          { key: 'b' },
        ],
        ariaLabel: 'Test list',
        name: 'test',
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    expect(radios).toHaveLength(2);
    expect(screen.getByTestId('ssl-group-header')).toBeInTheDocument();
  });

  it('selects a row on radio change and calls onSelect', async () => {
    const onSelect = vi.fn<(key: string) => void>();
    render(SingleSelectListHarness, {
      props: {
        items: [{ key: 'a' }, { key: 'b' }],
        ariaLabel: 'Test list',
        name: 'test',
        onSelect,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.change(radios[1]);
    expect(onSelect).toHaveBeenCalledWith('b');
  });

  it('applies selected class to the chosen row', async () => {
    render(SingleSelectListHarness, {
      props: {
        items: [{ key: 'a' }, { key: 'b' }],
        ariaLabel: 'Test list',
        name: 'test',
        selectedKey: 'a',
      },
    });
    const labels = screen.getByRole('radiogroup').querySelectorAll('label');
    expect(labels[0]).toHaveClass('selected');
    expect(labels[1]).not.toHaveClass('selected');
  });

  it('excludes header-only items from empty check', () => {
    render(SingleSelectListHarness, {
      props: {
        items: [{ key: 'group-1', isHeader: true }],
        ariaLabel: 'Test list',
        name: 'test',
      },
    });
    expect(screen.getByText('No items available.')).toBeInTheDocument();
  });
});
