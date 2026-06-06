# Slice File Format

Format for `specs/<feature>/slices.md` — the cross-session progress tracker.

This file serves three audiences:
- **Product manager**: behavior summary table + per-slice acceptance criteria → "what changed and when can I verify it?"
- **Architect**: before/after diagrams + pattern names + module table + decisions → "what's the shape change and are the trade-offs sound?"
- **Implementer**: tasks + layer targets + validation checkpoints → "what do I build and how do I verify it?"

Fields are designed for future GitHub issue compatibility: each slice maps to one issue with title, description, acceptance criteria, and blocked-by.

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

## S1: {Slice title} [{HITL|AFK|REFACTOR}]

**Layers**: {Route, Component, Store, API, Backend, lcc-rs}
**Blocked by**: None
**Complexity**: {small | medium | large}
**User stories**: {US1, US3 — from spec.md}

{One-paragraph description of the end-to-end behavior this slice delivers. Describe the behavior, not layer-by-layer implementation.}

**Acceptance criteria**:
- [ ] {Behavioral criterion verifiable by a product manager — what the user sees or can do}
- [ ] {Another behavioral criterion}

**Tasks**:
- [ ] S1-T1: Write integration test — {what the test proves}
- [ ] S1-T2: {Deepest layer} — {what to implement}
- [ ] S1-T3: {Next layer} — {what to implement}
- [ ] S1-T4: {Next layer} — {what to implement}
- [ ] S1-T5: Validate — {test suite(s) pass, implementation assertions}

---

## S2: {Slice title} [{HITL|AFK|REFACTOR}]

**Layers**: {layers}
**Blocked by**: S1
...
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

### Checkboxes

- `[ ]` — not started
- `[x]` — completed

The `/build` skill checks off tasks as it completes them. The status line at the top (`N/total slices complete`) is updated when all tasks in a slice are checked.

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

## S1: View CDI tree structure [HITL]

**Layers**: Route, Component, Orchestrator, Store, API, Backend, lcc-rs
**Blocked by**: None
**Complexity**: large
**User stories**: US1

The user connects to a node and navigates to its configuration page. The app reads the node's CDI XML, parses it into a tree structure, and renders the groups and fields in a navigable sidebar.

**Acceptance criteria**:
- [ ] Opening a node's config page shows its CDI tree with named groups and fields
- [ ] Groups are expandable and collapsible
- [ ] The tree structure matches the node's actual CDI (not hardcoded)

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

---

## S2: Read a single config value [AFK]

**Layers**: Component, Store, API, Backend, lcc-rs
**Blocked by**: S1
**Complexity**: medium
**User stories**: US2

The user selects a field in the CDI tree. The app reads the field's current value from the node via memory config protocol and displays it.

**Acceptance criteria**:
- [ ] Selecting a field shows its current value read from the node
- [ ] A loading indicator appears while the value is being read
- [ ] The displayed value matches what the node reports

**Tasks**:
- [ ] S2-T1: Write integration test — selecting a field shows its current value
- [ ] S2-T2: lcc-rs — memory config read for a single address/size
- [ ] S2-T3: Backend — command to read config value at address
- [ ] S2-T4: API — Tauri invoke binding for config read
- [ ] S2-T5: Store — config value state per field (loading/value/error)
- [ ] S2-T6: Component — field value display with loading state
- [ ] S2-T7: Validate — integration test passes; value matches node state
```
