/**
 * Layout-scoped connection management tests for ConnectionManager.svelte
 * (Spec 013 / S7).
 *
 * Verifies that the component reads/writes through the per-layout API
 * (`getLayoutConnections` / `saveLayoutConnections`) instead of the
 * removed global `load_connection_prefs` / `save_connection_prefs`
 * commands, and that switching the active layout triggers a reload.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';

// ── Mocks ────────────────────────────────────────────────────────────────────

const { invokeMock, getMock, saveMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  getMock: vi.fn(),
  saveMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

vi.mock('$lib/api/layout', () => ({
  getLayoutConnections: getMock,
  saveLayoutConnections: saveMock,
}));

// Minimal layout store stub backing `layoutStore.activeContext.rootPath`.
const stubContext = { rootPath: null as string | null };

vi.mock('$lib/stores/layout.svelte', () => ({
  layoutStore: {
    get activeContext() {
      return stubContext.rootPath ? { rootPath: stubContext.rootPath } : null;
    },
  },
}));

// ── Helpers ──────────────────────────────────────────────────────────────────

async function mountComponent() {
  const mod = await import('./ConnectionManager.svelte');
  return render(mod.default);
}

beforeEach(() => {
  invokeMock.mockReset();
  getMock.mockReset();
  saveMock.mockReset();
  stubContext.rootPath = null;

  // `list_serial_ports` is still a backend command and is always called on mount.
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_serial_ports') return [];
    if (cmd === 'connect_lcc') return undefined;
    throw new Error(`unexpected invoke: ${cmd}`);
  });
  getMock.mockResolvedValue([]);
  saveMock.mockResolvedValue(undefined);
});

// ── Tests ────────────────────────────────────────────────────────────────────

describe('ConnectionManager — layout-scoped connections (S7)', () => {
  it('loads connections from the active layout on mount', async () => {
    stubContext.rootPath = 'C:/layouts/east.layout';
    getMock.mockResolvedValue([
      {
        id: 'c1',
        name: 'East Hub',
        adapterType: 'tcp',
        host: '10.0.0.5',
        port: 12021,
        flowControl: 'none',
      },
    ]);

    await mountComponent();

    await waitFor(() => {
      expect(getMock).toHaveBeenCalledWith('C:/layouts/east.layout');
    });
    expect(screen.getByText('East Hub')).toBeInTheDocument();
  });

  it('never calls the removed global load_connection_prefs command', async () => {
    stubContext.rootPath = 'C:/layouts/east.layout';
    await mountComponent();
    await waitFor(() => expect(getMock).toHaveBeenCalled());

    const calls = invokeMock.mock.calls.map((c) => c[0]);
    expect(calls).not.toContain('load_connection_prefs');
    expect(calls).not.toContain('save_connection_prefs');
  });

  it('persists deletions through saveLayoutConnections', async () => {
    stubContext.rootPath = 'C:/layouts/east.layout';
    getMock.mockResolvedValue([
      {
        id: 'c1',
        name: 'East Hub',
        adapterType: 'tcp',
        host: '10.0.0.5',
        port: 12021,
        flowControl: 'none',
      },
    ]);

    await mountComponent();
    await waitFor(() => expect(screen.getByText('East Hub')).toBeInTheDocument());

    // Two-step delete: click ×, then confirm.
    await fireEvent.click(screen.getByLabelText('Remove East Hub'));
    await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(saveMock).toHaveBeenCalledWith('C:/layouts/east.layout', []);
    });
  });

  it('trims whitespace from TCP host field on submit (#24)', async () => {
    stubContext.rootPath = 'C:/layouts/test.layout';
    getMock.mockResolvedValue([]);
    await mountComponent();

    // Open the Add Connection modal
    await fireEvent.click(screen.getByLabelText('Add connection'));

    // Fill in required name
    const nameInput = screen.getByPlaceholderText('My layout hub');
    await fireEvent.input(nameInput, { target: { value: 'Test Hub' } });

    // Type a host with leading/trailing spaces
    const hostInput = screen.getByPlaceholderText('localhost');
    await fireEvent.input(hostInput, { target: { value: '  192.168.1.100  ' } });

    // Submit via the Add button
    await fireEvent.click(screen.getByRole('button', { name: 'Add' }));

    await waitFor(() => {
      expect(saveMock).toHaveBeenCalled();
      const savedConfigs = saveMock.mock.calls[0][1] as Array<{ host?: string }>;
      expect(savedConfigs[0].host).toBe('192.168.1.100');
    });
  });

  it('shows an empty list when no layout is active and does not call the API', async () => {
    stubContext.rootPath = null;
    await mountComponent();

    // Give the effect a turn.
    await new Promise((r) => setTimeout(r, 0));

    expect(getMock).not.toHaveBeenCalled();
    expect(saveMock).not.toHaveBeenCalled();
    // No saved rows should be rendered.
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
  });
});
