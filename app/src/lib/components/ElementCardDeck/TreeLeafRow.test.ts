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

// Mock $app/navigation
vi.mock('$app/navigation', () => ({
  goto: vi.fn(),
}));

// Mock bowties store
vi.mock('$lib/stores/bowties.svelte', () => ({
  bowtieCatalogStore: {
    nodeSlotMap: new Map(),
  },
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
      const { goto } = await import('$app/navigation');
      const bowtie = makeBowtie({ event_id_hex: '05.02.01.02.03.00.00.01' });
      render(TreeLeafRow, {
        props: { leaf: makeLeaf(), usedIn: bowtie },
      });
      const link = screen.getByRole('button', { name: /bowtie/i });
      await fireEvent.click(link);
      expect(goto).toHaveBeenCalledWith('/bowties?highlight=05.02.01.02.03.00.00.01');
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

    it('updates pendingEditsStore when text is entered', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

      // Store should have the edit
      const key = `${NODE_ID}:253:100`;
      const edit = pendingEditsStore.getEdit(key);
      expect(edit).toBeDefined();
      expect(edit?.pendingValue).toEqual({ type: 'string', value: 'new value' });
      expect(edit?.validationState).toBe('valid');

      pendingEditsStore.clearAll();
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

    it('updates pendingEditsStore when a number is entered', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

      const key = `${NODE_ID}:253:200`;
      const edit = pendingEditsStore.getEdit(key);
      expect(edit).toBeDefined();
      expect(edit?.pendingValue).toEqual({ type: 'int', value: 7 });
      expect(edit?.validationState).toBe('valid');

      pendingEditsStore.clearAll();
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
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

      const key = `${NODE_ID}:253:300`;
      const edit = pendingEditsStore.getEdit(key);
      expect(edit).toBeDefined();
      expect(edit?.pendingValue).toEqual({ type: 'int', value: 1 });
      expect(edit?.validationState).toBe('valid');

      pendingEditsStore.clearAll();
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

    it('updates pendingEditsStore when float value entered', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

      const key = `${NODE_ID}:253:400`;
      const edit = pendingEditsStore.getEdit(key);
      expect(edit).toBeDefined();
      expect(edit?.pendingValue).toEqual({ type: 'float', value: 2.718 });

      pendingEditsStore.clearAll();
    });

    it('sets validation state to invalid for non-numeric float input', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

      // Non-numeric input — store should record invalid state or no edit
      const key = `${NODE_ID}:253:401`;
      const edit = pendingEditsStore.getEdit(key);
      // Either no edit or an edit with invalid validation state
      if (edit) {
        expect(edit.validationState).toBe('invalid');
      } else {
        // No edit recorded for invalid input is also acceptable
        expect(edit).toBeUndefined();
      }

      pendingEditsStore.clearAll();
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

    it('stores parsed bytes when valid dotted-hex event ID entered', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

      const key = `${NODE_ID}:253:500`;
      const edit = pendingEditsStore.getEdit(key);
      expect(edit).toBeDefined();
      expect(edit?.pendingValue).toEqual(expect.objectContaining({
        type: 'eventId',
        bytes: [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xff],
      }));
      expect(edit?.validationState).toBe('valid');

      pendingEditsStore.clearAll();
    });

    it('sets validation state to invalid for malformed event ID', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

      const key = `${NODE_ID}:253:501`;
      const edit = pendingEditsStore.getEdit(key);
      expect(edit).toBeDefined();
      expect(edit?.validationState).toBe('invalid');

      pendingEditsStore.clearAll();
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
  });

  // ── T049: US6 — Input reverts to original value after discard ─────────────
  describe('T049: input shows original value when no pending edit in store', () => {
    const NODE_ID = '05.01.01.01.03.00';

    it('string input shows original leaf value when no pending edit exists', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

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

    it('store has no edit for field after clearForNode is called', async () => {
      const { pendingEditsStore } = await import('$lib/stores/pendingEdits.svelte');
      pendingEditsStore.clearAll();

      const leaf = makeLeaf({
        elementType: 'string',
        value: { type: 'string', value: 'Hello' },
        size: 20,
        address: 300,
        space: 253,
      });
      render(TreeLeafRow, { props: { leaf, nodeId: NODE_ID } });
      const input = screen.getByRole('textbox', { name: /test field/i });

      // Make an edit
      await fireEvent.input(input, { target: { value: 'World' } });
      const key = `${NODE_ID}:253:300`;
      expect(pendingEditsStore.getEdit(key)).toBeDefined();

      // Discard
      pendingEditsStore.clearForNode(NODE_ID);
      expect(pendingEditsStore.getEdit(key)).toBeUndefined();
    });
  });
});

