/**
 * T037: Vitest component tests for SegmentEntry.svelte
 *
 * Verifies unsaved-changes badge behavior (FR-012b):
 * - When hasPendingEdits is true, a pending edits indicator is visible
 * - When hasPendingEdits is false (default), no indicator is shown
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import SegmentEntry from './SegmentEntry.svelte';

describe('SegmentEntry.svelte – pending edits indicator (T037)', () => {
  const baseProps = {
    segmentId: 'seg:0',
    segmentName: 'Configuration',
  };

  it('shows no pending edits indicator by default', () => {
    render(SegmentEntry, { props: baseProps });
    expect(screen.queryByTitle(/unsaved changes/i)).not.toBeInTheDocument();
  });

  it('shows pending edits indicator when hasPendingEdits is true', () => {
    render(SegmentEntry, { props: { ...baseProps, hasPendingEdits: true } });
    expect(screen.getByTitle(/unsaved changes/i)).toBeInTheDocument();
  });

  it('does not show pending edits indicator when hasPendingEdits is false', () => {
    render(SegmentEntry, { props: { ...baseProps, hasPendingEdits: false } });
    expect(screen.queryByTitle(/unsaved changes/i)).not.toBeInTheDocument();
  });

  it('shows indicator alongside the segment name', () => {
    render(SegmentEntry, { props: { ...baseProps, hasPendingEdits: true } });
    expect(screen.getByText('Configuration')).toBeInTheDocument();
    expect(screen.getByTitle(/unsaved changes/i)).toBeInTheDocument();
  });
});
