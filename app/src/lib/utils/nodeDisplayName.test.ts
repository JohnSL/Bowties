import { describe, it, expect } from 'vitest';
import type { DiscoveredNode } from '$lib/api/tauri';
import type { LeafConfigNode, NodeConfigTree, TreeConfigValue } from '$lib/types/nodeTree';
import { resolveNodeDisplayName, resolveEffectiveUserName, resolveNodePartsFromSnip } from './nodeDisplayName';

/**
 * Canonical Display Name Fallback chain:
 *   user_name → "manufacturer — model" → model → Node ID hex
 *
 * Governing doc: product/architecture/naming-and-normalization.md
 */

const NODE_ID = '05.02.01.02.02.00';

function withSnip(snip: Partial<DiscoveredNode['snip_data'] & object>): Pick<DiscoveredNode, 'snip_data'> {
  return {
    snip_data: {
      manufacturer: '',
      model: '',
      hardware_version: '',
      software_version: '',
      user_name: '',
      user_description: '',
      ...snip,
    },
  };
}

describe('resolveNodeDisplayName', () => {
  it('falls back to the node ID when the node is null', () => {
    expect(resolveNodeDisplayName(NODE_ID, null)).toBe(NODE_ID);
  });

  it('falls back to the node ID when the node is undefined', () => {
    expect(resolveNodeDisplayName(NODE_ID, undefined)).toBe(NODE_ID);
  });

  it('falls back to the node ID when SNIP is absent', () => {
    expect(resolveNodeDisplayName(NODE_ID, { snip_data: null })).toBe(NODE_ID);
  });

  it('prefers the SNIP user name', () => {
    expect(resolveNodeDisplayName(NODE_ID, withSnip({ user_name: 'East Panel', manufacturer: 'Acme', model: 'IO16' })))
      .toBe('East Panel');
  });

  it('trims whitespace from the user name', () => {
    expect(resolveNodeDisplayName(NODE_ID, withSnip({ user_name: '  East Panel  ' }))).toBe('East Panel');
  });

  it('uses "manufacturer — model" when the user name is empty', () => {
    expect(resolveNodeDisplayName(NODE_ID, withSnip({ manufacturer: 'Acme', model: 'IO16' })))
      .toBe('Acme — IO16');
  });

  it('uses the model alone when only the model is present', () => {
    expect(resolveNodeDisplayName(NODE_ID, withSnip({ model: 'IO16' }))).toBe('IO16');
  });

  it('falls back to the node ID when all SNIP name fields are blank', () => {
    expect(resolveNodeDisplayName(NODE_ID, withSnip({}))).toBe(NODE_ID);
  });
});

/**
 * Effective User Name resolution — consults the edit layer (draft → offline →
 * baseline) for the ACDI User Name leaf (memory space 251) so an offline edit
 * to the node name is reflected in the Display Name. ADR-0003 point 4.
 */

const ACDI_USER_SPACE = 251;

function makeStringLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'User Name',
    description: null,
    elementType: 'string',
    address: 1,
    size: 63,
    space: ACDI_USER_SPACE,
    path: ['seg:0', 'elem:0'],
    value: null,
    eventRole: null,
    constraints: null,
    ...overrides,
  };
}

function makeTreeWith(leaves: LeafConfigNode[], space = ACDI_USER_SPACE): NodeConfigTree {
  return {
    nodeId: '05.02.01.02.02.00',
    identity: null,
    segments: [{ name: 'User Info', description: null, origin: 0, space, children: leaves }],
  };
}

/** A resolver that returns a draft override for a specific leaf, else baseline. */
function resolverWithDraft(target: LeafConfigNode, draft: TreeConfigValue) {
  return (leaf: LeafConfigNode): TreeConfigValue | null =>
    leaf === target ? draft : leaf.value;
}

