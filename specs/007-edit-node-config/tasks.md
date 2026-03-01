# Tasks: Editable Node Configuration with Save

**Input**: Design documents from `/specs/007-edit-node-config/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-ipc.md, quickstart.md

**Tests**: Tests are included per Constitution Principle III (Test-Driven Development is MANDATORY).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Protocol library**: `lcc-rs/src/`
- **Tauri backend**: `app/src-tauri/src/`
- **Frontend**: `app/src/lib/`
- **Frontend tests**: co-located with source (`.test.ts` alongside `.svelte`/`.ts`)
- **Rust tests**: inline `#[cfg(test)] mod tests` in source files

---

## Phase 1: Setup

**Purpose**: No new project structure needed — this feature extends existing codebase. Setup verifies the build is clean.

- [X] T001 Verify clean build: run `cargo check` in `lcc-rs/` and `cargo check` in `app/src-tauri/`, run `pnpm check` in `app/`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Protocol-level write support in lcc-rs and Tauri backend commands. ALL user stories depend on these — nothing can be tested end-to-end without write capability.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

### Protocol Layer (lcc-rs)

- [X] T002 [P] Implement `MemoryConfigCmd::build_write()` in `lcc-rs/src/protocol/memory_config.rs` — mirrors `build_read()` with write command bytes (0x00-0x03 instead of 0x40-0x43), includes data payload instead of read count. Support embedded format (spaces ≥ 0xFD) and generic format (spaces < 0xFD with separate space byte). Validate payload 1-64 bytes. Per research.md R1 and OpenLCB_Java `McsWriteMemo.fillRequest()`.
- [X] T003 [P] Implement `MemoryConfigCmd::build_update_complete()` in `lcc-rs/src/protocol/memory_config.rs` — builds 2-byte datagram `[0x20, 0xA8]` per S-9.7.4.2 §4.23. Takes source_alias and dest_alias, returns `Result<Vec<GridConnectFrame>>`.
- [X] T004 [P] Add unit tests for `build_write()` in `lcc-rs/src/protocol/memory_config.rs` — test embedded format (0xFD config space), generic format (0xFB ACDI User space), boundary cases (1-byte payload, 64-byte payload), invalid payload (0 bytes, 65 bytes). Verify byte-for-byte match with OpenLCB_Java reference output.
- [X] T005 [P] Add unit tests for `build_update_complete()` in `lcc-rs/src/protocol/memory_config.rs` — verify datagram payload is exactly `[0x20, 0xA8]` and frames are correctly constructed.
- [X] T006 Implement `LccConnection::write_memory()` in `lcc-rs/src/discovery.rs` — send write datagram(s), await Datagram Received OK, retry up to 3 times with 3-second timeout. For data > 64 bytes, chunk into sequential ≤64-byte writes with address advancing. Use `RequestWithNoReply` pattern (Datagram Received OK without Reply Pending = success). Per research.md R1 and R3.
- [X] T007 Implement `LccConnection::send_update_complete()` in `lcc-rs/src/discovery.rs` — send `[0x20, 0xA8]` datagram, await Datagram Received OK. Fire-and-forget per OpenLCB_Java `CdiPanel.runUpdateComplete()`.
- [X] T008 [P] Add unit tests for `write_memory()` in `lcc-rs/src/discovery.rs` using `MockTransport` — test successful single-chunk write, successful multi-chunk write (>64 bytes), retry on timeout, failure after 3 retries, correct address advancement for chunked writes.
- [X] T009 [P] Add unit test for `send_update_complete()` in `lcc-rs/src/discovery.rs` using `MockTransport` — verify correct datagram payload and acknowledgment handling.
- [X] T010 Export new public types and functions in `lcc-rs/src/lib.rs` — ensure `build_write`, `build_update_complete`, `write_memory`, `send_update_complete` are accessible from the Tauri backend.

### Tauri Backend Commands

