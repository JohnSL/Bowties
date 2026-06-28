# Dialog shell refactor — Fluent v9 design language

**Branch**: `fluent-ux-refactor` | **Date**: 2026-06-28 | **Parent spec**: [spec.md](spec.md)
**Visual reference**: [mockups.html](mockups.html) (mockup #10 = canonical Unsaved Changes dialog)

## Why this exists

The Bowties app accumulated ~17 modal surfaces, each rolling its own overlay, chrome,
button styling, keyboard handling, and ARIA. Visible drift includes overlays at
`0.35` / `0.45` / `0.5` opacity; title sizes 14/16/18/`1.1rem`; primary buttons in
Tailwind blue `#2563eb`, Fluent blue (implied), and `#0066cc`; destructive actions
shown variously as Tailwind red `#dc2626` and Fluent severeWarning orange `#ca5010`
(the latter is the wrong token — orange is Fluent's *warning glyph* color, not its
destructive button color).

The existing design intent was Fluent UI. This refactor formalises that intent as a
single dialog shell + token set built on **Fluent UI v9 tokens with v8-style header
and footer dividers** (a deliberate, documented departure from current Fluent), and
migrates every dialog onto it.

This is internal architecture work; no user-visible behavior changes. Visible polish
changes are expected and intentional.

## Design language summary

See [mockups.html](mockups.html) for the canonical visuals. Key tokens:

| Slot | Value | Fluent token |
|---|---|---|
| Surface | `#ffffff` | `colorNeutralBackground1` |
| Footer fill | `#fafafa` | `colorNeutralBackground2` |
| Hover surface | `#f5f5f5` | `colorSubtleBackgroundHover` |
| Border / divider | `#d1d1d1` | `colorNeutralStroke1` |
| Title text | `#242424` / 16px / weight 600 / 22px line | `colorNeutralForeground1` + `fontSizeBase400` |
| Body text | `#424242` / 14px / 20px line | `colorNeutralForeground2` + `fontSizeBase300` |
| Metadata | `#616161` / 12px | `colorNeutralForeground3` |
| Primary button | `#0f6cbd` → `#115ea3` hover → `#0c3b5e` pressed | `colorBrandBackground*` |
| Danger button | `#c50f1f` → `#b10e1c` hover | `colorStatusDangerBackground3*` |
| Warning glyph | `#bc4b09` | `colorStatusWarningForeground3` (glyph only — never on buttons) |
| Focus ring | `#2886de` + 2px halo | `colorStrokeFocus2` |
| Font stack | `'Segoe UI Variable', 'Segoe UI', system-ui, -apple-system, BlinkMacSystemFont, sans-serif` | |

Chrome rules:

- **Header**: white surface, `border-bottom: 1px solid var(--fluent-stroke1)`, 14px × 18px padding. Inline glyph (`⚠ ❌ ⓘ`) optional, title, then bare `×` close right-aligned.
- **Body**: white surface, 16px × 18px padding, inherits 14px body text.
- **Footer**: subtle background, `border-top: 1px solid var(--fluent-stroke1)`, 12px × 18px padding, right-aligned action cluster with 8px gap.
- **Close button (`×`)**: bare 28×28 icon target, no border, no background until hover (then `colorSubtleBackgroundHover` at 4px radius). Always maps to `onCancel` — same as Esc and overlay click.
- **Surface**: 8px radius, shadow `0 8px 24px rgba(0,0,0,.18), 0 0 2px rgba(0,0,0,.12)`.
- **Overlay**: `rgba(0,0,0,.35)`, fade-in 150ms, slide-in 180ms.
- **Widths**: `sm` 400 (confirms), `md` 480 (forms), `lg` 600 (pickers, viewers).

Destructive label convention: just **Discard** (or the verb alone — `Delete`, `Remove`). The orange ⚠ glyph in the title labels the dialog as a warning; the red button color names the action as destructive; the verb says what will happen.

## Component contracts

### `Dialog` — shell

Declarative shell only. Owns overlay, focus trap, Esc/overlay/× → `onCancel`, ARIA wiring.
Does **not** own workflow sequencing — per `frontend-components.instructions.md`,
callers decide what `onCancel` and any `onConfirm` actually do.

```ts
interface DialogProps {
  open: boolean;                          // caller owns visibility
  variant?: 'confirm' | 'form' | 'picker' | 'progress' | 'viewer';
  width?: 'sm' | 'md' | 'lg' | number;    // sm=400, md=480, lg=600
  closable?: boolean;                      // default true; false → hides ×, ignores Esc/overlay
  ariaLabel?: string;                      // when title slot is non-text (e.g. progress)
  onCancel: () => void;                    // Esc, overlay click, × button
  title?: Snippet;                         // header content; omit → no header divider
  children: Snippet;                       // body content
  actions?: Snippet;                       // footer content; omit → no footer
}
```

### `DialogTitle` — header content helper

```ts
interface DialogTitleProps {
  glyph?: 'warning' | 'error' | 'info' | null;  // ⚠ ❌ ⓘ in Fluent palette
  children: Snippet;                            // title text
}
```

### `DialogActions` — footer content helper

Right-aligned flex row with 8px gap. Authors place `<Button>`s inside.

### `Button`

```ts
interface ButtonProps {
  appearance?: 'primary' | 'secondary' | 'subtle' | 'outline';
  intent?: 'default' | 'danger';            // intent='danger' + appearance='primary' → red destructive
  size?: 'sm' | 'md';
  disabled?: boolean;
  type?: 'button' | 'submit';
  onclick?: (e: MouseEvent) => void;
  children: Snippet;
  // standard a11y / HTML attribute passthrough
}
```

Notes:

- `appearance="primary"` + `intent="default"` → Fluent brand blue.
- `appearance="primary"` + `intent="danger"` → Fluent danger red.
- `appearance="secondary"` → neutral outlined (Cancel).
- `appearance="subtle"` → text-only, hover surface fill (replaces existing `btn-link`).

Fluent v9 does not ship a `danger` button intent; we add it because destructive confirms
recur across the app and need a consistent treatment. This is documented in the ADR.

## File layout

```
app/src/lib/components/Dialog/
  Dialog.svelte
  DialogTitle.svelte
  DialogActions.svelte
  Button.svelte
  tokens.css                 // --fluent-* CSS custom properties
  Dialog.test.ts             // shell: Esc, overlay, ×, focus trap, slots
  Button.test.ts             // appearance + intent classes; click handling
  README.md                  // when to use each variant + examples
```

`tokens.css` is imported once in `+layout.svelte` (or `+page.svelte`'s global block).
Tokens use a `--fluent-*` prefix so they don't collide with existing
`--accent-blue` / `--text-*` family currently in use. A follow-up pass can sweep
non-dialog surfaces to the same tokens; that is out of scope here.

`README.md` is the discoverable design-language home — future AI sessions and humans
land there from `aiwiki/owners.md` instead of re-deriving from mockups.

## Surfaces to migrate

17 modal surfaces today:

| Cluster | Surface | Variant | Special notes |
|---|---|---|---|
| **Confirms** | `UnsavedChangesDialog.svelte` | confirm + danger | Currently orange; flip to red. Canonical first migration. |
| | `DiscardConfirmDialog.svelte` | confirm + danger | |
| | `ErrorDialog.svelte` | confirm + info | Has `Copy Error` secondary; preserve. |
| | `+page.svelte` inline: channel-removal | confirm + danger | Extract to named component. |
| | `+page.svelte` inline: delete-placeholder | confirm + danger | Extract; preserve `data-testid="confirm-delete-placeholder"`. |
| **Forms** | `Facilities/AddFacilityDialog.svelte` | form | Spec 018 dialog. |
| | `AddBoardDialog.svelte` | form | Header comment says it copies `DiscardConfirmDialog` — prior consistency attempt. |
| | `LayoutPicker/NewLayoutDialog.svelte` | form | |
| | `Bowtie/NewConnectionDialog.svelte` | form / picker hybrid | |
| | `ConnectionManager.svelte` inline modal | form | Uses native `<dialog>`; replace with shell. Loses native modal stacking (acceptable — no other native dialogs exist). |
| **Pickers** | `Bowtie/AddElementDialog.svelte` | picker | |
| **Progress** | `SaveProgressDialog.svelte` | progress | `closable={false}`, no footer, `aria-live="polite"`. |
| | `CdiDownloadDialog.svelte` | progress + actions | `closable` toggles by phase. |
| | `CdiRedownloadDialog.svelte` | progress + actions | `closable` toggles by phase. |
| | `+page.svelte` inline: layout-loading | progress | Extract. |
| **Viewers** | `AboutDialog.svelte` | viewer | Footer = Close. |
| | `CdiXmlViewer.svelte` modal pane | viewer | Wide, scroll-heavy → `width="lg"`. |

## Slice plan

Each slice ends in a runnable, demoable, test-green state.

### Slice 1 — App-wide font stack (HITL: visual scan) — ✅ done
Set the Fluent stack at `<body>`:

```css
font-family: 'Segoe UI Variable', 'Segoe UI', system-ui,
             -apple-system, BlinkMacSystemFont, sans-serif;
```

Removes the "dialog reads different from page" source at the root. Done first so
dialog visual diffs in later slices are not entangled with a global font shift.
Risk: minor glyph-width changes on tooltips/tables; functionally low risk.

### Slice 2 — Shell + tokens + canonical `UnsavedChangesDialog` migration (HITL: visual + diagnostic check) — ✅ done
Add `Dialog/`, `Button/`, `tokens.css`, tests, `README.md`, then rebuild
`UnsavedChangesDialog` on the shell in the same slice. The migrated dialog is
the canonical visual demo — better than a temp dev route, and the test pair
(shell + real consumer) exercises the design language end-to-end.

For the migration: keep `Props` and the breakdown formatter unchanged; flip the
destructive primary from orange `#ca5010` to red `#c50f1f`; add the bare `×`
close; update `UnsavedChangesDialog.test.ts` only where it queried by removed
class names.

**Shell tests**: renders title/body/actions slots, Esc/overlay/× call
`onCancel`, focus traps within the dialog, `closable={false}` disables
Esc/overlay/×, every button appearance + intent combination renders the
expected class set.

After this slice, the dialog matches mockup #10 and the design language is
locked.

### Slice 3 — Remaining confirms (AFK) — ✅ done
`DiscardConfirmDialog`, `ErrorDialog`. Extract the two `+page.svelte` inline
confirms (channel-removal, delete-placeholder) into named components and migrate.

### Slice 4 — Forms (AFK) — ✅ done
`AddFacilityDialog`, `AddBoardDialog`, `NewLayoutDialog`, `NewConnectionDialog`,
`ConnectionManager` modal. Latter replaces native `<dialog>` with the shell — add
an explicit Esc handler since the native element previously provided it.

### Slice 5 — Pickers (AFK) — ✅ done
`AddElementDialog`. Body density unchanged; only chrome swaps.

### Slice 6 — Progress / non-dismissible (HITL: keyboard / a11y) — ✅ done
`SaveProgressDialog`, `CdiDownloadDialog`, `CdiRedownloadDialog`, layout-loading
inline. `closable={false}` means: no `×`, Esc and overlay click do nothing,
`aria-live="polite"`. `CdiDownloadDialog`/`Redownload` toggle `closable` on/off
per phase (closable in idle/success/error, locked while downloading) — phase
logic stays in the consumer, not the shell.

### Slice 7 — Viewers (AFK) — ✅ done
`AboutDialog`, `CdiXmlViewer` modal pane. `width="lg"`, footer = single
`Close` secondary button.

### Slice 8 — Cleanup + docs (HITL: final sweep) — ✅ done
- Remove dead CSS from migrated dialogs (`.uc-*`, `.dc-*`, `.ed-*`, `.cm-*`, `.sp-*`).
- Update `aiwiki/owners.md` with the `Dialog/` cluster + variant taxonomy.
- Land the ADR (see Documentation, below).

## Tests strategy

- **Shell tests (new)**: focus trap, Esc / overlay / × wiring, slot rendering,
  `closable={false}` behavior, ARIA attributes.
- **Per-dialog tests (modify, not rewrite)**: existing tests are largely
  content-and-callback assertions (`getByText`, `getByRole('button', { name })`).
  They survive verbatim as long as accessible names are preserved. Tests that
  queried by removed class names (`uc-btn--confirm`, etc.) are the canary —
  update those selectors.
- **Route tests**: `page.route.test.ts` mocks most dialogs via
  `StubComponent.svelte`, so route tests are unaffected.
- **Visual regression**: out of scope unless we add Playwright snapshots in
  Slice 2 (recommend: skip for this refactor; rely on the demo route + manual
  review).

## Documentation updates

- **`aiwiki/owners.md`**: add a `Dialog/` entry under components with the variant
  taxonomy and a one-line cue for each variant.
- **New ADR** (`product/architecture/adr/NNNN-dialog-shell.md`): records four
  decisions —
  1. Fluent v9 tokens, v8-style header/footer dividers (deliberate departure).
  2. Destructive action = Fluent danger red on the button; severeWarning orange
     is the title glyph color only.
  3. Non-dismissible surfaces use `closable={false}` on the same shell, not a
     separate `<Progress>` component.
  4. Deprecate the native `<dialog>` element in `ConnectionManager` for shell
     consistency.
- **`mockups.html`**: stays in `specs/018/` (this refactor was scoped during 018).
  Linked from the ADR and this file.

## Risks and notes

1. **Visible churn across every dialog at once.** Worth screenshotting before /
   after for the release notes when this branch merges.
2. **Class-name selectors in tests.** A grep for `uc-btn|dc-btn|ed-btn|cm-|sp-`
   finds the brittle ones; update during the relevant migration slice.
3. **`closable={false}` per phase** (Cdi*Dialog) needs the consumer to compute
   the boolean from its store state — keep that logic in the consumer so the
   shell stays free of workflow awareness.
4. **Slice 1 font swap** is the largest non-dialog visual diff. If unwanted
   glyph-width shifts appear in non-dialog surfaces during the visual scan, narrow
   the change to a class scoped to dialog ancestors and defer the global swap to
   a separate change.
5. **`--fluent-*` token prefix** keeps the diff additive. A follow-up
   refactor can rename existing `--accent-blue` / `--text-*` to the Fluent
   names and remove the prefix; that is intentionally out of scope here.

## Decisions confirmed

- ✅ Fluent v9 tokens on v8-style chrome (mockup look).
- ✅ Destructive label = just the verb (`Discard`, `Delete`, `Remove`).
- ✅ Single design-language file under `specs/018/`; mockups stay in `specs/018/`.
- ✅ Branch `fluent-ux-refactor` owns the work.
- ✅ Token naming: `--fluent-*` prefix. Additive; legacy `--accent-blue` /
  `--text-*` family stays untouched in this branch. A later, separate sweep
  can drop the prefix once Fluent is the only token system.
- ✅ No temporary dev demo route. The migrated `UnsavedChangesDialog` from
  Slice 2 is the canonical visual lockup; `mockups.html` remains the static
  reference.

## Decisions still open

- ~~**ADR number**: next available in `product/architecture/adr/`.~~ Landed as
  [ADR-0014](../../product/architecture/adr/0014-dialog-shell.md).
