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
const { treesRef, metaRef, layoutRef, offlineRef } = vi.hoisted(() => ({
  treesRef: { map: new Map<string, NodeConfigTree>() },
  metaRef: { isDirty: false, editCount: 0, clearAll: vi.fn() },
  layoutRef: {
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
  offlineRef: {
    draftCount: 0,
    draftRows: [] as any[],
    pendingCount: 0,
    reloadFromBackend: vi.fn().mockResolvedValue(undefined) as any,
    revertAllPending: vi.fn().mockResolvedValue(undefined) as any,
    flushPendingToBackend: vi.fn().mockResolvedValue(0) as any,
    clear: vi.fn(),
  },
}));

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() { return treesRef.map; },
    clearAllModifiedValues: vi.fn(),
    applyOfflinePendingValues: vi.fn(),
  },
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

/** Build a minimal NodeConfigTree with `count` leaves that each have a modifiedValue. */
function makeDirtyTree(count = 1): NodeConfigTree {
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
  metaRef.isDirty = false;
  metaRef.editCount = 0;
  layoutRef.isOfflineMode = false;
  layoutRef.hasLayoutFile = false;
  layoutRef.isConnected = false;
  layoutRef.isLoaded = false;
  layoutRef.isDirty = false;
  offlineRef.draftCount = 0;
  offlineRef.draftRows = [];
  offlineRef.pendingCount = 0;
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

    it('calls writeModifiedValues when Save is clicked', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      expect(writeModifiedValues).toHaveBeenCalled();
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
      render(SaveControls);
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fireEvent.click(screen.getByRole('button', { name: /discard/i }));
      const revertBtn = await waitFor(() => screen.getByRole('button', { name: /revert/i }));
      await fireEvent.click(revertBtn);
      expect(discardModifiedValues).toHaveBeenCalled();
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
  describe('read-only rejection', () => {
    it('calls toast.push when readOnlyRejected > 0', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      const { toast } = await import('@zerodevx/svelte-toast');
      vi.mocked(writeModifiedValues).mockResolvedValueOnce({
        total: 2,
        succeeded: 0,
        failed: 0,
        readOnlyRejected: 2,
      });
      treesRef.map.set('node1', makeDirtyTree(2));
      render(SaveControls);
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      await waitFor(() => {
        expect(toast.push).toHaveBeenCalledWith(
          expect.stringMatching(/2 read-only fields reverted/i),
          expect.objectContaining({ classes: ['warn'] })
        );
      });
    });

    it('uses singular wording for exactly one read-only field', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      const { toast } = await import('@zerodevx/svelte-toast');
      vi.mocked(writeModifiedValues).mockResolvedValueOnce({
        total: 1,
        succeeded: 0,
        failed: 0,
        readOnlyRejected: 1,
      });
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      await waitFor(() => {
        expect(toast.push).toHaveBeenCalledWith(
          expect.stringMatching(/1 read-only field reverted/i),
          expect.objectContaining({ classes: ['warn'] })
        );
      });
    });

    it('does not call toast.push when readOnlyRejected is 0', async () => {
      const { toast } = await import('@zerodevx/svelte-toast');
      treesRef.map.set('node1', makeDirtyTree(1));
      render(SaveControls);
      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);
      await waitFor(() => screen.getByText(/✓ Saved/i));
      expect(toast.push).not.toHaveBeenCalled();
    });
  });

  // ── Offline vs online save routing ──────────────────────────────────────────

  describe('offline mode: save routes to onOfflineSave', () => {
    it('calls onOfflineSave (not writeModifiedValues) when offline with drafts', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      offlineRef.draftCount = 2;
      offlineRef.draftRows = [
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n1' },
      ];

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onOfflineSave: mockSave, onOfflineSaveAs: vi.fn() } });

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
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/1 unsaved edit$/i)).toBeInTheDocument();
      });
    });

    it('shows plural "unsaved edits" for multiple offline drafts', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      offlineRef.draftCount = 3;
      offlineRef.draftRows = [
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n1' },
        { status: 'pending', nodeId: 'n2' },
      ];

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/3 unsaved edits/i)).toBeInTheDocument();
      });
    });

    it('reloads offline store and clears trees after successful save', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onOfflineSave: mockSave, onOfflineSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(offlineRef.reloadFromBackend).toHaveBeenCalled();
        expect(layoutRef.markClean).toHaveBeenCalled();
      });
    });

    it('re-applies persisted pending rows to the tree after successful offline save', async () => {
      const { nodeTreeStore } = await import('$lib/stores/nodeTree.svelte');

      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
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

      const mockSave = vi.fn().mockResolvedValue(true);
      render(SaveControls, { props: { onOfflineSave: mockSave, onOfflineSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(nodeTreeStore.applyOfflinePendingValues).toHaveBeenCalledWith((offlineRef as any).persistedRows);
      });
    });

    it('reverts to idle when onOfflineSave returns false (user cancelled)', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      offlineRef.draftCount = 1;
      offlineRef.draftRows = [{ status: 'pending', nodeId: 'n1' }];

      const mockSave = vi.fn().mockResolvedValue(false);
      render(SaveControls, { props: { onOfflineSave: mockSave, onOfflineSaveAs: vi.fn() } });

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

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      expect(screen.queryByText(/unsaved edit/i)).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument();
    });

    it('shows one unsaved edit when the offline layout is dirty without draft rows', async () => {
      layoutRef.isOfflineMode = true;
      layoutRef.hasLayoutFile = true;
      layoutRef.isDirty = true;
      offlineRef.draftCount = 0;

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

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

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      await fireEvent.click(await waitFor(() => screen.getByRole('button', { name: /discard/i })));
      await fireEvent.click(await waitFor(() => screen.getByRole('button', { name: /revert/i })));

      await waitFor(() => {
        expect(nodeTreeStore.applyOfflinePendingValues).toHaveBeenCalledWith((offlineRef as any).persistedRows);
      });
    });
  });

  describe('online mode: save routes to writeModifiedValues', () => {
    it('calls writeModifiedValues (not onOfflineSave) when online with config edits', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      layoutRef.isOfflineMode = false;
      layoutRef.isConnected = true;
      treesRef.map.set('node1', makeDirtyTree(2));

      const mockSave = vi.fn();
      render(SaveControls, { props: { onOfflineSave: mockSave, onOfflineSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(writeModifiedValues).toHaveBeenCalled();
      });
      expect(mockSave).not.toHaveBeenCalled();
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

    it('routes Save to writeModifiedValues, not onOfflineSave', async () => {
      const { writeModifiedValues } = await import('$lib/api/config');
      treesRef.map.set('node1', makeDirtyTree(1));

      const mockOfflineSave = vi.fn();
      render(SaveControls, { props: { onOfflineSave: mockOfflineSave, onOfflineSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(writeModifiedValues).toHaveBeenCalled();
      });
      expect(mockOfflineSave).not.toHaveBeenCalled();
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
  });

  // ── T061: layoutStore.isDirty enables Save (persisted offline revert) ──────

  describe('T061: Save appears when layoutStore.isDirty is set (persisted offline revert)', () => {
    it('shows Save button when layoutStore.isDirty is true even with no draft edits', async () => {
      // Simulates: offline mode, user reverted a persisted change so draftCount=0
      // but layoutStore.isDirty=true (set by the revert button handler)
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /^save$/i })).toBeInTheDocument();
      });
    });

    it('Save button is hidden when neither drafts nor isDirty', async () => {
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = false;

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
      });
    });

    it('calls onOfflineSave when Save is clicked with isDirty set', async () => {
      const mockSave = vi.fn().mockResolvedValue(true);
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onOfflineSave: mockSave, onOfflineSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(mockSave).toHaveBeenCalled();
      });
    });

    it('markClean is called after successful isDirty save', async () => {
      const mockSave = vi.fn().mockResolvedValue(true);
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onOfflineSave: mockSave, onOfflineSaveAs: vi.fn() } });

      const saveBtn = await waitFor(() => screen.getByRole('button', { name: /^save$/i }));
      await fireEvent.click(saveBtn);

      await waitFor(() => {
        expect(layoutRef.markClean).toHaveBeenCalled();
      });
    });

    it('shows "1 unsaved edit" when only isDirty is set (no draft count)', async () => {
      // This is the persisted revert scenario: draftCount=0, isDirty=true
      // pendingEditCount should be 1 (not 0) so the label is meaningful
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 0;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/1 unsaved edit/i)).toBeInTheDocument();
      });
    });

    it('shows draftCount only (not draftCount+1) when drafts are present alongside isDirty', async () => {
      // If there are draft edits AND isDirty, show only draftCount — the dirty
      // flag is implicit in the drafts themselves, not an extra change to count
      layoutRef.isOfflineMode = true;
      offlineRef.draftCount = 3;
      layoutRef.isDirty = true;

      render(SaveControls, { props: { onOfflineSave: vi.fn(), onOfflineSaveAs: vi.fn() } });

      await waitFor(() => {
        expect(screen.getByText(/3 unsaved edits/i)).toBeInTheDocument();
      });
    });
  });
});
