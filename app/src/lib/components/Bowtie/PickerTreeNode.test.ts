/**
 * Tests for PickerTreeNode.svelte module-level helpers (S2c / ADR-0004).
 *
 * Pinned behaviour: role-based visibility consults
 * `effectiveLayoutStore.effectiveRole(nodeId, leaf)` rather than reading
 * `leaf.eventRole` directly. This is what makes pending classifications and
 * catalog-derived roles flow into the picker without a save round-trip
 * (Bug 2 from the save-flow-reorder spec).
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { EventRole, GroupConfigNode, LeafConfigNode } from '$lib/types/nodeTree';

// ── Controllable effectiveRole mock ──────────────────────────────────────────

const effectiveRoleMock = vi.fn<(nodeId: string, leaf: LeafConfigNode) => EventRole | null>();

vi.mock('$lib/layout', () => ({
  effectiveLayoutStore: {
    get preview() { return { bowties: [] }; },
    get effectiveBowties() { return []; },
    effectiveRole: (nodeId: string, leaf: LeafConfigNode) => effectiveRoleMock(nodeId, leaf),
    effectiveValue: () => null,
    slotsByRole: () => [],
    isSlotFree: () => true,
  },
}));

// Import AFTER mocks so the helpers resolve `$lib/layout` to the mock above.
const { hasMatchingDescendant, collapseGroupChain } = await import('./PickerTreeNode.svelte');

// ── Helpers ──────────────────────────────────────────────────────────────────

const NODE_ID = '05.01.01.01.00.00.00.01';

function leaf(name: string, address: number, baselineRole: EventRole | null = null): LeafConfigNode {
  return {
    kind: 'leaf',
    name,
    description: null,
    elementType: 'eventId',
    address,
    size: 8,
    space: 253,
    path: ['seg:0', `elem:${address}`],
    value: { type: 'eventId', bytes: [5, 1, 1, 1, 0, 0, 0, 1], hex: '05.01.01.01.00.00.00.01' },
    eventRole: baselineRole,
    constraints: null,
  };
}

function group(name: string, children: any[], path: string[] = ['grp:0']): GroupConfigNode {
  return {
    kind: 'group',
    name,
    description: null,
    displayName: name, // sidestep getInstanceDisplayName; not relevant for these tests
    address: 0,
    space: 253,
    path,
    children,
  } as GroupConfigNode;
}

beforeEach(() => {
  effectiveRoleMock.mockReset();
});

// ── hasMatchingDescendant ────────────────────────────────────────────────────

describe('hasMatchingDescendant — effectiveRole filtering (S2c)', () => {
  it('hides a leaf whose effective role does not match the role filter, even when leaf.eventRole is null', () => {
    // Baseline says null/unclassified, but a pending classification has set it to Consumer.
    // The Producer picker must hide it.
    const target = leaf('Slot', 100, null);
    effectiveRoleMock.mockImplementation(() => 'Consumer');

    const result = hasMatchingDescendant([target], '', 'Producer', '', undefined, NODE_ID);

    expect(result).toBe(false);
    expect(effectiveRoleMock).toHaveBeenCalledWith(NODE_ID, target);
  });

  it('shows a leaf whose pending classification matches the role filter, overriding baseline eventRole', () => {
    // Baseline says Consumer; pending classify says Producer. Producer picker must show it.
    const target = leaf('Slot', 100, 'Consumer');
    effectiveRoleMock.mockImplementation(() => 'Producer');

    const result = hasMatchingDescendant([target], '', 'Producer', '', undefined, NODE_ID);

    expect(result).toBe(true);
  });

  it('treats effective Ambiguous / null as matching any role filter (consistent with current behaviour)', () => {
    const target = leaf('Slot', 100, 'Producer');
    effectiveRoleMock.mockImplementation(() => null);

    expect(hasMatchingDescendant([target], '', 'Producer', '', undefined, NODE_ID)).toBe(true);
    expect(hasMatchingDescendant([target], '', 'Consumer', '', undefined, NODE_ID)).toBe(true);

    effectiveRoleMock.mockImplementation(() => 'Ambiguous');
    expect(hasMatchingDescendant([target], '', 'Producer', '', undefined, NODE_ID)).toBe(true);
    expect(hasMatchingDescendant([target], '', 'Consumer', '', undefined, NODE_ID)).toBe(true);
  });

  it('recurses into groups and consults effectiveRole on each descendant leaf', () => {
    const inner = leaf('Inner', 100, null);
    const tree = group('Outer', [inner]);
    effectiveRoleMock.mockImplementation(() => 'Consumer');

    expect(hasMatchingDescendant([tree], '', 'Producer', '', undefined, NODE_ID)).toBe(false);
    expect(hasMatchingDescendant([tree], '', 'Consumer', '', undefined, NODE_ID)).toBe(true);
    expect(effectiveRoleMock).toHaveBeenCalledWith(NODE_ID, inner);
  });
});

// ── collapseGroupChain ───────────────────────────────────────────────────────

describe('collapseGroupChain — effectiveRole filtering (S2c)', () => {
  it('does not collapse to a leaf whose effective role is filtered out', () => {
    // Group with two leaves: one Producer, one Consumer (effective).
    // Producer filter should leave one visible leaf → collapse to that leaf.
    const producerLeaf = leaf('PLeaf', 100, null);
    const consumerLeaf = leaf('CLeaf', 200, null);
    effectiveRoleMock.mockImplementation((_nid, l) =>
      l === producerLeaf ? 'Producer' : 'Consumer',
    );

    const tree = group('Wrap', [producerLeaf, consumerLeaf]);
    const result = collapseGroupChain(tree, '', 'Producer', '', undefined, NODE_ID);

    // Should collapse to the producer leaf
    expect(result.terminal).toBe(producerLeaf);
    expect(result.combinedLabel).toContain('PLeaf');
  });
});
