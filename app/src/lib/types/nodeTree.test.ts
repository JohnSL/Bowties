/**
 * Spec 007: Tests for the NodeConfigTree TypeScript types and tree helpers.
 */

import { describe, it, expect } from 'vitest';
import {
  isGroup,
  isLeaf,
  getChildrenAtPath,
  findLeafByAddress,
  countLeaves,
  collectEventIdLeaves,
} from '$lib/types/nodeTree';
import type {
  NodeConfigTree,
  SegmentNode,
  GroupConfigNode,
  LeafConfigNode,
  ConfigNode,
} from '$lib/types/nodeTree';

// ─── Fixtures ────────────────────────────────────────────────────────────────

/** A minimal leaf node */
function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Test Int',
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

/** A minimal group node */
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
    replicationCount: 1,
    path: ['seg:0', 'elem:0'],
    children,
    ...overrides,
  };
}

/** A minimal segment */
function makeSegment(
  children: ConfigNode[],
  overrides: Partial<SegmentNode> = {},
): SegmentNode {
  return {
    name: 'Configuration',
    description: null,
    origin: 0,
    space: 253,
    children,
    ...overrides,
  };
}

/** A minimal tree */
function makeTree(
  segments: SegmentNode[],
  nodeId = '05.02.01.02.03.00',
): NodeConfigTree {
  return { nodeId, identity: null, segments };
}

// ─── Type guards ─────────────────────────────────────────────────────────────

describe('type guards', () => {
  it('isGroup returns true for group nodes', () => {
    const node = makeGroup([]);
    expect(isGroup(node)).toBe(true);
    expect(isLeaf(node)).toBe(false);
  });

  it('isLeaf returns true for leaf nodes', () => {
    const node = makeLeaf();
    expect(isLeaf(node)).toBe(true);
    expect(isGroup(node)).toBe(false);
  });
});

// ─── getChildrenAtPath ───────────────────────────────────────────────────────

describe('getChildrenAtPath', () => {
  const leaf1 = makeLeaf({ name: 'Leaf A', path: ['seg:0', 'elem:0'] });
  const leaf2 = makeLeaf({ name: 'Leaf B', path: ['seg:0', 'elem:1', 'elem:0'] });
  const innerGroup = makeGroup([leaf2], {
    name: 'Inner',
    path: ['seg:0', 'elem:1'],
  });
  const tree = makeTree([makeSegment([leaf1, innerGroup])]);

  it('returns segment children for ["seg:0"]', () => {
    const children = getChildrenAtPath(tree, ['seg:0']);
    expect(children).toHaveLength(2);
    expect(children![0]).toBe(leaf1);
    expect(children![1]).toBe(innerGroup);
  });

  it('returns group children for deeper path', () => {
    const children = getChildrenAtPath(tree, ['seg:0', 'elem:1']);
    expect(children).toHaveLength(1);
    expect(children![0]).toBe(leaf2);
  });

  it('returns null for empty path', () => {
    expect(getChildrenAtPath(tree, [])).toBeNull();
  });

  it('returns null for invalid segment index', () => {
    expect(getChildrenAtPath(tree, ['seg:99'])).toBeNull();
  });

  it('returns null for non-matching group path', () => {
    expect(getChildrenAtPath(tree, ['seg:0', 'elem:42'])).toBeNull();
  });
});

// ─── findLeafByAddress ───────────────────────────────────────────────────────

describe('findLeafByAddress', () => {
  const leaf1 = makeLeaf({ name: 'Addr 100', address: 100 });
  const leaf2 = makeLeaf({ name: 'Addr 200', address: 200, elementType: 'eventId', size: 8 });
  const group = makeGroup([leaf2], { path: ['seg:0', 'elem:1'] });
  const tree = makeTree([makeSegment([leaf1, group])]);

  it('finds a top-level leaf', () => {
    const found = findLeafByAddress(tree, 100);
    expect(found).toBe(leaf1);
  });

  it('finds a nested leaf', () => {
    const found = findLeafByAddress(tree, 200);
    expect(found).toBe(leaf2);
  });

  it('returns null for missing address', () => {
    expect(findLeafByAddress(tree, 999)).toBeNull();
  });
});