- [X] T011 Implement `write_config_value` Tauri command in `app/src-tauri/src/commands/cdi.rs` — accepts `node_id: String`, `address: u32`, `space: u8`, `data: Vec<u8>`. Resolves node alias from discovered nodes, converts space byte to `AddressSpace` enum, calls `connection.write_memory()`. Returns `WriteResponse { address, space, success, error_code, error_message, retry_count }`. Per contracts/tauri-ipc.md.
- [X] T012 Implement `send_update_complete` Tauri command in `app/src-tauri/src/commands/cdi.rs` — accepts `node_id: String`, resolves alias, calls `connection.send_update_complete()`. Returns `Result<(), String>`.
- [X] T013 Add `WriteResponse` struct with serde Serialize in `app/src-tauri/src/commands/cdi.rs` — fields: `address: u32`, `space: u8`, `success: bool`, `error_code: Option<u16>`, `error_message: Option<String>`, `retry_count: u32`.
- [X] T014 Register `write_config_value` and `send_update_complete` commands in `app/src-tauri/src/lib.rs` invoke_handler.

### Frontend Types & API

- [X] T015 [P] Add `PendingEdit`, `WriteResult`, `SaveProgress`, `ValidationState`, `WriteState`, `SaveState` TypeScript types in `app/src/lib/types/nodeTree.ts` — per data-model.md entity definitions.
- [X] T016 [P] Create `serializeConfigValue()` utility function in `app/src/lib/utils/serialize.ts` — converts `TreeConfigValue` + `LeafType` + `size` to `number[]` byte array. Int: big-endian bytes of CDI size. String: UTF-8 + null terminator (NOT full-padded, per research.md R2). EventId: 8 raw bytes from dotted-hex. Float: IEEE 754 big-endian (4 or 8 bytes).
- [X] T017 [P] Add unit tests for `serializeConfigValue()` in `app/src/lib/utils/serialize.test.ts` — test int (1/2/4 byte sizes, boundary values), string (ASCII, UTF-8, empty, max-length), eventId (valid dotted-hex), float (4-byte, known IEEE 754 values). Verify byte-level match with OpenLCB_Java `ConfigRepresentation` output.
- [X] T018 [P] Create `writeConfigValue()` and `sendUpdateComplete()` API wrappers in `app/src/lib/api/config.ts` — typed Tauri invoke calls per contracts/tauri-ipc.md TypeScript signatures. Import `WriteResponse` type.
- [X] T019 Create `PendingEditsStore` class in `app/src/lib/stores/pendingEdits.svelte.ts` — Svelte 5 runes (`$state<Map>`) class-based singleton pattern matching `NodeTreeStore`. Map keyed by `"${nodeId}:${space}:${address}"`. Methods: `setEdit(key, PendingEdit)`, `removeEdit(key)`, `clearAll()`, `clearForNode(nodeId)`, `markWriting(key)`, `markError(key, message)`, `markClean(key)`. Getters: `dirtyCount`, `hasInvalid`, `hasPendingEdits`, `getDirtyForNode(nodeId)`, `getDirtyForSegment(nodeId, segmentOrigin)`, `getEdit(key)`. Per data-model.md and research.md R4.
- [X] T020 [P] Add unit tests for `PendingEditsStore` in `app/src/lib/stores/pendingEdits.test.ts` — test add/remove edit, dirty count, hasInvalid blocking, per-node and per-segment queries, state transitions (dirty → writing → clean, dirty → writing → error), clearAll, auto-remove when value reverts to original.

**Checkpoint**: Foundation ready — protocol writes work end-to-end, store tracks edits. User story implementation can begin.

---

## Phase 3: User Story 1 — Edit and Save a Single Configuration Field (Priority: P1) 🎯 MVP

**Goal**: User can edit a string or numeric field inline, see it marked as unsaved, click Save, and see the value written to the node with dirty indicator clearing.

**Independent Test**: Connect to any node with writable CDI string or integer field, change the value, save, confirm field reflects the written value. Save button disabled when no changes.

### Tests for User Story 1

- [X] T021 [P] [US1] Add component test for editable string input in `app/src/lib/components/ElementCardDeck/TreeLeafRow.test.ts` — render a string-type leaf, verify text input appears, type new value, verify `PendingEditsStore` receives the edit, verify dirty CSS class applied.
- [X] T022 [P] [US1] Add component test for editable numeric input in `app/src/lib/components/ElementCardDeck/TreeLeafRow.test.ts` — render an int-type leaf (no map entries), verify number input appears with min/max from constraints, change value, verify store updated.
- [X] T023 [P] [US1] Add component test for SaveControls in `app/src/lib/components/ElementCardDeck/SaveControls.test.ts` — verify Save button disabled when no pending edits, enabled when dirty edits exist, shows progress during save, disabled when invalid edits exist.

