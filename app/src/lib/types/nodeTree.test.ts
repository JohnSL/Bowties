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
  groupReplicatedChildren,
  resolvePillSelectionsForPath,
  buildElementLabel,
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

// ─── resolvePillSelectionsForPath ────────────────────────────────────────────

describe('resolvePillSelectionsForPath', () => {
  const nodeId = 'nodeId';

  // Helper: make a replicated instance group
  function makeInstance(
    outerIdx: number,
    instNum: number,
    innerChildren: ConfigNode[] = [],
  ): GroupConfigNode {
    return makeGroup(innerChildren, {
      name: 'G', instance: instNum, instanceLabel: `G ${instNum}`,
      replicationOf: 'G', replicationCount: 3,
      path: ['seg:0', `elem:${outerIdx}#${instNum}`],
    });
  }

  // Helper: make a wrapper group for a replicated set (instance === 0 in real trees,
  // but the path component is just "elem:N" without a hash)
  function makeWrapper(outerIdx: number, instances: GroupConfigNode[]): GroupConfigNode {
    return makeGroup(instances, {
      name: 'G', instance: 0, instanceLabel: 'G',
      replicationOf: 'G', replicationCount: instances.length,
      path: ['seg:0', `elem:${outerIdx}`],
    });
  }

  it('flat leaf — path with no replicated ancestors returns empty Map', () => {
    const leaf = makeLeaf({ path: ['seg:0', 'elem:2'] });
    const seg = makeSegment([leaf]);
    const result = resolvePillSelectionsForPath(nodeId, seg, ['seg:0', 'elem:2']);
    expect(result.size).toBe(0);
  });

  it('single-level replicated, instance 1 — emits correct 0-based index', () => {
    const inst1 = makeInstance(0, 1);
    const inst2 = makeInstance(0, 2);
    const inst3 = makeInstance(0, 3);
    const wrapper = makeWrapper(0, [inst1, inst2, inst3]);
    const seg = makeSegment([wrapper]);

    const result = resolvePillSelectionsForPath(nodeId, seg, ['seg:0', 'elem:0#1']);
    expect(result.size).toBe(1);
    // pillKey = nodeId:inst1.path.join('/') = "nodeId:seg:0/elem:0#1"
    expect(result.get('nodeId:seg:0/elem:0#1')).toBe(0);
  });

  it('single-level replicated, instance 3 — emits 0-based index 2', () => {
    const inst1 = makeInstance(0, 1);
    const inst2 = makeInstance(0, 2);
    const inst3 = makeInstance(0, 3);
    const wrapper = makeWrapper(0, [inst1, inst2, inst3]);
    const seg = makeSegment([wrapper]);

    const result = resolvePillSelectionsForPath(nodeId, seg, ['seg:0', 'elem:0#3']);
    expect(result.size).toBe(1);
    expect(result.get('nodeId:seg:0/elem:0#1')).toBe(2);
  });

  it('two-level nested replicated groups — emits both pill entries', () => {
    // Inner instances for each outer instance
    function makeInnerInst(outerInst: number, innerInst: number): GroupConfigNode {
      return makeGroup([], {
        name: 'I', instance: innerInst, instanceLabel: `I ${innerInst}`,
        replicationOf: 'I', replicationCount: 4,
        path: ['seg:0', `elem:0#${outerInst}`, `elem:1#${innerInst}`],
      });
    }
    function makeInnerWrapper(outerInst: number): GroupConfigNode {
      const instances = [1, 2, 3, 4].map(i => makeInnerInst(outerInst, i));
      return makeGroup(instances, {
        name: 'I', instance: 0, instanceLabel: 'I',
        replicationOf: 'I', replicationCount: 4,
        path: ['seg:0', `elem:0#${outerInst}`, 'elem:1'],
      });
    }
    const outerInst1 = makeGroup([makeInnerWrapper(1)], {
      name: 'G', instance: 1, instanceLabel: 'G 1', replicationOf: 'G', replicationCount: 3,
      path: ['seg:0', 'elem:0#1'],
    });
    const outerInst2 = makeGroup([makeInnerWrapper(2)], {
      name: 'G', instance: 2, instanceLabel: 'G 2', replicationOf: 'G', replicationCount: 3,
      path: ['seg:0', 'elem:0#2'],
    });
    const outerInst3 = makeGroup([makeInnerWrapper(3)], {
      name: 'G', instance: 3, instanceLabel: 'G 3', replicationOf: 'G', replicationCount: 3,
      path: ['seg:0', 'elem:0#3'],
    });
    const outerWrapper = makeWrapper(0, [outerInst1, outerInst2, outerInst3]);
    const seg = makeSegment([outerWrapper]);

    // Target: outer instance 2, inner instance 3
    const result = resolvePillSelectionsForPath(
      nodeId, seg, ['seg:0', 'elem:0#2', 'elem:1#3', 'elem:2'],
    );
    expect(result.size).toBe(2);
    // Outer pill: inst1.path = ['seg:0', 'elem:0#1'] → key "nodeId:seg:0/elem:0#1", index 1
    expect(result.get('nodeId:seg:0/elem:0#1')).toBe(1);
    // Inner pill: first inner sibling of outerInst2 = makeInnerInst(2, 1)
    //   path = ['seg:0', 'elem:0#2', 'elem:1#1'] → key "nodeId:seg:0/elem:0#2/elem:1#1", index 2
    expect(result.get('nodeId:seg:0/elem:0#2/elem:1#1')).toBe(2);
  });

  it('multi-wrapper siblings (two event sets) remain separate replicated groups', () => {
    // Simulates: consumer events (elem:0) and producer events (elem:1) both named
    // "Event" at the same segment level. They must remain separate wrappers so
    // connector rules can hide one event set without hiding the other.
    function makeEventInst(wrapperIdx: number, instNum: number): GroupConfigNode {
      return makeGroup([], {
        name: 'Event', instance: instNum, instanceLabel: `Event ${instNum}`,
        replicationOf: 'Event', replicationCount: 8,
        path: ['seg:0', `elem:${wrapperIdx}#${instNum}`],
      });
    }
    const consumerInsts = [1, 2, 3, 4, 5, 6, 7, 8].map(i => makeEventInst(0, i));
    const wrapperCons = makeGroup(consumerInsts, {
      name: 'Event', instance: 0, instanceLabel: 'Event',
      replicationOf: 'Event', replicationCount: 8,
      path: ['seg:0', 'elem:0'],
    });
    const producerInsts = [1, 2, 3, 4, 5, 6, 7, 8].map(i => makeEventInst(1, i));
    const wrapperProd = makeGroup(producerInsts, {
      name: 'Event', instance: 0, instanceLabel: 'Event',
      replicationOf: 'Event', replicationCount: 8,
      path: ['seg:0', 'elem:1'],
    });
    const seg = makeSegment([wrapperCons, wrapperProd]);

    // Navigate to producer wrapper (elem:1), instance 5
    const result = resolvePillSelectionsForPath(nodeId, seg, ['seg:0', 'elem:1#5']);
    expect(result.size).toBe(1);
    // Only the producer wrapper's own pill should be selected.
    expect(result.get('nodeId:seg:0/elem:1#1')).toBe(4);
    expect(result.has('nodeId:seg:0/elem:0')).toBe(false);
  });

  it('spacer before target — path-based lookup finds elem:1 wrapper at children[0]', () => {
    // Simulates: CDI elem:0 was a spacer (skipped); CDI elem:1 is the replicated set
    // pushed as children[0] with path ending in "elem:1".
    const inst1 = makeGroup([], {
      name: 'G', instance: 1, instanceLabel: 'G 1', replicationOf: 'G', replicationCount: 2,
      path: ['seg:0', 'elem:1#1'],
    });
    const inst2 = makeGroup([], {
      name: 'G', instance: 2, instanceLabel: 'G 2', replicationOf: 'G', replicationCount: 2,
      path: ['seg:0', 'elem:1#2'],
    });
    const wrapper = makeGroup([inst1, inst2], {
      name: 'G', instance: 0, instanceLabel: 'G', replicationOf: 'G', replicationCount: 2,
      path: ['seg:0', 'elem:1'],  // CDI index 1, but at array index 0
    });
    const seg = makeSegment([wrapper]); // children[0] has path "elem:1" (not "elem:0")

    const result = resolvePillSelectionsForPath(nodeId, seg, ['seg:0', 'elem:1#2']);
    expect(result.size).toBe(1);
    // inst1 is the first sibling: path ['seg:0', 'elem:1#1']
    expect(result.get('nodeId:seg:0/elem:1#1')).toBe(1); // instance 2 → index 1
  });

  it('non-replicated group wrapper — navigates through without emitting a pill', () => {
    const leaf = makeLeaf({ path: ['seg:0', 'elem:0', 'elem:1'] });
    const innerGroup = makeGroup([leaf], {
      name: 'Inner', instance: 1, replicationCount: 1,
      path: ['seg:0', 'elem:0'],
    });
    const seg = makeSegment([innerGroup]);

    const result = resolvePillSelectionsForPath(nodeId, seg, ['seg:0', 'elem:0', 'elem:1']);
    expect(result.size).toBe(0);
  });

  it('out-of-bounds instance index — returns partial Map and stops cleanly without throwing', () => {
    const inst1 = makeInstance(0, 1);
    const inst2 = makeInstance(0, 2);
    const wrapper = makeWrapper(0, [inst1, inst2]); // only 2 instances
    const seg = makeSegment([wrapper]);

    // instNum=5 exceeds wrapper.children.length → selectedInst is undefined → breaks
    expect(() => {
      const result = resolvePillSelectionsForPath(nodeId, seg, ['seg:0', 'elem:0#5', 'elem:1']);
      // Should return an entry for the outer level (since inst1 exists as firstSibling)
      // but stop before navigating deeper
      expect(result.get('nodeId:seg:0/elem:0#1')).toBe(4); // 5-1=4
    }).not.toThrow();
  });
});