// ─── countLeaves ─────────────────────────────────────────────────────────────

describe('countLeaves', () => {
  it('counts flat leaves', () => {
    const tree = makeTree([
      makeSegment([makeLeaf(), makeLeaf({ address: 1 })]),
    ]);
    expect(countLeaves(tree)).toBe(2);
  });

  it('counts nested leaves', () => {
    const tree = makeTree([
      makeSegment([
        makeLeaf(),
        makeGroup([makeLeaf({ address: 1 }), makeLeaf({ address: 2 })]),
      ]),
    ]);
    expect(countLeaves(tree)).toBe(3);
  });

  it('returns 0 for empty tree', () => {
    const tree = makeTree([]);
    expect(countLeaves(tree)).toBe(0);
  });
});

// ─── collectEventIdLeaves ────────────────────────────────────────────────────

describe('collectEventIdLeaves', () => {
  it('collects only eventId leaves', () => {
    const intLeaf = makeLeaf({ elementType: 'int', address: 0 });
    const eidLeaf1 = makeLeaf({ elementType: 'eventId', address: 8, size: 8 });
    const eidLeaf2 = makeLeaf({ elementType: 'eventId', address: 16, size: 8 });
    const group = makeGroup([eidLeaf2]);
    const tree = makeTree([makeSegment([intLeaf, eidLeaf1, group])]);

    const result = collectEventIdLeaves(tree);
    expect(result).toHaveLength(2);
    expect(result[0]).toBe(eidLeaf1);
    expect(result[1]).toBe(eidLeaf2);
  });

  it('returns empty for tree with no eventId leaves', () => {
    const tree = makeTree([makeSegment([makeLeaf({ elementType: 'int' })])]);
    expect(collectEventIdLeaves(tree)).toHaveLength(0);
  });
});

// ─── Integration: Tower-LCC dual Event group shape ───────────────────────────

describe('Tower-LCC dual Event group disambiguation', () => {
  it('two sibling groups with same name remain distinct at segment level', () => {
    // Simulates the serialized tree that the backend would produce for
    // two CDI <group replication="6"><name>Event</name> siblings.
    const consumerGroup = makeGroup(
      [makeLeaf({ elementType: 'eventId', address: 100, size: 8 })],
      {
        name: 'Event',
        instance: 1,
        instanceLabel: 'Event 1',
        replicationOf: 'Event',
        replicationCount: 6,
        path: ['seg:0', 'elem:0#1'],
      },
    );
    const producerGroup = makeGroup(
      [makeLeaf({ elementType: 'eventId', address: 200, size: 8 })],
      {
        name: 'Event',
        instance: 1,
        instanceLabel: 'Event 1',
        replicationOf: 'Event',
        replicationCount: 6,
        path: ['seg:0', 'elem:1#1'],
      },
    );

    const tree = makeTree([makeSegment([consumerGroup, producerGroup])]);

    // Both groups survive as distinct children — this is the bug fix:
    // the old code would flatten these into 12 identical "Event" items.
    const segChildren = getChildrenAtPath(tree, ['seg:0'])!;
    expect(segChildren).toHaveLength(2);
    expect(isGroup(segChildren[0])).toBe(true);
    expect(isGroup(segChildren[1])).toBe(true);

    // Paths differ — disambiguated by element index
    const g0 = segChildren[0] as GroupConfigNode;
    const g1 = segChildren[1] as GroupConfigNode;
    expect(g0.path).not.toEqual(g1.path);
    // elem:0#1 vs elem:1#1
    expect(g0.path[g0.path.length - 1]).toBe('elem:0#1');
    expect(g1.path[g1.path.length - 1]).toBe('elem:1#1');
  });
});