### Implementation for User Story 1

- [X] T024 [US1] Modify `TreeLeafRow.svelte` in `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte` — replace read-only `<span>` value display with conditional editable inputs: `<input type="text">` for string fields (maxlength = `leaf.size - 1`), `<input type="number">` for int fields without map entries (min/max from constraints). On change: create `PendingEdit` with original and pending values, call `pendingEditsStore.setEdit()`. On revert to original value: call `pendingEditsStore.removeEdit()`. Add CSS classes for dirty state (e.g., left-border accent). Keep `action` and `blob` types as read-only per FR-007a.
- [X] T025 [US1] Add input validation logic in `TreeLeafRow.svelte` — validate int fields against min/max constraints, validate string fields against max length. Set `validationState` on the `PendingEdit`. Apply invalid CSS class (distinct from dirty). Per FR-006, FR-010.
- [X] T026 [US1] Create `SaveControls.svelte` in `app/src/lib/components/ElementCardDeck/SaveControls.svelte` — Save button and progress indicator. Save disabled when `dirtyCount === 0` or `hasInvalid === true` (FR-014). On Save click: iterate all dirty edits, call `serializeConfigValue()` for each, invoke `writeConfigValue()` sequentially, update `PendingEditsStore` state (writing → clean or error) per field, show progress "Writing N of M..." (FR-013a). After all writes complete: invoke `sendUpdateComplete()` (FR-022). Update local `nodeTreeStore` value cache on success (FR-021).
- [X] T027 [US1] Add `SaveControls` toolbar to `SegmentView.svelte` in `app/src/lib/components/ElementCardDeck/SegmentView.svelte` — insert `<SaveControls>` between segment heading and children list. Pass current node ID and segment info. Visible when any pending edits exist for the current node.
- [X] T028 [US1] Add field-level dirty/clean CSS styling in `TreeLeafRow.svelte` — visual distinction for clean (default), unsaved/dirty (colored left border or background highlight), per FR-009. Auto-clear when field reverts to original value per FR-011.

**Checkpoint**: MVP complete — user can edit string/int fields, see dirty indicators, save to node, see indicators clear. Save disabled when nothing changed.

---

## Phase 4: User Story 2 — Edit Fields with Constrained Values (Priority: P1)

**Goal**: Integer fields with CDI map entries render as dropdown/select controls showing human-readable labels, writing the corresponding numeric value on save.

**Independent Test**: Find any CDI field with `<map>` entries, verify dropdown renders with labels, select a different option, confirm unsaved indicator appears, save and verify correct numeric value written.

### Tests for User Story 2

- [X] T029 [P] [US2] Add component test for dropdown select in `app/src/lib/components/ElementCardDeck/TreeLeafRow.test.ts` — render an int-type leaf WITH mapEntries, verify `<select>` renders with option labels, change selection, verify `PendingEditsStore` receives the numeric value (not label text).
- [X] T029b [P] [US2] Add component test for float input in `app/src/lib/components/ElementCardDeck/TreeLeafRow.test.ts` — render a float-type leaf, verify `<input type="number" step="any">` renders, enter non-numeric text and verify validation state is 'invalid', enter value outside min/max and verify constraint enforcement.

### Implementation for User Story 2

- [X] T030 [US2] Add `<select>` dropdown rendering in `TreeLeafRow.svelte` — when `leaf.elementType === 'int'` AND `leaf.constraints?.mapEntries` exists, render `<select>` with `<option>` elements: `value` = map entry numeric value, display text = map entry label. On change: create `PendingEdit` with numeric value. Apply dirty CSS class. Per FR-002.
- [X] T031 [US2] Add float field editing in `TreeLeafRow.svelte` — render `<input type="number" step="any">` for float-type leaves. Apply min/max constraints if present. Validate non-numeric input shows invalid state. Per FR-005.

**Checkpoint**: All P1 stories complete — string, int, int-with-map, and float fields are all editable inline.

---

## Phase 5: User Story 3 — Edit Event ID Fields (Priority: P2)

