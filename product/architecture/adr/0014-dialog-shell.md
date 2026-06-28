# Dialog shell: single shared `<Dialog>` on Fluent v9 tokens with v8-style chrome

## Context

The Bowties frontend accumulated ~17 modal surfaces (one per dialog component plus
several inline confirms in `+page.svelte`), each rolling its own overlay, focus
trap, Esc handling, button styling, close affordance, and ARIA wiring. The visible
drift was substantial:

- Overlays at `rgba(0,0,0,0.35)` / `0.45` / `0.5`.
- Title sizes 14 / 16 / 18 / `1.1rem`.
- Primary buttons in Tailwind blue `#2563eb`, Fluent blue `#0078d4`, and `#0066cc`.
- Destructive actions shown variously as Tailwind red `#dc2626` (older confirms),
  Fluent severeWarning orange `#ca5010` (newer `UnsavedChangesDialog`), and the
  "Continue" verb (channel-removal inline).
- One dialog (`ConnectionManager`) used the browser's native `<dialog>` element;
  most rolled their own `.overlay` + `.card` markup.
- Per-component focus-trap implementations duplicated across ~10 files.

The drift mattered for two reasons:

1. **Polish.** The newest dialog (`UnsavedChangesDialog`, from Spec 018) used the
   Fluent severeWarning orange (`#ca5010`) for its destructive primary button.
   That's a *misapplication* of the Fluent palette — severeWarning orange is the
   color of *warning glyphs and message-bar surfaces*, not destructive buttons.
   The dialog visibly diverged from the rest of the app while also not matching
   the Fluent system it was loosely based on.
2. **Maintenance.** Every new dialog re-derived the overlay/focus/Esc/× pattern
   from a sibling and quietly drifted from it. New surfaces consistently took
   500–1500 ms of human review per PR just to confirm chrome rules.

Anchoring on **Fluent UI v9** explicitly (rather than "vaguely Fluent-inspired"
per-dialog choices) gave us a published vocabulary for the question "what does a
dialog look like, and what's the destructive color called?" — without forcing us
to import a JS dependency or adopt a CSS-in-JS layer.

A complete inventory and migration plan lives in
[specs/018-block-indicator-facility/dialog-shell-refactor.md](../../../specs/018-block-indicator-facility/dialog-shell-refactor.md).
Mockup #10 in [specs/018-block-indicator-facility/mockups.html](../../../specs/018-block-indicator-facility/mockups.html)
is the canonical visual lockup.

## Decision

Adopt a single shared dialog shell at `app/src/lib/components/Dialog/` consisting
of `Dialog.svelte`, `DialogTitle.svelte`, `DialogActions.svelte`, `Button.svelte`,
and `tokens.css`, with the following anchoring choices:

### 1. Fluent v9 tokens with v8-style header and footer dividers (deliberate departure)

Token values come straight from Fluent v9 (`colorNeutralBackground1`,
`colorBrandBackground` = `#0f6cbd`, `colorNeutralStroke1` = `#d1d1d1`, type ramp
`fontSizeBase300 = 14`/`fontSizeBase400 = 16`, etc.), prefixed `--fluent-*` in
`Dialog/tokens.css`.

The chrome layout, however, uses **Fluent v8 / Win32 conventions**:
- Header has a `border-bottom: 1px solid var(--fluent-neutralStroke1)`.
- Footer has a `border-top` and a subtle background fill (`colorNeutralBackground2`).

Fluent v9 itself ships dialogs with flush chrome (no dividers, no footer fill).
We deliberately chose the v8 chrome because it makes the body / title / actions
zones distinguishable at a glance — important when the body is dense (dirty-count
breakdown, per-node download status, XML viewer). Documented inside `Dialog.svelte`
so the deviation reads as a choice, not an oversight.

### 2. Destructive action = Fluent danger red on the button; severeWarning orange is the title glyph color only

Destructive confirms use `<Button appearance="primary" intent="danger">`, which
maps to `colorStatusDangerBackground3` (`#c50f1f`) with the matching hover/pressed.
The optional warning glyph in `DialogTitle glyph="warning"` uses
`colorStatusWarningForeground3` (`#bc4b09`, severeWarning) — **glyph only, never
on buttons.** This separation matches Fluent v9's own token roles and makes the
dialog instantly readable: the title glyph says "this is a warning surface", the
button color says "this action is destructive".

Fluent v9 doesn't actually ship a `danger` intent for `<Button>`; we add it because
destructive confirms recur (currently 7 confirm surfaces use it) and need a
consistent treatment.

### 3. Non-dismissible surfaces use `closable={false}` on the same shell, not a separate `<Progress>` component

`Dialog` accepts a `closable?: boolean` prop (default `true`). When `false`, the
× close button is omitted, Esc is ignored, and overlay-click is a no-op. Progress
dialogs (`SaveProgressDialog`, `CdiDownloadDialog`, `CdiRedownloadDialog`,
`LayoutLoadingDialog`) all set `closable` per phase from their owning store so
the user can't dismiss an in-flight write or download mid-flight.

