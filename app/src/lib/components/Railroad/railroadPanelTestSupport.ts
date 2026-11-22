/** Shared helpers for the RailroadPanel integration tests (Automatic Block Signals editor). IPC is the only mock. */
import { expect, type Mock } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import RailroadPanel from './RailroadPanel.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
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
  await fireEvent.click(within(picker).getByRole('radio', { name }));
  await fireEvent.click(within(picker).getByRole('button', { name: 'Confirm' }));
  await waitFor(() => {
    expect(screen.queryByRole('dialog', { name: /add block/i })).not.toBeInTheDocument();
  });
}

/** Route IPC through `ipc` stubs, falling back to the templates list and the fake preview. */
export function mockIpc(invoke: Invoke, ipc: IpcStubs = {}) {
  invoke.mockImplementation((command: string, args?: unknown) => {
    if (command in ipc) return Promise.resolve(ipc[command](args));
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
    compose_block_signals: () => issues().length > 0
      ? { edits: [], claimedSlots: [], issues: issues() }
      : {
        edits: [{ nodeKey: SIGNAL_NODE, space: 253, address: 100, path: 'mast#1', value: { type: 'int', value: editValue() } }],
        claimedSlots: [{ nodeKey: SIGNAL_NODE, kind: 'mast', index: 1 }],
        issues: [],
      },
    sync_layout_drafts: () => undefined,
    clear_layout_drafts: () => undefined,
    discard_modified_values: () => 0,
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