**Goal**: Event ID fields accept dotted-hex notation (e.g., `05.01.01.01.22.00.00.FF`), validate format, and write 8 raw bytes on save.

**Independent Test**: Edit an event ID field to a valid new value, save, verify 8-byte value written correctly. Enter invalid input, verify field marked invalid and Save blocked.

### Tests for User Story 3

- [X] T032 [P] [US3] Add component test for event ID input in `app/src/lib/components/ElementCardDeck/TreeLeafRow.test.ts` — render an eventId-type leaf, verify text input with dotted-hex format, enter valid value and verify store updated, enter invalid value and verify validation state is 'invalid'.
- [X] T033 [P] [US3] Add validation test for event ID format in `app/src/lib/utils/serialize.test.ts` — test valid dotted-hex patterns, reject wrong byte count, reject non-hex characters, reject missing dots.

### Implementation for User Story 3

- [X] T034 [US3] Add event ID editing in `TreeLeafRow.svelte` — render `<input type="text">` for eventId-type leaves. Display current value in dotted-hex format. On change: validate input against regex `^[0-9A-Fa-f]{2}(\.[0-9A-Fa-f]{2}){7}$`. If valid: create PendingEdit with parsed bytes. If invalid: set validationState to 'invalid' with message. Per FR-004.
- [X] T035 [US3] Add event ID parsing in `app/src/lib/utils/serialize.ts` — `parseEventIdHex(dottedHex: string): number[] | null` function that converts dotted-hex string to 8-byte array, returning null for invalid format. Used by both validation and serialization.

**Checkpoint**: Event ID editing works — format validation catches invalid input, valid event IDs are serialized and written correctly.

---

## Phase 6: User Story 4 — Global Unsaved Changes Awareness (Priority: P2)

**Goal**: Unsaved-change indicators appear on field level, Save-area count, sidebar node entries, and sidebar segment entries. Navigation guards prevent accidental data loss.

**Independent Test**: Modify 3+ fields across groups, verify field-level indicators, Save-area count, sidebar node badge, sidebar segment badge all appear. Navigate to a different segment and confirm warning dialog.

### Tests for User Story 4

- [X] T036 [P] [US4] Add component test for unsaved-changes badge in `app/src/lib/components/ConfigSidebar/NodeEntry.test.ts` — render NodeEntry with `hasPendingEdits: true`, verify badge/dot is visible. With `false`, verify not visible.
- [X] T037 [P] [US4] Add component test for segment unsaved badge in `app/src/lib/components/ConfigSidebar/SegmentEntry.test.ts` — render SegmentEntry with `hasPendingEdits: true`, verify indicator visible.

### Implementation for User Story 4

- [X] T038 [US4] Add pending changes count display in `SaveControls.svelte` — show "N unsaved change(s)" text near Save button, derived from `pendingEditsStore.dirtyCount`. Per FR-012.
- [X] T039 [US4] Add unsaved-changes badge to `NodeEntry.svelte` in `app/src/lib/components/ConfigSidebar/NodeEntry.svelte` — add `hasPendingEdits` prop (boolean). When true, render a colored dot/badge similar to existing `.not-read-dot` pattern. Derive value from `pendingEditsStore.getDirtyForNode(nodeId).length > 0`. Per FR-012a.
- [X] T040 [US4] Add unsaved-changes badge to `SegmentEntry.svelte` in `app/src/lib/components/ConfigSidebar/SegmentEntry.svelte` — add `hasPendingEdits` prop (boolean). When true, render edit dot after segment name. Derive value from `pendingEditsStore.getDirtyForSegment(nodeId, segmentOrigin).length > 0`. Per FR-012b.
- [X] T041 [US4] Wire sidebar badges to `PendingEditsStore` in `app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte` — pass computed `hasPendingEdits` props to `NodeEntry` and `SegmentEntry` components based on store state. Ensure real-time updates per FR-012c.
- [X] T042 [US4] Add navigation guard for node/segment switching in `app/src/routes/config/+page.svelte` — when user selects different node or segment and `pendingEditsStore.hasPendingEdits` is true, show confirmation dialog offering Save, Discard, or Cancel. Use SvelteKit `beforeNavigate` for page-level navigation. Add custom guard in sidebar click handlers for within-page navigation. Per FR-026.

