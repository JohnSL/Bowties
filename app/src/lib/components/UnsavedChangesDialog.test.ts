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
    metadata: 0,
    channels: 0,
    facilities: 0,
    connectorSelections: 0,
    offlineDrafts: 0,
    offlineRevertedPersisted: 0,
    layoutStruct: 0,
    unsavedNewNodes: 0,
    unsavedRemovedNodes: 0,
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

describe('UnsavedChangesDialog — per-bucket lines', () => {
  it('renders the config bucket with node count', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ config: 3, configNodes: 2 }),
    });
    expect(screen.getByText('3 config edits across 2 nodes')).toBeInTheDocument();
  });

  it('uses singular forms for single-edit / single-node config', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ config: 1, configNodes: 1 }),
    });
    expect(screen.getByText('1 config edit across 1 node')).toBeInTheDocument();
  });

  it('renders the facilities bucket', () => {
    render(UnsavedChangesDialog, { props: baseProps({ facilities: 1 }) });
    expect(screen.getByText('1 facility edit')).toBeInTheDocument();
  });

  it('renders the channels bucket', () => {
    render(UnsavedChangesDialog, { props: baseProps({ channels: 2 }) });
    expect(screen.getByText('2 channel edits')).toBeInTheDocument();
  });

  it('renders the connectorSelections bucket', () => {
    render(UnsavedChangesDialog, { props: baseProps({ connectorSelections: 1 }) });
    expect(screen.getByText('1 connector selection change')).toBeInTheDocument();
  });

  it('renders the metadata, offline, and structural buckets', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({
        metadata: 1,
        offlineDrafts: 2,
        offlineRevertedPersisted: 1,
        layoutStruct: 1,
        unsavedNewNodes: 1,
        unsavedRemovedNodes: 2,
      }),
    });
    expect(screen.getByText('1 bowtie metadata edit')).toBeInTheDocument();
    expect(screen.getByText('2 offline drafts')).toBeInTheDocument();
    expect(screen.getByText('1 reverted persisted change')).toBeInTheDocument();
    expect(screen.getByText('layout structure edits')).toBeInTheDocument();
    expect(screen.getByText('1 new node not yet added to the layout'))
      .toBeInTheDocument();
    expect(screen.getByText('2 nodes removed but not yet saved'))
      .toBeInTheDocument();
  });

  it('suppresses zero-count buckets when mixed with non-zero', () => {
    render(UnsavedChangesDialog, {
      props: baseProps({ facilities: 1, channels: 2 }),
    });
    const list = screen.getByRole('list');
    expect(list.children).toHaveLength(2);
    expect(screen.getByText('1 facility edit')).toBeInTheDocument();
    expect(screen.getByText('2 channel edits')).toBeInTheDocument();
    expect(screen.queryByText(/config edit/)).not.toBeInTheDocument();
    expect(screen.queryByText(/metadata/)).not.toBeInTheDocument();
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
