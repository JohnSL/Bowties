import { describe, expect, it } from 'vitest';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import { signalOutputOwnerDescription, signalOutputRows } from './signalOutputLabels';

const out = (ordinal: number, label: string) => ({ ordinal, label, owner: null });

describe('signalOutputRows', () => {
  it('groups outputs into one row per header, in ordinal order', () => {
    const rows = signalOutputRows([
      out(1, '#1 H1-G'), out(2, '#2 H1-Y'), out(5, '#5 H2-G'), out(9, '#9 H3-G'),
    ]);
    expect(rows.map((r) => r.header)).toEqual(['H1', 'H2', 'H3']);
    expect(rows[0].cells.map((c) => c.label)).toEqual(['#1 H1-G', '#2 H1-Y']);
    expect(rows[2].cells.map((c) => c.label)).toEqual(['#9 H3-G']);
  });

  it('falls back to a single unlabeled row when any label does not parse', () => {
    const rows = signalOutputRows([out(1, '#1 H1-G'), out(2, 'Output two')]);
    expect(rows).toHaveLength(1);
    expect(rows[0].header).toBe('');
    expect(rows[0].cells.map((c) => c.label)).toEqual(['#1 H1-G', 'Output two']);
  });
});

function leaf(name: string, value: string) {
  return { kind: 'leaf' as const, name, description: null, elementType: 'string', value: { type: 'string' as const, value }, path: [] };
}
function instance(name: string, i: number, children: unknown[]) {
  return { kind: 'group' as const, name, description: null, instance: i, instanceLabel: `${name} ${i}`, replicationOf: name, replicationCount: 3, path: [], displayName: null, children };
}
function descTree(): NodeConfigTree {
  return {
    nodeId: 'n', identity: null,
    segments: [
      { name: 'Rule to Aspect', description: null, origin: 0, space: 1, seg: 0, children: [
        instance('Mast', 1, [leaf('Mast Description', '')]),
        instance('Mast', 3, [leaf('Mast Description', 'Cedar Hill East')]),
      ] },
      { name: 'Direct Lamp Control', description: null, origin: 0, space: 1, seg: 1, children: [
        instance('Lamp', 2, [leaf('Lamp Description', 'Crossing flasher')]),
      ] },
    ],
  } as unknown as NodeConfigTree;
}

describe('signalOutputOwnerDescription', () => {
  it('reads the Mast Description or Lamp Description of the owning group', () => {
    expect(signalOutputOwnerDescription(descTree(), 'Mast 3')).toBe('Cedar Hill East');
    expect(signalOutputOwnerDescription(descTree(), 'Direct lamp 2')).toBe('Crossing flasher');
  });

  it('is undefined when empty, unknown, or the tree is missing', () => {
    expect(signalOutputOwnerDescription(descTree(), 'Mast 1')).toBeUndefined();
    expect(signalOutputOwnerDescription(descTree(), 'Mast 9')).toBeUndefined();
    expect(signalOutputOwnerDescription(descTree(), 'Something else')).toBeUndefined();
    expect(signalOutputOwnerDescription(undefined, 'Mast 3')).toBeUndefined();
  });
});
