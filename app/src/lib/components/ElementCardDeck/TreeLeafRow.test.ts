/**
 * Spec 007: Tests for TreeLeafRow.svelte
 *
 * Covers:
 * - Displays leaf name and formatted value (horizontal layout)
 * - Inline descriptions visible by default, truncation toggle for long text
 * - Enum values mapped to labels via mapEntries
 * - Event IDs: monospace dotted hex, "(not set)" for all-zeros
 * - Event role badge displays correctly
 * - "Used in" bowtie cross-reference link
 *
 * Updated for plan-cdiConfigNavigator Steps 4-6.
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
      const value: TreeConfigValue = { type: 'eventId', bytes: [5, 2, 1, 2, 3, 0, 0, 1], hex: '05.02.01.02.03.00.00.01' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'eventId', value }) } });
      expect(screen.getByText('05.02.01.02.03.00.00.01')).toBeInTheDocument();
    });

    it('displays "(not set)" for all-zero event ID', () => {
      const value: TreeConfigValue = { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0], hex: '00.00.00.00.00.00.00.00' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'eventId', value }) } });
      expect(screen.getByText('(not set)')).toBeInTheDocument();
    });
  });

  describe('enum value mapping', () => {
    it('maps int value to enum label when mapEntries exist', () => {
      const value: TreeConfigValue = { type: 'int', value: 2 };
      const leaf = makeLeaf({
        value,
        constraints: {
          min: 0,
          max: 3,
          defaultValue: null,
          mapEntries: [
            { value: 0, label: 'Off' },
            { value: 1, label: 'On' },
            { value: 2, label: 'Toggle' },
            { value: 3, label: 'Hold' },
          ],
        },
      });
      render(TreeLeafRow, { props: { leaf } });
      expect(screen.getByText('Toggle')).toBeInTheDocument();
    });

    it('falls back to raw number when no matching mapEntry', () => {
      const value: TreeConfigValue = { type: 'int', value: 99 };
      const leaf = makeLeaf({
        value,
        constraints: {
          min: 0,
          max: 3,
          defaultValue: null,
          mapEntries: [
            { value: 0, label: 'Off' },
            { value: 1, label: 'On' },
          ],
        },
      });
      render(TreeLeafRow, { props: { leaf } });
      expect(screen.getByText('99')).toBeInTheDocument();
    });
  });

  describe('inline descriptions', () => {
    it('shows short description inline by default (no toggle needed)', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: 'Short description' }) },
      });
      expect(screen.getByText('Short description')).toBeInTheDocument();
    });

    it('truncates long descriptions with expand button', () => {
      const longDesc = 'A'.repeat(130); // > 120 char threshold
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: longDesc }) },
      });
      // Should show truncated text (100 chars + "…")
      expect(screen.getByText(/^A{100}…$/)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /expand/i })).toBeInTheDocument();
    });

    it('expands truncated description when [+] clicked', async () => {
      const longDesc = 'B'.repeat(130);
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: longDesc }) },
      });
      const expandBtn = screen.getByRole('button', { name: /expand/i });
      await fireEvent.click(expandBtn);
      expect(screen.getByText(longDesc)).toBeInTheDocument();
    });

    it('does not show description when null', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ description: null }) } });
      // Only name + value, nothing else
      expect(screen.queryByText(/…/)).not.toBeInTheDocument();
    });
  });

  describe('event role display', () => {
    it('displays Producer role badge', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ eventRole: 'Producer' }) },
      });
      expect(screen.getByText('Producer')).toBeInTheDocument();
    });

    it('displays Consumer role badge', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ eventRole: 'Consumer' }) },
      });
      expect(screen.getByText('Consumer')).toBeInTheDocument();
    });

    it('does not show role section when eventRole is null', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ eventRole: null }) } });
      expect(screen.queryByText('Producer')).not.toBeInTheDocument();
      expect(screen.queryByText('Consumer')).not.toBeInTheDocument();
    });
  });

  describe('usedIn cross-reference', () => {
    it('shows navigable link when usedIn is provided', () => {
      const bowtie = makeBowtie({ user_names: ['Yard Entry'] });
      render(TreeLeafRow, {
        props: { leaf: makeLeaf(), usedIn: bowtie },
      });
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

    it('does not show link when usedIn is undefined', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf() } });
      expect(screen.queryByRole('button', { name: /bowtie/i })).not.toBeInTheDocument();
    });
  });
});
