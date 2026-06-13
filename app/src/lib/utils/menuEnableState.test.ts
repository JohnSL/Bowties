import { describe, expect, it } from 'vitest';
import { computeMenuEnableState, type MenuEnableInputs } from './menuEnableState';

function baseInputs(overrides: Partial<MenuEnableInputs> = {}): MenuEnableInputs {
  return {
    connected: false,
    busy: false,
    hasSelection: false,
    hasSelectedSegment: false,
    selectedNodeHasCdi: false,
    selectedIsPlaceholder: false,
    selectedInRoster: false,
    layoutLoaded: false,
    layoutDirty: false,
    metaDirty: false,
    hasActiveLayout: false,
    hasLayoutFile: false,
    hasInMemoryEdits: false,
    pendingSyncCount: 0,
    ...overrides,
  };
}

describe('computeMenuEnableState', () => {
  it('disables everything from a cold/idle baseline', () => {
    expect(computeMenuEnableState(baseInputs())).toEqual({
      canViewCdi: false,
      canRedownloadCdi: false,
      canOpenLayout: false,
      canCloseLayout: false,
      canSaveLayout: false,
      canSaveLayoutAs: false,
      canSyncToBus: false,
      canAddPlaceholderBoard: false,
      canDeletePlaceholderBoard: false,
    });
  });

  describe('CDI items', () => {
    it('enables re-download CDI when connected, not busy, and a node is selected', () => {
      expect(computeMenuEnableState(baseInputs({ connected: true, hasSelection: true })).canRedownloadCdi).toBe(true);
    });

    it('disables re-download CDI while busy or disconnected', () => {
      expect(computeMenuEnableState(baseInputs({ connected: true, hasSelection: true, busy: true })).canRedownloadCdi).toBe(false);
      expect(computeMenuEnableState(baseInputs({ connected: false, hasSelection: true })).canRedownloadCdi).toBe(false);
    });

    it('enables view CDI for a selected segment', () => {
      expect(computeMenuEnableState(baseInputs({ connected: true, hasSelectedSegment: true })).canViewCdi).toBe(true);
    });

    it('enables view CDI for a selected node only when it has cached CDI', () => {
      expect(computeMenuEnableState(baseInputs({ connected: true, hasSelection: true, selectedNodeHasCdi: false })).canViewCdi).toBe(false);
      expect(computeMenuEnableState(baseInputs({ connected: true, hasSelection: true, selectedNodeHasCdi: true })).canViewCdi).toBe(true);
    });
  });

  describe('layout items', () => {
    it('enables open / save-as only with an active layout and not busy', () => {
      const active = computeMenuEnableState(baseInputs({ hasActiveLayout: true }));
      expect(active.canOpenLayout).toBe(true);
      expect(active.canSaveLayoutAs).toBe(true);

      const busy = computeMenuEnableState(baseInputs({ hasActiveLayout: true, busy: true }));
      expect(busy.canOpenLayout).toBe(false);
      expect(busy.canSaveLayoutAs).toBe(false);
    });

    it('enables close whenever a layout is active, regardless of busy', () => {
      expect(computeMenuEnableState(baseInputs({ hasActiveLayout: true })).canCloseLayout).toBe(true);
      expect(computeMenuEnableState(baseInputs({ hasActiveLayout: true, busy: true })).canCloseLayout).toBe(true);
    });

    it('enables save for an offline layout with in-memory edits', () => {
      const s = computeMenuEnableState(baseInputs({
        hasActiveLayout: true, hasLayoutFile: true, hasInMemoryEdits: true,
      }));
      expect(s.canSaveLayout).toBe(true);
    });

    it('enables save for a loaded layout with struct or metadata dirt', () => {
      expect(computeMenuEnableState(baseInputs({ layoutLoaded: true, layoutDirty: true })).canSaveLayout).toBe(true);
      expect(computeMenuEnableState(baseInputs({ layoutLoaded: true, metaDirty: true })).canSaveLayout).toBe(true);
      expect(computeMenuEnableState(baseInputs({ layoutLoaded: true })).canSaveLayout).toBe(false);
    });

    it('disables save while busy even with edits', () => {
      const s = computeMenuEnableState(baseInputs({
        busy: true, hasActiveLayout: true, hasLayoutFile: true, hasInMemoryEdits: true,
      }));
      expect(s.canSaveLayout).toBe(false);
    });
  });

  describe('offline / placeholder items', () => {
    it('enables sync-to-bus only when connected, offline-active, and changes pending', () => {
      const ready = baseInputs({ connected: true, hasActiveLayout: true, hasLayoutFile: true, pendingSyncCount: 2 });
      expect(computeMenuEnableState(ready).canSyncToBus).toBe(true);
      expect(computeMenuEnableState({ ...ready, pendingSyncCount: 0 }).canSyncToBus).toBe(false);
      expect(computeMenuEnableState({ ...ready, connected: false }).canSyncToBus).toBe(false);
    });

    it('enables add-placeholder only for an active offline layout when not busy', () => {
      expect(computeMenuEnableState(baseInputs({ hasActiveLayout: true, hasLayoutFile: true })).canAddPlaceholderBoard).toBe(true);
      expect(computeMenuEnableState(baseInputs({ hasActiveLayout: true, hasLayoutFile: false })).canAddPlaceholderBoard).toBe(false);
      expect(computeMenuEnableState(baseInputs({ hasActiveLayout: true, hasLayoutFile: true, busy: true })).canAddPlaceholderBoard).toBe(false);
    });

    it('enables delete-placeholder only for a selected in-roster placeholder', () => {
      const ready = baseInputs({
        hasActiveLayout: true, hasLayoutFile: true,
        hasSelection: true, selectedIsPlaceholder: true, selectedInRoster: true,
      });
      expect(computeMenuEnableState(ready).canDeletePlaceholderBoard).toBe(true);
      expect(computeMenuEnableState({ ...ready, selectedInRoster: false }).canDeletePlaceholderBoard).toBe(false);
      expect(computeMenuEnableState({ ...ready, selectedIsPlaceholder: false }).canDeletePlaceholderBoard).toBe(false);
    });
  });
});
