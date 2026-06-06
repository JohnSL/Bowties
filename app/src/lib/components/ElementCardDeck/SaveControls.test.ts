/**
 * T023: Vitest component tests for SaveControls.svelte
 *
 * Covers:
 * - Save/Discard hidden when no dirty leaves or bowtie metadata
 * - Save/Discard visible when dirty config leaves or metadata are present
 * - Shows unsaved change count
 * - Save button calls writeModifiedValues
 * - Discard button opens confirmation dialog
 * - Revert in dialog calls discardModifiedValues
 * - Cancel in dialog closes dialog without discarding
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte';
import SaveControls from './SaveControls.svelte';
import type { NodeConfigTree, LeafConfigNode, SegmentNode } from '$lib/types/nodeTree';

// ── Hoisted mock references ───────────────────────────────────────────────────
// vi.hoisted ensures these are available inside vi.mock() factories.
const { treesRef, metaRef, layoutRef, offlineRef, configChangesRef, connectorSelectionsRef, effectiveNodeRef } = vi.hoisted(() => ({
  treesRef: { map: new Map<string, NodeConfigTree>() },
  configChangesRef: { draftCount: 0, hasDraftsForNode: false },
  metaRef: { isDirty: false, editCount: 0, clearAll: vi.fn() },
  layoutRef: {
    layout: null as any,
    isOfflineMode: false,
    hasLayoutFile: false,
    isConnected: false,
    isLoaded: false,
    isDirty: false,
    markClean: vi.fn(),
    markDirty: vi.fn(),
    saveCurrentLayout: vi.fn().mockResolvedValue(undefined) as any,
    saveLayoutAs: vi.fn().mockResolvedValue(undefined) as any,
    revertToSaved: vi.fn(),
  },
  effectiveNodeRef: {
    unsavedInMemoryNodeIds: [] as string[],
    isDirty: false,
  },
  offlineRef: {
    draftCount: 0,
    draftRows: [] as any[],
    pendingCount: 0,
    revertedPersistedCount: 0,
    get effectiveRows() { return (this as any).persistedRows ?? []; },
    reloadFromBackend: vi.fn().mockResolvedValue(undefined) as any,
    revertAllPending: vi.fn().mockResolvedValue(undefined) as any,
    flushPendingToBackend: vi.fn().mockResolvedValue(0) as any,
    clear: vi.fn(),
  },
  connectorSelectionsRef: {
    hydrateFromLayout: vi.fn(),
    totalWarningCount: 0,
  },
}));

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() { return treesRef.map; },
  },
}));

vi.mock('$lib/stores/configChanges.svelte', () => ({
  configChangesStore: {
    draftEntries: () => Array.from({ length: configChangesRef.draftCount }, (_, i) => ({ key: `k${i}`, value: { type: 'int', value: i } })),
    clearAllDrafts: vi.fn(),
    commitForSave: vi.fn(),
    hasDraftsForNode: () => configChangesRef.hasDraftsForNode,
  },
}));

vi.mock('$lib/orchestration/configDraftOrchestrator', () => ({
  stageDraftsForOfflineSave: vi.fn(),
  discardAllConfigDrafts: vi.fn(),
}));

vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: metaRef,
}));

vi.mock('$lib/api/config', () => ({
  writeModifiedValues: vi.fn().mockResolvedValue({ succeeded: 1, failed: 0, readOnlyRejected: 0 }),
  discardModifiedValues: vi.fn().mockResolvedValue(0),
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: layoutRef,
}));

vi.mock('$lib/layout', () => ({
  effectiveNodeStore: effectiveNodeRef,
}));

vi.mock('$lib/stores/connectorSelections.svelte', () => ({
  connectorSelectionsStore: connectorSelectionsRef,
}));

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: offlineRef,
}));

vi.mock('$lib/stores/nodeInfo', () => ({
  updateNodeSnipField: vi.fn(),
}));

vi.mock('@zerodevx/svelte-toast', () => ({
  toast: { push: vi.fn(), pop: vi.fn() },
  SvelteToast: vi.fn(),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Build a minimal NodeConfigTree with `count` leaves. Also sets configChangesRef
 * so the presenter sees matching draft counts. */
