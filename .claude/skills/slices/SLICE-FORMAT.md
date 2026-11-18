# Slice File Format

Format for `specs/<feature>/slices.md` — the cross-session progress tracker.

This file serves three audiences:
- **Product manager**: behavior summary table + per-slice acceptance criteria → "what changed and when can I verify it?"
- **Architect**: before/after diagrams + pattern names + module table + decisions → "what's the shape change and are the trade-offs sound?"
- **Implementer**: tasks + layer targets + validation checkpoints → "what do I build and how do I verify it?"

Fields are designed for future GitHub issue compatibility: each slice maps to one issue with title, description, acceptance criteria, and blocked-by.

## Two Tiers: Roadmap + Just-In-Time Tasks

The file has **two tiers**, and `/slices` writes only the first:

- **Tier 1 — Roadmap** (authored by `/slices`): the ordered set of slices, each written as a **roadmap card** rich enough to review without seeing code — title, one-line intent, layer boundary, HITL/AFK/REFACTOR label, blocked-by, **acceptance criteria** (what you'll be able to test/demo), and — for slices that shift the architecture — a short **architecture note** (pattern introduced / seam touched). Each card starts at `status: sketched`. This is the whole-feature view the architecture firewall validated in `/design`.
- **Tier 2 — Task breakdown** (authored by `/build`, one slice at a time): the per-layer task list for a single slice, appended to that slice's card **only when `/build` is about to implement it**. Expanding the next slice flips its status `sketched → tasked`; finishing it flips `tasked → done`.

**Only the per-layer task breakdown is deferred — not the slice's behavioral contract.** Acceptance criteria and the architecture note are the slice's *purpose* and *impact*; you need them to review and approve a slice (especially a HITL one), and they are cheap and pivot-stable to author. What is genuinely pivot-fragile is the per-layer task list (`S1-T2: store…`, `S1-T3: backend…`), because earlier slices routinely invalidate it (re-slices, "Option 2/3" re-cuts). Plan **direction broadly, tasks narrowly**: the roadmap carries enough to review, but a mid-feature pivot still edits a card plus the one slice in flight — not a pre-written task list.

## Slice Status Lifecycle

Each slice carries a `status`:

| Status | Meaning | Set by |
|--------|---------|--------|
| `sketched` | Roadmap card (intent, boundary, acceptance criteria, arch note) written; no task breakdown yet | `/slices` |
| `tasked` | Per-layer task breakdown appended to the card; ready to implement or in progress | `/build` at slice start |
| `done` | All tasks checked; slice complete | `/build` at slice end |

`/build` only ever holds **one** slice at `tasked` at a time. After completing it, `/build` re-reads the roadmap, adjusts the *next* slice's card (boundary and, if learning changed them, acceptance criteria) in light of what was learned, then appends that slice's task breakdown.

## File Structure

```markdown
# Slices: {Feature Title}

Branch: {branch-name}
Generated: {YYYY-MM-DD}
Status: {N}/{total} slices complete

## Architecture

### Before

{Mermaid diagram showing the module-level architecture today. Show responsibilities and data flow between modules, not code details.}

### After

{Mermaid diagram showing the module-level architecture after all slices land.}

### Patterns

- **{Pattern name}** — {One-sentence explanation of what it means in this feature's context.}
- ...

### Module Changes

| Module | Today | After |
|---|---|---|
| {module name} | {current responsibility} | {new responsibility} |

### Behavior Summary

| Slice | User-visible change | Demoable? |
|---|---|---|
| S1: {title} | {what the user can see or do} | Yes |
| S2: {title} | {what the user can see or do} | Yes |
| S3: {title} | Invariant preserved: {what stays the same} | No (REFACTOR) |

---

## Roadmap

The ordered slice set. An overview table for at-a-glance scanning, followed by one **roadmap card** per slice. `/build` appends a task breakdown to a card when it implements that slice; it does not pre-author tasks.

| # | Slice title | Label | Blocked by | Status |
|---|---|---|---|---|
| S1 | {title} | HITL | None | sketched |
| S2 | {title} | AFK | S1 | sketched |
| S3 | {title} | REFACTOR | S2 | sketched |

### S1: {Slice title} [{HITL|AFK|REFACTOR}]

**Intent**: {what the user can see or do after this slice — for REFACTOR, the invariant preserved}
**Boundary**: {Route → Component → Store → API → Backend → lcc-rs — the layers this slice cuts}
**Blocked by**: {None | S{M}}
**Status**: sketched

**Acceptance criteria**:
- [ ] {Behavioral criterion verifiable by a product manager — what the user sees or can do}
- [ ] {Another behavioral criterion}

**Architecture note** *(HITL / new-seam slices only)*: {1–2 lines — the pattern this slice introduces or the seam it changes, and why it needs review. Omit for AFK/REFACTOR slices that reuse an established pattern.}

### S2: {Slice title} [{HITL|AFK|REFACTOR}]

**Intent**: {one-line intent}
**Boundary**: {layers}
**Blocked by**: S1
**Status**: sketched

**Acceptance criteria**:
- [ ] {criterion}

---

<!--
Tier 2 — Task breakdown. `/build` appends a Tasks block (and complexity / user-stories)
to a slice's card when it starts that slice, flipping the slice's status to `tasked`.
Do not pre-author the task breakdown. An expanded slice card looks like this:
-->

### S1: {Slice title} [{HITL|AFK|REFACTOR}]

**Intent**: {one-line intent}
**Boundary**: {layers}
**Blocked by**: {None | S{M}}
**Status**: tasked
**Complexity**: {small | medium | large}
**User stories**: {US1, US3 — from spec.md}

**Acceptance criteria**:
- [ ] {behavioral criterion}
- [ ] {behavioral criterion}

**Architecture note** *(HITL / new-seam only)*: {pattern / seam}

**Tasks**:
- [ ] S{N}-T1: Write integration test — {what the test proves}
- [ ] S{N}-T2: {Deepest layer} — {what to implement}
- [ ] S{N}-T3: {Next layer} — {what to implement}
- [ ] S{N}-T4: Validate — {test suite(s) pass, implementation assertions}
```

## Conventions

### Acceptance Criteria vs Task Validation

**Acceptance criteria** are behavioral — they describe what a product manager can verify without reading code:
- "User adds a TurnoutBoss placeholder and sees it in the sidebar with a badge"
- "Flipping Left/Right reshapes the config tree: Detector 3 section appears/disappears"
- "Saving and reopening restores the placeholder with all edited field values intact"

**Task validation** checkpoints are implementation-level and belong in the Tasks section:
- "cargo test -p bowties green"
- "vitest run passes"
- "repo-wide grep for removed identifiers returns zero"

For `[REFACTOR]` slices, acceptance criteria describe the invariant preserved:
- "Existing Tower-LCC layouts open and render identically to before the migration"
- "All connector + daughterboard combinations produce the same relevance/role outcomes"

### Task IDs

Format: `S{slice}-T{task}` — e.g., `S1-T1`, `S2-T3`.

Sequential within each slice. Task 1 is always the integration test. The last task is always validation.

Task IDs only exist once a slice is expanded (`status: tasked`). A `sketched` slice's card has acceptance criteria but no task breakdown yet.

### Checkboxes

- `[ ]` — not started
- `[x]` — completed

The `/build` skill checks off tasks as it completes them. The status line at the top (`N/total slices complete`) is updated when all tasks in a slice are checked.

### Slice Status

Every roadmap card carries a `status`: `sketched` → `tasked` → `done`.

- `sketched`: written by `/slices`; the card has intent, boundary, blocked-by, acceptance criteria, and (for HITL/new-seam slices) an architecture note — but no task breakdown.
- `tasked`: written by `/build` when it starts the slice; a per-layer Tasks block (plus complexity / user-stories) is now appended to the card.
- `done`: written by `/build` when all of the slice's tasks are checked.

Only one slice is `tasked` at a time. `/build` expands the next `sketched` slice only after the current one is `done`, re-reading the roadmap and adjusting the next slice's card (boundary, and acceptance criteria if learning changed them) first.

### HITL/AFK/REFACTOR Labels

In the slice header: `[HITL]`, `[AFK]`, or `[REFACTOR]`.

- **HITL**: `/build` presents the architectural context and numbered decisions to the user before implementing (see build skill HITL format)
- **AFK**: `/build` implements autonomously following established patterns
- **REFACTOR**: `/build` implements autonomously; acceptance criteria describe invariants preserved, not new behavior. No user-visible change.

### Complexity Estimates

- **small**: 1-2 layers, follows existing pattern, <30 min estimated
- **medium**: 3-4 layers, minor variation on existing pattern, 30-60 min estimated
- **large**: 5+ layers or new pattern, >60 min estimated

These help `/build` judge how many slices fit in a session.

### Blocked-by

Reference other slices: `S1`, `S1 + S2`, or `None`.

`/build` processes slices in order, respecting blocked-by. If a slice is blocked by an incomplete slice, it's skipped.

### Session Boundaries

`/build` always stops at a slice boundary — never mid-slice. When stopping, it updates the status line and leaves a session note:

```markdown
<!-- Session: YYYY-MM-DD — Completed S1-S3. Next: S4 (HITL). -->
```

## Example

```markdown
# Slices: Node Configuration Editor

Branch: 014-config-editor
Generated: 2026-05-17
Status: 0/4 slices complete

## Architecture

### Before

` ``mermaid
flowchart LR
    Bus[Bus discovery] --> Raw[Raw CDI XML<br/>in memory]
    Raw --> ???[No rendering path]
` ``

### After

` ``mermaid
flowchart LR
    Bus[Bus discovery] --> Backend[Backend<br/>CDI fetch + parse]
    Backend --> IPC[IPC] --> Store[Tree Store]
    Store --> Component[CDI Tree<br/>Sidebar]
    Component --> Route[Config Page]
    User[User edits] --> DraftStore[Draft Store] --> WriteOrch[Write Orchestrator] --> Backend
` ``

### Patterns

- **Read-render pipeline** — Backend fetches and parses CDI into a typed tree; frontend store caches the tree; component renders it. One fetch, one parse, one render.
- **Draft-over-baseline** — Edits layer on top of baseline values from the node. The store resolves the effective value (draft → baseline) so components never coordinate the fallback chain.
- **Write-back orchestrator** — A single orchestrator owns the multi-step write workflow (diff → write → verify → update baseline). Components trigger it; they don't sequence the steps.

### Module Changes

| Module | Today | After |
|---|---|---|
| lcc-rs CDI parser | Parses XML to raw DOM | Parses XML to typed tree structure |
| Backend CDI command | Does not exist | Fetches, caches, and serves parsed CDI trees |
| Tree Store | Does not exist | Caches parsed tree + per-field baseline values |
| Draft Store | Does not exist | Layers user edits over baseline; resolves effective value |
| CDI Sidebar component | Does not exist | Renders expandable/collapsible tree from store |
| Write Orchestrator | Does not exist | Owns diff → write → verify → update-baseline workflow |

### Behavior Summary

| Slice | User-visible change | Demoable? |
|---|---|---|
| S1: View CDI tree | User sees the node's config tree with groups and fields | Yes |
| S2: Read config value | Selecting a field shows its current value from the node | Yes |
| S3: Edit config value | Editing a field shows the modified value with a visual indicator | Yes |
| S4: Write changes | Writing changes persists them on the node | Yes |

---

## Roadmap

| # | Slice title | Label | Blocked by | Status |
|---|---|---|---|---|
| S1 | View CDI tree structure | HITL | None | tasked |
| S2 | Read a single config value | AFK | S1 | sketched |
| S3 | Edit a config value | HITL | S2 | sketched |
| S4 | Write changes to the node | AFK | S3 | sketched |

<!-- Only S1 is expanded (has a Tasks block). S2–S4 cards carry intent + acceptance criteria but no tasks until their turn. -->

### S1: View CDI tree structure [HITL]

**Intent**: User sees the node's config tree with named groups and fields
**Boundary**: Route → Component → Orchestrator → Store → API → Backend → lcc-rs
**Blocked by**: None
**Status**: tasked
**Complexity**: large
**User stories**: US1

**Acceptance criteria**:
- [ ] Opening a node's config page shows its CDI tree with named groups and fields
- [ ] Groups are expandable and collapsible
- [ ] The tree structure matches the node's actual CDI (not hardcoded)

**Architecture note** *(HITL — new seam)*: Establishes the read-render pipeline (backend fetch+parse → store cache → component render) and the first CDI IPC contract. Both are reused by every later config slice, so the shape is load-bearing — review before build.

**Tasks**:
- [ ] S1-T1: Write integration test — CDI tree renders after connecting to a node
- [ ] S1-T2: lcc-rs — CDI parse returns typed tree structure
- [ ] S1-T3: Backend — command to fetch and cache CDI for a node
- [ ] S1-T4: API — Tauri invoke binding for CDI fetch
- [ ] S1-T5: Store — CDI tree state with loading/loaded/error
- [ ] S1-T6: Orchestrator — CDI read workflow (fetch → parse → store)
- [ ] S1-T7: Component — CDI tree sidebar rendering
- [ ] S1-T8: Route — config page composition with sidebar
- [ ] S1-T9: Validate — integration test passes; vitest + cargo test green

### S2: Read a single config value [AFK]

**Intent**: Selecting a field shows its current value read from the node
**Boundary**: Component → Store → API → Backend → lcc-rs
**Blocked by**: S1
**Status**: sketched

**Acceptance criteria**:
- [ ] Selecting a field shows its current value read from the node
- [ ] A loading indicator appears while the value is being read
- [ ] The displayed value matches what the node reports

### S3: Edit a config value [HITL]

**Intent**: Editing a field shows the modified value with a visual indicator
**Boundary**: Component → Store
**Blocked by**: S2
**Status**: sketched

**Acceptance criteria**:
- [ ] Editing a field shows the modified value and marks it as changed
- [ ] The changed field is visually distinct from unchanged fields

**Architecture note** *(HITL — new seam)*: Introduces the draft-over-baseline pattern (edits layer over node baseline; store resolves the effective value). Decides where the draft state lives and how the effective value is resolved — review before build.

### S4: Write changes to the node [AFK]

**Intent**: Writing changes persists them on the node
**Boundary**: Orchestrator → Store → API → Backend → lcc-rs
**Blocked by**: S3
**Status**: sketched

**Acceptance criteria**:
- [ ] Writing changes sends the correct memory-config datagrams
- [ ] After a successful write, values persist on the node and the baseline updates

<!--
S2–S4 are NOT expanded yet (no Tasks block). When `/build` finishes S1 it will
re-read the roadmap, adjust S2's card in light of what S1 revealed, then append
S2's Tasks block and flip S2's status to `tasked`.
-->
```
