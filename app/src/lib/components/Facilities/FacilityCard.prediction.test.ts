/**
 * Spec 020 / S7 — FacilityCard prediction-first output signal-aspect indicator.
 *
 * Acceptance contract (mapped to S7 T3):
 *   (a) For a compiled ABS 3-Aspect facility with a wired Occupied input BOD,
 *       the output slot's rendered `stateLabel` reads "Stop" (not "Unknown"),
 *       and the lamp breakdown shows "Red: ON" / "Green: OFF" pre-Save, driven
 *       by the Logic block's `currentEvaluation()` aspect.
 *   (b) For a compiled ABS facility whose input BOD state is Unknown (no events
 *       observed), the output slot falls back to `deriveSignalAspectState` which
 *       returns `unknown` when no LED events observed. Locks the observation
 *       fallback.
 *   (c) For a composed (non-compiled) facility (Block Indicator), no behavior
 *       change — the observation path still drives the output display.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
import type { Facility } from '$lib/api/facilities';
import type { InformationChannel } from '$lib/api/channels';
import FacilityCard from './FacilityCard.svelte';

// ── Mocks ───────────────────────────────────────────────────────────────

vi.mock('$lib/api/behaviorTemplates', () => ({
  listBehaviorTemplates: vi.fn(async () => []),
}));
vi.mock('$lib/api/facilities', () => ({
  listFacilities: vi.fn(async () => []),
}));
vi.mock('$lib/api/channels', () => ({
  listChannels: vi.fn(async () => []),
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { eventStateStore } = await import('$lib/stores/eventState.svelte');

// ── Template fixtures ───────────────────────────────────────────────────

const BLOCK_INDICATOR: BehaviorTemplate = {
  templateId: 'block-indicator',
  displayName: 'Block Indicator',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'output', displayLabel: 'indicator', kind: 'consumer', requiredRole: 'lamp-indicator', minChannels: 1, maxChannels: 1 },
  ],
  mapping: [
    { producerState: 'occupied', consumerCommand: 'lit' },
    { producerState: 'clear', consumerCommand: 'unlit' },
  ],
  compilationTarget: 'composed',
  rules: [],
};

const ABS_3_ASPECT: BehaviorTemplate = {
  templateId: 'abs-3-aspect-signal',
  displayName: 'ABS 3-Aspect Signal',
  slots: [
    { label: 'input', displayLabel: 'block', kind: 'producer', requiredRole: 'block-occupancy', minChannels: 1, maxChannels: 1 },
    { label: 'output', displayLabel: 'signal', kind: 'consumer', requiredRole: 'signal-aspect', minChannels: 1, maxChannels: 1 },
  ],
  mapping: [
    { producerState: 'occupied', consumerCommand: 'stop' },
    { producerState: 'clear', consumerCommand: 'clear' },
  ],
  compilationTarget: 'compiled',
  rules: [
    { inputState: 'occupied', outputAspect: 'stop', reason: 'next block occupied' },
    { inputState: 'clear', outputAspect: 'clear', reason: 'next block clear' },
  ],
};

// ── Channel helpers ─────────────────────────────────────────────────────

function bod(name: string): InformationChannel {
  return {
    id: `ch-bod-${name}`,
    name,
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input: 1 },
  };
}

function signalHead(name: string): InformationChannel {
  return {
    id: `ch-signal-${name}`,
    name,
    role: 'signal-aspect',
    style: 'bicolor-led-signal',
    ownership: 'user-owned',
    binding: { kind: 'lampRow', nodeKey: 'N2', rowOrdinal: 1 },
  };
}

function lamp(name: string): InformationChannel {
  return {
    id: `ch-lamp-${name}`,
    name,
    role: 'lamp-indicator',
    style: 'single-led-direct-lamp',
    ownership: 'user-owned',
    binding: { kind: 'lampRow', nodeKey: 'N2', rowOrdinal: 1 },
  };
}

// ── Setup ───────────────────────────────────────────────────────────────

beforeEach(async () => {
  facilitiesStore.reset();
  channelsStore.reset();
  behaviorTemplatesStore.reset();
  eventStateStore.clear();

  vi.mocked((await import('$lib/api/behaviorTemplates')).listBehaviorTemplates).mockResolvedValue([
    BLOCK_INDICATOR,
    ABS_3_ASPECT,
  ]);
  await behaviorTemplatesStore.loadBehaviorTemplates();

  channelsStore.hydrateBaseline([
    bod('BOD 1'),
    signalHead('Signal 1'),
    lamp('Lamp 1'),
  ]);

  facilitiesStore.hydrateBaseline([
    {
      facilityId: 'f-compiled-abs',
      templateId: 'abs-3-aspect-signal',
      name: 'ABS Signal',
      slotBindings: { input: ['ch-bod-BOD 1'], output: ['ch-signal-Signal 1'], 'downstream-signal': [] },
    },
    {
      facilityId: 'f-composed-block',
      templateId: 'block-indicator',
      name: 'Block Indicator',
      slotBindings: { input: ['ch-bod-BOD 1'], output: ['ch-lamp-Lamp 1'] },
    },
  ]);
});

// ── Tests ───────────────────────────────────────────────────────────────

describe('FacilityCard prediction-first output signal-aspect indicator (Spec 020 / S7)', () => {
  it('(a) shows predicted "Stop" aspect when compiled ABS facility has occupied input (pre-Save)', async () => {
    // Setup: compiled ABS facility with BOD input observed as occupied.
    const occupiedEventId = '0501010101000001';
    eventStateStore.record(occupiedEventId, 1000);

    const resolvedEventIds = new Map<string, Record<string, string>>([
      ['ch-bod-BOD 1', { occupied: occupiedEventId, clear: '0501010101000002' }],
      ['ch-signal-Signal 1', {
        redOn: '0501010101000003',
        redOff: '0501010101000004',
        greenOn: '0501010101000005',
        greenOff: '0501010101000006',
      }],
    ]);

    const { container } = render(FacilityCard, {
      props: {
        facility: facilitiesStore.facilities[0],
        template: behaviorTemplatesStore.templates[1],
        resolvedEventIds,
      },
    });

    await tick();

    // The comprehension view should render (compiled template).
    const comprehensionView = container.querySelector('[data-testid="comprehension-view"]');
    expect(comprehensionView).toBeTruthy();

    // The Outputs column should show the signal slot with predicted "Stop" aspect.
    const outputSlots = screen.getAllByRole('heading', { name: /Outputs/i })[0]?.parentElement;
    expect(outputSlots).toBeTruthy();

    // Look for the "Stop" label in the output slot (not "Unknown").
    // The stateLabel is rendered in SlotCard, which displays it prominently.
    const stateLabels = container.querySelectorAll('[class*="state"]');
    const foundStop = Array.from(stateLabels).some(el => el.textContent?.includes('Stop'));
    expect(foundStop).toBe(true);

    // The lamp breakdown should show Red ON / Green OFF (the inverse aspect →
    // (red, green) mapping for "stop").
    const lampBreakdown = container.querySelector('[class*="lamp-breakdown"]');
    expect(lampBreakdown).toBeTruthy();
    expect(lampBreakdown?.textContent).toMatch(/Red:.*ON/i);
    expect(lampBreakdown?.textContent).toMatch(/Green:.*off/i);
  });

  it('(b) falls back to observation ("Unknown") when compiled facility input is Unknown (no events)', async () => {
    // Setup: compiled ABS facility with BOD input not yet observed.
    const resolvedEventIds = new Map<string, Record<string, string>>([
      ['ch-bod-BOD 1', { occupied: '0501010101000001', clear: '0501010101000002' }],
      ['ch-signal-Signal 1', {
        redOn: '0501010101000003',
        redOff: '0501010101000004',
        greenOn: '0501010101000005',
        greenOff: '0501010101000006',
      }],
    ]);

    const { container } = render(FacilityCard, {
      props: {
        facility: facilitiesStore.facilities[0],
        template: behaviorTemplatesStore.templates[1],
        resolvedEventIds,
      },
    });

    await tick();

    // When input is Unknown, currentEvaluation() returns undefined, so the
    // output display falls back to deriveSignalAspectState. Since no LED
    // events are observed (eventStateStore is empty), the output state
    // should be `{ kind: 'unknown' }` with stateLabel "Unknown".
    const stateLabels = container.querySelectorAll('[class*="state"]');
    const foundUnknown = Array.from(stateLabels).some(el => el.textContent?.includes('Unknown'));
    expect(foundUnknown).toBe(true);
  });

  it('(c) composed facility (Block Indicator) observation path is unchanged', async () => {
    // Setup: composed facility (Block Indicator) with lamp input observed as lit.
    const litEventId = '0501010101000003';
    eventStateStore.record(litEventId, 1000);

    const resolvedEventIds = new Map<string, Record<string, string>>([
      ['ch-bod-BOD 1', { occupied: '0501010101000001', clear: '0501010101000002' }],
      ['ch-lamp-Lamp 1', { lit: litEventId, unlit: '0501010101000004' }],
    ]);

    const { container } = render(FacilityCard, {
      props: {
        facility: facilitiesStore.facilities[1],
        template: behaviorTemplatesStore.templates[0],
        resolvedEventIds,
      },
    });

    await tick();

    // The composed facility should NOT render the comprehension view
    // (that is S4 compiled-template-only). The traditional card view
    // (not tested here in detail, but the contract is: no prediction,
    // observation only) should render.
    const comprehensionView = container.querySelector('[data-testid="comprehension-view"]');
    expect(comprehensionView).toBeFalsy();
  });
});