function makeDirtyTree(count = 1): NodeConfigTree {
  // Update the config changes mock to reflect dirty state.
  configChangesRef.draftCount += count;
  configChangesRef.hasDraftsForNode = true;

  const leaves: LeafConfigNode[] = Array.from({ length: count }, (_, i) => ({
    kind: 'leaf' as const,
    name: `Field ${i}`,
    description: null,
    elementType: 'int' as const,
    address: i,
    size: 1,
    space: 253,
    path: ['seg:0', `elem:${i}`],
    value: { type: 'int' as const, value: 0 },
    eventRole: null,
    constraints: null,
    modifiedValue: { type: 'int' as const, value: 99 },
  }));
  const seg: SegmentNode = {
    name: 'Configuration',
    description: null,
    origin: 0,
    space: 253,
    children: leaves,
  };
  return { nodeId: 'test-node', identity: null, segments: [seg] };
}

beforeEach(() => {
  treesRef.map = new Map();
  configChangesRef.draftCount = 0;
  configChangesRef.hasDraftsForNode = false;
  metaRef.isDirty = false;
  metaRef.editCount = 0;
  layoutRef.isOfflineMode = false;
  layoutRef.layout = null;
  layoutRef.hasLayoutFile = false;
  layoutRef.isConnected = false;
  layoutRef.isLoaded = false;
  layoutRef.isDirty = false;
  effectiveNodeRef.unsavedInMemoryNodeIds = [];
  effectiveNodeRef.isDirty = false;
  offlineRef.draftCount = 0;
  offlineRef.draftRows = [];
  offlineRef.pendingCount = 0;
  offlineRef.revertedPersistedCount = 0;
  connectorSelectionsRef.hydrateFromLayout.mockReset();
  connectorSelectionsRef.totalWarningCount = 0;
  vi.clearAllMocks();
});

