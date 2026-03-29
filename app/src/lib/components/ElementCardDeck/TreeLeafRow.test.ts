/**
 * Spec 007: Tests for TreeLeafRow.svelte
 *
 * Covers:
 * - Displays leaf name and formatted value (horizontal layout)
 * - Inline descriptions visible by default, truncation toggle for long text
 * - Enum values mapped to labels via mapEntries
 * - Event IDs: monospace dotted hex, "(not set)" for all-zeros
 * - Event role badge displays correctly
 * - "Used in" bowtie cross-reference link
 *
 * Updated for plan-cdiConfigNavigator Steps 4-6.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import TreeLeafRow from './TreeLeafRow.svelte';
import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
import type { BowtieCard } from '$lib/api/tauri';
import { setModifiedValue, triggerAction } from '$lib/api/config';

// Mock $app/navigation
vi.mock('$app/navigation', () => ({
  goto: vi.fn(),
}));

// Mock bowties store
vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    nodeSlotMap: new Map(),
    effectiveNodeSlotMap: new Map(),
    getDisplayName: vi.fn((id: string) => id),
  },
}));

vi.mock('$lib/stores/bowtieFocus.svelte', () => ({
  bowtieFocusStore: {
    highlightedEventIdHex: null,
    focusBowtie: vi.fn(),
    clearFocus: vi.fn(),
  },
}));

vi.mock('$lib/stores/configFocus.svelte', () => ({
  configFocusStore: {
    navigationRequest: null,
    leafFocusRequest: null,
    focusConfigField: vi.fn(),
    clearNavigation: vi.fn(),
    clearLeafFocus: vi.fn(),
    clearFocus: vi.fn(),
  },
}));

vi.mock('$lib/api/config', () => ({
  setModifiedValue: vi.fn(),
  triggerAction: vi.fn().mockResolvedValue(undefined),
}));

function makeLeaf(overrides: Partial<LeafConfigNode> = {}): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Test Field',
    description: null,
    elementType: 'int',
    address: 0,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0'],
    value: null,
    eventRole: null,
    constraints: null,
    ...overrides,
  };
}

function makeBowtie(overrides: Partial<BowtieCard> = {}): BowtieCard {
  return {
    event_id_hex: '05.02.01.02.03.00.00.01',
    user_names: ['Test Bowtie'],
    producers: [],
    consumers: [],
    ambiguous: [],
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('TreeLeafRow.svelte', () => {
  describe('basic rendering', () => {
    it('displays the leaf name', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ name: 'Speed Limit' }) } });
      expect(screen.getByText('Speed Limit')).toBeInTheDocument();
    });

    it('displays "—" when value is null', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ value: null }) } });
      expect(screen.getByText('—')).toBeInTheDocument();
    });

    it('displays integer value', () => {
      const value: TreeConfigValue = { type: 'int', value: 42 };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ value }) } });
      expect(screen.getByText('42')).toBeInTheDocument();
    });

    it('displays string value', () => {
      const value: TreeConfigValue = { type: 'string', value: 'Hello World' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'string', value }) } });
      expect(screen.getByText('Hello World')).toBeInTheDocument();
    });

    it('displays "(empty)" for empty string value', () => {
      const value: TreeConfigValue = { type: 'string', value: '' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'string', value }) } });
      expect(screen.getByText('(empty)')).toBeInTheDocument();
    });

    it('displays float value with precision', () => {
      const value: TreeConfigValue = { type: 'float', value: 3.14159 };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'float', value }) } });
      expect(screen.getByText('3.1416')).toBeInTheDocument();
    });

    it('displays event ID bytes in hex format', () => {
      const value: TreeConfigValue = { type: 'eventId', bytes: [5, 2, 1, 2, 3, 0, 0, 1], hex: '05.02.01.02.03.00.00.01' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'eventId', value }) } });
      expect(screen.getByText('05.02.01.02.03.00.00.01')).toBeInTheDocument();
    });

    it('displays "(not set)" for all-zero event ID', () => {
      const value: TreeConfigValue = { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0], hex: '00.00.00.00.00.00.00.00' };
      render(TreeLeafRow, { props: { leaf: makeLeaf({ elementType: 'eventId', value }) } });
      expect(screen.getByText('(not set)')).toBeInTheDocument();
    });
  });

  describe('enum value mapping', () => {
    it('maps int value to enum label when mapEntries exist', () => {
      const value: TreeConfigValue = { type: 'int', value: 2 };
      const leaf = makeLeaf({
        value,
        constraints: {
          min: 0,
          max: 3,
          defaultValue: null,
          mapEntries: [
            { value: 0, label: 'Off' },
            { value: 1, label: 'On' },
            { value: 2, label: 'Toggle' },
            { value: 3, label: 'Hold' },
          ],
        },
      });
      render(TreeLeafRow, { props: { leaf } });
      expect(screen.getByText('Toggle')).toBeInTheDocument();
    });

    it('falls back to raw number when no matching mapEntry', () => {
      const value: TreeConfigValue = { type: 'int', value: 99 };
      const leaf = makeLeaf({
        value,
        constraints: {
          min: 0,
          max: 3,
          defaultValue: null,
          mapEntries: [
            { value: 0, label: 'Off' },
            { value: 1, label: 'On' },
          ],
        },
      });
      render(TreeLeafRow, { props: { leaf } });
      expect(screen.getByText('99')).toBeInTheDocument();
    });
  });

  describe('inline descriptions', () => {
    it('shows short description inline by default (no toggle needed)', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: 'Short description' }) },
      });
      expect(screen.getByText('Short description')).toBeInTheDocument();
    });

    it('truncates long descriptions with expand button', () => {
      const longDesc = 'A'.repeat(130); // > 120 char threshold
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: longDesc }) },
      });
      // Should show truncated text (100 chars + "…")
      expect(screen.getByText(/^A{100}…$/)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /expand/i })).toBeInTheDocument();
    });

    it('expands truncated description when [+] clicked', async () => {
      const longDesc = 'B'.repeat(130);
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ description: longDesc }) },
      });
      const expandBtn = screen.getByRole('button', { name: /expand/i });
      await fireEvent.click(expandBtn);
      expect(screen.getByText(longDesc)).toBeInTheDocument();
    });

    it('does not show description when null', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ description: null }) } });
      // Only name + value, nothing else
      expect(screen.queryByText(/…/)).not.toBeInTheDocument();
    });
  });

  describe('event role display', () => {
    it('displays Producer role badge', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ eventRole: 'Producer' }) },
      });
      expect(screen.getByText('Producer')).toBeInTheDocument();
    });

    it('displays Consumer role badge', () => {
      render(TreeLeafRow, {
        props: { leaf: makeLeaf({ eventRole: 'Consumer' }) },
      });
      expect(screen.getByText('Consumer')).toBeInTheDocument();
    });

    it('does not show role section when eventRole is null', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf({ eventRole: null }) } });
      expect(screen.queryByText('Producer')).not.toBeInTheDocument();
      expect(screen.queryByText('Consumer')).not.toBeInTheDocument();
    });
  });

  describe('usedIn cross-reference', () => {
    it('shows navigable link when usedIn is provided', () => {
      const bowtie = makeBowtie({ user_names: ['Yard Entry'] });
      render(TreeLeafRow, {
        props: { leaf: makeLeaf(), usedIn: bowtie },
      });
      expect(screen.getByRole('button', { name: /bowtie/i })).toBeInTheDocument();
    });

    it('navigates to bowties page when link clicked', async () => {
      const { bowtieFocusStore } = await import('$lib/stores/bowtieFocus.svelte');
      const bowtie = makeBowtie({ event_id_hex: '05.02.01.02.03.00.00.01' });
      render(TreeLeafRow, {
        props: { leaf: makeLeaf(), usedIn: bowtie },
      });
      const link = screen.getByRole('button', { name: /bowtie/i });
      await fireEvent.click(link);
      expect(bowtieFocusStore.focusBowtie).toHaveBeenCalledWith('05.02.01.02.03.00.00.01');
    });

    it('does not show link when usedIn is undefined', () => {
      render(TreeLeafRow, { props: { leaf: makeLeaf() } });
      expect(screen.queryByRole('button', { name: /bowtie/i })).not.toBeInTheDocument();
    });
  });

  // ─── T021: Editable string input ──────────────────────────────────────────

  describe('editable string input (T021)', () => {
    const NODE_ID = '05.01.01.01.03.00';

    it('renders text input for string-type leaf when nodeId is provided', () => {
      const leaf = makeLeaf({
        elementType: 'string',
        value: { type: 'string', value: 'Hello' },
        size: 16,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox');
      expect(input).toBeInTheDocument();
    });

    it('does NOT render text input without nodeId (read-only fallback)', () => {
      const leaf = makeLeaf({
        elementType: 'string',
        value: { type: 'string', value: 'Hello' },
        size: 16,
      });
      render(TreeLeafRow, { props: { leaf } }); // no nodeId
      expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
      expect(screen.getByText('Hello')).toBeInTheDocument();
    });

    it('has maxlength equal to size - 1', () => {
      const leaf = makeLeaf({
        elementType: 'string',
        value: { type: 'string', value: '' },
        size: 16,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox');
      expect(input).toHaveAttribute('maxlength', '15');
    });

    it('calls setModifiedValue when text is entered', async () => {
      const leaf = makeLeaf({
        elementType: 'string',
        value: { type: 'string', value: 'old' },
        size: 16,
        address: 100,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox');
      await fireEvent.input(input, { target: { value: 'new value' } });

      expect(setModifiedValue).toHaveBeenCalledWith(NODE_ID, 100, 253, { type: 'string', value: 'new value' });
    });
  });

  // ─── T022: Editable numeric input ─────────────────────────────────────────

  describe('editable numeric input (T022)', () => {
    const NODE_ID = '05.01.01.01.03.00';

    it('renders number input for int-type leaf without mapEntries when nodeId is provided', () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 5 },
        size: 1,
        constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.getByRole('spinbutton')).toBeInTheDocument();
    });

    it('does NOT render number input for int with mapEntries (uses read-only display)', () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 1 },
        size: 1,
        constraints: {
          min: 0,
          max: 1,
          defaultValue: null,
          mapEntries: [{ value: 0, label: 'Off' }, { value: 1, label: 'On' }],
        },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
      expect(screen.getByText('On')).toBeInTheDocument();
    });

    it('has min and max attributes from constraints', () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 5 },
        size: 2,
        constraints: { min: 0, max: 1000, defaultValue: null, mapEntries: null },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('spinbutton');
      expect(input).toHaveAttribute('min', '0');
      expect(input).toHaveAttribute('max', '1000');
    });

    it('calls setModifiedValue when a number is entered', async () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 0 },
        size: 1,
        address: 200,
        space: 253,
        constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('spinbutton');
      await fireEvent.input(input, { target: { value: '7' } });

      expect(setModifiedValue).toHaveBeenCalledWith(NODE_ID, 200, 253, { type: 'int', value: 7 });
    });
  });

  // ── T029: US2 — Dropdown select for int with mapEntries ────────────────────
  describe('T029: dropdown select for int field with mapEntries', () => {
    const NODE_ID = '05.01.01.01.03.00';
    it('renders a <select> when int field has mapEntries', () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 1 },
        size: 1,
        constraints: {
          min: 0, max: 1, defaultValue: null,
          mapEntries: [{ value: 0, label: 'Off' }, { value: 1, label: 'On' }],
        },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.getByRole('combobox')).toBeInTheDocument();
    });

    it('populates select with option labels', () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 0 },
        size: 1,
        constraints: {
          min: 0, max: 2, defaultValue: null,
          mapEntries: [
            { value: 0, label: 'Slow' },
            { value: 1, label: 'Medium' },
            { value: 2, label: 'Fast' },
          ],
        },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.getByRole('option', { name: 'Slow' })).toBeInTheDocument();
      expect(screen.getByRole('option', { name: 'Medium' })).toBeInTheDocument();
      expect(screen.getByRole('option', { name: 'Fast' })).toBeInTheDocument();
    });

    it('stores numeric value (not label text) on change', async () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 0 },
        size: 1,
        address: 300,
        space: 253,
        constraints: {
          min: 0, max: 1, defaultValue: null,
          mapEntries: [{ value: 0, label: 'Off' }, { value: 1, label: 'On' }],
        },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const select = screen.getByRole('combobox');
      await fireEvent.change(select, { target: { value: '1' } });

      expect(setModifiedValue).toHaveBeenCalledWith(NODE_ID, 300, 253, { type: 'int', value: 1 });
    });

    it('does NOT render a number input for int with mapEntries', () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 1 },
        size: 1,
        constraints: {
          min: 0, max: 1, defaultValue: null,
          mapEntries: [{ value: 0, label: 'Off' }, { value: 1, label: 'On' }],
        },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
    });
  });

  // ── T029b: US2 — Float field editing ──────────────────────────────────────
  describe('T029b: float field input', () => {
    const NODE_ID = '05.01.01.01.03.00';
    it('renders a number input with step="any" for float fields', () => {
      const leaf = makeLeaf({
        elementType: 'float',
        value: { type: 'float', value: 3.14 },
        size: 4,
        constraints: null,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('spinbutton');
      expect(input).toHaveAttribute('step', 'any');
    });

    it('calls setModifiedValue when float value entered', async () => {
      const leaf = makeLeaf({
        elementType: 'float',
        value: { type: 'float', value: 1.0 },
        size: 4,
        address: 400,
        space: 253,
        constraints: null,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('spinbutton');
      await fireEvent.input(input, { target: { value: '2.718' } });

      expect(setModifiedValue).toHaveBeenCalledWith(NODE_ID, 400, 253, { type: 'float', value: 2.718 });
    });

    it('does not call setModifiedValue for non-numeric float input', async () => {
      const leaf = makeLeaf({
        elementType: 'float',
        value: { type: 'float', value: 1.0 },
        size: 4,
        address: 401,
        space: 253,
        constraints: null,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('spinbutton');
      await fireEvent.input(input, { target: { value: 'abc' } });

      expect(setModifiedValue).not.toHaveBeenCalled();
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });

    it('applies min/max constraints from float constraints', () => {
      const leaf = makeLeaf({
        elementType: 'float',
        value: { type: 'float', value: 0.5 },
        size: 4,
        constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('spinbutton');
      expect(input).toHaveAttribute('min', '0');
      expect(input).toHaveAttribute('max', '10');
    });
  });

  // ── T032: US3 — Event ID editing ──────────────────────────────────────────
  describe('T032: event ID field input', () => {
    const NODE_ID = '05.01.01.01.03.00';

    it('renders a text input for eventId fields when nodeId is provided', () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0] },
        size: 8,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.getByRole('textbox', { name: /test field/i })).toBeInTheDocument();
    });

    it('calls setModifiedValue with parsed bytes when valid dotted-hex event ID entered', async () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0] },
        size: 8,
        address: 500,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox', { name: /test field/i });
      await fireEvent.input(input, { target: { value: '05.01.01.01.22.00.00.FF' } });

      expect(setModifiedValue).toHaveBeenCalledWith(
        NODE_ID, 500, 253,
        expect.objectContaining({ type: 'eventId', bytes: [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff] }),
      );
    });

    it('does not call setModifiedValue for malformed event ID', async () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0] },
        size: 8,
        address: 501,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox', { name: /test field/i });
      await fireEvent.input(input, { target: { value: 'not-a-valid-event-id' } });

      expect(setModifiedValue).not.toHaveBeenCalled();
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });

    it('does NOT render event ID text input when no nodeId provided', () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0] },
        size: 8,
      });
      // No nodeId — use default empty string
      render(TreeLeafRow, { props: { leaf } });
      // Should show read-only display instead
      expect(screen.getByText('(not set)')).toBeInTheDocument();
    });

    it('does not call setModifiedValue when all-zero event ID is typed', async () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [5, 1, 1, 1, 22, 0, 0, 1], hex: '05.01.01.01.16.00.00.01' },
        size: 8,
        address: 502,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox', { name: /test field/i });
      await fireEvent.input(input, { target: { value: '00.00.00.00.00.00.00.00' } });

      expect(setModifiedValue).not.toHaveBeenCalled();
      expect(screen.getByRole('alert')).toBeInTheDocument();
      expect(screen.getByRole('alert').textContent).toContain('Event IDs starting with 00 are reserved placeholders');
    });

    it('accepts and writes FF.FF.FF.FF.FF.FF.FF.FF (empty slot)', async () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [5, 1, 1, 1, 22, 0, 0, 1], hex: '05.01.01.01.16.00.00.01' },
        size: 8,
        address: 503,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox', { name: /test field/i });
      await fireEvent.input(input, { target: { value: 'FF.FF.FF.FF.FF.FF.FF.FF' } });

      expect(setModifiedValue).toHaveBeenCalledWith(
        NODE_ID, 503, 253,
        expect.objectContaining({ type: 'eventId', bytes: [255, 255, 255, 255, 255, 255, 255, 255] }),
      );
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });

    it('shows placeholder note on initial render when committed value is all-zero', () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0], hex: '00.00.00.00.00.00.00.00' },
        size: 8,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });

      expect(screen.getByRole('listitem')).toHaveClass('eventid-placeholder');
      expect(screen.getByRole('listitem')).not.toHaveClass('invalid');
      expect(screen.getByText(/Unconfigured placeholder/)).toBeInTheDocument();
    });

    it('shows placeholder note when committed value has leading-zero (e.g. TCS LT-50 00.00.00.00.00.00.00.FF)', () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0xFF], hex: '00.00.00.00.00.00.00.FF' },
        size: 8,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });

      expect(screen.getByRole('listitem')).toHaveClass('eventid-placeholder');
      expect(screen.getByRole('listitem')).not.toHaveClass('invalid');
      expect(screen.getByText(/Unconfigured placeholder/)).toBeInTheDocument();
    });

    it('does not call setModifiedValue when leading-zero event ID is typed', async () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [5, 1, 1, 1, 22, 0, 0, 1], hex: '05.01.01.01.16.00.00.01' },
        size: 8,
        address: 502,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox', { name: /test field/i });
      await fireEvent.input(input, { target: { value: '00.00.00.00.00.00.00.FF' } });

      expect(setModifiedValue).not.toHaveBeenCalled();
      expect(screen.getByRole('alert')).toBeInTheDocument();
      expect(screen.getByRole('alert').textContent).toContain('Event IDs starting with 00 are reserved placeholders');
    });

    it('does not flag the row invalid on initial render when committed value is non-zero', () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [5, 1, 1, 1, 22, 0, 0, 1], hex: '05.01.01.01.16.00.00.01' },
        size: 8,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });

      expect(screen.getByRole('listitem')).not.toHaveClass('invalid');
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });

    it('hides New Connection button when event ID is a placeholder', () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0xFF], hex: '00.00.00.00.00.00.00.FF' },
        size: 8,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.queryByRole('button', { name: /create connection/i })).not.toBeInTheDocument();
    });

    it('shows New Connection button when event ID is a valid non-placeholder', () => {
      const leaf = makeLeaf({
        elementType: 'eventId',
        value: { type: 'eventId', bytes: [5, 1, 1, 1, 22, 0, 0, 1], hex: '05.01.01.01.16.00.00.01' },
        size: 8,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      expect(screen.getByRole('button', { name: /create connection/i })).toBeInTheDocument();
    });
  });

  // ── T049: US6 — Input reverts to original value after discard ─────────────
  describe('T049: input shows original value when no pending edit in store', () => {
    const NODE_ID = '05.01.01.01.03.00';

    it('string input shows original leaf value when no pending edit exists', () => {
      const leaf = makeLeaf({
        elementType: 'string',
        value: { type: 'string', value: 'Original Name' },
        size: 20,
        address: 100,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });

      const input = screen.getByRole('textbox', { name: /test field/i });
      expect(input).toHaveValue('Original Name');
    });

    it('int input shows original leaf value when no pending edit exists', () => {
      const leaf = makeLeaf({
        elementType: 'int',
        value: { type: 'int', value: 42 },
        size: 1,
        address: 200,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });

      const input = screen.getByRole('spinbutton', { name: /test field/i });
      expect(input).toHaveValue(42);
    });

    it('calls setModifiedValue when user types in a string field', async () => {
      const leaf = makeLeaf({
        elementType: 'string',
        value: { type: 'string', value: 'Hello' },
        size: 20,
        address: 300,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox', { name: /test field/i });
      await fireEvent.input(input, { target: { value: 'World' } });

      expect(setModifiedValue).toHaveBeenCalledWith(NODE_ID, 300, 253, { type: 'string', value: 'World' });
    });
  });
});

// ── Dirty / write-state display ───────────────────────────────────────────────

describe('dirty and write state display', () => {
  const NODE_ID = '05.01.01.01.03.00';

  it('applies dirty class when leaf has a modifiedValue', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 0 },
      modifiedValue: { type: 'int', value: 42 },
    });
    render(TreeLeafRow, { props: { leaf } });
    expect(screen.getByRole('listitem')).toHaveClass('dirty');
  });

  it('does not apply dirty class when modifiedValue is null', () => {
    const leaf = makeLeaf({
      value: { type: 'int', value: 0 },
      modifiedValue: null,
    });
    render(TreeLeafRow, { props: { leaf } });
    expect(screen.getByRole('listitem')).not.toHaveClass('dirty');
  });

  it('shows modifiedValue in input instead of original committed value', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 0 },
      constraints: { min: 0, max: 100, defaultValue: null, mapEntries: null },
      modifiedValue: { type: 'int', value: 55 },
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.getByRole('spinbutton')).toHaveValue(55);
  });

  it('disables input when writeState is writing', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 5 },
      constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      modifiedValue: { type: 'int', value: 7 },
      writeState: 'writing',
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.getByRole('spinbutton')).toBeDisabled();
  });

  it('shows write error message when writeState is error', () => {
    const leaf = makeLeaf({
      elementType: 'string',
      value: { type: 'string', value: 'Hello' },
      size: 16,
      modifiedValue: { type: 'string', value: 'New Value' },
      writeState: 'error',
      writeError: 'Node did not respond',
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.getByText(/⚠ Node did not respond/)).toBeInTheDocument();
  });

  it('applies write-error class when writeState is error', () => {
    const leaf = makeLeaf({
      modifiedValue: { type: 'int', value: 1 },
      writeState: 'error',
      writeError: 'Timeout',
    });
    render(TreeLeafRow, { props: { leaf } });
    expect(screen.getByRole('listitem')).toHaveClass('write-error');
  });
});

// ── T040: US5 — Action element trigger button ─────────────────────────────────

describe('T040: action element trigger button', () => {
  const NODE_ID = '05.01.01.01.03.00';

  it('renders a button for action-type leaf when nodeId provided', () => {
    const leaf = makeLeaf({ elementType: 'action', size: 1 });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.getByRole('button')).toBeInTheDocument();
  });

  it('shows leaf.buttonText on the button when set', () => {
    const leaf = makeLeaf({ elementType: 'action', size: 1, buttonText: 'Reset Now' });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.getByRole('button', { name: /reset now/i })).toBeInTheDocument();
  });

  it('falls back to "Trigger" when buttonText is not set', () => {
    const leaf = makeLeaf({ elementType: 'action', size: 1 });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    // The button uses aria-label for the accessible name (leaf name), so match by text content
    const btn = screen.getByRole('button');
    expect(btn).toBeInTheDocument();
    expect(btn).toHaveTextContent('Trigger');
  });

  it('does NOT render action button without nodeId (read-only fallback)', () => {
    const leaf = makeLeaf({ elementType: 'action', size: 1 });
    render(TreeLeafRow, { props: { leaf } }); // no nodeId
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('calls triggerAction when button is clicked (no dialog)', async () => {
    const leaf = makeLeaf({
      elementType: 'action', size: 1,
      address: 100, space: 253,
      actionValue: 255,
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    await fireEvent.click(screen.getByRole('button'));
    expect(triggerAction).toHaveBeenCalledWith(NODE_ID, 253, 100, 1, 255);
  });

  it('does not call triggerAction when dialog is cancelled', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    const leaf = makeLeaf({
      elementType: 'action', size: 1,
      address: 100, space: 253,
      dialogText: 'Are you sure?',
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    await fireEvent.click(screen.getByRole('button'));
    expect(confirmSpy).toHaveBeenCalledWith('Are you sure?');
    expect(triggerAction).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it('calls triggerAction when dialog is confirmed', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    const leaf = makeLeaf({
      elementType: 'action', size: 1,
      address: 200, space: 253,
      dialogText: 'Confirm?',
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    await fireEvent.click(screen.getByRole('button'));
    expect(triggerAction).toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it('action button is disabled when isNodeOffline is true', () => {
    const leaf = makeLeaf({ elementType: 'action', size: 1 });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID, isNodeOffline: true } });
    expect(screen.getByRole('button')).toBeDisabled();
  });
});

// ── T041: Slider hint — range input ──────────────────────────────────────────

describe('T041: int field with slider hint', () => {
  const NODE_ID = '05.01.01.01.03.00';

  it('renders a range input (not spinbutton) for int with hintSlider', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 5 },
      size: 1,
      constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      hintSlider: { immediate: false, tickSpacing: 1, showValue: false },
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    const slider = screen.getByRole('slider');
    expect(slider).toBeInTheDocument();
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
  });

  it('shows the current value label when showValue is true', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 7 },
      size: 1,
      constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      hintSlider: { immediate: false, tickSpacing: 1, showValue: true },
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    // The slider value label should show the current numeric value
    expect(screen.getByText('7')).toBeInTheDocument();
  });

  it('does not show value label when showValue is false', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 7 },
      size: 1,
      constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      hintSlider: { immediate: false, tickSpacing: 1, showValue: false },
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    // No .slider-value span should be present
    expect(document.querySelector('.slider-value')).not.toBeInTheDocument();
  });

  it('calls setModifiedValue on change when immediate is false', async () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 5 },
      size: 1,
      address: 50,
      space: 253,
      constraints: { min: 0, max: 10, defaultValue: null, mapEntries: null },
      hintSlider: { immediate: false, tickSpacing: 1, showValue: false },
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    const slider = screen.getByRole('slider');
    await fireEvent.change(slider, { target: { value: '8' } });
    expect(setModifiedValue).toHaveBeenCalledWith(NODE_ID, 50, 253, { type: 'int', value: 8 });
  });
});

// ── T042: Radio hint — radio buttons for int with map ────────────────────────

describe('T042: int field with radio button hint', () => {
  const NODE_ID = '05.01.01.01.03.00';

  it('renders radio inputs (not combobox) for int with hintRadio + mapEntries', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 0 },
      size: 1,
      constraints: {
        min: 0, max: 1, defaultValue: null,
        mapEntries: [{ value: 0, label: 'Off' }, { value: 1, label: 'On' }],
      },
      hintRadio: true,
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.getAllByRole('radio')).toHaveLength(2);
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
  });

  it('shows map labels as radio button labels', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 0 },
      size: 1,
      constraints: {
        min: 0, max: 2, defaultValue: null,
        mapEntries: [
          { value: 0, label: 'Slow' },
          { value: 1, label: 'Medium' },
          { value: 2, label: 'Fast' },
        ],
      },
      hintRadio: true,
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.getByLabelText('Slow')).toBeInTheDocument();
    expect(screen.getByLabelText('Medium')).toBeInTheDocument();
    expect(screen.getByLabelText('Fast')).toBeInTheDocument();
  });

  it('calls setModifiedValue with numeric value when radio is changed', async () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 0 },
      size: 1,
      address: 300,
      space: 253,
      constraints: {
        min: 0, max: 1, defaultValue: null,
        mapEntries: [{ value: 0, label: 'Off' }, { value: 1, label: 'On' }],
      },
      hintRadio: true,
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    const onRadio = screen.getByLabelText('On');
    await fireEvent.change(onRadio, { target: { value: '1' } });
    expect(setModifiedValue).toHaveBeenCalledWith(NODE_ID, 300, 253, { type: 'int', value: 1 });
  });
});

// ── T043: Reserved value in dropdown ─────────────────────────────────────────

describe('T043: reserved value option in dropdown select', () => {
  const NODE_ID = '05.01.01.01.03.00';

  it('shows disabled (Reserved: N) option when current value is not in mapEntries', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 99 }, // 99 not in map
      size: 1,
      constraints: {
        min: 0, max: 2, defaultValue: null,
        mapEntries: [
          { value: 0, label: 'Off' },
          { value: 1, label: 'On' },
          { value: 2, label: 'Auto' },
        ],
      },
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    const reservedOpt = screen.getByRole('option', { name: '(Reserved: 99)' });
    expect(reservedOpt).toBeInTheDocument();
    expect(reservedOpt).toBeDisabled();
  });

  it('does NOT show reserved option when current value is in mapEntries', () => {
    const leaf = makeLeaf({
      elementType: 'int',
      value: { type: 'int', value: 1 }, // 1 IS in map
      size: 1,
      constraints: {
        min: 0, max: 1, defaultValue: null,
        mapEntries: [{ value: 0, label: 'Off' }, { value: 1, label: 'On' }],
      },
    });
    render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
    expect(screen.queryByRole('option', { name: /reserved/i })).not.toBeInTheDocument();
  });
});