**Checkpoint**: Full change awareness — users see pending edits everywhere (fields, save area, sidebar), and are warned before losing data.

---

## Phase 7: User Story 5 — Handle Write Failures Gracefully (Priority: P2)

**Goal**: Failed writes show per-field error indicators distinct from "unsaved," and retry saves only re-attempt failed fields.

**Independent Test**: Disconnect node mid-save or simulate datagram rejection, verify error indicators on correct fields, retry re-attempts only failed fields.

### Tests for User Story 5

- [X] T043 [P] [US5] Add store test for error state transitions in `app/src/lib/stores/pendingEdits.test.ts` — test: mark field as writing → mark as error with message → verify error state and message. Re-edit error field → verify transitions back to dirty. Save again → verify only error/dirty fields re-attempted.

### Implementation for User Story 5

- [X] T044 [US5] Add error state CSS styling in `TreeLeafRow.svelte` — add visual distinction for error state (e.g., red border or error icon), separate from dirty/unsaved styling (amber/blue). Show error tooltip with `writeError` message on hover. Per FR-009, FR-020.
- [X] T045 [US5] Update save logic in `SaveControls.svelte` for partial failure — when iterating writes, on failure: call `pendingEditsStore.markError(key, errorMessage)`, continue with remaining fields. After save loop: if any failures occurred, set `SaveProgress.state` to `'partial-failure'`. Show summary ("Saved N of M, K failed"). Per FR-020.
- [X] T046 [US5] Add retry-only-failed behavior in `SaveControls.svelte` — on subsequent Save click, filter pending edits to only those with `writeState === 'dirty' || writeState === 'error'`. Skip already-clean fields. Per spec US5 acceptance scenario 3.

**Checkpoint**: Robust error handling — users see exactly which fields failed, why, and can retry without re-writing successful fields.

---

## Phase 8: User Story 6 — Discard Changes (Priority: P3)

**Goal**: Discard button reverts all modified fields to last-read values, with confirmation prompt.

**Independent Test**: Edit several fields, click Discard, confirm all revert to original values with indicators cleared.

### Tests for User Story 6

- [X] T047 [P] [US6] Add component test for Discard button in `app/src/lib/components/ElementCardDeck/SaveControls.test.ts` — verify Discard disabled when no edits, enabled when edits exist, renders confirmation dialog on click, reverts all fields on confirm.

### Implementation for User Story 6

- [X] T048 [US6] Add Discard button to `SaveControls.svelte` — Discard button next to Save button. Disabled when `dirtyCount === 0` (FR-025). On click: show confirmation dialog "Discard N unsaved changes?" (FR-024). On confirm: call `pendingEditsStore.clearAll()`, which removes all pending edits and restores all field displays to original values (FR-023).
- [X] T049 [US6] Ensure TreeLeafRow reflects discarded state — when `PendingEdit` is removed from store, the input control should revert to showing the `originalValue` from the `leaf.value`. Verify reactive binding from store → input value.

