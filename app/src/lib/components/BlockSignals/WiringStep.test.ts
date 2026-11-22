import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import BlockSignalsEditor from './BlockSignalsEditor.svelte';
import WiringStep from './WiringStep.svelte';
import { fakeView, spyActions } from './editorTestSupport';
import type { SignalOutputHost } from '$lib/api/signaling';
import type { DiscoveredNode } from '$lib/api/tauri';
import { nodeInfoStore } from '$lib/stores/nodeInfo';

const NODE_A = '020157100997';
const NODE_B = '020157100A01';

function snipNode(userName: string): DiscoveredNode {
  return {
    snip_data: { user_name: userName, model: 'Signal-LCC', manufacturer: 'RR-CirKits, Inc.' },
  } as unknown as DiscoveredNode;
}

beforeEach(() => {
  nodeInfoStore.set(new Map([[NODE_A, snipNode('Yard Signals')], [NODE_B, snipNode('Depot Signals')]]));
});
afterEach(() => nodeInfoStore.set(new Map()));

const STYLE = {
  styleId: 'two-driver-rg-signal',
  bindingKind: 'signalLccRgOutputs',
  appearances: { stop: ['red'], approach: ['red', 'green'], clear: ['green'] },
} as SignalOutputHost['style'];

const HOSTS: SignalOutputHost[] = [
  {
    nodeKey: NODE_A, nodeName: 'Signal-LCC · 02.01.57.10.09.97', style: STYLE, evidence: 'complete',
    outputs: [
      { ordinal: 1, label: '#1 H1-R', owner: null },
      { ordinal: 2, label: '#2 H1-G', owner: null },
      { ordinal: 3, label: '#3 H2-R', owner: 'Mill Signal' },
    ],
  },
  { nodeKey: NODE_B, nodeName: 'Signal-LCC · 02.01.57.10.0A.01', style: STYLE, evidence: 'complete', outputs: [] },
];

function show(extra = {}) {
  const actions = spyActions();
  const view = fakeView({ step: 'wiring', outputHosts: HOSTS, ...extra });
  render(WiringStep, { props: { view, actions } });
  return actions;
}

const chosen = (nodeKey: string, red: number | null, green: number | null) => () => ({ nodeKey, redOutput: red, greenOutput: green });

