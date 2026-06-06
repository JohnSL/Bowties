import { describe, it, expect } from 'vitest';
import {
  canonicalizeNodeId,
  computeDiscoveredOnlyNodeIds,
  computeUnsavedInMemoryNodeIds,
  isSavedOffBusNode,
  isUnsavedDiscoveredNode,
} from './nodeRoster';

describe('canonicalizeNodeId', () => {
  it('strips dots and uppercases', () => {
    expect(canonicalizeNodeId('02.01.57.00.00.01')).toBe('020157000001');
    expect(canonicalizeNodeId('020157000001')).toBe('020157000001');
    expect(canonicalizeNodeId('02.01.57.aa.bb.cc')).toBe('020157AABBCC');
  });
});

describe('computeDiscoveredOnlyNodeIds', () => {
  it('returns IDs present in currentNodeIds but absent from savedNodeIds (canonicalized)', () => {
    const saved = ['020157000001'];
    const current = ['02.01.57.00.00.01', '02.01.57.00.00.99'];
    expect(computeDiscoveredOnlyNodeIds(saved, current)).toEqual(['020157000099']);
  });

  it('deduplicates current IDs', () => {
    const result = computeDiscoveredOnlyNodeIds([], [
      '02.01.57.00.00.01',
      '020157000001',
    ]);
    expect(result).toEqual(['020157000001']);
  });

  it('returns [] when savedNodeIds is undefined (pre-S8 contexts treat all as saved)', () => {
    expect(computeDiscoveredOnlyNodeIds(undefined, ['02.01.57.00.00.01'])).toEqual([]);
  });

  it('returns [] when every current node is already in saved', () => {
    expect(
      computeDiscoveredOnlyNodeIds(['020157000001'], ['02.01.57.00.00.01']),
    ).toEqual([]);
  });
});

describe('isUnsavedDiscoveredNode', () => {
  it('true when nodeId not in savedNodeIds', () => {
    expect(isUnsavedDiscoveredNode('02.01.57.00.00.99', ['020157000001'])).toBe(true);
  });
  it('false when savedNodeIds is undefined', () => {
    expect(isUnsavedDiscoveredNode('02.01.57.00.00.99', undefined)).toBe(false);
  });
  it('false when nodeId matches in canonical form', () => {
    expect(isUnsavedDiscoveredNode('02.01.57.00.00.01', ['020157000001'])).toBe(false);
  });
});

describe('isSavedOffBusNode', () => {
  it('true when saved but not in currentNodeIds', () => {
    expect(
      isSavedOffBusNode('020157000001', ['020157000001'], ['02.01.57.00.00.99']),
    ).toBe(true);
  });
  it('false when on the bus', () => {
    expect(
      isSavedOffBusNode('020157000001', ['020157000001'], ['02.01.57.00.00.01']),
    ).toBe(false);
  });
  it('false when not saved at all', () => {
    expect(isSavedOffBusNode('020157000099', ['020157000001'], [])).toBe(false);
  });
});

describe('computeUnsavedInMemoryNodeIds (S8 promotion threshold)', () => {
  it('returns canonicalized IDs that are fully captured but not yet saved', () => {
    const saved = ['020157000001'];
    const fullyCaptured = ['02.01.57.00.00.01', '02.01.57.00.00.99'];
    expect(computeUnsavedInMemoryNodeIds(saved, fullyCaptured)).toEqual([
      '020157000099',
    ]);
  });

  it('excludes nodes that are saved (already in the roster)', () => {
    expect(
      computeUnsavedInMemoryNodeIds(['020157000001'], ['02.01.57.00.00.01']),
    ).toEqual([]);
  });

  it('excludes nodes that are not in the fully-captured set', () => {
    // Discovered node 99 is not fully captured (not in input), so it is not
    // an unsaved in-memory addition even though it is not in the saved
    // roster either. The "new" badge would still appear via the separate
    // `computeDiscoveredOnlyNodeIds` predicate.
    expect(computeUnsavedInMemoryNodeIds([], [])).toEqual([]);
  });

  it('deduplicates fully-captured IDs in any form', () => {
    expect(
      computeUnsavedInMemoryNodeIds([], [
        '02.01.57.00.00.99',
        '020157000099',
      ]),
    ).toEqual(['020157000099']);
  });

  it('returns [] when savedNodeIds is undefined (pre-S8 contexts)', () => {
    expect(
      computeUnsavedInMemoryNodeIds(undefined, ['02.01.57.00.00.99']),
    ).toEqual([]);
  });
});

// ── S8.5 / T9: placeholder-key safety ────────────────────────────────────────

describe('canonicalizeNodeId — placeholder keys', () => {
  it('preserves placeholder NodeKeys case-sensitively (does not uppercase the UUID)', () => {
    const key = 'placeholder:abcd-1234-5678-90ef';
    expect(canonicalizeNodeId(key)).toBe(key);
  });
});

describe('computeDiscoveredOnlyNodeIds — placeholder keys', () => {
  it('does not treat placeholder NodeKeys as discovered-only nodes', () => {
    // Placeholders are handled by the unified save path (inMemorySnapshotKeys).
    // They must NEVER surface in `discoveredOnlyNodeIds` (the sidebar badge
    // is for bus-discovered nodes only).
    const saved: string[] = [];
    const current = ['placeholder:aaaa-bbbb', '02.01.57.00.00.01'];
    expect(computeDiscoveredOnlyNodeIds(saved, current)).toEqual(['020157000001']);
  });
});

describe('computeUnsavedInMemoryNodeIds — placeholder keys', () => {
  it('includes placeholder NodeKeys in the unsaved set (S8.11 unification)', () => {
    const saved: string[] = [];
    const fullyCaptured = ['placeholder:xyz', '02.01.57.00.00.99'];
    expect(computeUnsavedInMemoryNodeIds(saved, fullyCaptured)).toEqual([
      'placeholder:xyz',
      '020157000099',
    ]);
  });
});

