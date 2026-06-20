/**
 * T015: Vitest unit tests for BowtieCard.svelte
 * TDD — written first; must FAIL until BowtieCard.svelte exists.
 *
 * Covers (FR-014, FR-004, FR-002):
 * - Renders card header: shows name if present, event_id_hex otherwise (FR-014)
 * - Renders producer column entries (FR-004)
 * - Renders consumer column entries (FR-004)
 * - Renders ambiguous_entries section when non-empty
 * - Hides ambiguous_entries section when ambiguous_entries is empty
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import BowtieCard from './BowtieCard.svelte';
import type { BowtieCard as BowtieCardType } from '$lib/api/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Regression: clicking Add producer/consumer must not trigger config navigation.
const { focusConfigFieldMock } = vi.hoisted(() => ({
  focusConfigFieldMock: vi.fn(),
}));
vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: {
    focusConfigField: focusConfigFieldMock,
    get navigationRequest() { return null; },
    get leafFocusRequest() { return null; },
    clearNavigation: vi.fn(),
    clearLeafFocus: vi.fn(),
    clearFocus: vi.fn(),
  },
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeEntry(overrides: { node_name?: string; element_label?: string } = {}) {
  return {
    node_key: '020157000001',
    node_name: overrides.node_name ?? 'Signal Node',
    element_path: ['seg:0', 'elem:0', 'elem:0'],
    element_label: overrides.element_label ?? 'Output Active',
    event_id: [5, 2, 1, 0, 0, 1, 0, 0],
    role: 'Producer' as const,
  };
}

function makeCard(overrides: Partial<BowtieCardType> = {}): BowtieCardType {
  return {
    event_id_hex: '05.02.01.00.00.01.00.00',
    event_id_bytes: [5, 2, 1, 0, 0, 1, 0, 0],
    producers: [makeEntry({ node_name: 'Button Node', element_label: 'Button Pressed' })],
    consumers: [{ ...makeEntry({ node_name: 'Signal Node', element_label: 'Go to Green' }), role: 'Consumer' as const }],
    ambiguous_entries: [],
    name: null,
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('BowtieCard.svelte', () => {
  beforeEach(() => {
    focusConfigFieldMock.mockClear();
  });

  // FR-014: Card header shows name if present, event_id_hex if not.

  it('shows event_id_hex as header when name is null (FR-014)', () => {
    render(BowtieCard, { props: { card: makeCard({ name: null }) } });
    // event_id_hex appears in both the header and ConnectorArrow; check at least one exists.
    expect(screen.getAllByText('05.02.01.00.00.01.00.00').length).toBeGreaterThanOrEqual(1);
  });

  it('shows the assigned name in header when name is set (FR-014)', () => {
    render(BowtieCard, {
      props: { card: makeCard({ name: 'Yard Button → Signal' }) },
    });
    expect(screen.getByText('Yard Button → Signal')).toBeInTheDocument();
  });

  // FR-004: Renders producer and consumer columns.

  it('renders producer entry node_name', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Button Node')).toBeInTheDocument();
  });

  it('renders producer entry element_label', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Button Pressed')).toBeInTheDocument();
  });

  it('renders consumer entry node_name', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Signal Node')).toBeInTheDocument();
  });

  it('renders consumer entry element_label', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Go to Green')).toBeInTheDocument();
  });

  // Ambiguous entries section.

  it('shows ambiguous section when ambiguous_entries is non-empty', () => {
    const card = makeCard({
      ambiguous_entries: [
        { ...makeEntry({ node_name: 'Unknown Node', element_label: 'Mystery Slot' }), role: 'Ambiguous' as const },
      ],
    });
    render(BowtieCard, { props: { card } });
    // Use the unique element_label from the ambiguous entry to confirm the section rendered.
    expect(screen.getByText('Mystery Slot')).toBeInTheDocument();
  });

  it('hides ambiguous section when ambiguous_entries is empty', () => {
    render(BowtieCard, { props: { card: makeCard({ ambiguous_entries: [] }) } });
    expect(screen.queryByText(/needs clarification/i)).not.toBeInTheDocument();
  });

  // FR-004: Three-column layout markers.

  it('renders a connector arrow between columns', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    // ConnectorArrow renders an arrow element containing '→'
    expect(screen.getByText('→')).toBeInTheDocument();
  });

  // Regression: Add producer / Add consumer must not trigger config navigation.
  // Root cause was that configFocusStore.pendingFocus was never cleared after
  // the +page.svelte effect consumed it. When ElementPicker loaded trees via
  // nodeTreeStore.loadTree() the effect re-ran, found pendingFocus still set,
  // and erroneously switched the active tab back to config.

  it('clicking Add producer does not call configFocusStore.focusConfigField (regression)', async () => {
    const onAddProducer = vi.fn();
    render(BowtieCard, { props: { card: makeCard(), onAddProducer } });
    const btn = screen.getByRole('button', { name: /\+ add producer/i });
    await fireEvent.click(btn);
    expect(focusConfigFieldMock).not.toHaveBeenCalled();
    expect(onAddProducer).toHaveBeenCalledOnce();
  });

  it('clicking Add consumer does not call configFocusStore.focusConfigField (regression)', async () => {
    const onAddConsumer = vi.fn();
    render(BowtieCard, { props: { card: makeCard(), onAddConsumer } });
    const btn = screen.getByRole('button', { name: /\+ add consumer/i });
    await fireEvent.click(btn);
    expect(focusConfigFieldMock).not.toHaveBeenCalled();
    expect(onAddConsumer).toHaveBeenCalledOnce();
  });

  it('clicking a producer element\'s label link calls configFocusStore.focusConfigField', async () => {
    render(BowtieCard, { props: { card: makeCard() } });
    const link = screen.getByRole('button', { name: /go to button pressed in configuration/i });
    await fireEvent.click(link);
    expect(focusConfigFieldMock).toHaveBeenCalledWith(
      '020157000001',
      ['seg:0', 'elem:0', 'elem:0'],
    );
  });

  // Regression: state_proxy_equality_mismatch on ambiguous entry click (T037).
  // Root cause: comparing $state proxy to reactive-loop variable with === fails.
  // The comparison always returns false, so RoleClassifyPrompt never renders
  // and onReclassifyRole never fires when user clicks the ? button.
  // Fix: store composite slot key (string) instead of object reference.

  it('clicking ambiguous entry ? button calls onReclassifyRole (regression T037)', async () => {
    const ambiguousEntry = { ...makeEntry({ node_name: 'Unknown', element_label: 'Unclear Slot' }), role: 'Ambiguous' as const };
    const onReclassifyRole = vi.fn();
    const card = makeCard({
      ambiguous_entries: [ambiguousEntry],
    });
    render(BowtieCard, { props: { card, onReclassifyRole } });

    // Find the ? button by aria-label (unique per ambiguous entry)
    const classifyBtn = screen.getByRole('button', { name: /classify role for unclear slot/i });
    expect(classifyBtn).toBeInTheDocument();

    // Click the ? button
    await fireEvent.click(classifyBtn);

    // Before fix: RoleClassifyPrompt does not render; clicking changes internal state
    // but the conditional {#if reclassifyingEntry === entry} stays false.
    // After fix: RoleClassifyPrompt should render with the correct entry.
    // We verify this indirectly: after clicking, there should be a confirmation
    // dialog visible. Check for text that only appears in RoleClassifyPrompt.

    // RoleClassifyPrompt renders "Producer" and "Consumer" buttons.
    // This assertion verifies the prompt component is now visible.
    expect(screen.getByRole('button', { name: /Producer/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Consumer/i })).toBeInTheDocument();
  });

  it('keeps ambiguous slot details visible while classify prompt is open', async () => {
    const ambiguousEntry = {
      ...makeEntry({ node_name: 'Unknown Node', element_label: 'Unclear Slot' }),
      role: 'Ambiguous' as const,
    };
    const card = makeCard({
      ambiguous_entries: [ambiguousEntry],
    });
    render(BowtieCard, { props: { card } });

    const classifyBtn = screen.getByRole('button', { name: /classify role for unclear slot/i });
    await fireEvent.click(classifyBtn);

    expect(screen.getByText('Unknown Node')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Producer/i })).toBeInTheDocument();
  });

  it('renders the classify prompt in a sidecar panel beside the ambiguous slot', async () => {
    const ambiguousEntry = {
      ...makeEntry({ node_name: 'Unknown Node', element_label: 'Unclear Slot' }),
      role: 'Ambiguous' as const,
    };
    const card = makeCard({
      ambiguous_entries: [ambiguousEntry],
    });
    const { container } = render(BowtieCard, { props: { card } });

    const classifyBtn = screen.getByRole('button', { name: /classify role for unclear slot/i });
    await fireEvent.click(classifyBtn);

    expect(container.querySelector('.ambiguous-classify-sidecar')).not.toBeNull();
    expect(container.querySelector('.ambiguous-classify-popover')).toBeNull();
  });

  it('selecting role in ambiguous entry prompt fires onReclassifyRole (regression T037)', async () => {
    const ambiguousEntry = { ...makeEntry({ node_name: 'Unknown', element_label: 'Unclear Slot' }), role: 'Ambiguous' as const };
    const onReclassifyRole = vi.fn();
    const card = makeCard({
      ambiguous_entries: [ambiguousEntry],
    });
    render(BowtieCard, { props: { card, onReclassifyRole } });

    // Click the ? button to show the prompt
    const classifyBtn = screen.getByRole('button', { name: /classify role for unclear slot/i });
    await fireEvent.click(classifyBtn);

    // Now the RoleClassifyPrompt should be visible. Click "Producer" role.
    const producerBtn = screen.getByRole('button', { name: /Producer/i });
    await fireEvent.click(producerBtn);

    // Verify onReclassifyRole was called with the correct arguments
    expect(onReclassifyRole).toHaveBeenCalledWith(
      ambiguousEntry.node_key,
      ambiguousEntry.element_path,
      'Producer',
    );
  });
});