describe('SaveControls.svelte', () => {
  describe('visibility', () => {
    it('renders no Save/Discard buttons when there are no pending edits', () => {
      render(SaveControls);
      expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument();
    });

    it('shows Save and Discard buttons when dirty config leaves exist', async () => {
      treesRef.map.set('node1', makeDirtyTree(2));
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /^save$/i })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /discard/i })).toBeInTheDocument();
      });
    });

    it('shows Save and Discard buttons when only bowtie metadata is dirty', async () => {
      metaRef.isDirty = true;
      metaRef.editCount = 1;
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /^save$/i })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /discard/i })).toBeInTheDocument();
      });
    });

    it('shows unsaved change count when dirty leaves exist', async () => {
      treesRef.map.set('node1', makeDirtyTree(3));
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByText(/3 unsaved changes/i)).toBeInTheDocument();
      });
    });

    it('keeps pending hint and discard dialog counts aligned for layout-only dirty state', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.isDirty = true;
      render(SaveControls);

      await waitFor(() => {
        expect(screen.getByText(/1 unsaved edit/i)).toBeInTheDocument();
      });

      await fireEvent.click(screen.getByRole('button', { name: /discard/i }));

      await waitFor(() => {
        expect(screen.getByText(/1 unsaved change/i)).toBeInTheDocument();
        expect(screen.getByText(/1 node/i)).toBeInTheDocument();
      });
    });

    it('shows "1 unsaved change" (singular) for a single dirty leaf', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByText(/1 unsaved change/i)).toBeInTheDocument();
      });
    });
  });

  describe('Save button state', () => {
    it('Save button not rendered when no pending edits', () => {
      render(SaveControls);
      expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
    });

    it('Save button is enabled when dirty leaves exist', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      await waitFor(() => {
        const btn = screen.getByRole('button', { name: /^save$/i });
        expect(btn).not.toBeDisabled();
      });
    });

    it('online save delegates to onSave — does not call writeModifiedValues directly (ADR-0001)', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      const mockOnSave = vi.fn().mockResolvedValue(true);
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls, { props: { onSave: mockOnSave } });
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      expect(mockOnSave).toHaveBeenCalled();
      expect(writeModifiedValues).not.toHaveBeenCalled();
    });
  });

  // ── S2: ADR-0001 — Online save delegates to three-phase flow ──────────────
  describe('S2: online save ordering (ADR-0001)', () => {
    it('cancel from save dialog sends zero bus writes', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      const mockOnSave = vi.fn().mockResolvedValue(false);
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls, { props: { onSave: mockOnSave } });
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      expect(mockOnSave).toHaveBeenCalled();
      expect(writeModifiedValues).not.toHaveBeenCalled();
    });

    it('delegates cleanup to the page route after a successful online save (orchestrator owns commitForSave)', async () => {
      // Post-Step 7 Option H: the component no longer calls
      // `configChangesStore.commitForSave` directly. Cleanup moved into
      // `saveLayoutOrchestrator.clearPersistedDrafts` (wired by
      // `+page.svelte`'s `saveCurrentCaptureToFile`). The component's
      // contract is just "call onSave; trust the page to clean up."
      const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
      const mockOnSave = vi.fn().mockResolvedValue(true);
      treesRef.map.set('node1', makeDirtyTree(2));
      render(SaveControls, { props: { onSave: mockOnSave } });
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      await waitFor(() => {
        expect(mockOnSave).toHaveBeenCalledTimes(1);
      });
      expect(configChangesStore.commitForSave).not.toHaveBeenCalled();
    });

    it('does not invoke onSave when the user cancels the save dialog', async () => {
      const mockOnSave = vi.fn().mockResolvedValue(false);
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls, { props: { onSave: mockOnSave } });
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      expect(mockOnSave).toHaveBeenCalledTimes(1);
    });
  });

  // ── T047: US6 — Discard with confirmation ─────────────────────────────────
  describe('T047: Discard button behavior', () => {
    it('Discard button not rendered when no pending edits', () => {
      render(SaveControls);
      expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument();
    });

    it('Discard button is enabled when edits exist', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /discard/i })).not.toBeDisabled();
      });
    });

    it('opens confirmation dialog on Discard click', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fireEvent.click(screen.getByRole('button', { name: /discard/i }));

      await waitFor(() => {
        expect(screen.getByRole('alertdialog')).toBeInTheDocument();
        expect(screen.getByText(/discard unsaved changes/i)).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /revert/i })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument();
      });
    });

    it('calls discardModifiedValues when Revert is clicked', async () => {
      const { discardModifiedValues } = await import('$lib/api/config');
      treesRef.map.set('node1', makeDirtyTree(1));
      layoutRef.layout = {
        schemaVersion: '1.0',
        bowties: {},
        roleClassifications: {},
        connectorSelections: {},
      };
      render(SaveControls);
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fireEvent.click(screen.getByRole('button', { name: /discard/i }));
      const revertBtn = await waitFor(() => screen.getByRole('button', { name: /revert/i }));
      await fireEvent.click(revertBtn);
      expect(discardModifiedValues).toHaveBeenCalled();
      expect(connectorSelectionsRef.hydrateFromLayout).toHaveBeenCalledWith(layoutRef.layout);
    });

    it('clears config drafts during online discard', async () => {
      const { discardAllConfigDrafts } = await import('$lib/orchestration/configDraftOrchestrator');
      treesRef.map.set('node1', makeDirtyTree(1));
      layoutRef.layout = {
        schemaVersion: '1.0',
        bowties: {},
        roleClassifications: {},
        connectorSelections: {},
      };
      render(SaveControls);
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fireEvent.click(screen.getByRole('button', { name: /discard/i }));
      const revertBtn = await waitFor(() => screen.getByRole('button', { name: /revert/i }));
      await fireEvent.click(revertBtn);
      expect(discardAllConfigDrafts).toHaveBeenCalled();
    });

    it('closes dialog without discarding when Cancel is clicked', async () => {
      const { discardModifiedValues } = await import('$lib/api/config');
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fireEvent.click(screen.getByRole('button', { name: /discard/i }));
      await waitFor(() => screen.getByRole('alertdialog'));
      const cancelBtn = screen.getByRole('button', { name: /^cancel$/i });
      await fireEvent.click(cancelBtn);
      await waitFor(() => {
        expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
      });
      expect(discardModifiedValues).not.toHaveBeenCalled();
    });
  });

  describe('toolbar variant', () => {
    it('renders with toolbar=false as a standalone toolbar element', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls, { props: { toolbar: false } });
      await waitFor(() => {
        expect(screen.getByRole('toolbar', { name: /configuration save controls/i })).toBeInTheDocument();
      });
    });

    it('renders with toolbar=true as a group (flush in app toolbar)', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls, { props: { toolbar: true } });
      await waitFor(() => {
        expect(screen.getByRole('group', { name: /configuration save controls/i })).toBeInTheDocument();
      });
    });
  });

  // ── Read-only rejection (0x1083) ──────────────────────────────────────────
  // NOTE: Read-only rejection feedback was previously tested via direct
  // writeModifiedValues calls. Since the S2 three-phase flow delegates bus
  // writes to the backend, read-only rejection is now surfaced via Tauri
  // progress events and will be implemented in S3 (save progress dialog).

  // ── Offline vs online save routing ──────────────────────────────────────────

  describe('offline mode: save routes to onSave', () => {
    it('calls onSave (not writeModifiedValues) when offline with drafts', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      configChangesRef.draftCount = 2;
      offlineRef.draftCount = 2;
      offlineRef.draftRows = [
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n1' },
      ];

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalled();
      });
      expect(writeModifiedValues).not.toHaveBeenCalled();
    });

    it('shows "unsaved edit" wording (not "unsaved change") in offline mode', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      configChangesRef.draftCount = 1;
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/1 unsaved edit$/i)).toBeInTheDocument();
      });
    });

    it('shows plural "unsaved edits" for multiple offline drafts', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      configChangesRef.draftCount = 3;
      offlineRef.draftCount = 3;
      offlineRef.draftRows = [
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n2' },
      ];

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/3 unsaved edits/i)).toBeInTheDocument();
      });
    });

    it('delegates offline save to the page route (no direct reloadFromBackend or markClean)', async () => {
      // Post-Step 7 Option H: SaveControls is a thin delegate.
      // `offlineChangesStore.reloadFromBackend` and `layoutStore.markClean`
      // are now called by `saveLayoutOrchestrator` via the wired
      // `reloadOfflineChanges` / `markClean` callbacks, not by the component.
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      configChangesRef.draftCount = 1;
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalledTimes(1);
      });
      expect(offlineRef.reloadFromBackend).not.toHaveBeenCalled();
      expect(layoutRef.markClean).not.toHaveBeenCalled();
    });

    it('does not call configChangesStore.commitForSave directly after offline save (orchestrator owns it)', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      configChangesRef.draftCount = 1;
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalledTimes(1);
      });
      const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
      expect(configChangesStore.commitForSave).not.toHaveBeenCalled();
    });

    it('reverts to idle when onSave returns false (user cancelled)', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      configChangesRef.draftCount = 1;
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];

      const mockSave = vi.fn().mockResolvedValue(false);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalled();
      });
      // Should not have called post-save cleanup
      expect(offlineRef.reloadFromBackend).not.toHaveBeenCalled();
    });

    it('does not show unsaved edits for saved pending rows alone', () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      layoutRef.isDirty = false;
      offlineRef.draftCount = 0;
      (offlineRef as any).persistedRows = [{
        changeId: 'persisted-1',
        kind: 'config',
        nodeId: '05.02.01.02.03.00',
        space: 253,
        offset: '0x00000000',
        baselineValue: '1',
        plannedValue: '2',
        status: 'pending',
      }];

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      expect(screen.queryByText(/unsaved edit/i)).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument();
    });

    it('shows one unsaved edit when the offline layout is dirty without draft rows', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      layoutRef.isDirty = true;
      offlineRef.draftCount = 0;

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/1 unsaved edit$/i)).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /^save$/i })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /discard/i })).toBeInTheDocument();
      });
    });
  });

  describe('offline discard replay', () => {
    it('re-applies persisted pending rows to the tree after offline discard', async () => {
      const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');

      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      configChangesRef.draftCount = 1;
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];
      (offlineRef as any).persistedRows = [{
        changeId: 'persisted-1',
        kind: 'config',
        nodeId: '05.02.01.02.03.00',
        space: 253,
        offset: '0x00000000',
        baselineValue: '1',
        plannedValue: '2',
        status: 'pending',
      }];

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await fireEvent.click(await waitFor(() => screen.getByRole('button', { name: /discard/i })));
      await fireEvent.click(await waitFor(() => screen.getByRole('button', { name: /revert/i })));

      await waitFor(async () => {
        const { discardAllConfigDrafts } = await import('$lib/orchestration/configDraftOrchestrator');
        expect(discardAllConfigDrafts).toHaveBeenCalled();
      });
    });
  });

  describe('online mode: save routes to onSave (three-phase flow)', () => {
    it('calls onSave (not writeModifiedValues) when online with config edits', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      layoutRef.isOfflineMode = false;
      layoutRef.isConnected = true;
      treesRef.map.set('node1', makeDirtyTree(2));

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalled();
      });
      expect(writeModifiedValues).not.toHaveBeenCalled();
    });

    it('shows "unsaved change" wording (not "unsaved edit") in online mode', async () => {
      layoutRef.isOfflineMode = false;
      treesRef.map.set('node1', makeDirtyTree(2));

      render(SaveControls);

      await waitFor(() => {
        expect(screen.getByText(/2 unsaved changes/i)).toBeInTheDocument();
      });
    });

    it('does not count offline drafts in online mode', async () => {
      layoutRef.isOfflineMode = false;
      layoutRef.isConnected = true;
      layoutRef.hasLayoutFile = true;
      offlineRef.draftCount = 5; // should be ignored
      treesRef.map.set('node1', makeDirtyTree(1));

      render(SaveControls);

      await waitFor(() => {
        // Should show 1 (config edit) not 5 (offline drafts)
        expect(screen.getByText(/1 unsaved change$/i)).toBeInTheDocument();
      });
    });
  });

  // ── Online with layout open (key regression scenario) ───────────────────────

  describe('online with layout: config edits detected and routed to hardware', () => {
    beforeEach(() => {
      layoutRef.isOfflineMode = false;
      layoutRef.hasLayoutFile = true;
      layoutRef.isConnected = true;
    });

    it('shows Save button when config tree has modifiedValue leaves', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /^save$/i })).toBeInTheDocument();
      });
    });

    it('hides Save button when no config edits and no offline drafts', () => {
      render(SaveControls);
      expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
    });

    it('ignores offline drafts (isOfflineMode is false even with layout open)', async () => {
      offlineRef.draftCount = 3;
      offlineRef.draftRows = [
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n2' },
      ];
      // No config edits — Save button should NOT appear
      render(SaveControls);
      expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
    });

    it('routes Save to onSave (three-phase flow), not writeModifiedValues', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      treesRef.map.set('node1', makeDirtyTree(1));

      const mockOfflineSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockOfflineSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockOfflineSave).toHaveBeenCalled();
      });
      expect(writeModifiedValues).not.toHaveBeenCalled();
    });

    it('shows "unsaved change" wording (online terminology)', async () => {
      treesRef.map.set('node1', makeDirtyTree(2));
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByText(/2 unsaved changes/i)).toBeInTheDocument();
      });
    });

    it('counts only config tree dirty leaves for pending count', async () => {
      offlineRef.draftCount = 10; // should be ignored
      treesRef.map.set('node1', makeDirtyTree(2));
      render(SaveControls);
      await waitFor(() => {
        expect(screen.getByText(/2 unsaved changes/i)).toBeInTheDocument();
      });
    });

    it('online save delegates to onSave; commitForSave is owned by the orchestrator', async () => {
      const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
      treesRef.map.set('node1', makeDirtyTree(1));

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalledTimes(1);
      });
      expect(configChangesStore.commitForSave).not.toHaveBeenCalled();
    });

    it('does not invoke onSave again when the page route reports cancellation', async () => {
      treesRef.map.set('node1', makeDirtyTree(1));

      const mockSave = vi.fn().mockResolvedValue(false);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      expect(mockSave).toHaveBeenCalledTimes(1);
    });

    it('invokes onSave when layout metadata is dirty without reaching into store cleanup', async () => {
      // The component now trusts the page route to handle commit ordering;
      // the previous "commit-after-onSave" assertion is covered by the
      // orchestrator's wired callbacks (clearMetadata / clearPersistedDrafts).
      const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
      treesRef.map.set('node1', makeDirtyTree(1));
      metaRef.isDirty = true;
      metaRef.editCount = 1;

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalledTimes(1);
      });
      expect(configChangesStore.commitForSave).not.toHaveBeenCalled();
      expect(metaRef.clearAll).not.toHaveBeenCalled();
    });
  });

  // ── T061: layoutStore.isDirty enables Save (persisted offline revert) ──────

  describe('T061: Save appears when layoutStore.isDirty is set (persisted offline revert)', () => {
    it('shows Save button when layoutStore.isDirty is true even with no draft edits', async () => {
      // Simulates: offline mode, user reverted a persisted change so draftCount=0
      // but layoutStore.isDirty=true (set by the revert button handler)
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /^save$/i })).toBeInTheDocument();
      });
    });

    it('Save button is hidden when neither drafts nor isDirty', async () => {
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = false;

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
      });
    });

    it('calls onSave when Save is clicked with isDirty set', async () => {
      const mockSave = vi.fn().mockResolvedValue(true);
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalled();
      });
    });

    it('delegates to onSave for isDirty saves (markClean is owned by the orchestrator)', async () => {
      // Post-Step 7 Option H: `layoutStore.markClean` is now invoked by
      // `saveLayoutOrchestrator` via the wired `markClean` callback.
      const mockSave = vi.fn().mockResolvedValue(true);
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalledTimes(1);
      });
      expect(layoutRef.markClean).not.toHaveBeenCalled();
    });

    it('shows "1 unsaved edit" when only isDirty is set (no draft count)', async () => {
      // This is the persisted revert scenario: draftCount=0, isDirty=true
      // pendingEditCount should be 1 (not 0) so the label is meaningful
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/1 unsaved edit/i)).toBeInTheDocument();
      });
    });

    it('counts only config drafts and layout dirty in offline pending total', async () => {
      // Offline draft rows in offlineChangesStore are persistence staging;
      // the pending count uses configDraftCount (display layer) + layout dirty.
      layoutRef.isOfflineMode = true;
      configChangesRef.draftCount = 3;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onSave: vi.fn(), onSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/4 unsaved edits/i)).toBeInTheDocument();
      });
    });
  });

  // ── Step 7 Option H: SaveControls is a thin delegate ──────────────────────
  //
  // These tests pin the post-refactor contract: SaveControls honours the
  // presenter's `canSave` gate (no parallel gate), invokes `onSave` exactly
  // once, and does NOT reach into store cleanup. Cleanup ownership moved
  // into `saveLayoutOrchestrator` via wired callbacks
  // (`markClean`, `clearMetadata`, `clearPersistedDrafts`,
  // `reloadOfflineChanges`).
  //
  // The first test is a regression contract for the Save no-op: an empty
  // layout + connect + Read all config produces `unsavedInMemoryNodeIds`
  // (effectiveNodeStore) but zero config drafts and zero metadata edits.
  // The old parallel gate `if (!hasNodeEdits && !hasLayoutMetadataEdits) return`
  // never fired `onSave`. The presenter says `canSave: true` in this state.

  describe('Step 7 Option H: thin-delegate contract', () => {
    it('regression: invokes onSave when only unsavedInMemoryNodeIds are dirty (empty layout + Read all)', async () => {
      // No config drafts, no metadata, no layoutIsDirty — only unsaved-in-memory
      // node additions. Presenter must say canSave: true; handler must fire.
      configChangesRef.draftCount = 0;
      metaRef.isDirty = false;
      metaRef.editCount = 0;
      layoutRef.isDirty = false;
      layoutRef.isOfflineMode = false;
      effectiveNodeRef.unsavedInMemoryNodeIds = ['nodeA'];
      effectiveNodeRef.isDirty = true;

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      expect(saveBtn).not.toBeDisabled();
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalledTimes(1);
      });
    });

    it('cleanup parity: neither online nor offline mode performs duplicate component-side cleanup', async () => {
      // Same dirty signals; only mode differs. After a successful save the
      // component touches no store cleanup in either mode — the orchestrator
      // owns markClean / clearAll / commitForSave / reloadFromBackend.
      const runFor = async (offline: boolean) => {
        vi.clearAllMocks();
        treesRef.map = new Map();
        treesRef.map.set('nodeA', makeDirtyTree(1));
        layoutRef.isOfflineMode = offline;
        layoutRef.hasLayoutFile = offline;
        const mockSave = vi.fn().mockResolvedValue(true);
        const { unmount } = render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });
        const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
        await fireEvent.click(saveBtn);
        await waitFor(() => expect(mockSave).toHaveBeenCalledTimes(1));
        expect(layoutRef.markClean).not.toHaveBeenCalled();
        expect(metaRef.clearAll).not.toHaveBeenCalled();
        expect(offlineRef.reloadFromBackend).not.toHaveBeenCalled();
        const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
        expect(configChangesStore.commitForSave).not.toHaveBeenCalled();
        unmount();
      };
      await runFor(false);
      await runFor(true);
    });

    it('no duplicate cleanup: handleSave does not call markClean, clearAll, commitForSave, or reloadFromBackend directly', async () => {
      // Property test: when onSave succeeds, none of the store-cleanup
      // methods are invoked from inside the component. The orchestrator
      // (wired by +page.svelte's saveCurrentCaptureToFile) owns them.
      const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
      treesRef.map.set('nodeA', makeDirtyTree(1));
      metaRef.isDirty = true;
      metaRef.editCount = 1;
      layoutRef.isDirty = true;

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onSave: mockSave, onSaveAs: vi.fn() } });
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => expect(mockSave).toHaveBeenCalledTimes(1));
      expect(layoutRef.markClean).not.toHaveBeenCalled();
      expect(metaRef.clearAll).not.toHaveBeenCalled();
      expect(offlineRef.reloadFromBackend).not.toHaveBeenCalled();
      expect(configChangesStore.commitForSave).not.toHaveBeenCalled();
    });
  });
});
