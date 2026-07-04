import { describe, it, expect, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

const { composeFacilityBowties } = await import('$lib/api/facilityBowties');

describe('composeFacilityBowties (Spec 018 / S6 — D2)', () => {
  it('passes the facilityId to the Tauri invoke shape the backend expects', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([]);
    await composeFacilityBowties('f-block-5');
    expect(invokeMock).toHaveBeenCalledWith('compose_facility_bowties', {
      facilityId: 'f-block-5',
    });
  });

  it('deserialises the response as CompositionOp[]', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([
      {
        consumerNodeKey: '05010101FF000002',
        consumerLeafPath: ['Direct Lamp Control', 'Lamp #2', 'Lamp On'],
        consumerLeafSpace: 253,
        consumerLeafAddress: 116,
        eventIdBytes: [2, 1, 1, 1, 0xff, 1, 0, 1],
        bowtieName: 'Block 5 — lit',
        createdByFacility: 'f-block-5',
      },
    ]);
    const ops = await composeFacilityBowties('f-block-5');
    expect(ops).toHaveLength(1);
    expect(ops[0].bowtieName).toBe('Block 5 — lit');
    expect(ops[0].createdByFacility).toBe('f-block-5');
    expect(ops[0].eventIdBytes.length).toBe(8);
  });

  it('surfaces backend errors as promise rejections', async () => {
    invokeMock.mockReset();
    invokeMock.mockRejectedValue('facility \'f-nope\' is unknown');
    await expect(composeFacilityBowties('f-nope')).rejects.toBe(
      "facility 'f-nope' is unknown",
    );
  });
});
