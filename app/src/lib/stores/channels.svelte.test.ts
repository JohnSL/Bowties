import { describe, it, expect, vi, beforeEach } from 'vitest';
import { flushSync } from 'svelte';
import type { InformationChannel } from '$lib/api/channels';

const listChannelsMock = vi.fn<() => Promise<InformationChannel[]>>(async () => []);

vi.mock('$lib/api/channels', () => ({
  listChannels: listChannelsMock,
}));

// Must import after mocks are set up
const { channelsStore } = await import('$lib/stores/channels.svelte');

beforeEach(() => {
  channelsStore.reset();
  listChannelsMock.mockReset();
  listChannelsMock.mockResolvedValue([]);
});

function makeChannel(overrides: Partial<InformationChannel> = {}): InformationChannel {
  return {
    id: '550e8400-e29b-41d4-a716-446655440000',
    name: 'West Yard — Connector A — Input 1',
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: {
      kind: 'connectorInput',
      nodeKey: '05010101FF000001',
      connector: 'connector-a',
      input: 1,
    },
    ...overrides,
  };
}

describe('channelsStore', () => {
  it('starts empty', () => {
    expect(channelsStore.channels).toEqual([]);
    expect(channelsStore.isEmpty).toBe(true);
  });

  it('loadChannels fetches from backend and populates state', async () => {
    const channels = [makeChannel(), makeChannel({ id: 'second', name: 'Input 2', binding: { kind: 'connectorInput', nodeKey: '05010101FF000001', connector: 'connector-a', input: 2 } })];
    listChannelsMock.mockResolvedValue(channels);

    await channelsStore.loadChannels();

    expect(channelsStore.channels).toEqual(channels);
    expect(channelsStore.isEmpty).toBe(false);
  });

  it('grouped groups channels by type', async () => {
    const channels = [
      makeChannel({ id: '1', name: 'Ch 1' }),
      makeChannel({ id: '2', name: 'Ch 2' }),
    ];
    listChannelsMock.mockResolvedValue(channels);
    await channelsStore.loadChannels();

    const grouped = channelsStore.grouped;
    expect(grouped.size).toBe(1);
    expect(grouped.get('block-occupancy')?.length).toBe(2);
  });

  describe('groupedByHardware (Spec 018 / S3)', () => {
    it('returns an empty map when no channels are present', () => {
      expect(channelsStore.groupedByHardware.size).toBe(0);
    });

    it('groups connectorInput channels by node + connector', () => {
      const ch1 = makeChannel({
        id: '1',
        binding: { kind: 'connectorInput', nodeKey: 'nodeA', connector: 'connector-a', input: 1 },
      });
      const ch2 = makeChannel({
        id: '2',
        binding: { kind: 'connectorInput', nodeKey: 'nodeA', connector: 'connector-a', input: 2 },
      });
      channelsStore.setChannels([ch1, ch2]);

      const groups = channelsStore.groupedByHardware;
      expect(groups.size).toBe(1);
      const onlyGroup = [...groups.values()][0];
      expect(onlyGroup.length).toBe(2);
    });

    it('separates channels on different connectors of the same node', () => {
      const chA = makeChannel({
        id: 'a',
        binding: { kind: 'connectorInput', nodeKey: 'nodeA', connector: 'connector-a', input: 1 },
      });
      const chB = makeChannel({
        id: 'b',
        binding: { kind: 'connectorInput', nodeKey: 'nodeA', connector: 'connector-b', input: 1 },
      });
      channelsStore.setChannels([chA, chB]);

      const groups = channelsStore.groupedByHardware;
      expect(groups.size).toBe(2);
    });

    it('separates channels on the same connector across different nodes', () => {
      const chA = makeChannel({
        id: 'a',
        binding: { kind: 'connectorInput', nodeKey: 'nodeA', connector: 'connector-a', input: 1 },
      });
      const chB = makeChannel({
        id: 'b',
        binding: { kind: 'connectorInput', nodeKey: 'nodeB', connector: 'connector-a', input: 1 },
      });
      channelsStore.setChannels([chA, chB]);

      expect(channelsStore.groupedByHardware.size).toBe(2);
    });

    it('groups lampRow channels under a per-node "direct-lamp-control" subsystem (S5-ready)', () => {
      const lamp1: InformationChannel = {
        id: 'lamp-1',
        name: 'Block 5 Indicator',
        role: 'lamp-indicator',
        style: 'single-led-direct-lamp',
        ownership: 'user-owned',
        binding: { kind: 'lampRow', nodeKey: 'nodeC', rowOrdinal: 7 },
      };
      const lamp2: InformationChannel = { ...lamp1, id: 'lamp-2', name: 'Block 6 Indicator', binding: { kind: 'lampRow', nodeKey: 'nodeC', rowOrdinal: 8 } };
      const detector = makeChannel({
        id: 'd1',
        binding: { kind: 'connectorInput', nodeKey: 'nodeA', connector: 'connector-a', input: 1 },
      });
      channelsStore.setChannels([detector, lamp1, lamp2]);

      const groups = channelsStore.groupedByHardware;
      expect(groups.size).toBe(2);
      // One group has the two lamp-indicator channels.
      const lampGroup = [...groups.values()].find((g) => g.length === 2);
      expect(lampGroup).toBeDefined();
      expect(lampGroup!.every((ch) => ch.binding.kind === 'lampRow')).toBe(true);
    });

    it('preserves insertion order across groups (first-seen wins)', () => {
      const chA = makeChannel({
        id: 'a',
        binding: { kind: 'connectorInput', nodeKey: 'nodeA', connector: 'connector-a', input: 1 },
      });
      const chB = makeChannel({
        id: 'b',
        binding: { kind: 'connectorInput', nodeKey: 'nodeB', connector: 'connector-a', input: 1 },
      });
      channelsStore.setChannels([chB, chA]);

      const keys = [...channelsStore.groupedByHardware.keys()];
      expect(keys[0]).toContain('nodeB');
      expect(keys[1]).toContain('nodeA');
    });
  });

  it('reset clears all state', async () => {
    listChannelsMock.mockResolvedValue([makeChannel()]);
    await channelsStore.loadChannels();
    expect(channelsStore.isEmpty).toBe(false);

    channelsStore.reset();

    expect(channelsStore.channels).toEqual([]);
    expect(channelsStore.isEmpty).toBe(true);
  });

  it('setChannels replaces channels directly', () => {
    const channels = [makeChannel()];
    channelsStore.setChannels(channels);
    expect(channelsStore.channels).toEqual(channels);
    expect(channelsStore.isEmpty).toBe(false);
  });
});

