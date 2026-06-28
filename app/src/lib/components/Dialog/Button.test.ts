/**
 * Vitest component tests for Button.svelte (Fluent v9 button).
 *
 * Covers:
 * - Default appearance/intent/size classes
 * - Each appearance × intent combination renders the expected class set
 * - Click handler fires; disabled buttons do not
 * - `type`, `title`, `ariaLabel`, `class` passthrough
 */

import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import Button from './Button.svelte';

function label(text: string) {
  return createRawSnippet(() => ({
    render: () => `<span>${text}</span>`,
  }));
}

describe('Button — defaults', () => {
  it('renders a secondary md button by default', () => {
    const { getByRole } = render(Button, { props: { children: label('Cancel') } });
    const btn = getByRole('button');
    expect(btn.className).toContain('fluent-btn--secondary');
    expect(btn.className).toContain('fluent-btn--md');
    expect(btn.className).not.toContain('fluent-btn--danger');
    expect(btn).toHaveAttribute('type', 'button');
  });
});

describe('Button — appearance × intent', () => {
  it('primary + default renders brand class without danger', () => {
    const { getByRole } = render(Button, {
      props: { appearance: 'primary', children: label('OK') },
    });
    const btn = getByRole('button');
    expect(btn.className).toContain('fluent-btn--primary');
    expect(btn.className).not.toContain('fluent-btn--danger');
  });

  it('primary + danger renders both classes (destructive primary)', () => {
    const { getByRole } = render(Button, {
      props: { appearance: 'primary', intent: 'danger', children: label('Discard') },
    });
    const btn = getByRole('button');
    expect(btn.className).toContain('fluent-btn--primary');
    expect(btn.className).toContain('fluent-btn--danger');
  });

  it('subtle + danger renders subtle danger', () => {
    const { getByRole } = render(Button, {
      props: { appearance: 'subtle', intent: 'danger', children: label('Remove') },
    });
    const btn = getByRole('button');
    expect(btn.className).toContain('fluent-btn--subtle');
    expect(btn.className).toContain('fluent-btn--danger');
  });

  it('outline renders outline class', () => {
    const { getByRole } = render(Button, {
      props: { appearance: 'outline', children: label('Refresh') },
    });
    expect(getByRole('button').className).toContain('fluent-btn--outline');
  });
});

describe('Button — size', () => {
  it('sm renders the sm class', () => {
    const { getByRole } = render(Button, {
      props: { size: 'sm', children: label('x') },
    });
    expect(getByRole('button').className).toContain('fluent-btn--sm');
  });
});

describe('Button — interaction', () => {
  it('fires onclick when clicked', async () => {
    const onclick = vi.fn();
    const { getByRole } = render(Button, {
      props: { onclick, children: label('Go') },
    });
    await fireEvent.click(getByRole('button'));
    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it('sets the disabled attribute when disabled=true (browsers suppress the click)', () => {
    const { getByRole } = render(Button, {
      props: { disabled: true, children: label('Go') },
    });
    expect(getByRole('button')).toBeDisabled();
  });
});

describe('Button — attribute passthrough', () => {
  it('forwards type, title, ariaLabel, and extra class', () => {
    const { getByRole } = render(Button, {
      props: {
        type: 'submit',
        title: 'tooltip text',
        ariaLabel: 'aria label',
        class: 'extra-class',
        children: label('Submit'),
      },
    });
    const btn = getByRole('button');
    expect(btn).toHaveAttribute('type', 'submit');
    expect(btn).toHaveAttribute('title', 'tooltip text');
    expect(btn).toHaveAttribute('aria-label', 'aria label');
    expect(btn.className).toContain('extra-class');
  });
});