describe('WiringStep', () => {
  it('lets the user pick the Signal LCC the selected signal runs on', async () => {
    const actions = show();
    expect(screen.getByRole('heading', { name: 'Cedar Hill East' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: /Runs on/ }));
    const dialog = screen.getByRole('dialog', { name: 'Choose a Signal LCC' });
    expect(within(dialog).getAllByRole('radio').map((r) => r.getAttribute('value'))).toEqual([NODE_B, NODE_A]);
    await fireEvent.click(within(dialog).getByRole('radio', { name: /Yard Signals/ }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Confirm' }));
    expect(actions.chooseWiringNode).toHaveBeenCalledWith('sig-1', NODE_A);
    expect(screen.queryByRole('dialog', { name: 'Choose a Signal LCC' })).not.toBeInTheDocument();
  });

  it('shows the chosen node\'s pins as Free, This signal or Taken and emits red and green choices', async () => {
    const actions = show({ wiringFor: chosen(NODE_A, 1, null) });
    const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
    expect(within(grid).getByRole('button', { name: /#1 H1-R.*This signal/ })).toBeDisabled();
    expect(within(grid).getByRole('button', { name: /#3 H2-R.*Taken/ })).toBeDisabled();

    expect(within(grid).getByText('#1 H1-R')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /Red lamp.*#1 H1-R/ })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /Green lamp.*No pin chosen/ })).toBeInTheDocument();

    // the red lamp is placed, so the green lamp is selected next
    await fireEvent.click(within(grid).getByRole('button', { name: /#2 H1-G.*Free/ }));
    expect(actions.chooseGreenOutput).toHaveBeenCalledWith('sig-1', 2);
    expect(actions.chooseRedOutput).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole('radio', { name: /Red lamp/ }));
    await fireEvent.click(within(grid).getByRole('button', { name: /#2 H1-G.*Free/ }));
    expect(actions.chooseRedOutput).toHaveBeenCalledWith('sig-1', 2);
  });

  it('keeps Customize lamps, Blink and Show disabled without explanatory "later" text', () => {
    show({ wiringFor: chosen(NODE_A, null, null) });
    expect(screen.getByRole('checkbox', { name: 'Customize lamps' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Blink' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Show' })).toBeDisabled();
    expect(document.body.textContent).not.toMatch(/later release/i);
  });

  it('lists blocking issues for the signal and its node in readable words, and the finish error', () => {
    show({
      wiringFor: chosen(NODE_A, 1, 2),
      issuesForSignal: () => [{ code: 'noFreeMast', signalId: 'sig-1', nodeKey: NODE_A, detail: 'x' }],
      issuesForNode: () => [
        { code: 'outputInUse', signalId: 'sig-1', nodeKey: NODE_A, detail: 'x' },
        { code: 'occupancyEventsMissing', signalId: 'sig-1', nodeKey: NODE_A, detail: 'x' },
      ],
      finishError: 'Could not stage the changes.',
    });
    const attention = screen.getByRole('list', { name: 'Choices needing attention' });
    const items = within(attention).getAllByRole('listitem').map((li) => li.textContent);
    expect(items).toHaveLength(3);
    expect(items[0]).toMatch(/no free mast/i);
    expect(items[1]).toMatch(/pin.*already (used|in use)/i);
    expect(items[2]).toMatch(/occupancy events/i);
    expect(screen.getByRole('alert')).toHaveTextContent('Could not stage the changes.');
  });

  it('has Done disabled until the store allows finishing, then emits finish', async () => {
    const actions = spyActions();
    const { rerender } = render(BlockSignalsEditor, { props: { view: fakeView({ step: 'wiring', outputHosts: HOSTS }), actions, detectedBlocks: [] } });
    expect(screen.getByRole('button', { name: 'Done' })).toBeDisabled();
    await rerender({ view: fakeView({ step: 'wiring', outputHosts: HOSTS, canFinish: true }), actions, detectedBlocks: [] });
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(actions.finish).toHaveBeenCalled();
  });
  it('lists Signal LCC nodes by display name with model and address underneath, sorted by name', async () => {
    show();
    await fireEvent.click(screen.getByRole('button', { name: /Runs on/ }));
    const dialog = screen.getByRole('dialog', { name: 'Choose a Signal LCC' });
    const rows = within(dialog).getAllByRole('radio').map((r) => r.closest('label')?.textContent?.replace(/\s+/g, ' ').trim());
    expect(rows).toEqual([
      'Depot Signals Signal-LCC · 02.01.57.10.0A.01',
      'Yard Signals Signal-LCC · 02.01.57.10.09.97',
    ]);
  });

  it('shows the chosen node on the Runs on button as display name over model and address', () => {
    show({ wiringFor: chosen(NODE_A, null, null) });
    const field = screen.getByRole('button', { name: /Runs on/ });
    expect(field.textContent?.replace(/\s+/g, ' ')).toContain('Yard Signals Signal-LCC · 02.01.57.10.09.97');
  });

  it('explains why the pins cannot be shown when the chosen node evidence is invalid', () => {
    const invalid: SignalOutputHost = {
      ...HOSTS[0], evidence: 'invalid', outputs: [],
      evidenceDetail: 'Output 3 is claimed by both Mast 1 and Mast 2.',
    };
    show({ outputHosts: [invalid, HOSTS[1]], wiringFor: chosen(NODE_A, null, null) });
    const panel = screen.getByRole('region', { name: 'Pins' });
    expect(panel).toHaveTextContent(
      "This Signal LCC's pins can't be used until its configuration is fixed: Output 3 is claimed by both Mast 1 and Mast 2.",
    );
    expect(within(panel).queryByRole('group')).not.toBeInTheDocument();
  });

  it('explains when the chosen node configuration has not been read yet', () => {
    const unavailable: SignalOutputHost = { ...HOSTS[0], evidence: 'unavailable', outputs: [] };
    show({ outputHosts: [unavailable, HOSTS[1]], wiringFor: chosen(NODE_A, null, null) });
    expect(screen.getByRole('region', { name: 'Pins' })).toHaveTextContent(
      "This Signal LCC's configuration hasn't been read yet, so its pins can't be shown.",
    );
  });
});
