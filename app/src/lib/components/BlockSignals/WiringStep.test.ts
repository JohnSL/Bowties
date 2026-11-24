import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConfigNode, NodeConfigTree } from '$lib/types/nodeTree';

const { focusConfigFieldMock } = vi.hoisted(() => ({ focusConfigFieldMock: vi.fn() }));
vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: { focusConfigField: focusConfigFieldMock },
}));
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import BlockSignalsEditor from './BlockSignalsEditor.svelte';
import WiringStep from './WiringStep.svelte';
import { fakeView, previewWith, spyActions } from './editorTestSupport';
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
      { ordinal: 3, label: '#3 H2-R', owner: 'Mast 2', facility: { facilityId: 'fac-1', name: 'Mill Signals' } },
      { ordinal: 4, label: '#4 H2-L', owner: 'Mast 3' },
      { ordinal: 5, label: '#5 H3-G', owner: 'Mast 3' },
      { ordinal: 6, label: '#6 H3-Y', owner: 'Direct lamp 1' },
    ],
  },
  { nodeKey: NODE_B, nodeName: 'Signal-LCC · 02.01.57.10.0A.01', style: STYLE, evidence: 'complete', outputs: [] },
];

function ownerTree(mast3Description = ''): NodeConfigTree {
  const group = (name: string, i: number, seg: number) => ({
    kind: 'group' as const, name, description: null, instance: i, instanceLabel: `${name} ${i}`,
    replicationOf: name, replicationCount: 4, path: [`seg:${seg}`, `elem:0#${i}`], displayName: null,
    children: name === 'Mast' && i === 3
      ? [{ kind: 'leaf', name: 'Mast Description', description: null, elementType: 'string', path: [], value: { type: 'string', value: mast3Description } } as unknown as ConfigNode]
      : [],
  });
  const segment = (name: string, seg: number, children: NodeConfigTree['segments'][number]['children']) => ({
    name, description: null, origin: 0, space: 1, children, seg,
  });
  return {
    nodeId: NODE_A, identity: null,
    segments: [
      segment('Rule to Aspect', 0, [1, 2, 3, 4].map((i) => group('Mast', i, 0))),
      segment('Direct Lamp Control', 1, [1, 2].map((i) => group('Lamp', i, 1))),
    ],
  } as unknown as NodeConfigTree;
}

