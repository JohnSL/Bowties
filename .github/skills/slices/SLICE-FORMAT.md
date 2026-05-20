# Slice File Format

Format for `specs/<feature>/slices.md` — the cross-session progress tracker.

Fields are designed for future GitHub issue compatibility: each slice maps to one issue with title, description, acceptance criteria, and blocked-by.

## File Structure

```markdown
# Slices: {Feature Title}

Branch: {branch-name}
Generated: {YYYY-MM-DD}
Status: {N}/{total} slices complete

---

## S1: {Slice title} [{HITL|AFK}]

**Layers**: {Route, Component, Store, API, Backend, lcc-rs}
**Blocked by**: None
**Complexity**: {small | medium | large}
**User stories**: {US1, US3 — from spec.md}

{One-paragraph description of the end-to-end behavior this slice delivers. Describe the behavior, not layer-by-layer implementation.}

**Acceptance criteria**:
- [ ] {Criterion 1}
- [ ] {Criterion 2}

**Tasks**:
- [ ] S1-T1: Write integration test — {what the test proves}
- [ ] S1-T2: {Deepest layer} — {what to implement}
- [ ] S1-T3: {Next layer} — {what to implement}
- [ ] S1-T4: {Next layer} — {what to implement}
- [ ] S1-T5: Validate — test passes, slice verified

---

## S2: {Slice title} [{HITL|AFK}]

**Layers**: {layers}
**Blocked by**: S1
...
```

## Conventions

### Task IDs

Format: `S{slice}-T{task}` — e.g., `S1-T1`, `S2-T3`.

Sequential within each slice. Task 1 is always the integration test. The last task is always validation.

### Checkboxes

- `[ ]` — not started
- `[x]` — completed

The `/build` skill checks off tasks as it completes them. The status line at the top (`N/total slices complete`) is updated when all tasks in a slice are checked.

### HITL/AFK Labels

In the slice header: `[HITL]` or `[AFK]`.

- **HITL**: `/build` presents the architectural pattern question to the user before implementing
- **AFK**: `/build` implements autonomously following established patterns

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

---

## S1: View CDI tree structure [HITL]

**Layers**: Route, Component, Orchestrator, Store, API, Backend, lcc-rs
**Blocked by**: None
**Complexity**: large
**User stories**: US1

The user connects to a node and navigates to its configuration page. The app reads the node's CDI XML, parses it into a tree structure, and renders the groups and fields in a navigable sidebar.

**Acceptance criteria**:
- [ ] CDI tree renders with correct group hierarchy
- [ ] Groups are expandable/collapsible
- [ ] Fields show names from CDI XML

**Tasks**:
- [ ] S1-T1: Write integration test — CDI tree renders after connecting to a node
- [ ] S1-T2: lcc-rs — CDI parse returns typed tree structure
- [ ] S1-T3: Backend — command to fetch and cache CDI for a node
- [ ] S1-T4: API — Tauri invoke binding for CDI fetch
- [ ] S1-T5: Store — CDI tree state with loading/loaded/error
- [ ] S1-T6: Orchestrator — CDI read workflow (fetch → parse → store)
- [ ] S1-T7: Component — CDI tree sidebar rendering
- [ ] S1-T8: Route — config page composition with sidebar
- [ ] S1-T9: Validate — integration test passes, tree is navigable

---

## S2: Read a single config value [AFK]

**Layers**: Component, Store, API, Backend, lcc-rs
**Blocked by**: S1
**Complexity**: medium
**User stories**: US2

The user selects a field in the CDI tree. The app reads the field's current value from the node via memory config protocol and displays it.

**Acceptance criteria**:
- [ ] Selecting a field triggers a memory read
- [ ] Field displays the current value from the node
- [ ] Loading state shown while reading

**Tasks**:
- [ ] S2-T1: Write integration test — selecting a field shows its current value
- [ ] S2-T2: lcc-rs — memory config read for a single address/size
- [ ] S2-T3: Backend — command to read config value at address
- [ ] S2-T4: API — Tauri invoke binding for config read
- [ ] S2-T5: Store — config value state per field (loading/value/error)
- [ ] S2-T6: Component — field value display with loading state
- [ ] S2-T7: Validate — integration test passes, value matches node state
```
