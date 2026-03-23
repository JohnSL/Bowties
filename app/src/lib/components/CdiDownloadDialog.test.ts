/**
 * Vitest component tests for CdiDownloadDialog.svelte
 *
 * Covers:
 * - Renders node names and IDs for all missing nodes
 * - Shows/hides the "Downloading…" status text based on `downloading` prop
 * - Disables Cancel and Download buttons while downloading
 * - Per-node downloadStatus indicators: spinner, ✓, ✗, none
 * - onDownload callback is invoked when Download is clicked
 * - onCancel callback is invoked when Cancel is clicked
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import CdiDownloadDialog from './CdiDownloadDialog.svelte';
import type { MissingCdiNode } from './CdiDownloadDialog.svelte';

const NODES: MissingCdiNode[] = [
  { nodeId: '05.01.01.01.03.00', nodeName: 'Alpha Node' },
  { nodeId: '05.01.01.01.03.01', nodeName: 'Beta Node' },
];

function baseProps(overrides: Partial<{
  nodes: MissingCdiNode[];
  downloading: boolean;
  downloadedCount: number;
  onDownload: () => void;
  onCancel: () => void;
}> = {}) {
  return {
    nodes: NODES,
    downloading: false,
    downloadedCount: 0,
    onDownload: vi.fn(),
    onCancel: vi.fn(),
    ...overrides,
  };
}

describe('CdiDownloadDialog.svelte – node list rendering', () => {
  it('renders node names', () => {
    render(CdiDownloadDialog, { props: baseProps() });
    expect(screen.getByText('Alpha Node')).toBeInTheDocument();
    expect(screen.getByText('Beta Node')).toBeInTheDocument();
  });

  it('renders node IDs', () => {
    render(CdiDownloadDialog, { props: baseProps() });
    expect(screen.getByText('05.01.01.01.03.00')).toBeInTheDocument();
    expect(screen.getByText('05.01.01.01.03.01')).toBeInTheDocument();
  });
});

describe('CdiDownloadDialog.svelte – download status text', () => {
  it('hides status text when not downloading', () => {
    render(CdiDownloadDialog, { props: baseProps({ downloading: false }) });
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('shows status text while downloading', () => {
    render(CdiDownloadDialog, {
      props: baseProps({ downloading: true, downloadedCount: 1 }),
    });
    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.getByRole('status').textContent).toMatch(/1 of 2/);
  });
});

describe('CdiDownloadDialog.svelte – button states', () => {
  it('enables buttons when not downloading', () => {
    render(CdiDownloadDialog, { props: baseProps({ downloading: false }) });
    expect(screen.getByRole('button', { name: /download/i })).not.toBeDisabled();
    expect(screen.getByRole('button', { name: /cancel/i })).not.toBeDisabled();
  });

  it('disables both buttons while downloading', () => {
    render(CdiDownloadDialog, { props: baseProps({ downloading: true, downloadedCount: 0 }) });
    expect(screen.getByRole('button', { name: /downloading/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /cancel/i })).toBeDisabled();
  });

  it('calls onDownload when Download is clicked', async () => {
    const onDownload = vi.fn();
    render(CdiDownloadDialog, { props: baseProps({ onDownload }) });
    await fireEvent.click(screen.getByRole('button', { name: /download/i }));
    expect(onDownload).toHaveBeenCalledOnce();
  });

  it('calls onCancel when Cancel is clicked', async () => {
    const onCancel = vi.fn();
    render(CdiDownloadDialog, { props: baseProps({ onCancel }) });
    await fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});

describe('CdiDownloadDialog.svelte – per-node downloadStatus indicators', () => {
  it('shows no indicator when downloadStatus is absent', () => {
    render(CdiDownloadDialog, { props: baseProps() });
    expect(screen.queryByLabelText(/downloading/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/downloaded/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/failed/i)).not.toBeInTheDocument();
  });

  it('shows a spinner for a node with downloadStatus "downloading"', () => {
    const nodes: MissingCdiNode[] = [
      { nodeId: '05.01.01.01.03.00', nodeName: 'Alpha Node', downloadStatus: 'downloading' },
    ];
    render(CdiDownloadDialog, { props: baseProps({ nodes, downloading: true, downloadedCount: 0 }) });
    expect(screen.getByLabelText(/downloading/i)).toBeInTheDocument();
  });

  it('shows a ✓ badge for a node with downloadStatus "done"', () => {
    const nodes: MissingCdiNode[] = [
      { nodeId: '05.01.01.01.03.00', nodeName: 'Alpha Node', downloadStatus: 'done' },
    ];
    render(CdiDownloadDialog, { props: baseProps({ nodes }) });
    expect(screen.getByLabelText(/downloaded/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/downloaded/i).textContent).toBe('✓');
  });

  it('shows a ✗ badge for a node with downloadStatus "failed"', () => {
    const nodes: MissingCdiNode[] = [
      { nodeId: '05.01.01.01.03.00', nodeName: 'Alpha Node', downloadStatus: 'failed' },
    ];
    render(CdiDownloadDialog, { props: baseProps({ nodes }) });
    expect(screen.getByLabelText(/failed/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/failed/i).textContent).toBe('✗');
  });

  it('shows mixed statuses correctly across multiple nodes', () => {
    const nodes: MissingCdiNode[] = [
      { nodeId: '05.01.01.01.03.00', nodeName: 'Alpha Node', downloadStatus: 'done' },
      { nodeId: '05.01.01.01.03.01', nodeName: 'Beta Node', downloadStatus: 'failed' },
    ];
    render(CdiDownloadDialog, { props: baseProps({ nodes }) });
    expect(screen.getByLabelText(/downloaded/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/failed/i)).toBeInTheDocument();
  });
});