describe('resolveEffectiveUserName', () => {
  it('returns null when the tree is null or undefined', () => {
    expect(resolveEffectiveUserName(null, l => l.value)).toBeNull();
    expect(resolveEffectiveUserName(undefined, l => l.value)).toBeNull();
  });

  it('returns null when no space-251 string leaf exists', () => {
    const tree = makeTreeWith([makeStringLeaf({ space: 253 })]);
    expect(resolveEffectiveUserName(tree, l => l.value)).toBeNull();
  });

  it('returns the baseline value when no edit layer overrides it', () => {
    const leaf = makeStringLeaf({ value: { type: 'string', value: 'Saved Name' } });
    const tree = makeTreeWith([leaf]);
    expect(resolveEffectiveUserName(tree, l => l.value)).toBe('Saved Name');
  });

  it('returns the draft override ahead of the baseline (offline edit)', () => {
    const leaf = makeStringLeaf({ value: { type: 'string', value: 'Old Name' } });
    const tree = makeTreeWith([leaf]);
    const resolve = resolverWithDraft(leaf, { type: 'string', value: 'New Name' });
    expect(resolveEffectiveUserName(tree, resolve)).toBe('New Name');
  });

  it('trims whitespace from the resolved name', () => {
    const leaf = makeStringLeaf({ value: { type: 'string', value: '  Trimmed  ' } });
    const tree = makeTreeWith([leaf]);
    expect(resolveEffectiveUserName(tree, l => l.value)).toBe('Trimmed');
  });

  it('returns null when the resolved name is empty or whitespace', () => {
    const leaf = makeStringLeaf({ value: { type: 'string', value: '   ' } });
    const tree = makeTreeWith([leaf]);
    expect(resolveEffectiveUserName(tree, l => l.value)).toBeNull();
  });

  it('picks the lowest-address string leaf (user name before description)', () => {
    const description = makeStringLeaf({ address: 64, value: { type: 'string', value: 'A description' } });
    const userName = makeStringLeaf({ address: 1, value: { type: 'string', value: 'The Name' } });
    const tree = makeTreeWith([description, userName]);
    expect(resolveEffectiveUserName(tree, l => l.value)).toBe('The Name');
  });

  it('finds the user-name leaf nested inside a group', () => {
    const leaf = makeStringLeaf({ value: { type: 'string', value: 'Nested Name' } });
    const tree: NodeConfigTree = {
      nodeId: '05.02.01.02.02.00',
      identity: null,
      segments: [{
        name: 'User Info', description: null, origin: 0, space: ACDI_USER_SPACE,
        children: [{
          kind: 'group', name: 'User Info', description: null, instance: 1,
          instanceLabel: 'User Info', replicationOf: 'User Info', replicationCount: 1,
          path: ['seg:0', 'grp:0'], children: [leaf],
        } as unknown as LeafConfigNode],
      }],
    };
    expect(resolveEffectiveUserName(tree, l => l.value)).toBe('Nested Name');
  });
});

/**
 * resolveNodePartsFromSnip — returns structured { name, model, manufacturer,
 * isUserNamed } for UI rendering without baking manufacturer into the name.
 */
describe('resolveNodePartsFromSnip', () => {
  it('returns node ID with nulls when node is null', () => {
    const parts = resolveNodePartsFromSnip(NODE_ID, null);
    expect(parts).toEqual({ name: NODE_ID, model: null, manufacturer: null, isUserNamed: false });
  });

  it('returns node ID with nulls when SNIP is absent', () => {
    const parts = resolveNodePartsFromSnip(NODE_ID, { snip_data: null });
    expect(parts).toEqual({ name: NODE_ID, model: null, manufacturer: null, isUserNamed: false });
  });

  it('uses user_name as name and exposes model and manufacturer separately', () => {
    const parts = resolveNodePartsFromSnip(NODE_ID, withSnip({
      user_name: 'Blocks Detection',
      manufacturer: 'RR-CirKits',
      model: 'Tower-LCC',
    }));
    expect(parts).toEqual({
      name: 'Blocks Detection',
      model: 'Tower-LCC',
      manufacturer: 'RR-CirKits',
      isUserNamed: true,
    });
  });

  it('falls back to model as name when user_name is empty', () => {
    const parts = resolveNodePartsFromSnip(NODE_ID, withSnip({
      manufacturer: 'RR-CirKits',
      model: 'Tower-LCC',
    }));
    expect(parts).toEqual({
      name: 'Tower-LCC',
      model: 'Tower-LCC',
      manufacturer: 'RR-CirKits',
      isUserNamed: false,
    });
  });

  it('falls back to node ID when both user_name and model are empty', () => {
    const parts = resolveNodePartsFromSnip(NODE_ID, withSnip({
      manufacturer: 'RR-CirKits',
    }));
    expect(parts).toEqual({
      name: NODE_ID,
      model: null,
      manufacturer: 'RR-CirKits',
      isUserNamed: false,
    });
  });

  it('marks isUserNamed true even when user_name equals model', () => {
    const parts = resolveNodePartsFromSnip(NODE_ID, withSnip({
      user_name: 'modulino_io',
      manufacturer: 'OpenMRN',
      model: 'modulino_io',
    }));
    expect(parts).toEqual({
      name: 'modulino_io',
      model: 'modulino_io',
      manufacturer: 'OpenMRN',
      isUserNamed: true,
    });
  });

  it('trims whitespace from all fields', () => {
    const parts = resolveNodePartsFromSnip(NODE_ID, withSnip({
      user_name: '  East Panel  ',
      manufacturer: '  Acme  ',
      model: '  IO16  ',
    }));
    expect(parts.name).toBe('East Panel');
    expect(parts.model).toBe('IO16');
    expect(parts.manufacturer).toBe('Acme');
  });
});