**Checkpoint**: Full edit lifecycle — users can edit, validate, save, handle errors, and discard with confidence.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [X] T050 [P] Disable editing for offline nodes in `TreeLeafRow.svelte` — when node is offline/disconnected, set all inputs to `disabled` but preserve existing `PendingEditsStore` entries (do NOT clear them). When node reconnects, re-enable inputs so the user can retry saving. Per FR-007 and spec edge case "node disconnects while user has unsaved edits."
- [X] T051 [P] Add proptest for write encoding roundtrip in `lcc-rs/src/protocol/memory_config.rs` — property test: for any valid (space, address, payload), `build_write()` produces frames that can be parsed back to the original command bytes. Verify address encoding roundtrip.
- [X] T052 [P] Add `AddressSpace::from_u8()` constructor in `lcc-rs/src/protocol/memory_config.rs` if not already present — converts `u8` space byte (0xFB, 0xFD, etc.) to `AddressSpace` enum. Needed by `write_config_value` Tauri command to convert frontend space byte to typed enum.
- [X] T053 Update nodeTreeStore cached values on successful write in `app/src/lib/stores/nodeTree.svelte.ts` — after `write_config_value` succeeds, update the `LeafNode.value` in the tree store so the UI reflects the new value without re-reading from the node. Per FR-021.
- [ ] T054 Run quickstart.md verification scenarios — manually test all 8 verification scenarios from quickstart.md against a real or simulated LCC node.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3 (US1 — P1)**: Depends on Phase 2 completion — MVP target
- **Phase 4 (US2 — P1)**: Depends on Phase 2; can start in parallel with US1 (edits different part of TreeLeafRow conditional)
- **Phase 5 (US3 — P2)**: Depends on Phase 2; can start in parallel with US1/US2
- **Phase 6 (US4 — P2)**: Depends on Phase 3 (needs SaveControls and dirty tracking working)
- **Phase 7 (US5 — P2)**: Depends on Phase 3 (needs save workflow working)
- **Phase 8 (US6 — P3)**: Depends on Phase 3 (needs SaveControls existing)
- **Phase 9 (Polish)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: After Phase 2 — no dependencies on other stories. **MVP deliverable.**
- **US2 (P1)**: After Phase 2 — independent of US1 (adds `<select>` branch to TreeLeafRow's conditional)
- **US3 (P2)**: After Phase 2 — independent of US1/US2 (adds eventId branch)
- **US4 (P2)**: After Phase 3 — needs SaveControls (T026) and PendingEditsStore (T019) working
- **US5 (P2)**: After Phase 3 — needs save workflow (T026) to add error handling to
- **US6 (P3)**: After Phase 3 — needs SaveControls (T026) to add Discard to

### Within Phase 2 (Foundational)

```
T002 ─┐
T003 ─┤ (parallel: different functions in same file)
T004 ─┤ (parallel: tests for T002)
T005 ─┘ (parallel: tests for T003)
      ↓
T006 ── T007 (sequential: depends on build_write/build_update_complete)
      ↓
T008 ─┐ (parallel: tests)
T009 ─┘
      ↓
T010 (export types)
      ↓
T011 ── T012 ── T013 ── T014 (sequential: Tauri commands)
      ↓
T015 ─┐ 
T016 ─┤ (parallel: different files)
T017 ─┤
T018 ─┘
      ↓
T019 ── T020 (store, then tests)
```

### Parallel Opportunities Per User Story

**US1 (Phase 3)**:
```
T021 ─┐
T022 ─┤ (parallel: tests in same file but independent)
T023 ─┘
  ↓
T024 → T025 → T028 (sequential: same component, layered changes)
  ↓
T026 → T027 (SaveControls then wire into SegmentView)
```

**US2 (Phase 4)**: T029 → T030 → T031 (sequential: test first, then dropdown, then float)

**US3 (Phase 5)**: T032, T033 (parallel tests) → T034, T035 (sequential: UI then parser)

**US4 (Phase 6)**: T036, T037 (parallel tests) → T038 → T039, T040 (parallel: different sidebar components) → T041 → T042

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Edit a string field → save → verify value written → indicator clears
5. Demo: single-field edit-and-save works end-to-end

### Incremental Delivery

1. Phase 1 + 2 → Foundation ready (protocol writes, store, types)
2. Phase 3 (US1) → Test independently → **MVP!** (edit string/int, save, dirty tracking)
3. Phase 4 (US2) → Test independently → Dropdowns and floats work
4. Phase 5 (US3) → Test independently → Event IDs editable
5. Phase 6 (US4) → Test independently → Sidebar badges + navigation guards
6. Phase 7 (US5) → Test independently → Error handling robust
7. Phase 8 (US6) → Test independently → Discard with confirmation
8. Phase 9 → Polish, proptest, offline handling, quickstart validation

### Parallel Team Strategy

With multiple developers:

1. Team completes Phase 1 + 2 together (protocol + Tauri + types)
2. Once Foundational is done:
   - Developer A: US1 (Phase 3) — core edit/save
   - Developer B: US2 (Phase 4) — dropdowns/floats
   - Developer C: US3 (Phase 5) — event IDs
3. After US1 merges:
   - Developer A: US4 (Phase 6) — sidebar badges + nav guards
   - Developer B: US5 (Phase 7) — error handling
   - Developer C: US6 (Phase 8) — discard

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable after Phase 2
- Tests are written FIRST per Constitution Principle III (TDD mandatory)
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Reference: OpenLCB_Java implementation in workspace (`MemoryConfigurationService.java`, `ConfigRepresentation.java`, `CdiPanel.java`)