function show(extra = {}, nodeTree: ((nodeKey: string) => NodeConfigTree | undefined) | undefined = undefined) {
  const actions = spyActions();
  const view = fakeView({ step: 'wiring', outputHosts: HOSTS, ...extra });
  render(WiringStep, { props: { view, actions, nodeTree } });
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

  it('shows the chosen node\'s pins as Free, This signal or used by its owner and emits red and green choices', async () => {
    const actions = show({ wiringFor: chosen(NODE_A, 1, null) });
    const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
    expect(within(grid).getByRole('button', { name: /#1 H1-R.*This signal/ })).toBeDisabled();
    expect(within(grid).getByRole('button', { name: '#3 H2-R: Used by Mill Signals' })).toBeDisabled();
    expect(within(grid).getByRole('button', { name: '#4 H2-L: Used by Mast 3, not managed by Bowties. Show details' })).toBeEnabled();
    expect(within(grid).getByRole('button', { name: /#3 H2-R/ })).toHaveTextContent('Mill Signals');
    expect(within(grid).getByRole('button', { name: /#4 H2-L/ })).toHaveTextContent('Mast 3');
    expect(document.body.textContent).not.toContain('Taken');

    expect(screen.getByRole('button', { name: 'Red lamp: #1 H1-R' })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getByRole('button', { name: 'Green lamp: No pin' })).toHaveAttribute('aria-pressed', 'true');

    // the red lamp is placed, so the green lamp is selected next
    await fireEvent.click(within(grid).getByRole('button', { name: /#2 H1-G.*Free/ }));
    expect(actions.chooseGreenOutput).toHaveBeenCalledWith('sig-1', 2);
    expect(actions.chooseRedOutput).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole('button', { name: /Red lamp/ }));
    expect(screen.getByRole('button', { name: /Red lamp/ })).toHaveAttribute('aria-pressed', 'true');
    await fireEvent.click(within(grid).getByRole('button', { name: /#2 H1-G.*Free/ }));
    expect(actions.chooseRedOutput).toHaveBeenCalledWith('sig-1', 2);
  });

  it('lays the pins out one row per header with a who line, selected-lamp and this-signal styling, and a legend', () => {
    show({ wiringFor: chosen(NODE_A, 1, 2) });
    const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
    expect(within(grid).getAllByText(/^H[123]$/).map((h) => h.textContent)).toEqual(['H1', 'H2', 'H3']);
    expect(within(grid).queryByRole('button', { name: /^H1$/ })).not.toBeInTheDocument();
    const pin = (re: RegExp) => within(grid).getByRole('button', { name: re });
    expect(pin(/#1 H1-R/)).toHaveTextContent('H1-R');
    expect(pin(/#1 H1-R/)).toHaveTextContent('Red');
    expect(pin(/#1 H1-R/)).toHaveClass('selected');
    expect(pin(/#2 H1-G/)).toHaveTextContent('Green');
    expect(pin(/#2 H1-G/)).toHaveClass('mine');
    expect(pin(/#2 H1-G/)).not.toHaveClass('selected');
    expect(pin(/#3 H2-R/)).toHaveClass('facility');
    expect(pin(/#3 H2-R/)).toHaveTextContent('Mill Signals');
    expect(pin(/#4 H2-L/)).toHaveClass('outside');
    expect(pin(/#4 H2-L/)).toHaveTextContent('Mast 3');
    const key = screen.getByText('Selected lamp').parentElement!;
    expect(key.textContent).toMatch(/Selected lamp.*This signal.*Used by a facility.*Used outside Bowties.*Free/);
    expect(key.textContent).not.toContain('Assigned twice');
  });

  it('labels every pin with its number and says who holds it, and opens a details card for an outside owner', async () => {
    focusConfigFieldMock.mockClear();
    show({ wiringFor: chosen(NODE_A, null, null) }, (key) => (key === NODE_A ? ownerTree('Cedar Hill East') : undefined));
    const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
    expect(within(grid).getByRole('button', { name: /#1 H1-R: Free/ })).toHaveTextContent(/^#1 H1-R\s*Free$/);
    expect(within(grid).getByRole('button', { name: /#4 H2-L/ })).toHaveTextContent(/^#4 H2-L\s*Mast 3$/);
    expect(screen.queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();

    await fireEvent.click(within(grid).getByRole('button', { name: /#4 H2-L/ }));
    const card = screen.getByRole('region', { name: 'Mast 3, not managed by Bowties' });
    expect(within(card).getByRole('heading', { name: /Mast 3/ })).toHaveTextContent('Not managed by Bowties');
    expect(card).toHaveTextContent('Description: "Cedar Hill East"');
    expect(card).toHaveTextContent('Rule to Aspect · drives #4 H2-L, #5 H3-G');
    expect(card).toHaveTextContent('Configured on Yard Signals outside Bowties, for example by hand, by another tool, or in earlier testing.');
    expect(within(grid).getByRole('button', { name: /#4 H2-L/ })).toHaveClass('inspected');
    expect(within(grid).getByRole('button', { name: /#5 H3-G/ })).toHaveClass('inspected');
    expect(within(grid).getByRole('button', { name: /#6 H3-Y/ })).not.toHaveClass('inspected');
    expect(within(card).getByRole('button', { name: 'Release and use…' })).toBeEnabled();
    expect(card).not.toHaveTextContent(/Aspects set by Logic/);

    await fireEvent.click(within(card).getByRole('button', { name: 'Show Mast 3 in Configuration' }));
    expect(focusConfigFieldMock).toHaveBeenCalledWith(NODE_A, ['seg:0', 'elem:0#3']);

    expect(within(card).queryByRole('button', { name: 'Close' })).not.toBeInTheDocument();
    await fireEvent.click(within(card).getByRole('button', { name: 'Close Mast 3 details' }));
    expect(screen.queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();
    expect(within(grid).getByRole('button', { name: /#4 H2-L/ })).toHaveFocus();
  });

  it('closes the details card with Escape without letting the key reach the editor dialog', async () => {
    show({ wiringFor: chosen(NODE_A, null, null) });
    const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
    await fireEvent.click(within(grid).getByRole('button', { name: /#4 H2-L/ }));
    const card = screen.getByRole('region', { name: 'Mast 3, not managed by Bowties' });
    const reachedWindow = vi.fn();
    window.addEventListener('keydown', reachedWindow);
    await fireEvent.keyDown(within(card).getByRole('button', { name: 'Close Mast 3 details' }), { key: 'Escape' });
    window.removeEventListener('keydown', reachedWindow);
    expect(screen.queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();
    expect(reachedWindow).not.toHaveBeenCalled();
    expect(within(grid).getByRole('button', { name: /#4 H2-L/ })).toHaveFocus();
  });

  it('says there is no description, omits Show in Configuration without a node tree, and closes when the owner goes away', async () => {
    show({ wiringFor: chosen(NODE_A, null, null) });
    const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
    await fireEvent.click(within(grid).getByRole('button', { name: /#6 H3-Y/ }));
    const card = screen.getByRole('region', { name: 'Direct lamp 1, not managed by Bowties' });
    expect(card).toHaveTextContent('No description.');
    expect(within(card).queryByRole('button', { name: /in Configuration/ })).not.toBeInTheDocument();
    await fireEvent.click(within(grid).getByRole('button', { name: /#6 H3-Y/ }));
    expect(screen.queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();
  });

  it('shows the signal as a head card of lamp buttons, keeps Customize lamps disabled and has no Blink or Show', () => {
    show({ wiringFor: chosen(NODE_A, 1, null) });
    const lamps = screen.getByRole('region', { name: 'Lamps' });
    expect(within(lamps).getByRole('heading', { name: 'Head 1 · Searchlight (red and green)' })).toBeInTheDocument();
    expect(within(lamps).getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual(['Red lamp: #1 H1-R', 'Green lamp: No pin']);
    expect(within(lamps).getByText('No pin')).toHaveClass('none');
    expect(screen.queryByRole('radio', { name: /lamp/i })).not.toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Customize lamps' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Blink' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Show' })).not.toBeInTheDocument();
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

  it('names the pin and its owner when a chosen pin is in use', () => {
    show({
      wiringFor: chosen(NODE_A, 4, 2),
      issuesForNode: () => [{
        code: 'outputInUse', signalId: 'sig-1', nodeKey: NODE_A, detail: 'x',
        bindingIssue: { kind: 'signalOutputConflict', signalId: 'sig-1', nodeKey: NODE_A, output: 4, owner: 'Mast 3' },
      }],
    });
    const attention = screen.getByRole('list', { name: 'Choices needing attention' });
    expect(attention).toHaveTextContent('#4 H2-L is used by Mast 3, configured outside Bowties.');
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

  it('treats pins of the facility being edited as free', async () => {
    const actions = show({ facilityId: 'fac-1', wiringFor: chosen(NODE_A, null, null) });
    const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
    const own = within(grid).getByRole('button', { name: /#3 H2-R: Free/ });
    expect(own).toBeEnabled();
    expect(own).toHaveTextContent(/^#3 H2-R\s*Free$/);
    await fireEvent.click(own);
    expect(actions.chooseRedOutput).toHaveBeenCalledWith('sig-1', 3);
  });

  it('keeps pins of other facilities disabled', () => {
    show({ facilityId: 'fac-other', wiringFor: chosen(NODE_A, null, null) });
    expect(screen.getByRole('button', { name: '#3 H2-R: Used by Mill Signals' })).toBeDisabled();
  });

  describe('releasing a pin used outside Bowties', () => {
    const OWNERS = [{ nodeKey: NODE_A, owner: 'Mast 3', matchedLogic: [2, 4], keptLogic: [4] }];
    const showWithOwners = () => show({
      wiringFor: chosen(NODE_A, null, null),
      preview: { ...previewWith(), outsideOwners: OWNERS },
    });
    const openCard = async (pin: RegExp) => {
      const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
      await fireEvent.click(within(grid).getByRole('button', { name: pin }));
      return { grid, card: screen.getByRole('region', { name: /not managed by Bowties/ }) };
    };

    it('shows the Logic that set aspects of a mast and confirms the release in the card', async () => {
      const actions = showWithOwners();
      const { card } = await openCard(/#5 H3-G/);
      expect(card).toHaveTextContent('Aspects set by Logic 2, Logic 4');
      await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
      expect(card).toHaveTextContent('Release Mast 3? All 2 of its pins (#4 H2-L, #5 H3-G) become free, and Mast 3, Logic 2 are reset when you save. Discard undoes it.');
      expect(card).toHaveTextContent("Logic 4 also sends other events, so it is kept; only its actions that set Mast 3's aspects are disconnected.");
      expect(card).toHaveTextContent('Other boards or JMRI may still send events to Mast 3.');
      expect(within(card).queryByRole('button', { name: 'Release and use…' })).not.toBeInTheDocument();

      await fireEvent.click(within(card).getByRole('button', { name: 'Cancel' }));
      expect(within(card).getByRole('button', { name: 'Release and use…' })).toHaveFocus();
      expect(card).not.toHaveTextContent('Release Mast 3?');
      expect(actions.releaseAndPlaceOutput).not.toHaveBeenCalled();

      await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
      await fireEvent.click(within(card).getByRole('button', { name: 'Release and put Red on #5 H3-G' }));
      expect(actions.releaseAndPlaceOutput).toHaveBeenCalledWith('sig-1', 'red', { nodeKey: NODE_A, owner: 'Mast 3' }, 5);
    });

    it('places the selected lamp and names just the row for a Direct Lamp row', async () => {
      const actions = showWithOwners();
      await fireEvent.click(screen.getByRole('button', { name: /Green lamp/ }));
      const { card } = await openCard(/#6 H3-Y/);
      expect(card).toHaveTextContent('Direct Lamp Control row · drives #6 H3-Y');
      expect(card).not.toHaveTextContent('Aspects set by');
      await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
      expect(card).toHaveTextContent('Release Direct lamp 1? Its pin (#6 H3-Y) becomes free, and Direct lamp 1 is reset when you save. Discard undoes it.');
      expect(card).not.toHaveTextContent('JMRI');
      await fireEvent.click(within(card).getByRole('button', { name: 'Release and put Green on #6 H3-Y' }));
      expect(actions.releaseAndPlaceOutput).toHaveBeenCalledWith('sig-1', 'green', { nodeKey: NODE_A, owner: 'Direct lamp 1' }, 6);
    });

    it('closes only the card with Escape or ×, from the confirmation too', async () => {
      showWithOwners();
      const { card } = await openCard(/#4 H2-L/);
      await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
      await fireEvent.click(within(card).getByRole('button', { name: 'Close Mast 3 details' }));
      expect(screen.queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();
      const again = (await openCard(/#4 H2-L/)).card;
      expect(again).not.toHaveTextContent('Release Mast 3?');
    });

    it('lets a chosen pin held outside Bowties open its owner and release it for that lamp', async () => {
      const actions = show({
        wiringFor: chosen(NODE_A, 4, null),
        preview: { ...previewWith(), outsideOwners: OWNERS },
      });
      const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
      const pin = within(grid).getByRole('button', { name: '#4 H2-L: Red, held by Mast 3 outside Bowties. Show details' });
      expect(pin).toBeEnabled();
      expect(pin).toHaveTextContent('Red');
      expect(pin).toHaveTextContent('⚠');
      expect(pin).toHaveAttribute('data-owner', 'Mast 3');

      await fireEvent.click(pin);
      const card = screen.getByRole('region', { name: 'Mast 3, not managed by Bowties' });
      await fireEvent.click(within(card).getByRole('button', { name: 'Release and use…' }));
      expect(within(card).queryByRole('button', { name: /Release and put/ })).not.toBeInTheDocument();
      await fireEvent.click(within(card).getByRole('button', { name: 'Release Mast 3' }));
      expect(actions.releaseAndPlaceOutput).toHaveBeenCalledWith('sig-1', 'red', { nodeKey: NODE_A, owner: 'Mast 3' }, 4);
    });

    it('closes the card when the held chosen pin is clicked again, and leaves other chosen pins disabled', async () => {
      show({ wiringFor: chosen(NODE_A, 4, 1) });
      const grid = screen.getByRole('group', { name: 'Pins on Yard Signals' });
      expect(within(grid).getByRole('button', { name: /#1 H1-R.*This signal/ })).toBeDisabled();
      const pin = within(grid).getByRole('button', { name: /#4 H2-L: Red, held by Mast 3/ });
      await fireEvent.click(pin);
      expect(screen.getByRole('region', { name: /Mast 3, not managed/ })).toBeInTheDocument();
      await fireEvent.click(pin);
      expect(screen.queryByRole('region', { name: /not managed by Bowties/ })).not.toBeInTheDocument();
    });

    it('offers no release on a pin a facility owns', () => {
      showWithOwners();
      expect(screen.getByRole('button', { name: /#3 H2-R: Used by Mill Signals/ })).toBeDisabled();
    });
  });
});