describe('groupReplicatedChildren — wrapper group handling', () => {
  it('single wrapper (instance=0, replicationCount>1) is classified as group, not replicatedSet', () => {
    // Rust backend produces: segment.children = [wrapperGroup]
    const wrapper = makeGroup([], {
      name: 'Channel',
      instance: 0,
      instanceLabel: 'Channel',
      replicationOf: 'Channel',
      replicationCount: 3,
      path: ['seg:0', 'elem:0'],
    });

    const result = groupReplicatedChildren([wrapper]);

    // Wrapper alone should be classified as 'group' (not replicatedSet)
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe('group');
  });

  it('wrapper children (instance=1,2,3) are grouped into a replicatedSet', () => {
    // The instances inside the wrapper
    const inst1 = makeGroup([makeLeaf({ name: 'F1' })], {
      instance: 1, instanceLabel: 'Ch 1', replicationOf: 'Channel',
      replicationCount: 3, path: ['seg:0', 'elem:0#1'],
    });
    const inst2 = makeGroup([makeLeaf({ name: 'F2' })], {
      instance: 2, instanceLabel: 'Ch 2', replicationOf: 'Channel',
      replicationCount: 3, path: ['seg:0', 'elem:0#2'],
    });
    const inst3 = makeGroup([makeLeaf({ name: 'F3' })], {
      instance: 3, instanceLabel: 'Ch 3', replicationOf: 'Channel',
      replicationCount: 3, path: ['seg:0', 'elem:0#3'],
    });

    // When called on the wrapper's children, produces a replicatedSet
    const result = groupReplicatedChildren([inst1, inst2, inst3]);

    expect(result).toHaveLength(1);
    expect(result[0].type).toBe('replicatedSet');
    if (result[0].type === 'replicatedSet') {
      expect(result[0].instances).toHaveLength(3);
      expect(result[0].templateName).toBe('Channel');
    }
  });

  it('two wrappers with same replicationOf at different element positions remain separate groups', () => {
    // CDI with consumer events + producer events, both named "Event"
    const wrapper1 = makeGroup([], {
      name: 'Event', instance: 0, instanceLabel: 'Event',
      replicationOf: 'Event', replicationCount: 8, path: ['seg:0', 'elem:0'],
    });
    const wrapper2 = makeGroup([], {
      name: 'Event', instance: 0, instanceLabel: 'Event',
      replicationOf: 'Event', replicationCount: 8, path: ['seg:0', 'elem:1'],
    });

    const result = groupReplicatedChildren([wrapper1, wrapper2]);

    // Two wrappers at different element positions remain separate groups
    // so connector rules can hide one event set without hiding the other
    expect(result).toHaveLength(2);
    expect(result[0].type).toBe('group');
    expect(result[1].type).toBe('group');
  });
});

