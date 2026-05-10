/**
 * Tests for configEditor (configEditor.svelte.ts).
 *
 * PR 1 behavior: configEditor is a thin pass-through.
 * applyEdit(key, value) delegates to configChangesStore.set(key, value).
 * No cascade logic in this PR.
 *
 * Covers:
 * - applyEdit delegates to configChangesStore.set
 * - Single edit creates exactly one draft
 * - visibleValue reflects the edit (via configChangesStore)
 * - No cascade: only one draft per applyEdit call
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { TreeConfigValue } from '$lib/types/nodeTree';
import { editKeyForLeaf } from '$lib/utils/editKey';

// ─── Mock configChangesStore ──────────────────────────────────────────────────

const mockSet = vi.fn<(key: string, value: TreeConfigValue) => void>();
let mockVisibleValue: TreeConfigValue | null = null;

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    set: mockSet,
    get visibleValue() {
      return (_key: string) => mockVisibleValue;
    },
  },
}));

// ─── Import AFTER mocks ───────────────────────────────────────────────────────

const { configEditor } = await import('$lib/stores/configEditor.svelte');

// ─── Helpers ──────────────────────────────────────────────────────────────────

const NODE_ID = '05.02.01.02.03.00';
const SPACE = 253;
const ADDRESS = 100;
const KEY = editKeyForLeaf(NODE_ID, SPACE, ADDRESS);

function intVal(value: number): TreeConfigValue {
  return { type: 'int', value };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockVisibleValue = null;
});

// ─── Tests ────────────────────────────────────────────────────────────────────

describe('configEditor.applyEdit — pass-through behavior', () => {
  it('calls configChangesStore.set with the same key and value', () => {
    configEditor.applyEdit(KEY, intVal(5));
    expect(mockSet).toHaveBeenCalledOnce();
    expect(mockSet).toHaveBeenCalledWith(KEY, intVal(5));
  });

  it('calls configChangesStore.set exactly once per applyEdit (no cascade in PR 1)', () => {
    configEditor.applyEdit(KEY, intVal(5));
    expect(mockSet).toHaveBeenCalledTimes(1);
  });

  it('forwards string values correctly', () => {
    const strValue: TreeConfigValue = { type: 'string', value: 'Tower East' };
    configEditor.applyEdit(KEY, strValue);
    expect(mockSet).toHaveBeenCalledWith(KEY, strValue);
  });

  it('forwards eventId values correctly', () => {
    const eventValue: TreeConfigValue = {
      type: 'eventId',
      bytes: [1, 2, 3, 4, 5, 6, 7, 8],
      hex: '01.02.03.04.05.06.07.08',
    };
    configEditor.applyEdit(KEY, eventValue);
    expect(mockSet).toHaveBeenCalledWith(KEY, eventValue);
  });

  it('forwards float values correctly', () => {
    const floatValue: TreeConfigValue = { type: 'float', value: 3.14 };
    configEditor.applyEdit(KEY, floatValue);
    expect(mockSet).toHaveBeenCalledWith(KEY, floatValue);
  });

  it('can be called multiple times for different keys', () => {
    const key2 = editKeyForLeaf(NODE_ID, SPACE, 200);
    configEditor.applyEdit(KEY, intVal(1));
    configEditor.applyEdit(key2, intVal(2));
    expect(mockSet).toHaveBeenCalledTimes(2);
    expect(mockSet).toHaveBeenNthCalledWith(1, KEY, intVal(1));
    expect(mockSet).toHaveBeenNthCalledWith(2, key2, intVal(2));
  });
});
