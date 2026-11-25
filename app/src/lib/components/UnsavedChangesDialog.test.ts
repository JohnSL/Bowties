/**
 * Vitest component tests for UnsavedChangesDialog.svelte
 *
 * Covers:
 * - Per-bucket count rendering with pluralisation
 * - Zero-count buckets are suppressed
 * - Mixed-bucket combination renders one line per non-zero bucket
 * - Cancel / Confirm callbacks fire on click
 * - Escape triggers Cancel
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import UnsavedChangesDialog from './UnsavedChangesDialog.svelte';
import type { DirtyBreakdown } from '$lib/layout';

function emptyBreakdown(): DirtyBreakdown {
  return {
    config: 0,
    configNodes: 0,
    configChanges: 0,
    metadata: 0,
    channels: 0,
    facilities: 0,
    connectorSelections: 0,
    pendingDrafts: 0,
    revertedPendingChanges: 0,
    layoutStruct: 0,
    unsavedNewNodes: 0,
    unsavedRemovedNodes: 0,
    waitingPendingChanges: 0,
  };
}

function baseProps(
  breakdown: Partial<DirtyBreakdown> = {},
  overrides: Partial<{
    message: string;
    confirmLabel: string;
    onConfirm: () => void;
    onCancel: () => void;
  }> = {},
) {
  return {
    message: 'You have unsaved changes. Continue?',
    breakdown: { ...emptyBreakdown(), ...breakdown },
    confirmLabel: 'Discard & Continue',
    onConfirm: vi.fn(),
    onCancel: vi.fn(),
    ...overrides,
  };
}

describe('UnsavedChangesDialog — rendering', () => {
  it('renders the message text', () => {
    render(UnsavedChangesDialog, { props: baseProps({ facilities: 1 }) });
    expect(screen.getByText(/you have unsaved changes\. continue\?/i))
      .toBeInTheDocument();
  });

  it('renders the configurable confirm label', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ facilities: 1 }, { confirmLabel: 'Exit Without Saving' }),
    });
    expect(screen.getByRole('button', { name: /exit without saving/i }))
      .toBeInTheDocument();
  });

  it('renders no breakdown list when every bucket is zero', () => {
    render(UnsavedChangesDialog, { props: baseProps() });
    expect(screen.queryByRole('list')).not.toBeInTheDocument();
  });
});

describe('UnsavedChangesDialog — lines', () => {
  it('shows configuration with a count', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ config: 3, configNodes: 2, configChanges: 3 }),
    });
    expect(screen.getByText('3 configuration changes')).toBeInTheDocument();
  });

  it('uses the singular for one configuration change', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ config: 1, configNodes: 1, configChanges: 1 }),
    });
    expect(screen.getByText('1 configuration change')).toBeInTheDocument();
  });

  it('counts a draft row with no config draft as configuration (distinct count from the aggregate)', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ pendingDrafts: 2, revertedPendingChanges: 1, configChanges: 3 }),
    });
    expect(screen.getByText('3 configuration changes')).toBeInTheDocument();
    expect(screen.getAllByRole('listitem')).toHaveLength(1);
  });

  it('lists other edits as plain lines without numbers', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({
        metadata: 2,
        facilities: 3,
        channels: 2,
        connectorSelections: 1,
        layoutStruct: 1,
        unsavedNewNodes: 4,
        unsavedRemovedNodes: 2,
      }),
    });
    expect(screen.getByText('Bowtie names')).toBeInTheDocument();
    expect(screen.getByText('Facility edits')).toBeInTheDocument();
    expect(screen.getByText('Channel edits')).toBeInTheDocument();
    expect(screen.getByText('Connector selections')).toBeInTheDocument();
    expect(screen.getByText('Layout structure edits')).toBeInTheDocument();
    expect(screen.getByText('New nodes not yet added to the layout')).toBeInTheDocument();
    expect(screen.getByText('Removed nodes not yet saved')).toBeInTheDocument();
  });

  it('lists the rows waiting for the board with a count', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ facilities: 1, waitingPendingChanges: 2 }),
    });
    expect(screen.getByText('Facility edits')).toBeInTheDocument();
    expect(screen.getByText('2 waiting to be written to the board')).toBeInTheDocument();
  });

  it('suppresses zero-count buckets when mixed with non-zero', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ facilities: 1, channels: 2 }),
    });
    const list = screen.getByRole('list');
    expect(list.children).toHaveLength(2);
    expect(screen.queryByText(/configuration/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Bowtie names/)).not.toBeInTheDocument();
  });
});

describe('UnsavedChangesDialog — interactions', () => {
  it('fires onCancel when Cancel is clicked', async () => {
    const onCancel = vi.fn();
    render(UnsavedChangesDialog, {
      props: baseProps({ facilities: 1 }, { onCancel }),
    });
    await fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('fires onConfirm when the confirm button is clicked', async () => {
    const onConfirm = vi.fn();
    render(UnsavedChangesDialog, {
      props: baseProps({ facilities: 1 }, { onConfirm }),
    });
    await fireEvent.click(
      screen.getByRole('button', { name: /discard & continue/i }),
    );
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it('fires onCancel when Escape is pressed', async () => {
    const onCancel = vi.fn();
    render(UnsavedChangesDialog, {
      props: baseProps({ facilities: 1 }, { onCancel }),
    });
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
