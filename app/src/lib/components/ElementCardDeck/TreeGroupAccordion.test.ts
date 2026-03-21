/**
 * Tests for TreeGroupAccordion.svelte
 *
 * Covers:
 * - All groups render children always-visible (no accordion/collapse)
 * - Non-replicated groups render inline with section label
 * - Replicated groups render with PillSelector (label + pill on left)
 * - Pill navigates between instances
 * - Description displays when present
 * - Nested groups and leaves render recursively
 * - groupReplicatedChildren helper groups consecutive siblings
 *
 * Updated: accordion controls removed — labels and pills only.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TreeGroupAccordion from './TreeGroupAccordion.svelte';
import type { GroupConfigNode, LeafConfigNode, ConfigNode } from '$lib/types/nodeTree';
import { groupReplicatedChildren, getInstanceDisplayName } from '$lib/types/nodeTree';
import { pillSelections, setPillSelection } from '$lib/stores/pillSelection';

// Mock stores
vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    nodeSlotMap: new Map(),
    effectiveNodeSlotMap: new Map(),
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
    displayName: null,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  pillSelections.set(new Map());  // reset persisted pill state between tests
});

describe('TreeGroupAccordion.svelte', () => {
  describe('inline (non-replicated) groups — always visible', () => {
    it('renders children immediately visible (no accordion)', () => {
      const group = makeGroup([makeLeaf({ name: 'Inline Field' })], {
        instanceLabel: 'Settings',
        replicationCount: 1,
      });
      render(TreeGroupAccordion, {
        props: { group, nodeId: '02.01.00.00.00.01' },
      });

      // Children should be visible without clicking
      expect(screen.getByText('Inline Field')).toBeInTheDocument();
    });

    it('displays instanceLabel as section label', () => {
      const group = makeGroup([], {
        instanceLabel: 'Advanced',
        replicationCount: 1,
      });
      render(TreeGroupAccordion, {
        props: { group, nodeId: '02.01.00.00.00.01' },
      });

      expect(screen.getByText('Advanced')).toBeInTheDocument();
    });

    it('displays description when present', () => {
      const group = makeGroup([], {
        instanceLabel: 'Event',
        description: 'Producer events for this node',
        replicationCount: 1,
      });
      render(TreeGroupAccordion, {
        props: { group, nodeId: '02.01.00.00.00.01' },
      });

      expect(screen.getByText('Producer events for this node')).toBeInTheDocument();
    });

    it('has no expand/collapse buttons', () => {
      const group = makeGroup([makeLeaf()], {
        instanceLabel: 'Channel 1',
      });
      render(TreeGroupAccordion, {
        props: { group, nodeId: '02.01.00.00.00.01' },
      });

      // No accordion button should exist
      expect(screen.queryByRole('button', { name: /channel 1/i })).not.toBeInTheDocument();
      // But label is visible
      expect(screen.getByText('Channel 1')).toBeInTheDocument();
      // Children always visible
      expect(screen.getByText('Test Field')).toBeInTheDocument();
    });
  });

  describe('pill selector mode', () => {
    function makeSiblings() {
      const instance1 = makeGroup([makeLeaf({ name: 'Field A' })], {
        instance: 1,
        instanceLabel: 'Line 1',
        replicationOf: 'Line',
        replicationCount: 3,
        path: ['seg:0', 'elem:0#1'],
      });
      const instance2 = makeGroup([makeLeaf({ name: 'Field B' })], {
        instance: 2,
        instanceLabel: 'Line 2',
        replicationOf: 'Line',
        replicationCount: 3,
        path: ['seg:0', 'elem:0#2'],
      });
      const instance3 = makeGroup([makeLeaf({ name: 'Field C' })], {
        instance: 3,
        instanceLabel: 'Line 3',
        replicationOf: 'Line',
        replicationCount: 3,
        path: ['seg:0', 'elem:0#3'],
      });
      return [instance1, instance2, instance3];
    }

    it('renders pill selector when siblings provided', () => {
      const [instance1, instance2, instance3] = makeSiblings();

      render(TreeGroupAccordion, {
        props: {
          group: instance1,
          nodeId: '02.01.00.00.00.01',
          siblings: [instance1, instance2, instance3],
        },
      });

      // Should show the template name "Line" as section label
      expect(screen.getByText('Line')).toBeInTheDocument();
      // Should have a pill button with aria-haspopup="listbox"
      const pillBtn = screen.getByRole('button', { name: /line 1/i });
      expect(pillBtn).toHaveAttribute('aria-haspopup', 'listbox');
    });

    it('shows children of selected instance always visible (no accordion)', () => {
      const [instance1, instance2, instance3] = makeSiblings();

      render(TreeGroupAccordion, {
        props: {
          group: instance1,
          nodeId: '02.01.00.00.00.01',
          siblings: [instance1, instance2, instance3],
        },
      });

      // First instance children visible immediately — no expand needed
      expect(screen.getByText('Field A')).toBeInTheDocument();
    });

    it('has no accordion toggle — only the pill button exists', () => {
      const [instance1, instance2, instance3] = makeSiblings();

      render(TreeGroupAccordion, {
        props: {
          group: instance1,
          nodeId: '02.01.00.00.00.01',
          siblings: [instance1, instance2, instance3],
        },
      });

      // Only button should be the pill selector, not an expand/collapse toggle
      const buttons = screen.getAllByRole('button');
      expect(buttons).toHaveLength(1);
      expect(buttons[0]).toHaveAttribute('aria-haspopup', 'listbox');
    });

    it('clicking a pill switches the visible instance content', async () => {
      const [instance1, instance2] = makeSiblings();
      const nodeId = '02.01.00.00.00.01';

      render(TreeGroupAccordion, {
        props: { group: instance1, nodeId, siblings: [instance1, instance2] },
      });

      expect(screen.getByText('Field A')).toBeInTheDocument();

      // Open dropdown then select Line 2
      await fireEvent.click(screen.getByRole('button', { name: /line 1/i }));
      await fireEvent.click(screen.getAllByRole('option')[1]);

      expect(screen.getByText('Field B')).toBeInTheDocument();
      expect(screen.queryByText('Field A')).not.toBeInTheDocument();
    });

    it('clicking a pill persists the selection to the store', async () => {
      const [instance1, instance2, instance3] = makeSiblings();
      const nodeId = '02.01.00.00.00.01';

      render(TreeGroupAccordion, {
        props: { group: instance1, nodeId, siblings: [instance1, instance2, instance3] },
      });

      // Open dropdown then select Line 2 (index 1)
      await fireEvent.click(screen.getByRole('button', { name: /line 1/i }));
      await fireEvent.click(screen.getAllByRole('option')[1]);

      const key = `${nodeId}:${instance1.path.join('/')}`;
      expect(get(pillSelections).get(key)).toBe(1);
    });

    it('restores a persisted selection on mount', () => {
      const [instance1, instance2, instance3] = makeSiblings();
      const nodeId = '02.01.00.00.00.01';

      // Pre-seed the store with index 1 (Line 2) before mounting
      const key = `${nodeId}:${instance1.path.join('/')}`;
      setPillSelection(key, 1);

      render(TreeGroupAccordion, {
        props: { group: instance1, nodeId, siblings: [instance1, instance2, instance3] },
      });

      // Should display instance 2's content and show "Line 2" in the pill
      expect(screen.getByText('Field B')).toBeInTheDocument();
      expect(screen.queryByText('Field A')).not.toBeInTheDocument();
      expect(screen.getByRole('button', { name: /line 2/i })).toBeInTheDocument();
    });
  });

  describe('nested groups — always visible', () => {
    it('renders nested groups and leaves without collapse', () => {
      const innerGroup = makeGroup([makeLeaf({ name: 'Nested Value' })], {
        instanceLabel: 'Channel 1',
        replicationCount: 1,
        path: ['seg:0', 'elem:0', 'elem:0'],
      });
      const outerGroup = makeGroup([innerGroup], {
        instanceLabel: 'Port 1',
        replicationCount: 1,
      });

      render(TreeGroupAccordion, { props: { group: outerGroup, nodeId: '02.01.00.00.00.01' } });

      // Both labels visible
      expect(screen.getByText('Port 1')).toBeInTheDocument();
      expect(screen.getByText('Channel 1')).toBeInTheDocument();
      // Nested leaf visible without any clicks
      expect(screen.getByText('Nested Value')).toBeInTheDocument();
    });
  });
});

describe('groupReplicatedChildren', () => {
  it('passes through leaves unchanged', () => {
    const leaf = makeLeaf();
    const result = groupReplicatedChildren([leaf]);
    expect(result).toEqual([{ type: 'leaf', node: leaf }]);
  });

  it('passes through non-replicated groups unchanged', () => {
    const group = makeGroup([], { replicationCount: 1 });
    const result = groupReplicatedChildren([group]);
    expect(result).toEqual([{ type: 'group', node: group }]);
  });

  it('groups consecutive replicated siblings into replicatedSet', () => {
    const inst1 = makeGroup([], { instance: 1, replicationOf: 'Line', replicationCount: 3, path: ['seg:0', 'elem:0#1'] });
    const inst2 = makeGroup([], { instance: 2, replicationOf: 'Line', replicationCount: 3, path: ['seg:0', 'elem:0#2'] });
    const inst3 = makeGroup([], { instance: 3, replicationOf: 'Line', replicationCount: 3, path: ['seg:0', 'elem:0#3'] });

    const result = groupReplicatedChildren([inst1, inst2, inst3]);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe('replicatedSet');
    if (result[0].type === 'replicatedSet') {
      expect(result[0].instances).toHaveLength(3);
      expect(result[0].templateName).toBe('Line');
    }
  });

  it('does not group non-consecutive replicated groups', () => {
    const inst1 = makeGroup([], { instance: 1, replicationOf: 'Line', replicationCount: 2, path: ['seg:0', 'elem:0#1'] });
    const other = makeGroup([], { replicationCount: 1, replicationOf: 'Other', path: ['seg:0', 'elem:1'] });
    const inst2 = makeGroup([], { instance: 2, replicationOf: 'Line', replicationCount: 2, path: ['seg:0', 'elem:0#2'] });

    const result = groupReplicatedChildren([inst1, other, inst2]);
    expect(result).toHaveLength(3);
  });

  it('preserves ordering of mixed children', () => {
    const leaf1 = makeLeaf({ name: 'First', path: ['seg:0', 'elem:0'] });
    const inst1 = makeGroup([], { instance: 1, replicationOf: 'Ev', replicationCount: 2, path: ['seg:0', 'elem:1#1'] });
    const inst2 = makeGroup([], { instance: 2, replicationOf: 'Ev', replicationCount: 2, path: ['seg:0', 'elem:1#2'] });
    const leaf2 = makeLeaf({ name: 'Last', path: ['seg:0', 'elem:2'] });

    const result = groupReplicatedChildren([leaf1, inst1, inst2, leaf2]);
    expect(result).toHaveLength(3);
    expect(result[0].type).toBe('leaf');
    expect(result[1].type).toBe('replicatedSet');
    expect(result[2].type).toBe('leaf');
  });
});

describe('getInstanceDisplayName', () => {
  it('uses first string leaf value when available', () => {
    const group = makeGroup(
      [makeLeaf({ name: 'Description', elementType: 'string', value: { type: 'string', value: 'CTC Push' } })],
      { instance: 16, instanceLabel: 'Line 16' },
    );
    expect(getInstanceDisplayName(group)).toBe('CTC Push (16)');
  });

  it('falls back to instanceLabel when no string value', () => {
    const group = makeGroup(
      [makeLeaf({ name: 'Count', elementType: 'int', value: { type: 'int', value: 5 } })],
      { instance: 3, instanceLabel: 'Event 3' },
    );
    expect(getInstanceDisplayName(group)).toBe('Event 3');
  });

  it('falls back to instanceLabel when string is empty', () => {
    const group = makeGroup(
      [makeLeaf({ name: 'Description', elementType: 'string', value: { type: 'string', value: '' } })],
      { instance: 1, instanceLabel: 'Line 1' },
    );
    expect(getInstanceDisplayName(group)).toBe('Line 1');
  });
});
