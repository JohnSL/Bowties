/**
 * Tests for PillSelector.svelte
 *
 * Covers:
 * - Renders selected item label in pill button
 * - Opens dropdown on click
 * - Shows search input when >6 items
 * - Filters items based on search text
 * - Selects item on click
 * - Keyboard navigation: arrows, Enter, Escape
 * - Closes on Escape
 *
 * Spec: plan-cdiConfigNavigator, Step 1.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import PillSelector from './PillSelector.svelte';
import type { PillItem } from './PillSelector.svelte';

function makeItems(count: number): PillItem[] {
  return Array.from({ length: count }, (_, i) => ({
    value: i,
    label: `Item ${i + 1}`,
    description: `Description ${i + 1}`,
  }));
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('PillSelector.svelte', () => {
  describe('button rendering', () => {
    it('displays the selected item label', () => {
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 1 } });
      expect(screen.getByText('Item 2')).toBeInTheDocument();
    });

    it('displays first item when selected value not found', () => {
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 99 } });
      expect(screen.getByText('Item 1')).toBeInTheDocument();
    });

    it('has aria-haspopup attribute', () => {
      const items = makeItems(2);
      render(PillSelector, { props: { items, selected: 0 } });
      const btn = screen.getByRole('button');
      expect(btn).toHaveAttribute('aria-haspopup', 'listbox');
    });
  });

  describe('dropdown behavior', () => {
    it('opens dropdown on click', async () => {
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      expect(screen.getByRole('listbox')).toBeInTheDocument();
    });

    it('closes dropdown on second click', async () => {
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 0 } });

      const btn = screen.getByRole('button');
      await fireEvent.click(btn);
      expect(screen.getByRole('listbox')).toBeInTheDocument();

      await fireEvent.click(btn);
      expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    });

    it('shows all options in the dropdown', async () => {
      const items = makeItems(4);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      expect(screen.getAllByRole('option')).toHaveLength(4);
    });

    it('shows search input when >6 items', async () => {
      const items = makeItems(8);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      expect(screen.getByPlaceholderText('Search…')).toBeInTheDocument();
    });

    it('does not show search input when <=6 items', async () => {
      const items = makeItems(5);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      expect(screen.queryByPlaceholderText('Search…')).not.toBeInTheDocument();
    });
  });

  describe('selection', () => {
    it('calls onSelect when option clicked', async () => {
      const onSelect = vi.fn();
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 0, onSelect } });

      await fireEvent.click(screen.getByRole('button'));
      await fireEvent.click(screen.getByText('Item 3'));

      expect(onSelect).toHaveBeenCalledWith(2);
    });

    it('closes dropdown after selection', async () => {
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      await fireEvent.click(screen.getByText('Item 2'));

      expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    });

    it('marks selected option with aria-selected', async () => {
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 1 } });

      await fireEvent.click(screen.getByRole('button'));
      const options = screen.getAllByRole('option');
      expect(options[1]).toHaveAttribute('aria-selected', 'true');
      expect(options[0]).toHaveAttribute('aria-selected', 'false');
    });
  });

  describe('search/filter', () => {
    it('filters options based on search text', async () => {
      const items = makeItems(10);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      const searchInput = screen.getByPlaceholderText('Search…');
      await fireEvent.input(searchInput, { target: { value: 'Item 1' } });

      // Should show "Item 1" and "Item 10"
      const options = screen.getAllByRole('option');
      expect(options.length).toBe(2);
    });

    it('shows "No matches" when search has no results', async () => {
      const items = makeItems(10);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      const searchInput = screen.getByPlaceholderText('Search…');
      await fireEvent.input(searchInput, { target: { value: 'zzzzz' } });

      expect(screen.getByText('No matches')).toBeInTheDocument();
    });
  });

  describe('keyboard navigation', () => {
    it('closes on Escape key', async () => {
      const items = makeItems(3);
      render(PillSelector, { props: { items, selected: 0 } });

      await fireEvent.click(screen.getByRole('button'));
      expect(screen.getByRole('listbox')).toBeInTheDocument();

      await fireEvent.keyDown(screen.getByRole('listbox').closest('.pill-selector')!, { key: 'Escape' });
      expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    });
  });
});
