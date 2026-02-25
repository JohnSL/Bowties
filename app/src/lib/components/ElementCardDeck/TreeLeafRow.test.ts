/**
 * Spec 007: Tests for TreeLeafRow.svelte
 *
 * Covers:
 * - Displays leaf name and formatted value
 * - Description toggle shows/hides description
 * - Event role badge displays correctly
 * - "Used in" bowtie cross-reference link
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TreeLeafRow from './TreeLeafRow.svelte';
import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import type { BowtieCard } from '$lib/api/tauri';

// Mock $app/navigation
vi.mock('$app/navigation', () => ({
  goto: vi.fn(),
}));

// Mock bowties store
vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    nodeSlotMap: new Map(),
  },
}));

function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Test Field',
    description: null,
    elementType: 'int',
    address: 0,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0'],
    value: null,
    eventRole: null,
    constraints: null,
    ...overrides,
  };
}

function makeBowtie(overrides: Partial<BowtieCard> = {}): BowtieCard {
  return {
    event_id_hex: '05.02.01.02.03.00.00.01',
    user_names: ['Test Bowtie'],
    producers: [],
    consumers: [],
    ambiguous: [],
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('TreeLeafRow.svelte', () => {
  describe('basic rendering', () => {
    it('displays the leaf name', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ name: 'Speed Limit' }) } });
      expect(screen.getByText('Speed Limit')).toBeInTheDocument();
    });

    it('displays "—" when value is null', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ value: null }) } });
      expect(screen.getByText('—')).toBeInTheDocument();
    });

    it('displays integer value', () => {
      const value: TreeConfigValue = { type: 'int', value: 42 };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ value }) } });
      expect(screen.getByText('42')).toBeInTheDocument();
    });

    it('displays string value', () => {
      const value: TreeConfigValue = { type: 'string', value: 'Hello World' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'string', value }) } });
      expect(screen.getByText('Hello World')).toBeInTheDocument();
    });

    it('displays "(empty)" for empty string value', () => {
      const value: TreeConfigValue = { type: 'string', value: '' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'string', value }) } });
      expect(screen.getByText('(empty)')).toBeInTheDocument();
    });

    it('displays float value with precision', () => {
      const value: TreeConfigValue = { type: 'float', value: 3.14159 };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'float', value }) } });
      expect(screen.getByText('3.1416')).toBeInTheDocument();
    });

    it('displays event ID bytes in hex format', () => {
      const value: TreeConfigValue = { type: 'eventId', bytes: [5, 2, 1, 2, 3, 0, 0, 1] };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'eventId', value }) } });
      expect(screen.getByText('05.02.01.02.03.00.00.01')).toBeInTheDocument();
    });

    it('displays "(free)" for all-zero event ID', () => {
      const value: TreeConfigValue = { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0] };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'eventId', value }) } });
      expect(screen.getByText('(free)')).toBeInTheDocument();
    });
  });

  describe('description toggle', () => {
    it('shows toggle button when description is present', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: 'This is a test description' }) },
      });
      expect(screen.getByRole('button', { name: /toggle/i })).toBeInTheDocument();
    });

    it('does not show toggle button when no description', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ description: null }) } });
      expect(screen.queryByRole('button', { name: /toggle/i })).not.toBeInTheDocument();
    });

    it('shows description after clicking toggle', async () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: 'My detailed description' }) },
      });
      const toggle = screen.getByRole('button', { name: /toggle/i });
      await fireEvent.click(toggle);
      expect(screen.getByText('My detailed description')).toBeInTheDocument();
    });

    it('hides description after clicking toggle twice', async () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: 'Hidden description' }) },
      });
      const toggle = screen.getByRole('button', { name: /toggle/i });
      await fireEvent.click(toggle);
      await fireEvent.click(toggle);
      expect(screen.queryByText('Hidden description')).not.toBeInTheDocument();
    });
  });

  describe('event role display', () => {
    it('displays Producer role badge', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ eventRole: 'Producer' }) },
      });
      expect(screen.getByText('Producer')).toBeInTheDocument();
      expect(screen.getByText(/Role:/)).toBeInTheDocument();
    });

    it('displays Consumer role badge', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ eventRole: 'Consumer' }) },
      });
      expect(screen.getByText('Consumer')).toBeInTheDocument();
    });

    it('does not show role section when eventRole is null', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ eventRole: null }) } });
      expect(screen.queryByText(/Role:/)).not.toBeInTheDocument();
    });
  });

  describe('usedIn cross-reference', () => {
    it('shows "Used in" link when usedIn is provided', () => {
      const bowtie = makeBowtie({ user_names: ['Yard Entry'] });
      render(TreeLeafRow, {
        props: { leaf: makeLeaf(), usedIn: bowtie },
      });
      expect(screen.getByText('Used in:')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /bowtie/i })).toBeInTheDocument();
    });

    it('navigates to bowties page when link clicked', async () => {
      const { goto } = await import('$app/navigation');
      const bowtie = makeBowtie({ event_id_hex: '05.02.01.02.03.00.00.01' });
      render(TreeLeafRow, {
        props: { leaf: makeLeaf(), usedIn: bowtie },
      });
      const link = screen.getByRole('button', { name: /bowtie/i });
      await fireEvent.click(link);
      expect(goto).toHaveBeenCalledWith('/bowties?highlight=05.02.01.02.03.00.00.01');
    });

    it('does not show "Used in" when usedIn is undefined', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf() } });
      expect(screen.queryByText('Used in:')).not.toBeInTheDocument();
    });
  });
});
