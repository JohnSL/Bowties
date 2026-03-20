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
const { treesRef, metaRef } = vi.hoisted(() => ({
  treesRef: { map: new Map<string, NodeConfigTree>() },
  metaRef: { isDirty: false, editCount: 0, clearAll: vi.fn() },
}));

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    get trees() { return treesRef.map; },
  },
}));

vi.mock('$lib/stores/bowtieMetadata.svelte', () => ({
  bowtieMetadataStore: metaRef,
}));

vi.mock('$lib/api/config', () => ({
  writeModifiedValues: vi.fn().mockResolvedValue({ succeeded: 1, failed: 0 }),
  discardModifiedValues: vi.fn().mockResolvedValue(0),
}));

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: {
    saveCurrentLayout: vi.fn().mockResolvedValue(undefined),
    saveLayoutAs: vi.fn().mockResolvedValue(undefined),
    revertToSaved: vi.fn(),
  },
}));

vi.mock('$lib/stores/nodeInfo', () => ({
  updateNodeSnipField: vi.fn(),
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
});
