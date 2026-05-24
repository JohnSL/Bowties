import { describe, expect, it, vi, beforeEach } from 'vitest';
import { hasUnsavedPromptChanges } from './unsavedChangesGuard';

const mockHasDraftsForNode = vi.fn().mockReturnValue(false);

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    hasDraftsForNode: (...args: unknown[]) => mockHasDraftsForNode(...args),
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  mockHasDraftsForNode.mockReturnValue(false);
});

describe('hasUnsavedPromptChanges', () => {
  it('returns false when no drafts, no metadata dirty, no offline drafts, layout clean', () => {
    expect(hasUnsavedPromptChanges(['node-1'], false, 0, false)).toBe(false);
  });

  it('treats draft offline edits as unsaved', () => {
    expect(hasUnsavedPromptChanges(['node-1'], false, 1, false)).toBe(true);
  });

  it('treats config drafts as unsaved', () => {
    mockHasDraftsForNode.mockReturnValue(true);
    expect(hasUnsavedPromptChanges(['node-1'], false, 0, false)).toBe(true);
  });

  it('treats bowtie metadata dirty as unsaved', () => {
    expect(hasUnsavedPromptChanges(['node-1'], true, 0, false)).toBe(true);
  });

  it('treats dirty in-memory layout metadata as unsaved', () => {
    expect(hasUnsavedPromptChanges(['node-1'], false, 0, true)).toBe(true);
  });

  it('treats layoutDirty as the single carrier for unsaved discovered nodes (S8)', () => {
    // S8 moved the discovered-node signal into `layoutStore.isDirty` itself;
    // callers pass that single flag through the existing `layoutDirty`
    // parameter rather than a separate count.
    expect(hasUnsavedPromptChanges(['node-1'], false, 0, true)).toBe(true);
    expect(hasUnsavedPromptChanges(['node-1'], false, 0, false)).toBe(false);
  });
});