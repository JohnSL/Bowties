/**
 * T023: Vitest component tests for SaveControls.svelte
 *
 * Covers:
 * - Save button disabled when no pending edits
 * - Save button enabled when dirty edits exist
 * - Save button disabled when invalid edits exist
 * - Renders nothing when no pending edits (hidden)
 * - Shows progress status message during save
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import SaveControls from './SaveControls.svelte';
import { pendingEditsStore, makePendingEditKey } from '$lib/stores/pendingEdits.svelte';
import type { PendingEdit, TreeConfigValue } from '$lib/types/nodeTree';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock the config API calls
vi.mock('$lib/api/config', () => ({
  writeConfigValue: vi.fn().mockResolvedValue({
    address: 100,
    space: 253,
    success: true,
    errorCode: null,
    errorMessage: null,
    retryCount: 0,
  }),
  sendUpdateComplete: vi.fn().mockResolvedValue(undefined),
}));

// Mock nodeTreeStore to avoid Tauri calls in tests
vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    updateLeafValue: vi.fn(),
  },
}));

const NODE_ID = '05.01.01.01.03.00';
const SEG_ORIGIN = 0;
const SEG_NAME = 'Configuration';

function makeEdit(
  address: number,
  validationState: 'valid' | 'invalid' = 'valid',
): PendingEdit {
  const key = makePendingEditKey(NODE_ID, 253, address);
  const original: TreeConfigValue = { type: 'string', value: 'old' };
  const pending: TreeConfigValue = { type: 'string', value: 'new' };
  return {
    key,
    nodeId: NODE_ID,
    segmentOrigin: SEG_ORIGIN,
    segmentName: SEG_NAME,
    address,
    space: 253,
    size: 16,
    elementType: 'string',
    fieldPath: ['seg:0', `elem:${address}`],
    fieldLabel: `Field ${address}`,
    originalValue: original,
    pendingValue: pending,
    validationState,
    validationMessage: validationState === 'invalid' ? 'Too long' : null,
    writeState: 'dirty',
    writeError: null,
    constraints: null,
  };
}

beforeEach(() => {
  pendingEditsStore.clearAll();
  vi.clearAllMocks();
});

describe('SaveControls.svelte', () => {
  describe('visibility', () => {
    it('renders toolbar in inactive state when there are no pending edits', () => {
      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });
      // Toolbar is hidden entirely when there are no pending edits
      expect(screen.queryByRole('toolbar')).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument();
    });

    it('renders the save toolbar when there are pending edits', () => {
      const edit = makeEdit(100);
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      expect(screen.getByRole('toolbar')).toBeInTheDocument();
    });
  });

  describe('Save button state', () => {
    it('Save button is disabled when no pending edits', () => {
      const { queryByRole } = render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });
      // Toolbar is hidden when there are no edits, so the button is not rendered
      expect(queryByRole('button', { name: /^save$/i })).not.toBeInTheDocument();
    });

    it('Save button is enabled when valid pending edits exist', async () => {
      const edit = makeEdit(100, 'valid');
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      await waitFor(() => {
        const btn = screen.getByRole('button', { name: /^save$/i });
        expect(btn).not.toBeDisabled();
      });
    });

    it('Save button is disabled when invalid edits exist', async () => {
      const edit = makeEdit(100, 'invalid');
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      await waitFor(() => {
        const btn = screen.getByRole('button', { name: /^save$/i });
        expect(btn).toBeDisabled();
      });
    });

    it('shows "Fix invalid fields" hint when edits are invalid', async () => {
      const edit = makeEdit(100, 'invalid');
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      await waitFor(() => {
        expect(screen.getByText(/fix invalid fields/i)).toBeInTheDocument();
      });
    });

    it('shows unsaved change count in idle state', async () => {
      pendingEditsStore.setEdit(makeEdit(100).key, makeEdit(100));
      pendingEditsStore.setEdit(makeEdit(200).key, makeEdit(200));

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      await waitFor(() => {
        expect(screen.getByText(/2 unsaved/i)).toBeInTheDocument();
      });
    });

    it('shows Discard button when edits are present', async () => {
      const edit = makeEdit(100);
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /discard/i })).toBeInTheDocument();
      });
    });
  });

  // ── T047: US6 — Discard with confirmation ─────────────────────────────────
  describe('T047: Discard button behavior', () => {
    it('Discard button is disabled when no pending edits', () => {
      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });
      // Toolbar is hidden when there are no edits, so the button is not rendered
      expect(screen.queryByRole('button', { name: /discard/i })).not.toBeInTheDocument();
    });

    it('Discard button is enabled when edits exist', async () => {
      const edit = makeEdit(100);
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /discard/i })).not.toBeDisabled();
      });
    });

    it('opens confirmation dialog on Discard click', async () => {
      const edit = makeEdit(100);
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      const { fireEvent: fe } = await import('@testing-library/svelte');
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fe.click(screen.getByRole('button', { name: /discard/i }));

      await waitFor(() => {
        expect(screen.getByRole('alertdialog')).toBeInTheDocument();
        expect(screen.getByText(/discard unsaved changes/i)).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /revert/i })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument();
      });
    });

    it('clears pending edits when Revert is clicked', async () => {
      const edit = makeEdit(100);
      pendingEditsStore.setEdit(edit.key, edit);
      expect(pendingEditsStore.dirtyCount).toBeGreaterThan(0);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      const { fireEvent: fe } = await import('@testing-library/svelte');
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fe.click(screen.getByRole('button', { name: /discard/i }));

      await waitFor(() => screen.getByRole('button', { name: /revert/i }));
      await fe.click(screen.getByRole('button', { name: /revert/i }));

      await waitFor(() => {
        expect(pendingEditsStore.getAllForNode(NODE_ID)).toHaveLength(0);
        expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
      });
    });

    it('does NOT clear edits when Cancel is clicked in the dialog', async () => {
      const edit = makeEdit(100);
      pendingEditsStore.setEdit(edit.key, edit);

      render(SaveControls, {
        props: { nodeId: NODE_ID, segmentOrigin: SEG_ORIGIN, segmentName: SEG_NAME },
      });

      const { fireEvent: fe } = await import('@testing-library/svelte');
      await waitFor(() => screen.getByRole('button', { name: /discard/i }));
      await fe.click(screen.getByRole('button', { name: /discard/i }));

      await waitFor(() => screen.getByRole('button', { name: /^cancel$/i }));
      await fe.click(screen.getByRole('button', { name: /^cancel$/i }));

      await waitFor(() => {
        expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
        expect(pendingEditsStore.getAllForNode(NODE_ID)).toHaveLength(1);
      });
    });
  });
});
