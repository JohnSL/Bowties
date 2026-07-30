import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import AddFacilityDialog from './AddFacilityDialog.svelte';
import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';

function tmpl(id: string, name: string): BehaviorTemplate {
  return {
    templateId: id,
    displayName: name,
    slots: [],
    mapping: [],
    compilationTarget: { kind: 'signal', aspectTable: [] },
    rules: [],
  } as unknown as BehaviorTemplate;
}

describe('AddFacilityDialog', () => {
  let onConfirm: ReturnType<typeof vi.fn>;
  let onCancel: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onConfirm = vi.fn();
    onCancel = vi.fn();
  });

  it('renders templates as a selectable radio list', () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('abs3', 'ABS 3-Aspect'), tmpl('abs2', 'ABS 2-Aspect')],
        onConfirm,
        onCancel,
      },
    });
    const radiogroup = screen.getByRole('radiogroup', { name: /behavior templates/i });
    const radios = within(radiogroup).getAllByRole('radio');
    expect(radios).toHaveLength(2);
    expect(screen.getByText('ABS 3-Aspect')).toBeInTheDocument();
    expect(screen.getByText('ABS 2-Aspect')).toBeInTheDocument();
  });

  it('defaults selection to the first template', () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('abs3', 'ABS 3-Aspect'), tmpl('abs2', 'ABS 2-Aspect')],
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
        templates: [tmpl('abs3', 'ABS 3-Aspect'), tmpl('abs2', 'ABS 2-Aspect')],
        onConfirm,
        onCancel,
      },
    });
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.change(radios[1]);

    const nameInput = screen.getByPlaceholderText('e.g. Block 5');
    await fireEvent.input(nameInput, { target: { value: '  Block 7  ' } });

    const confirmBtn = screen.getByRole('button', { name: /add facility/i });
    await fireEvent.click(confirmBtn);

    expect(onConfirm).toHaveBeenCalledWith(
      expect.objectContaining({ templateId: 'abs2' }),
      'Block 7',
    );
  });

  it('shows error when name is empty on confirm', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('abs3', 'ABS 3-Aspect')],
        onConfirm,
        onCancel,
      },
    });
    const confirmBtn = screen.getByRole('button', { name: /add facility/i });
    await fireEvent.click(confirmBtn);

    expect(screen.getByRole('alert')).toHaveTextContent(/enter a name/i);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('Cancel button calls onCancel', async () => {
    render(AddFacilityDialog, {
      props: {
        templates: [tmpl('abs3', 'ABS 3-Aspect')],
        onConfirm,
        onCancel,
      },
    });
    await fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalled();
  });
});
