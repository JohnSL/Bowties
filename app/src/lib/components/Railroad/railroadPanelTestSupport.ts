/** Shared helpers for the RailroadPanel integration tests (Automatic Block Signals editor). IPC is the only mock. */
import { expect, type Mock } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import RailroadPanel from './RailroadPanel.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import type { Facility } from '$lib/api/facilities';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
import type { InformationChannel } from '$lib/api/channels';
import type { BlockSignalsPreview, BlockSignalsPreviewRequest, ListedBlock, TravelDirection } from '$lib/api/signaling';
import { effectiveLayoutStore } from '$lib/layout';

export type Invoke = Mock<(command: string, args?: unknown) => unknown>;
export type IpcStubs = Record<string, (args?: unknown) => unknown>;

export const stubNodeName = (key: string) =>
  key === '05010101FF000001' ? 'TowerLCC-1' : `Node(${key})`;

export function bodChannel(input: number): InformationChannel {
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

export const blocks = [
  { ...bodChannel(1), id: 'block:cedar', name: 'Cedar Hill' },
  { ...bodChannel(2), id: 'block:mill', name: 'Mill curve' },
  { ...bodChannel(3), id: 'block:river', name: 'River bridge' },
];

export async function startBlockSignals(name = 'New Automatic Block Signals') {
  await fireEvent.click(screen.getByRole('radio', { name: /^Automatic Block Signals/ }));
  await fireEvent.input(screen.getByLabelText('Name'), { target: { value: name } });
  await fireEvent.click(screen.getByRole('button', { name: 'Add facility' }));
}

/** A small stand-in for the backend: one signal per signal block, in list order. */
export function fakePreview(request: BlockSignalsPreviewRequest): BlockSignalsPreview {
  const groups: ListedBlock[][] = [];
  request.blocks.forEach((block, index) => {
    if (index > 0 && block.joinedToPrevious) groups[groups.length - 1].push(block);
    else groups.push([block]);
  });
  const chains: Array<{ run: 'withList' | 'againstList'; governs: TravelDirection; groups: ListedBlock[][] }> = [
    { run: 'withList', governs: request.direction, groups },
  ];
  if (request.movements === 'bothDirections') {
    chains.push({
      run: 'againstList',
      governs: request.direction === 'eastward' ? 'westward' : 'eastward',
      groups: [...groups].reverse().map((group) => [...group].reverse()),
    });
  }
  const signals = chains.flatMap(({ run, governs, groups: chain }) => chain.map((group, index) => {
    const signalId = `signal:${request.facilityId}:${group[0].blockId}:${governs}`;
    const last = index === chain.length - 1;
    const turnoutIds = group.flatMap((block) => block.turnouts.map((t) => t.turnoutId));
    const treatAsStop = last && run === 'withList' && request.exitEnd.kind === 'treatAsStop';
    return {
      signalId,
      name: request.signalNames[signalId] ?? `${group[0].name} ${governs === 'eastward' ? 'East' : 'West'}`,
      governs,
      run,
      entranceBlockId: group[0].blockId,
      protectedBlockIds: group.map((block) => block.blockId),
      turnoutIds,
      settings: { aspects: request.settings.aspects, approachLighting: request.settings.approachLighting },
      overridden: false,
      next: treatAsStop
        ? { kind: 'treatedAsStop' as const }
        : last ? { kind: 'nothingBeyond' as const } : { kind: 'signal' as const, signalId: 'next' },
      rules: [
        { condition: { kind: 'blockOccupied' as const }, indication: 'stop' as const },
        ...turnoutIds.map((turnoutId) => ({
          condition: { kind: 'turnoutNotLined' as const, turnoutId },
          indication: request.settings.turnoutNotLined === 'restricting' ? 'restricting' as const : 'stop' as const,
        })),
        treatAsStop
          ? { condition: { kind: 'clearNothingBeyondTreatedAsStop' as const }, indication: 'approach' as const }
          : { condition: { kind: 'blockClear' as const }, indication: 'clear' as const },
      ],
      observation: 'unavailable' as const,
    };
  }));
  return {
    facilityName: request.facilityName,
    direction: request.direction,
    movements: request.movements,
    blocks: request.blocks,
    entranceEnd: request.entranceEnd,
    exitEnd: request.exitEnd,
    signals,
    capability: { kind: 'incompletePreview', stage: 'unavailable', save: 'unavailable', apply: 'unavailable' },
    issues: request.blocks[0]?.blockId === 'block:river'
      ? [{ kind: 'missingDetectionFacts', blockId: 'block:river', missingStates: ['occupied', 'clear'] }]
      : [],
  };
}

export function lastRequest(invoke: Invoke): BlockSignalsPreviewRequest {
  const calls = invoke.mock.calls.filter(([command]) => command === 'preview_block_signals');
  return (calls[calls.length - 1][1] as { request: BlockSignalsPreviewRequest }).request;
}

export async function addBlock(name: string) {
  await fireEvent.click(screen.getByRole('button', { name: 'Add block…' }));
  const picker = await screen.findByRole('dialog', { name: /add block/i });
  await fireEvent.click(within(picker).getByLabelText(name));
  await fireEvent.click(within(picker).getByRole('button', { name: 'Confirm' }));
  await waitFor(() => expect(picker).not.toBeInTheDocument());
}

/** Route IPC through `ipc` stubs, falling back to the templates list and the fake preview. */
export function mockIpc(invoke: Invoke, ipc: IpcStubs = {}) {
  invoke.mockImplementation((command: string, args?: unknown) => {
    if (command in ipc) return Promise.resolve(ipc[command](args));
    if (command === 'sync_layout_drafts') return Promise.resolve(undefined);
    if (command === 'list_behavior_templates') {
      return Promise.resolve([
        { templateId: 'block-indicator', displayName: 'Block Indicator', slots: [], mapping: [] },
        {
          templateId: 'automatic-block-signals',
          displayName: 'Automatic Block Signals',
          slots: [{ label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 1, maxChannels: null }],
          mapping: [],
        },
      ]);
    }
    if (command === 'preview_block_signals') return Promise.resolve(fakePreview((args as { request: BlockSignalsPreviewRequest }).request));
    throw new Error(`unexpected IPC command: ${command}`);
  });
}

export async function renderRailroadPanel(resolvedEventIds?: ReadonlyMap<string, Record<string, string>>) {
  await behaviorTemplatesStore.loadBehaviorTemplates();
  // the route supplies Used by from the facade; do the same here
  const usedBy = (channelId: string) => effectiveLayoutStore.channelUsageMap.get(channelId) ?? [];
  render(RailroadPanel, { props: { nodeName: stubNodeName, resolvedEventIds, usedBy } });
}

export async function openEditor(
  invoke: Invoke,
  name = 'Cedar Hill — Eastward',
  resolvedEventIds?: ReadonlyMap<string, Record<string, string>>,
  ipc: IpcStubs = {},
) {
  channelsStore.hydrateBaseline(blocks);
  mockIpc(invoke, ipc);
  await renderRailroadPanel(resolvedEventIds);
  await fireEvent.click(screen.getByTestId('add-facility-button'));
  await startBlockSignals(name);
  return screen.getByRole('dialog', { name: /new automatic block signals/i });
}

export const SIGNAL_NODE = '020157000001';

export const hosts = [{
  nodeKey: SIGNAL_NODE,
  nodeName: 'Signal LCC 1',
  style: {
    styleId: 'two-driver-rg-signal',
    bindingKind: 'signalLccRgOutputs',
    appearances: { stop: ['red'], approach: ['red', 'green'], clear: ['green'] },
  },
  evidence: 'complete',
  outputs: [
    { ordinal: 1, label: '#1 H1-G', owner: null },
    { ordinal: 2, label: '#2 H2-R', owner: null },
    { ordinal: 3, label: 'H1-R', owner: null },
    { ordinal: 4, label: '#4 H2-G', owner: null },
  ],
}];

/** What compose plans for each wired signal: mast 1 and the lowest Logic, in wiring order. */
function plannedFor(request: { facilityId: string; wiring: Array<{ signalId: string }> }) {
  return {
    signalResources: request.wiring.map((w) => ({
      signalId: w.signalId,
      resources: { nodeKey: SIGNAL_NODE, mast: 1, trackCircuits: [] },
    })),
    logicPlacement: [{
      nodeKey: SIGNAL_NODE,
      groups: request.wiring.map((w, i) => ({ facilityId: request.facilityId, signalId: w.signalId, first: 1 + i, length: 1 })),
    }],
  };
}

/** IPC stubs for the Done flow: preview carries the Signal LCC host; compose reports `issues()` or one mast edit. */
export function signalIpc(
  issues: () => unknown[] = () => [],
  editValue: () => number = () => 3,
): IpcStubs {
  return {
    preview_block_signals: (args: unknown) => ({
      ...fakePreview((args as { request: BlockSignalsPreviewRequest }).request),
      outputHosts: hosts,
    }),
    compose_block_signals: (args: unknown) => issues().length > 0
      ? { edits: [], signalResources: [], logicPlacement: [], issues: issues() }
      : {
        edits: [{ nodeKey: SIGNAL_NODE, space: 253, address: 100, path: 'mast#1', value: { type: 'int', value: editValue() } }],
        ...plannedFor((args as { request: Parameters<typeof plannedFor>[0] }).request),
        issues: [],
      },
    sync_layout_drafts: () => undefined,
    clear_layout_drafts: () => undefined,
  };
}

/** Addresses of the Logic event leaves a removal replaces with fresh event IDs. */
const REMOVAL_EVENT_ADDRESSES = [202, 210];

/** Give the Signal LCC a node tree holding the Logic event leaves, as a loaded node has. */
export function seedSignalNodeTree() {
  nodeTreeStore.setTree(SIGNAL_NODE, {
    nodeId: SIGNAL_NODE,
    identity: null,
    connectorProfile: null,
    profileApplied: true,
    unknownVariants: [],
    segments: [{
      name: 'Conditionals',
      description: null,
      origin: 0,
      space: 253,
      children: REMOVAL_EVENT_ADDRESSES.map((address) => ({
        kind: 'leaf',
        name: `Event ${address}`,
        description: null,
        elementType: 'eventId',
        address,
        size: 8,
        space: 253,
        path: ['Conditionals', `Event ${address}`],
        value: { type: 'eventId', bytes: [1, 1, 1, 1, 1, 1, 0, 1], hex: '0101010101010001' },
        eventRole: 'Consumer',
        constraints: null,
        readOnly: false,
      })),
    }],
  } as unknown as Parameters<typeof nodeTreeStore.setTree>[1]);
}

/**
 * IPC stub for removing the saved facility: seven release edits (mast 1
 * function, description and one lamp; Logic 1 function, description and two
 * fresh event IDs), or `issues`. Call `seedSignalNodeTree()` first.
 */
export function removalIpc(issues: () => unknown[] = () => []): IpcStubs {
  const release = (address: number, path: string, value: unknown) => ({ nodeKey: SIGNAL_NODE, space: 253, address, path, value });
  return {
    ...signalIpc(),
    compose_block_signals: () => issues().length > 0
      ? { edits: [], signalResources: [], logicPlacement: [], issues: issues() }
      : {
        edits: [
          release(100, 'mast#1/function', { type: 'int', value: 0 }),
          release(101, 'mast#1/description', { type: 'string', value: '' }),
          release(102, 'mast#1/lamp', { type: 'int', value: 0 }),
          release(200, 'logic#1/function', { type: 'int', value: 0 }),
          release(201, 'logic#1/description', { type: 'string', value: '' }),
          ...REMOVAL_EVENT_ADDRESSES.map((address) => release(address, `logic#1/event@${address}`, { type: 'freshEventId' })),
        ],
        signalResources: [],
        logicPlacement: [{ nodeKey: SIGNAL_NODE, groups: [] }],
        issues: [],
      },
  };
}

/**
 * IPC stub for releasing a pin held outside Bowties: pins 3 and 4 are held by an
 * untracked "Mast 2" (Logic 1 sets its aspects) until a request names it in
 * `release`; compose then returns the signal's mast edit plus Mast 2's reset
 * (a function edit and two fresh event IDs). Call `seedSignalNodeTree()` first.
 */
export function releaseOutsideIpc(): IpcStubs {
  const isReleased = (request: { release?: Array<{ owner: string }> }) =>
    (request.release ?? []).some((entry) => entry.owner === 'Mast 2');
  const reset = (address: number, path: string, value: unknown) => ({ nodeKey: SIGNAL_NODE, space: 253, address, path, value });
  return {
    ...signalIpc(),
    preview_block_signals: (args: unknown) => {
      const request = (args as { request: BlockSignalsPreviewRequest & { release?: Array<{ owner: string }> } }).request;
      const free = isReleased(request);
      return {
        ...fakePreview(request),
        outputHosts: [{
          ...hosts[0],
          outputs: [
            { ordinal: 1, label: '#1 H1-G', owner: null },
            { ordinal: 2, label: '#2 H2-R', owner: null },
            { ordinal: 3, label: '#3 H3-R', owner: free ? null : 'Mast 2' },
            { ordinal: 4, label: '#4 H3-G', owner: free ? null : 'Mast 2' },
          ],
        }],
        outsideOwners: free ? [] : [{ nodeKey: SIGNAL_NODE, owner: 'Mast 2', matchedLogic: [1], keptLogic: [] }],
      };
    },
    compose_block_signals: (args: unknown) => {
      const request = (args as { request: Parameters<typeof plannedFor>[0] & { release?: Array<{ owner: string }> } }).request;
      return {
        edits: [
          { nodeKey: SIGNAL_NODE, space: 253, address: 100, path: 'mast#1', value: { type: 'int', value: 3 } },
          ...(isReleased(request)
            ? [
              reset(300, 'mast#2/function', { type: 'int', value: 0 }),
              ...REMOVAL_EVENT_ADDRESSES.map((address) => reset(address, `logic#1/event@${address}`, { type: 'freshEventId' })),
            ]
            : []),
        ],
        ...plannedFor(request),
        issues: [],
      };
    },
  };
}

export async function chooseWiringNode(editor: HTMLElement) {
  await fireEvent.click(await within(editor).findByRole('button', { name: /Runs on/ }));
  const picker = await screen.findByRole('dialog', { name: 'Choose a Signal LCC' });
  await fireEvent.click(within(picker).getByRole('radio', { name: /02.01.57.00.00.01/ }));
  await fireEvent.click(within(picker).getByRole('button', { name: 'Confirm' }));
}

export async function wireFirstSignal(editor: HTMLElement) {
  await addBlock('Cedar Hill');
  await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Aspects' }));
  await fireEvent.click(await within(editor).findByRole('button', { name: 'Next: Appearance' }));
  await fireEvent.click(within(editor).getByRole('button', { name: 'Next: Wiring' }));
  await chooseWiringNode(editor);
  const grid = await within(editor).findByRole('group', { name: 'Pins on 02.01.57.00.00.01' });
  await fireEvent.click(within(grid).getByRole('button', { name: /H1-R/ }));
  await fireEvent.click(within(grid).getByRole('button', { name: /#1 H1-G/ }));
}

export const SAVED_FACILITY_NAME = 'Cedar Hill — Eastward';
export const SAVED_FACILITY_ID = 'b01e7a6a-51a3-41d3-a340-b83ca1aa7c34';
export const SAVED_SIGNAL_CHANNEL_ID = 'f741d85e-a696-4741-b79a-21a857f9d867';

/** The layout as it loads from disk: one saved one-block Automatic Block Signals facility on the Signal LCC (red #3, green #1). */
export function seedSavedBlockSignals() {
  const facility = {
    facilityId: SAVED_FACILITY_ID,
    templateId: 'automatic-block-signals',
    name: SAVED_FACILITY_NAME,
    slotBindings: { signals: [SAVED_SIGNAL_CHANNEL_ID] },
    blockSignals: {
      direction: 'eastward',
      movements: 'oneDirection',
      entranceEnd: { kind: 'noPlant' },
      exitEnd: { kind: 'nothingBeyond' },
      blocks: [{ blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] }],
      settings: { aspects: 3, turnoutNotLined: 'stop', approachLighting: 'alwaysLit' },
      signals: [{
        signalId: `signal:${SAVED_FACILITY_ID}:block:cedar:eastward`,
        name: 'Cedar Hill East',
        protectedBlock: 'block:cedar',
        channelId: SAVED_SIGNAL_CHANNEL_ID,
        resources: { nodeKey: SIGNAL_NODE, mast: 1, trackCircuits: [] },
      }],
    },
  } as Facility;
  const signalChannel = {
    id: SAVED_SIGNAL_CHANNEL_ID,
    name: 'Cedar Hill East',
    role: 'signal-indication',
    style: 'two-driver-rg-signal',
    ownership: 'user-owned',
    binding: { kind: 'signalLccRgOutputs', nodeKey: SIGNAL_NODE, greenOutput: 1, redOutput: 3 },
  } as InformationChannel;
  channelsStore.hydrateBaseline([...blocks, signalChannel]);
  facilitiesStore.hydrateBaseline([facility], [{
    nodeKey: SIGNAL_NODE,
    groups: [{ facilityId: SAVED_FACILITY_ID, signalId: `signal:${SAVED_FACILITY_ID}:block:cedar:eastward`, first: 1, length: 1 }],
  }]);
}

// ── Panel facility (Spec 021) ────────────────────────────────────────────

const PANEL_TEMPLATE = {
  templateId: 'panel',
  displayName: 'Panel',
  slots: [{ label: 'lamps', displayLabel: 'lamps', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 0, maxChannels: null }],
  mapping: [],
};

/**
 * IPC stubs for the Panel flow: the template list, `panel_wiring_facts` returning `facts`, and
 * `compose_panel` returning one Lamp Selection edit per requested lamp (row = pin number).
 */
export function panelIpc(facts: unknown): IpcStubs {
  return {
    list_behavior_templates: () => [
      { templateId: 'block-indicator', displayName: 'Block Indicator', slots: [], mapping: [] },
      PANEL_TEMPLATE,
    ],
    panel_wiring_facts: () => facts,
    compose_panel: (args: unknown) => {
      const { lamps } = (args as { request: { lamps: Array<{ entryId: string; nodeKey: string; output: number }> } }).request;
      return {
        edits: lamps.map((lamp) => ({
          nodeKey: lamp.nodeKey,
          space: 253,
          address: 1000 + lamp.output,
          path: `lamp#${lamp.output}/selection`,
          value: { type: 'int', value: lamp.output },
        })),
        assignments: lamps.map((lamp) => ({ entryId: lamp.entryId, nodeKey: lamp.nodeKey, row: lamp.output })),
        issues: [],
      };
    },
    sync_layout_drafts: () => undefined,
    clear_layout_drafts: () => undefined,
  };
}

/** Open the Panel editor through + Add facility, then Panel, on the Railroad panel. */
export async function openPanelEditor(invoke: Invoke, ipc: IpcStubs, name = 'Eagle Creek fascia') {
  channelsStore.hydrateBaseline(blocks);
  mockIpc(invoke, ipc);
  await renderRailroadPanel();
  await fireEvent.click(screen.getByTestId('add-facility-button'));
  await fireEvent.click(screen.getByRole('radio', { name: /^Panel/ }));
  await fireEvent.input(screen.getByLabelText('Name'), { target: { value: name } });
  await fireEvent.click(screen.getByRole('button', { name: 'Add facility' }));
  return screen.getByRole('dialog', { name });
}

/** Wire the listed blocks (in panel order) to board pins: select the block, then click its pin. */
export async function wirePanelBlocks(editor: HTMLElement, wiring: Array<[blockName: string, pin: RegExp]>) {
  for (const [blockName, pin] of wiring) {
    await fireEvent.click(within(within(editor).getByRole('listbox', { name: 'Panel blocks' })).getByText(blockName));
    await fireEvent.click(within(editor).getByRole('button', { name: pin }));
  }
}
