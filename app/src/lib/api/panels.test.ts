import { describe, it, expect, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

const { panelWiringFacts, composePanel } = await import('$lib/api/panels');
import type { ComposePanelRequest, PanelWiringFacts } from '$lib/api/panels';

describe('panelWiringFacts', () => {
  it('asks for the facts of the named boards and returns pins with their owners', async () => {
    invokeMock.mockReset();
    const facts: PanelWiringFacts = {
      boards: [{
        nodeKey: '050101010700',
        name: 'Tower',
        pins: [
          { output: 9, label: '#9 H3-G', group: 'H3', owner: { kind: 'free' } },
          { output: 3, label: '#3 H1-G', group: 'H1', owner: { kind: 'panel', facilityId: 'p1', name: 'Panel', row: 3 } },
          { output: 4, label: '#4 H1-R', group: 'H1', owner: { kind: 'outside', row: null, description: 'Mast 2' } },
        ],
      }],
      issues: [{ nodeKey: '050101010800', detail: 'not loaded' }],
    };
    invokeMock.mockResolvedValue(facts);

    const result = await panelWiringFacts(['050101010700']);

    expect(invokeMock).toHaveBeenCalledWith('panel_wiring_facts', { request: { nodeKeys: ['050101010700'] } });
    expect(result).toEqual(facts);
  });

  it('omits nodeKeys to ask for every board', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({ boards: [], issues: [] });
    await panelWiringFacts();
    expect(invokeMock).toHaveBeenCalledWith('panel_wiring_facts', { request: {} });
  });
});

describe('composePanel', () => {
  it('sends the compose request and returns edits, row assignments and issues', async () => {
    invokeMock.mockReset();
    const response = {
      edits: [],
      assignments: [{ entryId: 'e1', nodeKey: '050101010700', row: 9 }],
      issues: [],
    };
    invokeMock.mockResolvedValue(response);
    const request: ComposePanelRequest = {
      facilityId: 'p1',
      record: { litWhen: 'occupied', entries: [{ entryId: 'e1', blockId: 'b1' }] },
      lamps: [{ entryId: 'e1', nodeKey: '050101010700', output: 9, name: 'Lamp 1' }],
    };

    const result = await composePanel(request);

    expect(invokeMock).toHaveBeenCalledWith('compose_panel', { request });
    expect(result.assignments[0].row).toBe(9);
  });
});