// ─── buildElementLabel ───────────────────────────────────────────────────────

describe('buildElementLabel', () => {
  it('includes the segment name as the first ancestor', () => {
    const leaf = makeLeaf({ name: 'Event On', address: 100 });
    const group = makeGroup([leaf], {
      name: 'Button B',
      instance: 1,
      instanceLabel: 'Button B 1',
      path: ['seg:0', 'elem:0#1'],
    });
    const seg = makeSegment([group], { name: 'Buttons' });
    const tree = makeTree([seg]);

    const result = buildElementLabel(tree, leaf);
    expect(result).toBe('Buttons.Button B 1.Event On');
  });

  it('omits segment name when it is empty', () => {
    const leaf = makeLeaf({ name: 'Value', address: 50 });
    const seg = makeSegment([leaf], { name: '' });
    const tree = makeTree([seg]);

    const result = buildElementLabel(tree, leaf);
    expect(result).toBe('Value');
  });

  it('works with nested groups and includes segment prefix', () => {
    const leaf = makeLeaf({ name: 'Producer Event ID', address: 200, elementType: 'eventId', size: 8 });
    const innerGroup = makeGroup([leaf], {
      name: 'Events',
      instance: 1,
      instanceLabel: 'Events 1',
      path: ['seg:0', 'elem:0', 'elem:0'],
    });
    const outerGroup = makeGroup([innerGroup], {
      name: 'Channel',
      instance: 1,
      instanceLabel: 'Channel 1',
      path: ['seg:0', 'elem:0'],
    });
    const seg = makeSegment([outerGroup], { name: 'Configuration' });
    const tree = makeTree([seg]);

    const result = buildElementLabel(tree, leaf);
    expect(result).toBe('Configuration.Channel 1.Events 1.Producer Event ID');
  });

  it('uses resolveValue for group instance display names', () => {
    const nameLeaf = makeLeaf({
      name: 'Description',
      elementType: 'string',
      address: 0,
      size: 32,
      value: { type: 'string', value: 'GPIO13' },
    });
    const eventLeaf = makeLeaf({ name: 'Event On', address: 32, elementType: 'eventId', size: 8 });
    const group = makeGroup([nameLeaf, eventLeaf], {
      name: 'Pin',
      instance: 1,
      instanceLabel: 'Pin 1',
      path: ['seg:0', 'elem:0#1'],
    });
    const seg = makeSegment([group], { name: 'I/O Pins' });
    const tree = makeTree([seg]);

    const resolver = (l: LeafConfigNode) => l.value;
    const result = buildElementLabel(tree, eventLeaf, resolver);
    expect(result).toBe('I/O Pins.GPIO13 (1).Event On');
  });
});
