/**
 * Tests for BowtieMetadataStore.
 *
 * Key areas:
 *  - createBowtie / deleteBowtie / renameBowtie / addTag / removeTag
 *  - getDirtyFields() reads _edits directly (not the already-mutated layout)
 *  - clearAll() clears via Map reassignment so isDirty and getDirtyFields() reset reliably
 *  - allEventIds reflects only pending create edits
 *  - Planning bowtie lifecycle (placeholder key create → clearAll → badge gone)
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { LayoutFile } from '$lib/types/bowtie';

// ─── Mutable layout state shared between mock and tests ─────────────────────

const mockLayout: { current: LayoutFile | null } = { current: null };
const layoutStoreMock = {
  get layout() { return mockLayout.current; },
  newLayout: vi.fn(() => {
    mockLayout.current = { schemaVersion: '1.0', bowties: {}, roleClassifications: {} };
  }),
  updateLayout: vi.fn((l: LayoutFile) => { mockLayout.current = l; }),
};

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('$lib/stores/layout.svelte', () => ({ layoutStore: layoutStoreMock }));

// Import after mocks so the singleton uses our mock layout store.
const { bowtieMetadataStore } = await import('$lib/stores/bowtieMetadata.svelte');

const REAL_ID = '05.01.01.01.FF.00.00.01';
const PLANNING_ID = 'planning-1774043332542';

// ─── Helpers ─────────────────────────────────────────────────────────────────

function seedLayout(bowties: Record<string, { name?: string; tags: string[] }> = {}) {
  mockLayout.current = {
    schemaVersion: '1.0',
    bowties: Object.fromEntries(
      Object.entries(bowties).map(([k, v]) => [k, { name: v.name, tags: v.tags }])
    ),
    roleClassifications: {},
  };
}

beforeEach(() => {
  mockLayout.current = null;
  vi.clearAllMocks();
  // Clear store state between tests via clearAll.
  bowtieMetadataStore.clearAll();
});

// ─── isDirty ─────────────────────────────────────────────────────────────────

describe('isDirty', () => {
  it('is false when no edits are pending', () => {
    expect(bowtieMetadataStore.isDirty).toBe(false);
  });

  it('is true after createBowtie', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'New Bowtie');
    expect(bowtieMetadataStore.isDirty).toBe(true);
  });

  it('is true after renameBowtie', () => {
    seedLayout({ [REAL_ID]: { name: 'Old', tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New');
    expect(bowtieMetadataStore.isDirty).toBe(true);
  });

  it('is true after addTag', () => {
    seedLayout({ [REAL_ID]: { tags: [] } });
    bowtieMetadataStore.addTag(REAL_ID, 'yard');
    expect(bowtieMetadataStore.isDirty).toBe(true);
  });
});

// ─── clearAll ────────────────────────────────────────────────────────────────

describe('clearAll', () => {
  it('resets isDirty to false', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'Test');
    expect(bowtieMetadataStore.isDirty).toBe(true);
    bowtieMetadataStore.clearAll();
    expect(bowtieMetadataStore.isDirty).toBe(false);
  });

  it('clears getDirtyFields for a just-created planning bowtie', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'Test');
    expect(bowtieMetadataStore.getDirtyFields(PLANNING_ID).size).toBe(1);
    bowtieMetadataStore.clearAll();
    expect(bowtieMetadataStore.getDirtyFields(PLANNING_ID).size).toBe(0);
  });

  it('clears getDirtyFields for a renamed bowtie', () => {
    seedLayout({ [REAL_ID]: { name: 'Old', tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New');
    expect(bowtieMetadataStore.getDirtyFields(REAL_ID).has('name')).toBe(true);
    bowtieMetadataStore.clearAll();
    expect(bowtieMetadataStore.getDirtyFields(REAL_ID).size).toBe(0);
  });

  it('clears allEventIds after creating a planning bowtie', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'Test');
    expect(bowtieMetadataStore.allEventIds).toContain(PLANNING_ID);
    bowtieMetadataStore.clearAll();
    expect(bowtieMetadataStore.allEventIds).toHaveLength(0);
  });

  it('resets editCount to 0', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'Test');
    bowtieMetadataStore.renameBowtie(PLANNING_ID, 'Edited');
    expect(bowtieMetadataStore.editCount).toBeGreaterThan(0);
    bowtieMetadataStore.clearAll();
    expect(bowtieMetadataStore.editCount).toBe(0);
  });
});

// ─── getDirtyFields ───────────────────────────────────────────────────────────

describe('getDirtyFields', () => {
  it('returns empty set when no edits exist', () => {
    expect(bowtieMetadataStore.getDirtyFields(REAL_ID).size).toBe(0);
  });

  it('returns {name} when a bowtie is created with a name', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'Named');
    expect(bowtieMetadataStore.getDirtyFields(PLANNING_ID)).toEqual(new Set(['name']));
  });

  it('returns {name} when a bowtie is created without a name', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID);
    expect(bowtieMetadataStore.getDirtyFields(PLANNING_ID)).toEqual(new Set(['name']));
  });

  it('returns {name} after renameBowtie', () => {
    seedLayout({ [REAL_ID]: { name: 'Old', tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New');
    expect(bowtieMetadataStore.getDirtyFields(REAL_ID)).toEqual(new Set(['name']));
  });

  it('returns {tags} after addTag', () => {
    seedLayout({ [REAL_ID]: { tags: [] } });
    bowtieMetadataStore.addTag(REAL_ID, 'yard');
    expect(bowtieMetadataStore.getDirtyFields(REAL_ID)).toEqual(new Set(['tags']));
  });

  it('returns {tags} after removeTag', () => {
    seedLayout({ [REAL_ID]: { tags: ['yard'] } });
    bowtieMetadataStore.removeTag(REAL_ID, 'yard');
    expect(bowtieMetadataStore.getDirtyFields(REAL_ID)).toEqual(new Set(['tags']));
  });

  it('returns both {name, tags} when rename and tag edits are pending', () => {
    seedLayout({ [REAL_ID]: { name: 'Old', tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New');
    bowtieMetadataStore.addTag(REAL_ID, 'yard');
    const fields = bowtieMetadataStore.getDirtyFields(REAL_ID);
    expect(fields).toEqual(new Set(['name', 'tags']));
  });

  it('does not bleed dirty fields from one bowtie to another', () => {
    const OTHER_ID = '05.01.01.01.FF.00.00.02';
    seedLayout({ [REAL_ID]: { tags: [] }, [OTHER_ID]: { tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'Changed');
    expect(bowtieMetadataStore.getDirtyFields(OTHER_ID).size).toBe(0);
  });
});

// ─── Planning bowtie lifecycle ───────────────────────────────────────────────

describe('planning bowtie lifecycle', () => {
  it('createBowtie with placeholder key adds to allEventIds', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'My Plan');
    expect(bowtieMetadataStore.allEventIds).toContain(PLANNING_ID);
  });

  it('placeholder key is preserved exactly (not uppercased)', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'My Plan');
    const ids = bowtieMetadataStore.allEventIds;
    expect(ids).toContain(PLANNING_ID);
    expect(ids).not.toContain(PLANNING_ID.toUpperCase());
  });

  it('getDirtyFields shows "name" for created planning bowtie', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'My Plan');
    expect(bowtieMetadataStore.getDirtyFields(PLANNING_ID).has('name')).toBe(true);
  });

  it('after clearAll isDirty is false and getDirtyFields is empty — simulates post-save state', () => {
    // Simulate: user creates planning bowtie, clicks Save, clearAll() is called
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'My Plan');
    expect(bowtieMetadataStore.isDirty).toBe(true);

    bowtieMetadataStore.clearAll();

    expect(bowtieMetadataStore.isDirty).toBe(false);
    expect(bowtieMetadataStore.getDirtyFields(PLANNING_ID).size).toBe(0);
    expect(bowtieMetadataStore.allEventIds).toHaveLength(0);
  });
});

// ─── allEventIds ──────────────────────────────────────────────────────────────

describe('allEventIds', () => {
  it('is empty when no create edits exist', () => {
    expect(bowtieMetadataStore.allEventIds).toHaveLength(0);
  });

  it('includes created event IDs', () => {
    const ID2 = 'planning-999';
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'A');
    bowtieMetadataStore.createBowtie(ID2, 'B');
    expect(bowtieMetadataStore.allEventIds).toEqual(expect.arrayContaining([PLANNING_ID, ID2]));
    expect(bowtieMetadataStore.allEventIds).toHaveLength(2);
  });

  it('does not include rename or tag edits', () => {
    seedLayout({ [REAL_ID]: { name: 'Old', tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New');
    bowtieMetadataStore.addTag(REAL_ID, 'tag');
    expect(bowtieMetadataStore.allEventIds).toHaveLength(0);
  });
});

// ─── rename ───────────────────────────────────────────────────────────────────

describe('renameBowtie', () => {
  it('updates effective metadata name', () => {
    seedLayout({ [REAL_ID]: { name: 'Old', tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New');
    expect(bowtieMetadataStore.getMetadata(REAL_ID)?.name).toBe('New');
  });

  it('a second rename overwrites the first (deduplicates the edit key)', () => {
    seedLayout({ [REAL_ID]: { name: 'Old', tags: [] } });
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New1');
    bowtieMetadataStore.renameBowtie(REAL_ID, 'New2');
    expect(bowtieMetadataStore.getMetadata(REAL_ID)?.name).toBe('New2');
    // Only one rename edit should exist (same map key)
    expect(bowtieMetadataStore.editCount).toBe(1);
  });
});

// ─── tag management ───────────────────────────────────────────────────────────

describe('tag management', () => {
  it('addTag adds a tag to effective metadata', () => {
    seedLayout({ [REAL_ID]: { tags: [] } });
    bowtieMetadataStore.addTag(REAL_ID, 'yard');
    expect(bowtieMetadataStore.getMetadata(REAL_ID)?.tags).toContain('yard');
  });

  it('removeTag removes a pending addTag (cancels out)', () => {
    seedLayout({ [REAL_ID]: { tags: [] } });
    bowtieMetadataStore.addTag(REAL_ID, 'yard');
    bowtieMetadataStore.removeTag(REAL_ID, 'yard');
    expect(bowtieMetadataStore.getMetadata(REAL_ID)?.tags).not.toContain('yard');
  });

  it('removeTag on an existing layout tag removes it from effective metadata', () => {
    seedLayout({ [REAL_ID]: { tags: ['yard', 'main'] } });
    bowtieMetadataStore.removeTag(REAL_ID, 'yard');
    const meta = bowtieMetadataStore.getMetadata(REAL_ID);
    expect(meta?.tags).not.toContain('yard');
    expect(meta?.tags).toContain('main');
  });
});

// ─── adoptEventId ─────────────────────────────────────────────────────────────

describe('adoptEventId', () => {
  it('in-session created bowtie: re-keys create edit to real event ID', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'Future');
    bowtieMetadataStore.adoptEventId(PLANNING_ID, REAL_ID);
    expect(bowtieMetadataStore.allEventIds).toContain(REAL_ID);
    expect(bowtieMetadataStore.allEventIds).not.toContain(PLANNING_ID);
    expect(bowtieMetadataStore.getMetadata(REAL_ID)?.name).toBe('Future');
  });

  it('in-session created bowtie: planning placeholder removed from layout after adopt', () => {
    bowtieMetadataStore.createBowtie(PLANNING_ID, 'Future');
    // _applyToLayout() has now written planning-xxx into the layout
    expect(mockLayout.current?.bowties[PLANNING_ID]).toBeDefined();
    bowtieMetadataStore.adoptEventId(PLANNING_ID, REAL_ID);
    // The delete edit must purge the placeholder from the layout
    expect(mockLayout.current?.bowties[PLANNING_ID]).toBeUndefined();
    expect(mockLayout.current?.bowties[REAL_ID]).toBeDefined();
  });

  it('file-loaded bowtie: removes placeholder and adds real event ID to layout', () => {
    seedLayout({ [PLANNING_ID]: { name: 'Future', tags: [] } });
    bowtieMetadataStore.adoptEventId(PLANNING_ID, REAL_ID);
    expect(mockLayout.current?.bowties[PLANNING_ID]).toBeUndefined();
    expect(mockLayout.current?.bowties[REAL_ID]).toBeDefined();
  });

  it('file-loaded bowtie: preserves the original name', () => {
    seedLayout({ [PLANNING_ID]: { name: 'Future', tags: [] } });
    bowtieMetadataStore.adoptEventId(PLANNING_ID, REAL_ID);
    expect(bowtieMetadataStore.getMetadata(REAL_ID)?.name).toBe('Future');
  });

  it('file-loaded bowtie: marks store as dirty', () => {
    seedLayout({ [PLANNING_ID]: { name: 'Future', tags: [] } });
    bowtieMetadataStore.adoptEventId(PLANNING_ID, REAL_ID);
    expect(bowtieMetadataStore.isDirty).toBe(true);
  });
});

// ─── demoteToPlanningBowtie ───────────────────────────────────────────────────

describe('demoteToPlanningBowtie', () => {
  it('removes the real event ID from the layout', () => {
    seedLayout({ [REAL_ID]: { name: 'Blink', tags: [] } });
    bowtieMetadataStore.demoteToPlanningBowtie(REAL_ID);
    expect(mockLayout.current?.bowties[REAL_ID]).toBeUndefined();
  });

  it('adds a planning-prefixed entry to the layout', () => {
    seedLayout({ [REAL_ID]: { name: 'Blink', tags: [] } });
    bowtieMetadataStore.demoteToPlanningBowtie(REAL_ID);
    const keys = Object.keys(mockLayout.current?.bowties ?? {});
    expect(keys.some(k => k.startsWith('planning-'))).toBe(true);
  });

  it('preserves the bowtie name in the new planning entry', () => {
    seedLayout({ [REAL_ID]: { name: 'Blink', tags: [] } });
    bowtieMetadataStore.demoteToPlanningBowtie(REAL_ID);
    const keys = Object.keys(mockLayout.current?.bowties ?? {});
    const planningKey = keys.find(k => k.startsWith('planning-'))!;
    expect(mockLayout.current?.bowties[planningKey]?.name).toBe('Blink');
  });

  it('preserves tags in the new planning entry', () => {
    seedLayout({ [REAL_ID]: { name: 'Blink', tags: ['yard', 'main'] } });
    bowtieMetadataStore.demoteToPlanningBowtie(REAL_ID);
    const keys = Object.keys(mockLayout.current?.bowties ?? {});
    const planningKey = keys.find(k => k.startsWith('planning-'))!;
    expect(mockLayout.current?.bowties[planningKey]?.tags).toEqual(
      expect.arrayContaining(['yard', 'main'])
    );
  });

  it('marks store as dirty', () => {
    seedLayout({ [REAL_ID]: { name: 'Blink', tags: [] } });
    bowtieMetadataStore.demoteToPlanningBowtie(REAL_ID);
    expect(bowtieMetadataStore.isDirty).toBe(true);
  });

  it('new planning entry appears in allEventIds', () => {
    seedLayout({ [REAL_ID]: { name: 'Blink', tags: [] } });
    bowtieMetadataStore.demoteToPlanningBowtie(REAL_ID);
    const allIds = bowtieMetadataStore.allEventIds;
    expect(allIds.some(id => id.startsWith('planning-'))).toBe(true);
    expect(allIds).not.toContain(REAL_ID);
  });
});
