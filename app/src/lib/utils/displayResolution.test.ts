/**
 * Tests for unified display resolution (ADR-0003).
 *
 * Verifies the resolution waterfall priority for both values and roles:
 *   resolveValue: draft → offline pending → baseline
 *   resolveRole:  pending edit → saved layout → catalog → CDI baseline
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { LeafConfigNode, TreeConfigValue, EventRole } from '$lib/types/nodeTree';
import type { RoleClassification } from '$lib/types/bowtie';

// ─── Mock dependencies ────────────────────────────────────────────────────────

let mockOverride: TreeConfigValue | null = null;
vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    overrideValue: (_key: string): TreeConfigValue | null => mockOverride,
  },
}));

let mockRoleClassification: RoleClassification | undefined = undefined;
vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: {
    getRoleClassification: (_key: string): RoleClassification | undefined =>
      mockRoleClassification,
  },
}));

let mockCatalogRole: 'Producer' | 'Consumer' | null = null;
vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    getRoleForSlot: (
      _nodeId: string,
      _path: string[],
    ): 'Producer' | 'Consumer' | null => mockCatalogRole,
  },
}));

// Import after mocks
const { resolveValue, resolveRole, makeValueResolver } = await import(
  '$lib/utils/displayResolution'
);

// ─── Helpers ──────────────────────────────────────────────────────────────────

const NODE_ID = '05.02.01.02.03.00';

function intVal(value: number): TreeConfigValue {
  return { type: 'int', value };
}

function strVal(value: string): TreeConfigValue {
  return { type: 'string', value };
}

function makeLeaf(
  overrides: Partial<LeafConfigNode> = {},
): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Test Field',
    description: null,
    elementType: 'int',
    address: 100,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0'],
    value: intVal(7),
    eventRole: null,
    constraints: null,
    ...overrides,
  };
}

beforeEach(() => {
  mockOverride = null;
  mockRoleClassification = undefined;
  mockCatalogRole = null;
});

// ─── resolveValue ────────────────────────────────────────────────────────────

describe('resolveValue', () => {
  it('returns leaf.value when no override layer exists', () => {
    const leaf = makeLeaf({ value: intVal(7) });
    expect(resolveValue(NODE_ID, leaf)).toEqual(intVal(7));
  });

  it('returns override (draft or offlinePending) when configChangesStore has one', () => {
    mockOverride = intVal(42);
    const leaf = makeLeaf({ value: intVal(7) });
    expect(resolveValue(NODE_ID, leaf)).toEqual(intVal(42));
  });

  it('returns null when leaf has no value and no override exists', () => {
    const leaf = makeLeaf({ value: null });
    expect(resolveValue(NODE_ID, leaf)).toBeNull();
  });

  it('falls back to leaf.value when override is null', () => {
    mockOverride = null;
    const leaf = makeLeaf({ value: strVal('baseline') });
    expect(resolveValue(NODE_ID, leaf)).toEqual(strVal('baseline'));
  });
});

describe('makeValueResolver', () => {
  it('returns a resolver bound to the given node', () => {
    mockOverride = intVal(99);
    const resolver = makeValueResolver(NODE_ID);
    const leaf = makeLeaf({ value: intVal(7) });
    expect(resolver(leaf)).toEqual(intVal(99));
  });
});

// ─── resolveRole ─────────────────────────────────────────────────────────────

describe('resolveRole', () => {
  it('returns the role classification (pending edit or saved layout) when present', () => {
    mockRoleClassification = { role: 'Producer' };
    mockCatalogRole = 'Consumer'; // should be ignored
    const leaf = makeLeaf({ eventRole: 'Ambiguous' as EventRole });
    expect(resolveRole(NODE_ID, leaf)).toBe('Producer');
  });

  it('falls back to the catalog role when no classification exists', () => {
    mockRoleClassification = undefined;
    mockCatalogRole = 'Consumer';
    const leaf = makeLeaf({ eventRole: 'Ambiguous' as EventRole });
    expect(resolveRole(NODE_ID, leaf)).toBe('Consumer');
  });

  it('falls back to leaf.eventRole when no classification and no catalog role exist', () => {
    mockRoleClassification = undefined;
    mockCatalogRole = null;
    const leaf = makeLeaf({ eventRole: 'Producer' as EventRole });
    expect(resolveRole(NODE_ID, leaf)).toBe('Producer');
  });

  it('returns null when no layer has a role', () => {
    mockRoleClassification = undefined;
    mockCatalogRole = null;
    const leaf = makeLeaf({ eventRole: null });
    expect(resolveRole(NODE_ID, leaf)).toBeNull();
  });

  it('returns Ambiguous when only the CDI baseline classifies the leaf as Ambiguous', () => {
    mockRoleClassification = undefined;
    mockCatalogRole = null;
    const leaf = makeLeaf({ eventRole: 'Ambiguous' as EventRole });
    expect(resolveRole(NODE_ID, leaf)).toBe('Ambiguous');
  });
});
