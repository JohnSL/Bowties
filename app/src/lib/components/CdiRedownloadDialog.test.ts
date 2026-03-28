/**
 * Vitest component tests for CdiRedownloadDialog.svelte
 *
 * Covers:
 * - Renders node name and ID
 * - Shows a spinner (downloading indicator) while downloading
 * - Shows a Cancel button while downloading
 * - Clicking Cancel calls cancelCdiDownload and transitions to "Cancelling…"
 * - Transitions to done state after a successful download
 * - Calls onClose ~800ms after a successful download (fake timers)
 * - Transitions to failed state and shows error message on download failure
 * - Retry button triggers a second download attempt
 * - Close button calls onClose in the failed state
 * - Calls onClose immediately when the download error indicates cancellation
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import CdiRedownloadDialog from './CdiRedownloadDialog.svelte';

// ── Hoisted mock refs ─────────────────────────────────────────────────────────

const { downloadCdiMock, cancelCdiDownloadMock } = vi.hoisted(() => ({
  downloadCdiMock: vi.fn(),
  cancelCdiDownloadMock: vi.fn(),
}));

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('$lib/api/cdi', () => ({
  downloadCdi: downloadCdiMock,
  cancelCdiDownload: cancelCdiDownloadMock,
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

function baseProps(overrides: Partial<{
  nodeId: string;
  nodeName: string;
  onClose: () => void;
}> = {}) {
  return {
    nodeId: '05.01.01.01.03.00',
    nodeName: 'Test Node',
    onClose: vi.fn(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('CdiRedownloadDialog.svelte – rendering', () => {
  it('renders the node ID in the node list', () => {
    downloadCdiMock.mockReturnValue(new Promise(() => {}));
    render(CdiRedownloadDialog, { props: baseProps() });
    expect(screen.getByText('05.01.01.01.03.00')).toBeInTheDocument();
  });

  it('renders the node name at least once (list row + body text)', () => {
    downloadCdiMock.mockReturnValue(new Promise(() => {}));
    render(CdiRedownloadDialog, { props: baseProps() });
    expect(screen.getAllByText('Test Node').length).toBeGreaterThan(0);
  });
});

describe('CdiRedownloadDialog.svelte – downloading state', () => {
  it('shows a downloading spinner immediately', () => {
    downloadCdiMock.mockReturnValue(new Promise(() => {}));
    render(CdiRedownloadDialog, { props: baseProps() });
    expect(screen.getByLabelText(/downloading/i)).toBeInTheDocument();
  });

  it('shows a Cancel button while downloading', () => {
    downloadCdiMock.mockReturnValue(new Promise(() => {}));
    render(CdiRedownloadDialog, { props: baseProps() });
    expect(screen.getByRole('button', { name: /^cancel$/i })).toBeInTheDocument();
  });

  it('clicking Cancel calls cancelCdiDownload', async () => {
    downloadCdiMock.mockReturnValue(new Promise(() => {}));
    cancelCdiDownloadMock.mockResolvedValue(undefined);
    render(CdiRedownloadDialog, { props: baseProps() });
    await fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    expect(cancelCdiDownloadMock).toHaveBeenCalledOnce();
  });

  it('Cancel button shows "Cancelling…" and becomes disabled after click', async () => {
    downloadCdiMock.mockReturnValue(new Promise(() => {}));
    cancelCdiDownloadMock.mockResolvedValue(undefined);
    render(CdiRedownloadDialog, { props: baseProps() });
    await fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /cancelling/i })).toBeDisabled();
    });
  });
});

describe('CdiRedownloadDialog.svelte – success state', () => {
  it('shows the downloaded indicator after a successful download', async () => {
    downloadCdiMock.mockResolvedValue(undefined);
    render(CdiRedownloadDialog, { props: baseProps() });
    await waitFor(() => {
      expect(screen.getByLabelText(/downloaded/i)).toBeInTheDocument();
    });
  });

  it('shows the success body text after a successful download', async () => {
    downloadCdiMock.mockResolvedValue(undefined);
    render(CdiRedownloadDialog, { props: baseProps() });
    await waitFor(() => {
      expect(screen.getByText(/downloaded successfully/i)).toBeInTheDocument();
    });
  });

  it('calls onClose ~800ms after a successful download', async () => {
    vi.useFakeTimers();
    downloadCdiMock.mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(CdiRedownloadDialog, { props: baseProps({ onClose }) });
    await vi.waitFor(() => expect(screen.getByLabelText(/downloaded/i)).toBeInTheDocument());
    expect(onClose).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(800);
    expect(onClose).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });
});

describe('CdiRedownloadDialog.svelte – failure state', () => {
  it('shows the failed indicator after a download error', async () => {
    downloadCdiMock.mockRejectedValue(new Error('Connection lost'));
    render(CdiRedownloadDialog, { props: baseProps() });
    await waitFor(() => {
      expect(screen.getByLabelText(/failed/i)).toBeInTheDocument();
    });
  });

  it('shows the error message in an alert after a download error', async () => {
    downloadCdiMock.mockRejectedValue(new Error('Connection lost'));
    render(CdiRedownloadDialog, { props: baseProps() });
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });
  });

  it('shows Retry and Close buttons in the failed state', async () => {
    downloadCdiMock.mockRejectedValue(new Error('Connection lost'));
    render(CdiRedownloadDialog, { props: baseProps() });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /close/i })).toBeInTheDocument();
    });
  });

  it('Retry button triggers a second download attempt', async () => {
    downloadCdiMock
      .mockRejectedValueOnce(new Error('Connection lost'))
      .mockReturnValue(new Promise(() => {})); // stays loading on retry
    render(CdiRedownloadDialog, { props: baseProps() });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
    });
    await fireEvent.click(screen.getByRole('button', { name: /retry/i }));
    expect(downloadCdiMock).toHaveBeenCalledTimes(2);
  });

  it('Close button calls onClose in the failed state', async () => {
    downloadCdiMock.mockRejectedValue(new Error('Connection lost'));
    const onClose = vi.fn();
    render(CdiRedownloadDialog, { props: baseProps({ onClose }) });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /close/i })).toBeInTheDocument();
    });
    await fireEvent.click(screen.getByRole('button', { name: /close/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});

describe('CdiRedownloadDialog.svelte – cancellation flow', () => {
  it('calls onClose immediately when the download error is a cancellation', async () => {
    downloadCdiMock.mockRejectedValue(new Error('CDI download cancelled'));
    const onClose = vi.fn();
    render(CdiRedownloadDialog, { props: baseProps({ onClose }) });
    await waitFor(() => {
      expect(onClose).toHaveBeenCalledOnce();
    });
  });
});
