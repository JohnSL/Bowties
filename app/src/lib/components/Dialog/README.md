# Dialog — Fluent v9 dialog shell for Bowties

A single declarative shell for every modal in the app, plus matching `Button`
primitives. Owns chrome, focus, and ARIA — not workflow.

Visual reference: [mockups.html](../../../../../specs/018-block-indicator-facility/mockups.html)
(mockup #10 is the canonical confirm).

Refactor plan: [dialog-shell-refactor.md](../../../../../specs/018-block-indicator-facility/dialog-shell-refactor.md).

## When to use which variant

| Variant                          | Width | Closable | Footer | Use for                                |
|----------------------------------|-------|----------|--------|----------------------------------------|
| Confirm (info)                   | `sm`  | yes      | yes    | Yes/no, OK/cancel                      |
| Confirm (destructive)            | `sm`  | yes      | yes    | `Discard`, `Delete`, `Remove`          |
| Form                             | `md`  | yes      | yes    | Add / Edit a small object              |
| Picker                           | `lg`  | yes      | yes    | Choose from a list with search         |
| Progress (in-flight)             | `md`  | **no**   | none   | Save, download — block while running   |
| Progress with phased dismissibility | `md` | toggled | yes    | Caller flips `closable` per phase      |
| Viewer                           | `lg`  | yes      | yes (Close) | Read-only content (About, XML)    |

## Authoring a dialog

```svelte
<script lang="ts">
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';

  let open = $state(true);

  function cancel() { open = false; }
  function confirm() { /* … */ open = false; }
</script>

<Dialog
  {open}
  width="sm"
  role="alertdialog"
  onCancel={cancel}
>
  {#snippet title()}
    <DialogTitle glyph="warning">Discard changes?</DialogTitle>
  {/snippet}

  <p>This will discard all unsaved edits.</p>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={cancel}>Cancel</Button>
      <Button appearance="primary" intent="danger" onclick={confirm}>Discard</Button>
    </DialogActions>
  {/snippet}
</Dialog>
```

## Conventions

- **Button order**: safe action (Cancel) first, primary action last. Fluent v9
  ordering — and matches the Mac/Web convention used everywhere else in the app.
- **Destructive label** is the verb alone: `Discard`, `Delete`, `Remove`. Do
  not say `Discard & Continue`; the action is named by the button color and
  glyph, the verb names what happens.
- **Warning glyph (⚠)** in `DialogTitle` is the only place orange
  (`--fluent-warningGlyph`) appears in dialogs. It tags the dialog as a warning;
  the *button* color (red) tags the action as destructive. Never use orange on
  a button.
- **`role="alertdialog"`** for destructive or error confirms; plain `dialog`
  otherwise.
- **`initialFocus`** defaults to `'first'` — which puts focus on the safe
  (Cancel) action by default, the Fluent norm. Use `'last'` only when the
  primary action is unambiguously the next step (e.g., progress dialogs whose
  only action is `Close`).

## What the shell does NOT do

- It doesn't decide what `onCancel` or `onConfirm` mean — those are callers'
  concerns. Per `frontend-components.instructions.md`, multi-step workflows
  belong in orchestrators, not in this shell.
- It doesn't manage open/closed state. The caller owns `open`.
- It doesn't choose a default focus target beyond `first` / `last` /
  `none`. If a specific element should be focused, the caller can bind a ref
  inside the body and call `.focus()` after mount.

## Keyboard contract

| Key            | Behavior                                       |
|----------------|------------------------------------------------|
| `Esc`          | `onCancel` (when `closable`)                   |
| `Tab`          | Cycles focus within the dialog (trap)          |
| `Shift+Tab`    | Cycles focus backward within the dialog        |
| Click on overlay | `onCancel` (when `closable`)                 |
| Click on ×     | `onCancel` (when `closable`; only shown if header present) |
| `Enter`        | **Not handled by the shell.** Use a `type="submit"` button inside a `<form>` to wire Enter to a primary action. |

## Tokens

All visual values come from [tokens.css](./tokens.css). The file uses a
`--fluent-*` prefix to avoid colliding with the existing `--accent-blue` /
`--text-*` family. A later sweep can drop the prefix once those legacy
variables are gone.
