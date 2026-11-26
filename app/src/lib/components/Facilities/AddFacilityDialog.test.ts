import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import AddFacilityDialog from './AddFacilityDialog.svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';

function tmpl(id: string, name: string): BehaviorTemplate {
  return {
    templateId: id,
    displayName: name,
    slots: [],
    mapping: [],
  };
}

describe('AddFacilityDialog', () => {
  let onConfirm: Mock<(template: BehaviorTemplate, name: string) => void>;
  let onCancel: Mock<() => void>;

  beforeEach(() => {
    onConfirm = vi.fn<(template: BehaviorTemplate, name: string) => void>();
    onCancel = vi.fn<() => void>();
  });

  it('renders templates as a selectable radio list', () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('block-indicator', 'Block Indicator'), tmpl('crossing-indicator', 'Crossing Indicator')],
        onConfirm,
        onCancel,
      },
    });
    const radiogroup = screen.getByRole('radiogroup', { name: /behavior templates/i });
    const radios = within(radiogroup).getAllByRole('radio');
    expect(radios).toHaveLength(2);
    expect(screen.getByText('Block Indicator')).toBeInTheDocument();
    expect(screen.getByText('Crossing Indicator')).toBeInTheDocument();
  });

  it('defaults selection to the first template', () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('block-indicator', 'Block Indicator'), tmpl('crossing-indicator', 'Crossing Indicator')],
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    expect(radios[0]).toBeChecked();
    expect(radios[1]).not.toBeChecked();
  });

  it('Confirm calls onConfirm with the selected template and trimmed name', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('block-indicator', 'Block Indicator'), tmpl('crossing-indicator', 'Crossing Indicator')],
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.change(radios[1]);

    const nameInput = screen.getByPlaceholderText('e.g. Block 5');
    await fireEvent.input(nameInput, { target: { value: '  Crossing 7  ' } });

    const confirmBtn = screen.getByRole('button', { name: /add facility/i });
    await fireEvent.click(confirmBtn);

    expect(onConfirm).toHaveBeenCalledWith(
      expect.objectContaining({ templateId: 'crossing-indicator' }),
      'Crossing 7',
    );
  });

  it('shows error when name is empty on confirm', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('block-indicator', 'Block Indicator')],
        onConfirm,
        onCancel,
      },
    });
    const confirmBtn = screen.getByRole('button', { name: /add facility/i });
    await fireEvent.click(confirmBtn);

    expect(screen.getByRole('alert')).toHaveTextContent(/enter a name/i);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('disables Add facility for blank or whitespace-only names for every template', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('block-indicator', 'Block Indicator'), tmpl('automatic-block-signals', 'Automatic Block Signals')],
        onConfirm,
        onCancel,
      },
    });

    const addFacility = screen.getByRole('button', { name: /add facility/i });
    expect(addFacility).toBeDisabled();

    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.click(radios[1]);
    await fireEvent.input(screen.getByPlaceholderText('e.g. Block 5'), { target: { value: '   ' } });
    expect(addFacility).toBeDisabled();

    await fireEvent.input(screen.getByPlaceholderText('e.g. Block 5'), { target: { value: ' Line 1 ' } });
    expect(addFacility).toBeEnabled();
  });

  it('Cancel button calls onCancel', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('block-indicator', 'Block Indicator')],
        onConfirm,
        onCancel,
      },
    });
    await fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalled();
  });

  it('offers Panel with its description and examples, and confirming a name emits it', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [
          {
            ...tmpl('panel', 'Panel'),
            description: 'Lamps on a fascia or control panel that show which blocks are occupied.',
            examples: 'Fascia mimic · dispatcher panel · helix or staging indicators',
          },
          tmpl('block-indicator', 'Block Indicator'),
        ],
        onConfirm,
        onCancel,
      },
    });
    expect(screen.getByText('Panel')).toBeInTheDocument();
    expect(screen.getByText(/Lamps on a fascia or control panel/)).toBeInTheDocument();
    expect(screen.getByText('Fascia mimic · dispatcher panel · helix or staging indicators')).toBeInTheDocument();

    await fireEvent.input(screen.getByPlaceholderText('e.g. Block 5'), { target: { value: 'Eagle Creek fascia' } });
    await fireEvent.click(screen.getByRole('button', { name: /add facility/i }));
    expect(onConfirm).toHaveBeenCalledWith(expect.objectContaining({ templateId: 'panel' }), 'Eagle Creek fascia');
  });

  it('shows an unavailable facility type disabled, with its note, and never selects it', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [
          { ...tmpl('interlocking-plant', 'Interlocking Plant'), unavailableNote: 'Coming in a later release' },
          tmpl('automatic-block-signals', 'Automatic Block Signals'),
        ],
        onConfirm,
        onCancel,
      },
    });

    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    expect(radios[0]).toBeDisabled();
    expect(radios[1]).toBeChecked();
    expect(screen.getByText('Coming in a later release')).toBeInTheDocument();

    await fireEvent.input(screen.getByPlaceholderText('e.g. Block 5'), { target: { value: 'Main' } });
    await fireEvent.click(screen.getByRole('button', { name: /add facility/i }));
    expect(onConfirm).toHaveBeenCalledWith(expect.objectContaining({ templateId: 'automatic-block-signals' }), 'Main');
  });
});
