/**
 * Spec 007: Tests for TreeGroupAccordion.svelte
 *
 * Covers:
 * - Replicated groups render as collapsible accordions (collapsed by default)
 * - Non-replicated groups render inline (always visible)
 * - Expand/collapse toggles child visibility
 * - Displays instanceLabel correctly
 * - Description displays when present
 * - Nested groups and leaves render recursively
 * - Wrapper groups (instance=0) display group description
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TreeGroupAccordion from './TreeGroupAccordion.svelte';
import type { GroupConfigNode, LeafConfigNode, ConfigNode } from '$lib/types/nodeTree';

// Mock stores
vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    nodeSlotMap: new Map(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: vi.fn(),
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
    path: ['seg:0', 'elem:0', 'elem:0'],
    value: { type: 'int', value: 42 },
    eventRole: null,
    constraints: null,
    ...overrides,
  };
}

function makeGroup(
  children: ConfigNode[],
  overrides: Partial<GroupConfigNode> = {},
): GroupConfigNode {
  return {
    kind: 'group',
    name: 'Test Group',
    description: null,
    instance: 1,
    instanceLabel: 'Test Group 1',
    replicationOf: 'Test Group',
    replicationCount: 3,
    path: ['seg:0', 'elem:0'],
    children,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('TreeGroupAccordion.svelte', () => {
  describe('collapsible (replicated) groups', () => {
    it('renders as collapsed by default', () => {
      const group = makeGroup([makeLeaf()], { instanceLabel: 'Channel 1' });
      render(TreeGroupAccordion, { props: { group, nodeId: '02.01.00.00.00.01' } });

      // Header should be visible
      expect(screen.getByRole('button', { name: /channel 1/i })).toBeInTheDocument();
      // Body should not be visible
      expect(screen.queryByText('Test Field')).not.toBeInTheDocument();
    });

    it('displays instanceLabel in the header', () => {
      const group = makeGroup([], { instanceLabel: 'Event 3' });
      render(TreeGroupAccordion, { props: { group, nodeId: '02.01.00.00.00.01' } });

      expect(screen.getByText('Event 3')).toBeInTheDocument();
    });

    it('expands and shows children when clicked', async () => {
      const group = makeGroup([makeLeaf({ name: 'Speed Value' })], { instanceLabel: 'Port 1' });
      render(TreeGroupAccordion, { props: { group, nodeId: '02.01.00.00.00.01' } });

      const header = screen.getByRole('button', { name: /port 1/i });
      await fireEvent.click(header);

      expect(screen.getByText('Speed Value')).toBeInTheDocument();
    });

    it('collapses when clicked twice', async () => {
      const group = makeGroup([makeLeaf({ name: 'Hidden Field' })], { instanceLabel: 'Line 2' });
      render(TreeGroupAccordion, { props: { group, nodeId: '02.01.00.00.00.01' } });

      const header = screen.getByRole('button', { name: /line 2/i });
      await fireEvent.click(header);
      expect(screen.getByText('Hidden Field')).toBeInTheDocument();

      await fireEvent.click(header);
      expect(screen.queryByText('Hidden Field')).not.toBeInTheDocument();
    });

    it('displays description when expanded', async () => {
      const group = makeGroup([], {
        instanceLabel: 'Event 1',
        description: 'Consumer events for this line',
      });
      render(TreeGroupAccordion, { props: { group, nodeId: '02.01.00.00.00.01' } });

      await fireEvent.click(screen.getByRole('button', { name: /event 1/i }));
      expect(screen.getByText('Consumer events for this line')).toBeInTheDocument();
    });

    it('sets aria-expanded attribute correctly', async () => {
      const group = makeGroup([], { instanceLabel: 'Ch 1' });
      render(TreeGroupAccordion, { props: { group, nodeId: '02.01.00.00.00.01' } });

      const header = screen.getByRole('button', { name: /ch 1/i });
      expect(header).toHaveAttribute('aria-expanded', 'false');

      await fireEvent.click(header);
      expect(header).toHaveAttribute('aria-expanded', 'true');
    });
  });

  describe('inline (non-replicated) groups', () => {
    it('renders children immediately visible (no accordion)', () => {
      const group = makeGroup([makeLeaf({ name: 'Inline Field' })], {
        instanceLabel: 'Settings',
        replicationCount: 1,
      });
      render(TreeGroupAccordion, {
        props: { group, nodeId: '02.01.00.00.00.01', collapsible: false },
      });

      // Children should be visible without clicking
      expect(screen.getByText('Inline Field')).toBeInTheDocument();
      // No expand button
      expect(screen.queryByRole('button')).not.toBeInTheDocument();
    });

    it('displays instanceLabel as section header', () => {
      const group = makeGroup([], {
        instanceLabel: 'Advanced',
        replicationCount: 1,
      });
      render(TreeGroupAccordion, {
        props: { group, nodeId: '02.01.00.00.00.01', collapsible: false },
      });

      expect(screen.getByText('Advanced')).toBeInTheDocument();
    });
  });

  describe('wrapper groups (instance=0)', () => {
    it('wrapper group displays its description', async () => {
      const wrapper = makeGroup([], {
        instance: 0,
        instanceLabel: 'Event',
        description: 'Producer events',
        replicationCount: 6,
      });
      render(TreeGroupAccordion, { props: { group: wrapper, nodeId: '02.01.00.00.00.01' } });

      await fireEvent.click(screen.getByRole('button', { name: /event/i }));
      expect(screen.getByText('Producer events')).toBeInTheDocument();
    });

    it('wrapper group contains nested instance groups', async () => {
      const instance1 = makeGroup([makeLeaf({ name: 'Event ID', path: ['seg:0', 'elem:0', 'inst:1', 'elem:0'] })], {
        instance: 1,
        instanceLabel: 'Event 1',
        replicationCount: 6,
        path: ['seg:0', 'elem:0', 'inst:1'],
      });
      const instance2 = makeGroup([makeLeaf({ name: 'Event ID', path: ['seg:0', 'elem:0', 'inst:2', 'elem:0'] })], {
        instance: 2,
        instanceLabel: 'Event 2',
        replicationCount: 6,
        path: ['seg:0', 'elem:0', 'inst:2'],
      });
      const wrapper = makeGroup([instance1, instance2], {
        instance: 0,
        instanceLabel: 'Event',
        description: 'Consumer events',
        replicationCount: 6,
      });

      render(TreeGroupAccordion, { props: { group: wrapper, nodeId: '02.01.00.00.00.01' } });

      // Expand wrapper
      await fireEvent.click(screen.getByRole('button', { name: /^event$/i }));

      // Instance groups should be visible as nested accordions
      expect(screen.getByRole('button', { name: /event 1/i })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /event 2/i })).toBeInTheDocument();
    });
  });

  describe('nested groups', () => {
    it('renders nested group as accordion inside parent', async () => {
      const innerGroup = makeGroup([makeLeaf({ name: 'Nested Value' })], {
        instanceLabel: 'Channel 1',
        replicationCount: 3,
        path: ['seg:0', 'elem:0', 'elem:0'],
      });
      const outerGroup = makeGroup([innerGroup], {
        instanceLabel: 'Port 1',
        replicationCount: 2,
      });

      render(TreeGroupAccordion, { props: { group: outerGroup, nodeId: '02.01.00.00.00.01' } });

      // Expand outer
      await fireEvent.click(screen.getByRole('button', { name: /port 1/i }));
      // Inner should be visible but collapsed
      expect(screen.getByRole('button', { name: /channel 1/i })).toBeInTheDocument();
      expect(screen.queryByText('Nested Value')).not.toBeInTheDocument();

      // Expand inner
      await fireEvent.click(screen.getByRole('button', { name: /channel 1/i }));
      expect(screen.getByText('Nested Value')).toBeInTheDocument();
    });
  });
});
