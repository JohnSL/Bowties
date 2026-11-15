import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import DialogActionsHarness from './DialogActions.test.harness.svelte';

describe('DialogActions', () => {
  it('keeps an optional leading action separate from the trailing action cluster', () => {
    render(DialogActionsHarness);
    expect(screen.getByTestId('dialog-actions-leading')).toHaveTextContent('Cancel');
    expect(screen.getByTestId('dialog-actions-trailing')).toHaveTextContent('Back Next');
  });
});
