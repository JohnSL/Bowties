/**
 * Contract tests for configChangesStore (configChanges.svelte.ts).
 *
 * These tests encode behavioral guarantees from the edit-layer refactor plan.
 * They must pass both in PR 1 (no production callers yet) and after PR 2
 * when all callers have been switched.
 *
 * Covers:
 * - Layer priority: draft > offlinePending > baseline
 * - changeLayers ordered list
 * - Draft management: set, revert, clearAllDrafts, clearDraftsForNode
 * - Per-node queries: countDraftsForNode, hasDraftsForNode
 * - Edge cases: unknown key, no-op revert, missing tree
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { NodeConfigTree, LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import type { OfflineChangeRow } from '$lib/api/sync';
import { editKeyForLeaf, addressToOffsetHex } from '$lib/utils/editKey';
import { normalizeNodeId } from '$lib/utils/nodeId';

// ─── Mock dependencies ────────────────────────────────────────────────────────

// Mock nodeTreeStore
let mockTreesMap = new Map<string, NodeConfigTree>();

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() {
      return mockTreesMap;
    },
  },
}));

// Mock offlineChangesStore
let mockPersistedRows: OfflineChangeRow[] = [];
let mockDraftRows: OfflineChangeRow[] = [];

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: {
    findPersistedConfigChange: vi.fn(
      (nodeId: string, space: number, offset: string): OfflineChangeRow | null => {
        return (
          mockPersistedRows.find(
            (r) =>
              normalizeNodeId(r.nodeId ?? '') === normalizeNodeId(nodeId) &&
              r.space === space &&
              r.offset === offset &&
              r.status === 'pending',
          ) ?? null
        );
      },
    ),
    findEffectiveConfigChange: vi.fn(
      (nodeId: string, space: number, offset: string): OfflineChangeRow | null => {
        const match = (r: OfflineChangeRow) =>
          normalizeNodeId(r.nodeId ?? '') === normalizeNodeId(nodeId) &&
          r.space === space &&
          r.offset === offset &&
          r.status === 'pending';
        const persisted = mockPersistedRows.find(match) ?? null;
        const draft = mockDraftRows.find(match) ?? null;
        if (draft) {
          if (draft.plannedValue === draft.baselineValue && persisted) return null;
          return draft.plannedValue !== draft.baselineValue ? draft : null;
        }
        return persisted;
      },
    ),
    findDraftConfigChange: vi.fn(
      (nodeId: string, space: number, offset: string): OfflineChangeRow | null => {
        return (
          mockDraftRows.find(
            (r) =>
              normalizeNodeId(r.nodeId ?? '') === normalizeNodeId(nodeId) &&
              r.space === space &&
              r.offset === offset &&
              r.status === 'pending',
          ) ?? null
        );
      },
    ),
  },
}));

// ─── Import store AFTER mocks ────────────────────────────────────────────────

const { configChangesStore } = await import('$lib/stores/configChanges.svelte');

// ─── Helpers ──────────────────────────────────────────────────────────────────

const NODE_ID = '05.02.01.02.03.00';
const NORMALIZED_NODE_ID = '050201020300';
const SPACE = 253;
const ADDRESS = 100;
const KEY = editKeyForLeaf(NODE_ID, SPACE, ADDRESS);
const OFFSET_HEX = addressToOffsetHex(ADDRESS);

function intVal(value: number): TreeConfigValue {
  return { type: 'int', value };
}

function strVal(value: string): TreeConfigValue {
  return { type: 'string', value };
}

function makeLeaf(address: number, value: TreeConfigValue | null): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Test Field',
    description: null,
    elementType: 'int',
    address,
    size: 1,
    space: SPACE,
    path: ['seg:0', 'elem:0'],
    value,
    eventRole: null,
    constraints: null,
  };
}

function makeTree(nodeId: string, leafAddress: number, leafValue: TreeConfigValue | null): NodeConfigTree {
  return {
    nodeId,
    identity: null,
    segments: [
      {
        name: 'Config',
        description: null,
        origin: 0,
        space: SPACE,
        children: [makeLeaf(leafAddress, leafValue)],
      },
    ],
  };
}

function makeOfflineRow(
  overrides: Partial<OfflineChangeRow> = {},
): OfflineChangeRow {
  return {
    changeId: 'row-1',
    kind: 'config',
    nodeId: NODE_ID,
    space: SPACE,
    offset: OFFSET_HEX,
    baselineValue: '1',
    plannedValue: '7',
    status: 'pending',
    ...overrides,
  };
}

beforeEach(() => {
  configChangesStore.clearAllDrafts();
  mockTreesMap = new Map();
  mockPersistedRows = [];
  mockDraftRows = [];
  vi.clearAllMocks();
});

// ─── Layer priority ───────────────────────────────────────────────────────────

describe('layer priority — visibleValue', () => {
  it('returns null when no tree is loaded (unknown key)', () => {
    expect(configChangesStore.visibleValue(KEY)).toBeNull();
  });

  it('returns baseline (leaf.value) when no edits and no offline rows', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(3));
  });

  it('returns offlinePending value when persisted row exists and no draft', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(7));
  });

  it('ignores offlineChanges draft rows when no configChanges draft exists', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockDraftRows = [makeOfflineRow({ plannedValue: '5' })];
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(3));
  });

  it('returns draft value when draft exists and no persisted row', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    configChangesStore.set(KEY, intVal(9));
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(9));
  });

  it('returns draft value (top layer wins) when both draft and persisted row exist', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(9));
  });

  it('falls back to offlinePending after draft is reverted', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    configChangesStore.revert(KEY);
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(7));
  });

  it('falls back to baseline after both draft and persisted row are cleared', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    configChangesStore.revert(KEY);
    mockPersistedRows = [];
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(3));
  });

  it('handles dotted and undotted node IDs for the same tree', () => {
    // Tree is stored with dotted nodeId; key is built from normalized form
    mockTreesMap.set('050201020300', makeTree('050201020300', ADDRESS, intVal(42)));
    const key = editKeyForLeaf('050201020300', SPACE, ADDRESS);
    expect(configChangesStore.visibleValue(key)).toEqual(intVal(42));
  });
});

// ─── changeLayers ─────────────────────────────────────────────────────────────

describe('overrideValue — draft/offlinePending without baseline tree walk', () => {
  it('returns null when no draft and no offline row exist', () => {
    expect(configChangesStore.overrideValue(KEY)).toBeNull();
  });

  it('returns null even when a baseline tree exists (skips baseline)', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    expect(configChangesStore.overrideValue(KEY)).toBeNull();
  });

  it('returns draft value when draft exists', () => {
    configChangesStore.set(KEY, intVal(9));
    expect(configChangesStore.overrideValue(KEY)).toEqual(intVal(9));
  });

  it('returns offlinePending value when persisted row exists and no draft', () => {
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    expect(configChangesStore.overrideValue(KEY)).toEqual(intVal(7));
  });

  it('returns draft value (top layer wins) when both draft and persisted row exist', () => {
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    expect(configChangesStore.overrideValue(KEY)).toEqual(intVal(9));
  });

  it('falls back to offlinePending after draft is reverted', () => {
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    configChangesStore.revert(KEY);
    expect(configChangesStore.overrideValue(KEY)).toEqual(intVal(7));
  });
});

describe('changeLayers', () => {
  it('returns empty array when no tree and no drafts', () => {
    expect(configChangesStore.changeLayers(KEY)).toHaveLength(0);
  });

  it('returns [baseline] when only leaf.value exists', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    const layers = configChangesStore.changeLayers(KEY);
    expect(layers).toHaveLength(1);
    expect(layers[0]).toEqual({ type: 'baseline', value: intVal(3) });
  });

  it('returns [offlinePending, baseline] when persisted row exists, no draft', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    const layers = configChangesStore.changeLayers(KEY);
    expect(layers).toHaveLength(2);
    expect(layers[0]).toEqual({ type: 'offlinePending', value: intVal(7) });
    expect(layers[1]).toEqual({ type: 'baseline', value: intVal(3) });
  });

  it('does not surface offlineChanges draft rows as a display layer', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockDraftRows = [makeOfflineRow({ plannedValue: '7' })];
    const layers = configChangesStore.changeLayers(KEY);
    expect(layers).toEqual([{ type: 'baseline', value: intVal(3) }]);
  });

  it('returns [draft, baseline] when draft exists, no persisted row', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    configChangesStore.set(KEY, intVal(9));
    const layers = configChangesStore.changeLayers(KEY);
    expect(layers).toHaveLength(2);
    expect(layers[0]).toEqual({ type: 'draft', value: intVal(9) });
    expect(layers[1]).toEqual({ type: 'baseline', value: intVal(3) });
  });

  it('returns [draft, offlinePending, baseline] when all three layers exist', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    const layers = configChangesStore.changeLayers(KEY);
    expect(layers).toHaveLength(3);
    expect(layers[0]).toEqual({ type: 'draft', value: intVal(9) });
    expect(layers[1]).toEqual({ type: 'offlinePending', value: intVal(7) });
    expect(layers[2]).toEqual({ type: 'baseline', value: intVal(3) });
  });

  it('annotation "from" value uses next-lower layer — draft over offline pending', () => {
    // "From → To" annotation = layers[1].value → layers[0].value
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    const layers = configChangesStore.changeLayers(KEY);
    // Top layer is draft (9), "from" is offlinePending (7), not baseline (3)
    expect(layers[0].value).toEqual(intVal(9)); // "to"
    expect(layers[1].value).toEqual(intVal(7)); // "from"
  });

  it('drops draft layer after revert, leaving offlinePending as top', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    configChangesStore.revert(KEY);
    const layers = configChangesStore.changeLayers(KEY);
    expect(layers).toHaveLength(2);
    expect(layers[0].type).toBe('offlinePending');
    expect(layers[1].type).toBe('baseline');
  });
});

// ─── Draft management ─────────────────────────────────────────────────────────

describe('set', () => {
  it('creates a draft entry; visibleValue returns it', () => {
    configChangesStore.set(KEY, intVal(5));
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(5));
  });

  it('overwrites an existing draft', () => {
    configChangesStore.set(KEY, intVal(5));
    configChangesStore.set(KEY, intVal(8));
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(8));
  });

  it('works with string values', () => {
    configChangesStore.set(KEY, strVal('Tower East'));
    expect(configChangesStore.visibleValue(KEY)).toEqual(strVal('Tower East'));
  });
});

describe('revert', () => {
  it('removes the draft and returns null when no other layers exist', () => {
    configChangesStore.set(KEY, intVal(5));
    configChangesStore.revert(KEY);
    expect(configChangesStore.visibleValue(KEY)).toBeNull();
  });

  it('is a no-op for a key with no draft', () => {
    expect(() => configChangesStore.revert(KEY)).not.toThrow();
    expect(configChangesStore.visibleValue(KEY)).toBeNull();
  });
});

describe('clearAllDrafts', () => {
  it('removes all draft entries', () => {
    const key2 = editKeyForLeaf(NODE_ID, SPACE, 200);
    configChangesStore.set(KEY, intVal(1));
    configChangesStore.set(key2, intVal(2));
    configChangesStore.clearAllDrafts();
    expect(configChangesStore.visibleValue(KEY)).toBeNull();
    expect(configChangesStore.visibleValue(key2)).toBeNull();
  });

  it('does not affect offline rows (those are owned by offlineChangesStore)', () => {
    mockPersistedRows = [makeOfflineRow({ plannedValue: '7' })];
    configChangesStore.set(KEY, intVal(9));
    configChangesStore.clearAllDrafts();
    // offlinePending layer should still be visible
    expect(configChangesStore.visibleValue(KEY)).toEqual(intVal(7));
  });
});

describe('clearDraftsForNode', () => {
  it('removes only drafts for the specified node', () => {
    const OTHER_NODE = '05.02.01.02.03.01';
    const otherKey = editKeyForLeaf(OTHER_NODE, SPACE, ADDRESS);
    configChangesStore.set(KEY, intVal(1));
    configChangesStore.set(otherKey, intVal(2));
    configChangesStore.clearDraftsForNode(NODE_ID);
    expect(configChangesStore.visibleValue(KEY)).toBeNull();
    expect(configChangesStore.visibleValue(otherKey)).toEqual(intVal(2));
  });

  it('accepts both dotted and undotted node IDs', () => {
    configChangesStore.set(KEY, intVal(1));
    configChangesStore.clearDraftsForNode(NORMALIZED_NODE_ID);
    expect(configChangesStore.visibleValue(KEY)).toBeNull();
  });
});

// ─── Per-node queries ─────────────────────────────────────────────────────────

describe('countDraftsForNode', () => {
  it('returns 0 when no drafts exist', () => {
    expect(configChangesStore.countDraftsForNode(NODE_ID)).toBe(0);
  });

  it('counts drafts for the specified node', () => {
    const key2 = editKeyForLeaf(NODE_ID, SPACE, 200);
    configChangesStore.set(KEY, intVal(1));
    configChangesStore.set(key2, intVal(2));
    expect(configChangesStore.countDraftsForNode(NODE_ID)).toBe(2);
  });

  it('does not count drafts for other nodes', () => {
    const OTHER_NODE = '05.02.01.02.03.01';
    const otherKey = editKeyForLeaf(OTHER_NODE, SPACE, ADDRESS);
    configChangesStore.set(KEY, intVal(1));
    configChangesStore.set(otherKey, intVal(2));
    expect(configChangesStore.countDraftsForNode(NODE_ID)).toBe(1);
    expect(configChangesStore.countDraftsForNode(OTHER_NODE)).toBe(1);
  });

  it('accepts both dotted and undotted node IDs for counting', () => {
    configChangesStore.set(KEY, intVal(1));
    expect(configChangesStore.countDraftsForNode(NORMALIZED_NODE_ID)).toBe(1);
  });
});

describe('hasDraftsForNode', () => {
  it('returns false when no drafts exist', () => {
    expect(configChangesStore.hasDraftsForNode(NODE_ID)).toBe(false);
  });

  it('returns true when at least one draft exists', () => {
    configChangesStore.set(KEY, intVal(1));
    expect(configChangesStore.hasDraftsForNode(NODE_ID)).toBe(true);
  });

  it('returns false after all drafts for the node are cleared', () => {
    configChangesStore.set(KEY, intVal(1));
    configChangesStore.clearDraftsForNode(NODE_ID);
    expect(configChangesStore.hasDraftsForNode(NODE_ID)).toBe(false);
  });

  // Spec 014, ADR-0008 — placeholder NodeKeys must be addressable by all
  // draft-management methods without colliding with live NodeIDs.
  it('isolates drafts by NodeKey — placeholder vs. live node', () => {
    const placeholderKey = 'placeholder:01234567-89ab-cdef-0123-456789abcdef';
    const placeholderEditKey = editKeyForLeaf(placeholderKey, SPACE, ADDRESS);

    configChangesStore.set(KEY, intVal(1));
    configChangesStore.set(placeholderEditKey, intVal(2));

    expect(configChangesStore.hasDraftsForNode(NODE_ID)).toBe(true);
    expect(configChangesStore.hasDraftsForNode(placeholderKey)).toBe(true);
    expect(configChangesStore.countDraftsForNode(NODE_ID)).toBe(1);
    expect(configChangesStore.countDraftsForNode(placeholderKey)).toBe(1);

    configChangesStore.clearDraftsForNode(placeholderKey);
    expect(configChangesStore.hasDraftsForNode(placeholderKey)).toBe(false);
    expect(configChangesStore.hasDraftsForNode(NODE_ID)).toBe(true);
  });
});

describe('hasDraftsUnderPath', () => {
  it('returns false when no drafts exist for the node', () => {
    expect(configChangesStore.hasDraftsUnderPath(NODE_ID, 'seg:0')).toBe(false);
  });

  it('returns true when a draft exists under the given path prefix', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    configChangesStore.set(KEY, intVal(9));
    expect(configChangesStore.hasDraftsUnderPath(NODE_ID, 'seg:0')).toBe(true);
  });

  it('returns false when drafts exist but under a different path prefix', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));
    configChangesStore.set(KEY, intVal(9));
    // The test tree puts the leaf at path ['seg:0', 'elem:0'] — asking for seg:1 should miss
    expect(configChangesStore.hasDraftsUnderPath(NODE_ID, 'seg:1')).toBe(false);
  });

  it('returns false when drafts exist for a different node', () => {
    const OTHER_NODE = '05.02.01.02.03.01';
    const otherKey = editKeyForLeaf(OTHER_NODE, SPACE, ADDRESS);
    configChangesStore.set(otherKey, intVal(9));
    expect(configChangesStore.hasDraftsUnderPath(NODE_ID, 'seg:0')).toBe(false);
  });
});

describe('draft iteration and reconciliation', () => {
  it('returns draft entries with canonical keys and values', () => {
    const key2 = editKeyForLeaf(NODE_ID, SPACE, 200);
    configChangesStore.set(KEY, intVal(1));
    configChangesStore.set(key2, intVal(2));

    expect(configChangesStore.draftEntries()).toEqual([
      { key: KEY, value: intVal(1) },
      { key: key2, value: intVal(2) },
    ]);
  });

  it('prunes only drafts whose refreshed baseline now matches the draft', () => {
    const key2 = editKeyForLeaf(NODE_ID, SPACE, 200);
    configChangesStore.set(KEY, intVal(7));
    configChangesStore.set(key2, intVal(9));
    mockTreesMap.set(
      NODE_ID,
      {
        nodeId: NODE_ID,
        identity: null,
        segments: [
          {
            name: 'Config',
            description: null,
            origin: 0,
            space: SPACE,
            children: [
              makeLeaf(ADDRESS, intVal(7)),
              makeLeaf(200, intVal(3)),
            ],
          },
        ],
      },
    );

    expect(configChangesStore.pruneResolvedDraftsForNode(NODE_ID)).toEqual([KEY]);
    expect(configChangesStore.draftEntries()).toEqual([
      { key: key2, value: intVal(9) },
    ]);
  });

  it('leaves drafts untouched when the refreshed baseline still differs', () => {
    configChangesStore.set(KEY, intVal(7));
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, intVal(3)));

    expect(configChangesStore.pruneResolvedDraftsForNode(NODE_ID)).toEqual([]);
    expect(configChangesStore.draftEntries()).toEqual([
      { key: KEY, value: intVal(7) },
    ]);
  });
});

// ─── Edge cases ───────────────────────────────────────────────────────────────

describe('edge cases', () => {
  it('visibleValue returns null for unknown key with no tree and no drafts', () => {
    expect(configChangesStore.visibleValue('UNKNOWN:253:999')).toBeNull();
  });

  it('changeLayers returns empty array for unknown key', () => {
    expect(configChangesStore.changeLayers('UNKNOWN:253:999')).toEqual([]);
  });

  it('revert on non-existent key is a no-op', () => {
    expect(() => configChangesStore.revert('UNKNOWN:253:999')).not.toThrow();
  });

  it('clearAllDrafts on empty store is a no-op', () => {
    expect(() => configChangesStore.clearAllDrafts()).not.toThrow();
  });

  it('handles leaf with null value (baseline layer absent)', () => {
    mockTreesMap.set(NODE_ID, makeTree(NODE_ID, ADDRESS, null));
    const layers = configChangesStore.changeLayers(KEY);
    // null leaf.value → no baseline layer
    expect(layers).toHaveLength(0);
    expect(configChangesStore.visibleValue(KEY)).toBeNull();
  });
});
