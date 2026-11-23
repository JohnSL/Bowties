import { describe, it, expect, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

const { composeBlockSignalsRemoval } = await import('$lib/api/signaling');

describe('composeBlockSignalsRemoval', () => {
  it('asks the compose command to release the facility claimed slots', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({ edits: [], claimedSlots: [], issues: [] });
    await composeBlockSignalsRemoval('abs:main');
    expect(invokeMock).toHaveBeenCalledWith('compose_block_signals', {
      request: { facilityId: 'abs:main', remove: true },
    });
  });
});