describe('channelsStore rename', () => {
  it('renameChannel updates the channel name in the effective view', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);

    const accepted = channelsStore.renameChannel('ch-1', 'Renamed');

    expect(accepted).toBe(true);
    expect(channelsStore.channels[0].name).toBe('Renamed');
  });

  it('renameChannel rejects empty name and returns false', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);

    const accepted = channelsStore.renameChannel('ch-1', '   ');

    expect(accepted).toBe(false);
    expect(channelsStore.channels[0].name).toBe('Original');
  });

  it('renameChannel trims whitespace from the new name', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);

    channelsStore.renameChannel('ch-1', '  Trimmed  ');

    expect(channelsStore.channels[0].name).toBe('Trimmed');
  });

  it('rename marks store as dirty and increments editCount', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);
    expect(channelsStore.isDirty).toBe(false);
    expect(channelsStore.editCount).toBe(0);

    channelsStore.renameChannel('ch-1', 'New Name');

    expect(channelsStore.isDirty).toBe(true);
    expect(channelsStore.editCount).toBe(1);
  });

  it('discard clears pending renames', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);
    channelsStore.renameChannel('ch-1', 'Renamed');
    expect(channelsStore.isDirty).toBe(true);

    channelsStore.discard();

    expect(channelsStore.isDirty).toBe(false);
    expect(channelsStore.channels[0].name).toBe('Original');
  });

  it('hydrateBaseline clears pending renames', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);
    channelsStore.renameChannel('ch-1', 'Renamed');

    channelsStore.hydrateBaseline([{ ...ch, name: 'Renamed' }]);

    expect(channelsStore.isDirty).toBe(false);
    expect(channelsStore.pendingRenames.size).toBe(0);
    expect(channelsStore.channels[0].name).toBe('Renamed');
  });

  it('rename applies to pending creations too', () => {
    const ch = makeChannel({ id: 'new-ch', name: 'Auto Name' });
    channelsStore.addPendingChannels([ch]);

    channelsStore.renameChannel('new-ch', 'Custom Name');

    expect(channelsStore.channels[0].name).toBe('Custom Name');
  });

  it('pendingRenames exposes the rename map for save flush', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);

    channelsStore.renameChannel('ch-1', 'New');

    expect(channelsStore.pendingRenames.get('ch-1')).toBe('New');
  });

  it('suppresses rename when the new name equals the current name (ADR-0012 no-op)', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);
    expect(channelsStore.isDirty).toBe(false);

    const accepted = channelsStore.renameChannel('ch-1', 'Original');

    expect(accepted).toBe(false);
    expect(channelsStore.isDirty).toBe(false);
    expect(channelsStore.pendingRenames.size).toBe(0);
  });

  it('suppresses rename when trimmed new name equals current name', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);

    const accepted = channelsStore.renameChannel('ch-1', '  Original  ');

    expect(accepted).toBe(false);
    expect(channelsStore.isDirty).toBe(false);
  });

  it('rename back to baseline name removes pending rename (revert)', () => {
    const ch = makeChannel({ id: 'ch-1', name: 'Original' });
    channelsStore.setChannels([ch]);

    // Rename away from baseline
    channelsStore.renameChannel('ch-1', 'Changed');
    expect(channelsStore.isDirty).toBe(true);
    expect(channelsStore.pendingRenames.has('ch-1')).toBe(true);

    // Rename back to baseline
    const accepted = channelsStore.renameChannel('ch-1', 'Original');

    expect(accepted).toBe(true);
    expect(channelsStore.isDirty).toBe(false);
    expect(channelsStore.pendingRenames.has('ch-1')).toBe(false);
  });
});

