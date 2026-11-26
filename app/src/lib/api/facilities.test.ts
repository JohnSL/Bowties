import { describe, it, expect, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

const { composeFacilityRemoval } = await import('$lib/api/facilities');

describe('composeFacilityRemoval', () => {
  it('asks the facility removal command for the facility and returns its claims', async () => {
    invokeMock.mockReset();
    const response = {
      edits: [],
      logicPlacement: [],
      issues: [],
      claims: [{ nodeKey: '05010101FF02', kind: 'directLamp', index: 2, lampSelection: null }],
    };
    invokeMock.mockResolvedValue(response);
    await expect(composeFacilityRemoval('pnl:yard')).resolves.toEqual(response);
    expect(invokeMock).toHaveBeenCalledWith('compose_facility_removal', { facilityId: 'pnl:yard' });
  });
});
