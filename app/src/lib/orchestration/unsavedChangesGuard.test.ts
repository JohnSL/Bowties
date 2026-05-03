import { describe, expect, it } from 'vitest';
import type { LeafConfigNode, NodeConfigTree } from '$lib/types/nodeTree';
import { hasUnsavedPromptChanges } from './unsavedChangesGuard';

function makeTree(modifiedValue: LeafConfigNode['modifiedValue']): NodeConfigTree {
  return {
    nodeId: '02.01.57.00.00.01',
    identity: null,
    segments: [
      {
        name: 'Config',
        description: null,
        origin: 0,
        space: 253,
        children: [
          {
            kind: 'leaf',
            name: 'Field 1',
            description: null,
            elementType: 'int',
            address: 1,
            size: 1,
            space: 253,
            path: ['seg:0', 'elem:0'],
            value: { type: 'int', value: 1 },
            modifiedValue,
            eventRole: null,
            constraints: null,
          },
        ],
      },
    ],
  };
}

describe('hasUnsavedPromptChanges', () => {
  it('does not treat saved pending-sync state as unsaved when draftCount is zero', () => {
    expect(hasUnsavedPromptChanges([makeTree(null)], false, 0, false)).toBe(false);
  });

  it('treats draft offline edits as unsaved', () => {
    expect(hasUnsavedPromptChanges([makeTree(null)], false, 1, false)).toBe(true);
  });

  it('treats modified tree values as unsaved', () => {
    expect(hasUnsavedPromptChanges([makeTree({ type: 'int', value: 2 })], false, 0, false)).toBe(true);
  });

  it('treats dirty in-memory layout metadata as unsaved', () => {
    expect(hasUnsavedPromptChanges([makeTree(null)], false, 0, true)).toBe(true);
  });
});