describe('channelsStore deleteChannels', () => {
  it('marks baseline channels as pending deletions', () => {
    const ch1 = makeChannel({ id: 'ch-1', name: 'Channel 1' });
    const ch2 = makeChannel({ id: 'ch-2', name: 'Channel 2' });
    channelsStore.setChannels([ch1, ch2]);

    channelsStore.deleteChannels(['ch-1']);

    expect(channelsStore.channels).toHaveLength(1);
    expect(channelsStore.channels[0].id).toBe('ch-2');
    expect(channelsStore.pendingDeletions.has('ch-1')).toBe(true);
  });

  it('removes pending creations directly instead of tracking as deletions', () => {
    channelsStore.addPendingChannels([
      makeChannel({ id: 'pending-1', name: 'Pending' }),
    ]);

    channelsStore.deleteChannels(['pending-1']);

    expect(channelsStore.channels).toHaveLength(0);
    expect(channelsStore.pendingDeletions.size).toBe(0);
  });

  it('handles mixed baseline and pending creation deletions', () => {
    channelsStore.setChannels([makeChannel({ id: 'base-1', name: 'Base' })]);
    channelsStore.addPendingChannels([makeChannel({ id: 'pend-1', name: 'Pend' })]);

    channelsStore.deleteChannels(['base-1', 'pend-1']);

    expect(channelsStore.channels).toHaveLength(0);
    expect(channelsStore.pendingDeletions.has('base-1')).toBe(true);
    expect(channelsStore.pendingDeletions.has('pend-1')).toBe(false);
  });

  it('marks store as dirty when deletions are pending', () => {
    channelsStore.setChannels([makeChannel({ id: 'ch-1' })]);
    expect(channelsStore.isDirty).toBe(false);

    channelsStore.deleteChannels(['ch-1']);

    expect(channelsStore.isDirty).toBe(true);
  });

  it('includes deletions in editCount', () => {
    channelsStore.setChannels([makeChannel({ id: 'ch-1' }), makeChannel({ id: 'ch-2' })]);

    channelsStore.deleteChannels(['ch-1', 'ch-2']);

    expect(channelsStore.editCount).toBe(2);
  });

  it('discard clears pending deletions', () => {
    channelsStore.setChannels([makeChannel({ id: 'ch-1' })]);
    channelsStore.deleteChannels(['ch-1']);
    expect(channelsStore.isDirty).toBe(true);

    channelsStore.discard();

    expect(channelsStore.isDirty).toBe(false);
    expect(channelsStore.channels).toHaveLength(1);
    expect(channelsStore.pendingDeletions.size).toBe(0);
  });

  it('hydrateBaseline clears pending deletions', () => {
    channelsStore.setChannels([makeChannel({ id: 'ch-1' }), makeChannel({ id: 'ch-2' })]);
    channelsStore.deleteChannels(['ch-1']);

    channelsStore.hydrateBaseline([makeChannel({ id: 'ch-2' })]);

    expect(channelsStore.isDirty).toBe(false);
    expect(channelsStore.pendingDeletions.size).toBe(0);
    expect(channelsStore.channels).toHaveLength(1);
  });

  it('cleans up pending renames for deleted channels', () => {
    channelsStore.setChannels([makeChannel({ id: 'ch-1', name: 'Original' })]);
    channelsStore.renameChannel('ch-1', 'Renamed');
    expect(channelsStore.pendingRenames.has('ch-1')).toBe(true);

    channelsStore.deleteChannels(['ch-1']);

    expect(channelsStore.pendingRenames.has('ch-1')).toBe(false);
  });

  it('isEmpty returns true when all channels are deleted', () => {
    channelsStore.setChannels([makeChannel({ id: 'ch-1' })]);
    expect(channelsStore.isEmpty).toBe(false);

    channelsStore.deleteChannels(['ch-1']);

    expect(channelsStore.isEmpty).toBe(true);
  });
});
