/**
 * T036: Vitest component tests for NodeEntry.svelte
 *
 * Verifies unsaved-changes badge behavior (FR-012a):
 * - When hasPendingEdits is true, a pending edits badge/indicator is visible
 * - When hasPendingEdits is false (default), no badge is shown
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import NodeEntry from './NodeEntry.svelte';

describe('NodeEntry.svelte – pending edits badge (T036)', () => {
  const baseProps = {
    nodeId: '05.01.01.01.03.00',
    nodeName: 'Test Node',
  };

  it('shows no pending edits badge by default', () => {
    render(NodeEntry, { props: baseProps });
    expect(screen.queryByTitle(/unsaved changes/i)).not.toBeInTheDocument();
  });

  it('shows pending edits indicator when hasPendingEdits is true', () => {
    render(NodeEntry, { props: { ...baseProps, hasPendingEdits: true } });
    expect(screen.getByTitle(/unsaved changes/i)).toBeInTheDocument();
  });

  it('does not show pending edits indicator when hasPendingEdits is false', () => {
    render(NodeEntry, { props: { ...baseProps, hasPendingEdits: false } });
    expect(screen.queryByTitle(/unsaved changes/i)).not.toBeInTheDocument();
  });

  it('shows pending edits badge alongside the node name', async () => {
    render(NodeEntry, { props: { ...baseProps, hasPendingEdits: true } });
    expect(screen.getByText('Test Node')).toBeInTheDocument();
    expect(screen.getByTitle(/unsaved changes/i)).toBeInTheDocument();
  });
});
