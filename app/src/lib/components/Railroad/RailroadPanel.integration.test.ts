import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/svelte';
import RailroadPanel from './RailroadPanel.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import type { InformationChannel } from '$lib/api/channels';

function bodChannel(input: number): InformationChannel {
  return {
    id: `ch-bod-${input}`,
    name: `TowerLCC-1 BOD A${input}`,
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: {
      kind: 'connectorInput',
      nodeKey: '05010101FF000001',
      connector: 'connector-a',
      input,
    },
  };
}

describe('RailroadPanel (Spec 018 / S3 — hardware-organised Channels panel)', () => {
  const stubNodeName = (key: string) =>
    key === '05010101FF000001' ? 'TowerLCC-1' : `Node(${key})`;
  const stubDaughterboardName = (_nodeKey: string, _connector: string) => 'BOD-8';

  beforeEach(() => {
    channelsStore.reset();
    facilitiesStore.reset();
  });

  it('composes a Facilities section above a Channels section', () => {
    render(RailroadPanel, { props: { nodeName: stubNodeName } });
    expect(screen.getByTestId('facilities-section')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /^channels$/i })).toBeInTheDocument();
  });

  describe('with 8 BOD-8 channels on TowerLCC-1 connector A', () => {
    beforeEach(() => {
      const channels = Array.from({ length: 8 }, (_, i) => bodChannel(i + 1));
      channelsStore.hydrateBaseline(channels);
    });

    it('renders the 6-column header row including "Used by"', () => {
      render(RailroadPanel, {
        props: { nodeName: stubNodeName, daughterboardName: stubDaughterboardName },
      });
      const table = screen.getByRole('table');
      const headers = within(table).getAllByRole('columnheader');
      // 6 columns: state-dot (header text may be empty), Name, Role / Style,
      // Location, State, Used by.
      expect(headers.length).toBe(6);
      const labels = headers.map((h) => h.textContent?.trim() ?? '');
      expect(labels).toEqual(
        expect.arrayContaining(['Name', 'Role / Style', 'Location', 'State', 'Used by']),
      );
    });

    it('groups channels under a node + connector + daughter-board header row', () => {
      render(RailroadPanel, {
        props: { nodeName: stubNodeName, daughterboardName: stubDaughterboardName },
      });
      // Group-header row text mentions the node, the connector, and the
      // daughter-board id (mockup 4/8: "TowerLCC-1 · Connector A · BOD-8").
      const groupHeader = screen.getByText(
        (content) =>
          content.includes('TowerLCC-1') &&
          /connector\s*a/i.test(content) &&
          /bod-?8/i.test(content),
      );
      expect(groupHeader).toBeInTheDocument();
      // The group header should span all 6 columns (single <td colspan="6">).
      const groupRow = groupHeader.closest('tr');
      expect(groupRow).not.toBeNull();
      const groupCells = groupRow!.querySelectorAll('td');
      expect(groupCells.length).toBe(1);
      expect(groupCells[0].getAttribute('colspan')).toBe('6');
    });

    it('renders 8 channel rows with name + HW badge + role/style + location + "—" Used-by', () => {
      render(RailroadPanel, {
        props: { nodeName: stubNodeName, daughterboardName: stubDaughterboardName },
      });

      // All 8 channels appear by their default names.
      for (let i = 1; i <= 8; i++) {
        expect(screen.getByText(`TowerLCC-1 BOD A${i}`)).toBeInTheDocument();
      }

      // Inspect the first row in detail.
      const row1Name = screen.getByText('TowerLCC-1 BOD A1');
      const row1 = row1Name.closest('tr');
      expect(row1).not.toBeNull();
      const row1Cells = within(row1!).getAllByRole('cell');
      expect(row1Cells.length).toBe(6);

      // Ownership badge: HW for hardware-owned channels.
      expect(within(row1!).getByText(/^HW$/)).toBeInTheDocument();
      // Role label (human-readable) and style id present.
      expect(within(row1!).getByText(/block occupancy/i)).toBeInTheDocument();
      expect(within(row1!).getByText('bod-block-detector-input')).toBeInTheDocument();
      // Location renders connector + input ordinal.
      expect(
        within(row1!).getByText((content) =>
          /connector\s*a/i.test(content) && /input\s*1\b/i.test(content),
        ),
      ).toBeInTheDocument();
      // Used-by column shows em-dash for unbound channels (S3: no facilities yet).
      const usedByCell = row1Cells[row1Cells.length - 1];
      expect(usedByCell.textContent?.trim()).toBe('—');
    });
  });
});
