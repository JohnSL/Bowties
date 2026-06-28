/**
 * Vitest component tests for Dialog.svelte (Fluent v9 shell).
 *
 * Covers:
 * - Open/closed rendering
 * - title / actions / closable variants
 * - × close button presence and wiring to onCancel
 * - Esc key → onCancel (when closable); ignored when not closable
 * - Overlay click → onCancel; clicks inside surface do not
 * - Tab focus trap
 * - initialFocus="first" | "last"
 */

import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import Dialog from './Dialog.svelte';

function snippetWith(html: string) {
  return createRawSnippet(() => ({ render: () => html }));
}

function baseProps(overrides: Record<string, unknown> = {}) {
  return {
    open: true,
    onCancel: vi.fn(),
    children: snippetWith('<p>body text</p>'),
    ...overrides,
  };
}

describe('Dialog — rendering', () => {
  it('renders nothing when open=false', () => {
    const { queryByRole } = render(Dialog, {
      props: baseProps({ open: false }),
    });
    expect(queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('renders the body when open=true', () => {
    const { getByText, getByRole } = render(Dialog, { props: baseProps() });
    expect(getByRole('dialog')).toBeInTheDocument();
    expect(getByText('body text')).toBeInTheDocument();
  });

  it('renders the title slot when provided', () => {
    const { getByText } = render(Dialog, {
      props: baseProps({ title: snippetWith('<span>Hello title</span>') }),
    });
    expect(getByText('Hello title')).toBeInTheDocument();
  });

  it('renders the actions slot when provided', () => {
    const { getByText } = render(Dialog, {
      props: baseProps({
        title: snippetWith('<span>T</span>'),
        actions: snippetWith('<button>OK</button>'),
      }),
    });
    expect(getByText('OK')).toBeInTheDocument();
  });

  it('omits header (and × close) when title is not provided', () => {
    const { queryByLabelText } = render(Dialog, { props: baseProps() });
    expect(queryByLabelText('Close')).not.toBeInTheDocument();
  });

  it('uses role="alertdialog" when requested', () => {
    const { getByRole } = render(Dialog, {
      props: baseProps({ role: 'alertdialog' }),
    });
    expect(getByRole('alertdialog')).toBeInTheDocument();
  });
});

describe('Dialog — closable=true (default)', () => {
  it('renders the × close button when there is a title', () => {
    const { getByLabelText } = render(Dialog, {
      props: baseProps({ title: snippetWith('<span>T</span>') }),
    });
    expect(getByLabelText('Close')).toBeInTheDocument();
  });

  it('× click fires onCancel', async () => {
    const onCancel = vi.fn();
    const { getByLabelText } = render(Dialog, {
      props: baseProps({ onCancel, title: snippetWith('<span>T</span>') }),
    });
    await fireEvent.click(getByLabelText('Close'));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('Escape fires onCancel', async () => {
    const onCancel = vi.fn();
    render(Dialog, { props: baseProps({ onCancel }) });
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('overlay click fires onCancel', async () => {
    const onCancel = vi.fn();
    const { container } = render(Dialog, { props: baseProps({ onCancel }) });
    const overlay = container.querySelector('.fluent-dialog-overlay') as HTMLElement;
    expect(overlay).toBeTruthy();
    await fireEvent.click(overlay);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('clicks inside the surface do NOT fire onCancel', async () => {
    const onCancel = vi.fn();
    const { getByText } = render(Dialog, { props: baseProps({ onCancel }) });
    await fireEvent.click(getByText('body text'));
    expect(onCancel).not.toHaveBeenCalled();
  });
});

describe('Dialog — closable=false (progress / non-dismissible)', () => {
  it('omits the × close button even with a title', () => {
    const { queryByLabelText } = render(Dialog, {
      props: baseProps({
        closable: false,
        title: snippetWith('<span>Saving…</span>'),
      }),
    });
    expect(queryByLabelText('Close')).not.toBeInTheDocument();
  });

  it('Escape does NOT fire onCancel', async () => {
    const onCancel = vi.fn();
    render(Dialog, { props: baseProps({ onCancel, closable: false }) });
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('overlay click does NOT fire onCancel', async () => {
    const onCancel = vi.fn();
    const { container } = render(Dialog, {
      props: baseProps({ onCancel, closable: false }),
    });
    const overlay = container.querySelector('.fluent-dialog-overlay') as HTMLElement;
    await fireEvent.click(overlay);
    expect(onCancel).not.toHaveBeenCalled();
  });
});

describe('Dialog — initial focus', () => {
  it('focuses the first focusable by default', async () => {
    const { findByText } = render(Dialog, {
      props: baseProps({
        closable: false,
        title: snippetWith('<span>T</span>'),
        actions: snippetWith('<div><button>First</button><button>Last</button></div>'),
      }),
    });
    const first = await findByText('First');
    // queueMicrotask runs after render; await a tick.
    await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));
    expect(document.activeElement).toBe(first);
  });

  it('focuses the last focusable when initialFocus="last"', async () => {
    const { findByText } = render(Dialog, {
      props: baseProps({
        closable: false,
        title: snippetWith('<span>T</span>'),
        initialFocus: 'last',
        actions: snippetWith('<div><button>First</button><button>Last</button></div>'),
      }),
    });
    const last = await findByText('Last');
    await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));
    expect(document.activeElement).toBe(last);
  });
});

describe('Dialog — focus trap', () => {
  it('Tab from the last focusable wraps to the first', async () => {
    const { findByText } = render(Dialog, {
      props: baseProps({
        closable: false,
        title: snippetWith('<span>T</span>'),
        actions: snippetWith('<div><button>First</button><button>Last</button></div>'),
      }),
    });
    const first = await findByText('First');
    const last = await findByText('Last');
    last.focus();
    await fireEvent.keyDown(window, { key: 'Tab' });
    expect(document.activeElement).toBe(first);
  });

  it('Shift+Tab from the first focusable wraps to the last', async () => {
    const { findByText } = render(Dialog, {
      props: baseProps({
        closable: false,
        title: snippetWith('<span>T</span>'),
        actions: snippetWith('<div><button>First</button><button>Last</button></div>'),
      }),
    });
    const first = await findByText('First');
    const last = await findByText('Last');
    first.focus();
    await fireEvent.keyDown(window, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(last);
  });
});