We considered a parallel `<Progress>` component with no close affordance. Rejected
because the chrome — overlay, surface, title, body, footer, header divider — is
identical; the only difference is the dismissibility contract. Splitting created
two parallel widget families needing parallel maintenance for the same look.

### 4. Deprecate the native `<dialog>` element in `ConnectionManager` for shell consistency

`ConnectionManager` previously used the HTML `<dialog open>` element wrapped in a
custom `.cm-overlay` div for click-outside-to-close. The native element gave
browser-managed modal stacking for free, but no other dialog in the app used it
(every other "modal" rolled its own overlay), so the stacking-context benefit was
not actually being exploited.

Migrating `ConnectionManager` to the shared shell costs nothing visible to the
user, removes ~50 lines of dead overlay/header CSS, and unifies the modal
infrastructure. Esc handling, previously inherited from the native `<dialog>`'s
default close-on-Esc, is now explicit in the shell.

## Considered options

- **Adopt Fluent v9 components directly via `@fluentui/web-components`.** Rejected:
  pulls in a web-components runtime, a separate styling layer, and shadow-DOM
  encapsulation that would not interoperate cleanly with the existing Svelte
  components, Tailwind utilities, and Prism syntax highlighting.
- **Wrap a third-party headless dialog library (e.g. Bits UI, Melt UI).** Rejected
  as YAGNI: we need overlay + focus trap + Esc + × + ARIA, all of which are
  ~150 lines of Svelte 5. A dependency would carry its own opinions about Tab
  trap edge cases and animation that we'd have to override anyway.
- **Use Fluent v9 flush chrome (no header divider, no footer fill).** Rejected on
  the user's explicit visual preference and because Bowties dialog bodies are
  denser than typical Fluent v9 surfaces.
- **Keep destructive primary = severeWarning orange `#ca5010` (status quo of the
  newest dialog).** Rejected because orange-on-buttons misapplies the Fluent
  severeWarning token and reads as "warning" rather than "destructive" —
  exactly the polish gap that triggered this refactor.
- **Build a separate `<ProgressDialog>` component for non-dismissible surfaces.**
  Rejected as duplication: see Decision 3.
- **Use the native `<dialog>` element for every dialog (extend the
  `ConnectionManager` pattern).** Rejected because native `<dialog>` doesn't
  ship with focus-trap behavior, requires `dialog.showModal()` imperatively,
  and tangles with our `{#if open}` conditional-render pattern.
- **Rename existing `--accent-blue` / `--text-*` CSS variables to Fluent names
  during this refactor.** Rejected for scope: those tokens are referenced
  across every non-dialog component. A later sweep can drop the `--fluent-*`
  prefix once Fluent is the only token system.

## Consequences

**Positive**

- One canonical visual language for every dialog. New dialogs land by composing
  `<Dialog>` + `<DialogTitle>` + `<DialogActions>` + `<Button>`, not by copying
  another dialog's `.css` and tweaking pixels.
- One canonical accessibility surface (focus trap, Esc, ARIA roles, overlay-click
  semantics). Per-component focus-trap reimplementations are gone.
- One canonical destructive-action visual (red), one canonical brand-action visual
  (blue), one canonical warning-glyph visual (orange). The misapplication that
  triggered the refactor cannot recur without explicitly typing the wrong token.
- Dead-code shrinkage: ~750 lines of bespoke dialog CSS removed across the migrated
  surfaces; ~120 lines added in the shared shell + button + tokens. Total LOC
  delta for the migrated set is roughly -600.
- Test stability: per-dialog tests that queried by accessible name (`getByRole`,
  `getByText`) survived unchanged across the entire migration; only chrome-level
  selectors (which were already brittle) needed updates.
- The design system is **discoverable from the codebase**:
  `app/src/lib/components/Dialog/README.md` is the authoring guide; `aiwiki/owners.md`
  points at it from every migrated dialog's entry.

**Negative**

- The `--fluent-*` token prefix coexists with the legacy `--accent-blue` /
  `--text-*` family until a follow-up sweep retires the latter. New code reading
  Bowties styles must know both exist.
- The v8-style chrome (header/footer dividers) is a deliberate departure from
  current Fluent v9. Anyone validating against the live Fluent docs will see a
  mismatch; the deviation is documented inline in `Dialog.svelte` and here.
- Native `<dialog>`'s top-layer browser stacking benefit (free interaction with
  future `<dialog>`-based modals from other parts of the stack) is gone. No
  other native dialogs exist in the app today, so the regression is currently
  hypothetical.
- The `Button` component adds an `intent="danger"` appearance that Fluent v9
  doesn't ship — a small, documented Bowties extension to the Fluent vocabulary.
- The text-stack font (`'Segoe UI Variable', ...`) was applied at the route's
  global `:global(html, body)` block. ~14 components still re-declare the same
  stack locally to work around form-control non-inheritance; each will retire
  its declaration as it touches the shell. Until then, two declarations of the
  same stack coexist in those components.

## Status

Accepted (2026-06-28) — `fluent-ux-refactor` branch.
