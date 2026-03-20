# Tasks: Editable Bowties

**Input**: Design documents from `/specs/009-editable-bowties/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-ipc.md, quickstart.md

**Tests**: Not explicitly requested — test tasks omitted. Constitution principle III (TDD) should be followed per-task as appropriate.

**Organization**: Tasks grouped by user story. P1 stories ordered by dependency (persistence → sync infrastructure → core creation → editing → role classification → config-first entry). P2 stories follow.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Exact file paths included in descriptions

## Path Conventions

- **Backend**: `app/src-tauri/src/` (Rust, Tauri 2)
- **Frontend**: `app/src/` (TypeScript, Svelte 5, SvelteKit 2.9)
- **Types**: `app/src/lib/types/` (shared TS types)
- **Stores**: `app/src/lib/stores/` (Svelte 5 runes stores)
- **Components**: `app/src/lib/components/` (Svelte 5 components)
- **API wrappers**: `app/src/lib/api/` (Tauri IPC invoke wrappers)

---

## Phase 1: Setup

**Purpose**: Add new dependencies, create module structure

- [x] T001 Add `tauri-plugin-dialog = "2"` to `app/src-tauri/Cargo.toml` dependencies and `@tauri-apps/plugin-dialog` to `app/package.json`
- [x] T002 Register `tauri_plugin_dialog::init()` plugin in Tauri builder and add `"dialog"` to `plugins` in `app/src-tauri/tauri.conf.json`
- [x] T003 [P] Create backend layout module with `app/src-tauri/src/layout/mod.rs` declaring `pub mod types;` and `pub mod io;` submodules

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types, API wrappers, and state model extensions that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 [P] Define Rust layout types (`LayoutFile`, `BowtieMetadata`, `RoleClassification`) with serde derives and validation in `app/src-tauri/src/layout/types.rs`
- [x] T005 [P] Define TypeScript editable bowtie types (`LayoutFile`, `BowtieMetadata`, `RoleClassification`, `BowtieState`, `BowtieEditKind`, `BowtieMetadataEdit`, `ElementSelection`, `EventIdResolution`, `WriteOperation`, `WriteStep`, `EditableBowtiePreview`, `PreviewBowtieCard`) in `app/src/lib/types/bowtie.ts`
- [x] T006 [P] Create frontend Tauri IPC API wrappers (`loadLayout`, `saveLayout`, `getRecentLayout`, `setRecentLayout`, `buildBowtieCatalog`) in `app/src/lib/api/bowties.ts`
- [x] T007 Extend `BowtieCard` Rust struct with `name: Option<String>`, `tags: Vec<String>`, and `state: BowtieState` fields; add `BowtieState` enum (`Active`, `Incomplete`, `Planning`) in `app/src-tauri/src/state.rs`

**Checkpoint**: Foundation ready — user story implementation can now begin

---

## Phase 3: User Story 7 — YAML Layout File Persistence (Priority: P1)

**Goal**: Enable saving and loading bowtie metadata (names, tags, role classifications) to a user-managed YAML file with native OS file dialogs

**Independent Test**: Create named bowties, save the layout file to a chosen location, close the app, reopen, open the saved file, and verify all names and tags are restored

### Implementation for User Story 7

- [x] T008 [US7] Implement layout file I/O: `load_file()` with schema validation and corrupted-YAML degraded mode (FR-026), `save_file()` with atomic write (temp → flush → rename), and `validate_schema()` in `app/src-tauri/src/layout/io.rs`
- [x] T009 [US7] Implement `load_layout` and `save_layout` Tauri commands that call layout I/O functions and emit `layout-loaded` / `layout-save-error` events in `app/src-tauri/src/commands/bowties.rs`
- [x] T010 [P] [US7] Implement `get_recent_layout` and `set_recent_layout` Tauri commands using `app_data_dir/recent-layout.json` persistence in `app/src-tauri/src/commands/bowties.rs`
- [x] T011 [US7] Register new commands (`load_layout`, `save_layout`, `get_recent_layout`, `set_recent_layout`) in Tauri builder `invoke_handler` in `app/src-tauri/src/lib.rs`
- [x] T012 [US7] Extend `build_bowtie_catalog` command to accept optional `LayoutFile` metadata parameter; merge role classifications into event role resolution, merge bowtie names/tags onto matching cards by event ID, create planning-state cards for unmatched layout entries in `app/src-tauri/src/commands/bowties.rs`
- [x] T013 [US7] Create layout store with file open/save via `@tauri-apps/plugin-dialog` native dialogs, layout path tracking, dirty state, and `loadLayout`/`saveLayout`/`saveLayoutAs` methods in `app/src/lib/stores/layout.svelte.ts`
- [x] T014 [US7] Add layout file toolbar controls (Open Layout, Save Layout, Save As) to the app toolbar in `app/src/routes/+page.svelte`
- [x] T015 [US7] Implement recent layout auto-reopen on app startup: check `getRecentLayout()`, offer to reopen, call `loadLayout()` on confirm in `app/src/lib/stores/layout.svelte.ts`

**Checkpoint**: Layout files can be saved, loaded, and merged with discovered node state. Metadata persists across sessions.

---

## Phase 4: User Story 2 — Bidirectional Sync and Unsaved Change Tracking (Priority: P1)

**Goal**: Config tab edits automatically update bowties and vice versa; all changes tracked as unsaved with visual indicators; single Save/Discard for everything

**Independent Test**: Edit an event ID in the Config tab and verify Bowties tab updates; modify a bowtie and verify Config tab reflects the change; make several changes, verify unsaved indicators appear, save, verify indicators clear

### Implementation for User Story 2

- [x] T016 [P] [US2] Create `BowtieMetadataStore` with `$state` runes tracking edits map, loaded layout file, layout path, and dirty flag; implement mutations (`createBowtie`, `deleteBowtie`, `renameBowtie`, `addTag`, `removeTag`, `classifyRole`, `reclassifyRole`, `clearAll`) and queries (`isDirty`, `getMetadata`, `getRoleClassification`, `getAllTags`) in `app/src/lib/stores/bowtieMetadata.svelte.ts`
- [x] T017 [P] [US2] Extend `pendingEditsStore` to support bowtie-originated event ID edits: add `source: 'config' | 'bowtie'` discriminator to pending edit entries so both tabs can create pending writes through the same store in `app/src/lib/stores/pendingEdits.svelte.ts`
- [x] T018 [US2] Implement `EditableBowtiePreview` derived computation using `$derived` rune that merges live `BowtieCatalog` + pending event ID edits from `pendingEditsStore` + metadata from `BowtieMetadataStore` to produce current user-visible bowtie state with per-card dirty flags in `app/src/lib/stores/bowties.svelte.ts`
- [x] T019 [US2] Implement unified save flow: write all pending config edits to nodes sequentially (existing write protocol), then save bowtie metadata to YAML via layout store; handle YAML save failure with retry/Save As prompt per FR-018d in `app/src/lib/stores/pendingEdits.svelte.ts`
- [x] T020 [US2] Implement unified discard flow: clear `pendingEditsStore` and `BowtieMetadataStore` together, reverting node event slots to pre-edit values and undoing metadata changes in `app/src/lib/stores/pendingEdits.svelte.ts`
- [x] T021 [P] [US2] Add unsaved-change indicators (dirty dot/badge) to `BowtieCard` component for cards with pending changes; highlight dirty fields (name, tags, elements) in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [x] T022 [P] [US2] Add global unsaved-changes indicator to app toolbar that reads dirty state from both `pendingEditsStore` and `BowtieMetadataStore` in `app/src/routes/+page.svelte`
- [x] T023 [US2] Implement reactive bowtie catalog recomputation: when config tab event ID values change in `pendingEditsStore`, trigger `EditableBowtiePreview` recalculation to update bowtie memberships in `app/src/lib/stores/bowties.svelte.ts`

**Checkpoint**: Changes from either tab flow bidirectionally. Unsaved indicators visible. Save writes to nodes + YAML together. Discard reverts everything.

---

## Phase 5: User Story 1 — Create a Connection from the Bowties Tab (Priority: P1) 🎯 MVP

**Goal**: Users can visually create producer-to-consumer connections via a dual element picker dialog without typing event IDs

**Independent Test**: Open the Bowties tab, click + New Connection, select a producer and consumer from discovered nodes, verify the bowtie card appears, verify event ID written to both slots, verify name persists in YAML after restart

### Implementation for User Story 1

- [x] T024 [P] [US1] Create `ElementPicker` component: browsable tree of nodes → segments → groups → event slots; filter by role (producer/consumer); search by name/path; gray out elements with no free slots (FR-012); show selection preview card with CDI path and slot count in `app/src/lib/components/Bowtie/ElementPicker.svelte`
- [x] T025 [US1] Create `NewConnectionDialog` component: dual panels (producer left, consumer right) each containing an `ElementPicker`; optional name input field; Create Connection button disabled until both sides selected (FR-034); selection preview cards in `app/src/lib/components/Bowtie/NewConnectionDialog.svelte`
- [x] T026 [US1] Implement event ID selection rules (FR-002) in `NewConnectionDialog`: (1) one side connected → use its event ID, (2) both connected to different bowties → prompt user to choose, (3) both unconnected → use producer's current event ID; generate pending edits accordingly in `app/src/lib/components/Bowtie/NewConnectionDialog.svelte`
- [x] T027 [US1] Add **+ New Connection** button to `BowtieCatalogPanel` that opens `NewConnectionDialog`; on dialog confirm, create pending edits in `pendingEditsStore` and metadata in `BowtieMetadataStore` in `app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte`
- [x] T028 [US1] Implement multi-node sequential write with rollback: track `WriteOperation` / `WriteStep` state; on step failure, attempt rollback of already-succeeded steps; surface detailed error if rollback fails (FR-029a) in `app/src/lib/stores/pendingEdits.svelte.ts`
- [x] T029 [US1] Add write operation feedback to `BowtieCard`: spinner during write, green confirmation on success, red error with retry option (FR-030) in `app/src/lib/components/Bowtie/BowtieCard.svelte`

**Checkpoint**: Users can create new connections visually. The core MVP workflow is functional end-to-end.

---

## Phase 6: User Story 5 — Add and Remove Elements from Existing Bowties (Priority: P1)

**Goal**: Users can iteratively build bowties by adding producers/consumers to existing connections and remove elements with slot restoration

**Independent Test**: Add a second consumer to an existing bowtie, verify both consumers share the same event ID; remove one consumer, verify its slot is cleared on the node

### Implementation for User Story 5

- [x] T030 [US5] Add **+ Add producer** and **+ Add consumer** action buttons to `BowtieCard` that open `ElementPicker` filtered by role; write the bowtie's existing event ID to the selected element's first free slot as a pending edit in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [x] T031 [US5] Implement remove-element flow on `BowtieCard`: remove button per element; create pending edit restoring the slot to its original value; handle state transitions (active → incomplete) in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [x] T032 [US5] Add deletion confirmation prompt when removing the last element from both sides: offer to keep as planning-state bowtie or delete entirely (FR-011) in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [x] T033 [US5] Add incomplete-state visual indicator to `BowtieCard` when one side has zero elements (FR-010) in `app/src/lib/components/Bowtie/BowtieCard.svelte`

**Checkpoint**: Bowties can be grown and trimmed. Element removal correctly restores node slot values. State transitions (active ↔ incomplete ↔ planning) work visually.

---

## Phase 7: User Story 8 — Classify Ambiguous Event Roles (Priority: P1)

**Goal**: Users can classify ambiguous event slots (from nodes without profiles) as producer or consumer; classifications persist in layout file

**Independent Test**: Discover a node with no profile, open NewConnectionDialog, verify ambiguous elements prompt for classification, classify them, verify classification persists after restart

### Implementation for User Story 8

- [x] T034 [P] [US8] Create `RoleClassifyPrompt` component: inline prompt asking user to classify an ambiguous element as Producer or Consumer; styled consistently with existing UI patterns in `app/src/lib/components/Bowtie/RoleClassifyPrompt.svelte`
- [x] T035 [US8] Extend `ElementPicker` to visually distinguish ambiguous elements with a "?" badge (FR-015b); trigger `RoleClassifyPrompt` on selection of an ambiguous element; place element on correct picker side after classification in `app/src/lib/components/Bowtie/ElementPicker.svelte`
- [x] T036 [US8] Wire role classification persistence: `BowtieMetadataStore.classifyRole()` saves classification keyed by `{nodeId}:{elementPath}` in layout YAML via `roleClassifications` section; loaded classifications applied during catalog merge in `app/src/lib/stores/bowtieMetadata.svelte.ts`
- [x] T037 [US8] Add re-classify role support on `BowtieCard`: allow clicking a role badge to change classification; update element placement from producer to consumer side (or vice versa) on all affected bowties (FR-015d) in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [x] T038 [US8] Show ambiguous elements in a dedicated "ambiguous" section on `BowtieCard` (between producers and consumers) until the user classifies their role (FR-015e) in `app/src/lib/components/Bowtie/BowtieCard.svelte`

**Checkpoint**: Nodes without profiles can participate in bowties. Classifications persist. Re-classification moves elements between sides.

---

## Phase 8: User Story 3 — Create a Connection Starting from a Config Element (Priority: P1)

**Goal**: Users can initiate connection creation from the Configuration tab via a context action, with one side of the dialog pre-filled

**Independent Test**: Right-click a producer event slot in the Config tab, select "Create Connection from Here," pick a consumer, verify the bowtie appears in both tabs

### Implementation for User Story 3

- [x] T039 [US3] Add "Create Connection from Here" context action (button or right-click menu item) to `TreeLeafRow` for event slot elements with at least one free slot in `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`
- [x] T040 [US3] Extend `NewConnectionDialog` to accept an optional pre-filled `ElementSelection` for either the producer or consumer side; auto-detect role from element's event role annotation and place on correct side in `app/src/lib/components/Bowtie/NewConnectionDialog.svelte`
- [x] T041 [US3] Wire context action to dialog: `TreeLeafRow` dispatches event with element selection data; parent page catches event and opens `NewConnectionDialog` with pre-fill in `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`

**Checkpoint**: Config-first entry point works. Same dialog, same result — just a different starting point.

---

## Phase 9: User Story 6 — Name and Rename Bowties (Priority: P2)

**Goal**: Users can name bowties at creation and rename inline; names appear in card headers, config cross-references, and are searchable

**Independent Test**: Create a named bowtie, rename it inline, verify name updates in the card, YAML file, and "Used in:" references in the Config tab

### Implementation for User Story 6

- [ ] T042 [US6] Add inline name editing to `BowtieCard` header: pencil icon triggers contenteditable or input field; on blur/Enter, commit rename via `BowtieMetadataStore.renameBowtie()` in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [ ] T043 [US6] Add "Used in: [connection name]" cross-reference display on event slot fields in the Configuration tab; link text navigates to the Bowties tab filtered to that bowtie in `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`
- [ ] T044 [US6] Add filter bar to `BowtieCatalogPanel` with text search matching against bowtie names; filter `EditableBowtiePreview` results reactively in `app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte`

**Checkpoint**: Bowties have meaningful names. Users can find connections by name. Config tab shows which bowtie uses each event slot.

---

## Phase 10: User Story 4 — Intent-First Bowtie Creation (Priority: P2)

**Goal**: Users can create empty named bowties for layout planning; elements are added later; event ID adopted from first attached element

**Independent Test**: Create an empty named bowtie, verify it persists, add a producer and consumer, verify event ID is assigned and written

### Implementation for User Story 4

- [ ] T045 [US4] Extend `NewConnectionDialog` to allow creation with only a name and no element selections: Create button enabled when name is provided even without element picks in `app/src/lib/components/Bowtie/NewConnectionDialog.svelte`
- [ ] T046 [US4] Add planning-state visual treatment to `BowtieCard`: show name with "No elements yet" placeholder, prominent + Add buttons, and distinct styling (dashed border or muted colors) in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [ ] T047 [US4] Implement event ID adoption flow in bowtie store: when first element is added to a planning bowtie, adopt that element's current event ID (no node write); when second element added on opposite side, write adopted event ID to the new slot in `app/src/lib/stores/bowties.svelte.ts`
- [ ] T048 [US4] Ensure planning-state bowties persist in YAML layout file and survive app restart: created with a placeholder key, re-keyed when event ID is adopted in `app/src/lib/stores/layout.svelte.ts`

**Checkpoint**: Design-first workflow works. Empty bowties persist and can be populated later.

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: Tag management, safety guards, and end-to-end validation

- [ ] T049 [P] Add tag management UI to `BowtieCard`: add/remove tag chips, auto-suggest from `BowtieMetadataStore.getAllTags()` in `app/src/lib/components/Bowtie/BowtieCard.svelte`
- [ ] T050 [P] Add prompt-to-save guard when closing app, opening different layout, or creating new layout with unsaved changes (FR-024) in `app/src/routes/+page.svelte`
- [ ] T051 Verify all write operations block on offline nodes with descriptive error messages per FR-028/FR-029 in `app/src/lib/stores/pendingEdits.svelte.ts`
- [ ] T052 [P] Validate YAML file human-readability: ensure `serde_yaml_ng` output uses readable formatting, sorted keys, and properly escaped special characters (FR-025) in `app/src-tauri/src/layout/io.rs`
- [ ] T053 [P] Run quickstart.md validation: execute all 5 workflows end-to-end against discovered nodes
- [ ] T054 [P] Performance check: verify bowtie catalog rebuild completes in <1s for layouts with 50–100 bowties per plan.md performance goals

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup — **BLOCKS all user stories**
- **US7 (Phase 3)**: Depends on Foundational — provides persistence for all other stories
- **US2 (Phase 4)**: Depends on US7 — store infrastructure needs layout integration
- **US1 (Phase 5)**: Depends on US2 — needs pendingEditsStore + BowtieMetadataStore infrastructure
- **US5 (Phase 6)**: Depends on US1 — reuses ElementPicker, extends BowtieCard
- **US8 (Phase 7)**: Depends on US1 — enhances ElementPicker and BowtieCard created in US1
- **US3 (Phase 8)**: Depends on US1 — reuses NewConnectionDialog with pre-fill
- **US6 (Phase 9)**: Depends on US2 — needs metadata store; can parallel with US5/US8
- **US4 (Phase 10)**: Depends on US1 — extends NewConnectionDialog and BowtieCard
- **Polish (Phase 11)**: Depends on all user stories being complete

### User Story Dependencies

```
Setup (1) → Foundational (2) → US7 (3) → US2 (4) → US1 (5) ─┬─► US5 (6)
                                                               ├─► US8 (7)
                                                               ├─► US3 (8)
                                                               └─► US4 (10)
                                              US2 (4) ─────────► US6 (9)
All stories ──────────────────────────────────────────────────► Polish (11)
```

### Within Each User Story

- Backend types/commands before frontend stores
- Stores before components
- Core component before extensions
- File I/O before UI controls

### Parallel Opportunities

- **Phase 2**: T004, T005, T006 can all run in parallel (different files, no dependencies)
- **Phase 3**: T010 can run in parallel with T009 (different command concerns)
- **Phase 4**: T016 and T017 can run in parallel (different store files); T021 and T022 can run in parallel (different component files)
- **Phase 5**: T024 can run in parallel with other Phase 5 prep (ElementPicker is a standalone component)
- **Phase 7**: T034 can run in parallel with other Phase 7 work (standalone component)
- **After Phase 5**: US5, US8, US3 can proceed in parallel (they extend different aspects of US1)
- **US6 (Phase 9)**: Can run in parallel with US5/US8/US3 (only needs US2 infrastructure)

---

## Parallel Example: Phase 2 (Foundational)

```
# Launch all independent type/API tasks together:
T004: "Define Rust layout types in app/src-tauri/src/layout/types.rs"
T005: "Define TypeScript bowtie types in app/src/lib/types/bowtie.ts"
T006: "Create frontend API wrappers in app/src/lib/api/bowties.ts"

# Then sequentially:
T007: "Extend BowtieCard in app/src-tauri/src/state.rs" (depends on T004 for BowtieState type)
```

## Parallel Example: After Phase 5 (US1 complete)

```
# These can all proceed in parallel (different files, different concerns):
US5 (Phase 6): Add/remove elements on BowtieCard
US8 (Phase 7): RoleClassifyPrompt + ElementPicker ambiguous handling
US3 (Phase 8): TreeLeafRow context action
US6 (Phase 9): Inline name editing + filter bar
```

---

## Implementation Strategy

### MVP First (Through Phase 5: User Story 1)

1. Complete Phase 1: Setup (dependencies, module structure)
2. Complete Phase 2: Foundational (types, API wrappers, state model)
3. Complete Phase 3: US7 — Layout file persistence
4. Complete Phase 4: US2 — Bidirectional sync and unsaved tracking
5. Complete Phase 5: US1 — Create connection from Bowties tab
6. **STOP and VALIDATE**: Test the core creation workflow end-to-end
7. Deploy/demo if ready — users can create, name, and save connections

### Incremental Delivery

1. **Foundation** (Phases 1–2): Types and infrastructure ready
2. **Persistence** (Phase 3, US7): Layout files work → can save/load metadata
3. **Sync Infrastructure** (Phase 4, US2): Bidirectional updates + unsaved tracking
4. **Core MVP** (Phase 5, US1): Visual connection creation → **demo-ready**
5. **Editing** (Phase 6, US5): Add/remove elements → iterative bowtie building
6. **Role Classification** (Phase 7, US8): Ambiguous nodes supported → works with all hardware
7. **Config Entry** (Phase 8, US3): Config-first creation → alternative workflow
8. **Naming** (Phase 9, US6): Inline rename + search → better organization
9. **Planning Mode** (Phase 10, US4): Intent-first creation → design-first workflow
10. **Polish** (Phase 11): Tags, guards, validation → production-ready

### Key Files by Story

| Story | Backend Files | Frontend Files |
|-------|--------------|----------------|
| US7 | `layout/types.rs`, `layout/io.rs`, `commands/bowties.rs` | `stores/layout.svelte.ts`, `+page.svelte` |
| US2 | — | `stores/bowtieMetadata.svelte.ts`, `stores/pendingEdits.svelte.ts`, `stores/bowties.svelte.ts`, `BowtieCard.svelte` |
| US1 | — | `ElementPicker.svelte`, `NewConnectionDialog.svelte`, `BowtieCatalogPanel.svelte` |
| US5 | — | `BowtieCard.svelte` |
| US8 | — | `RoleClassifyPrompt.svelte`, `ElementPicker.svelte`, `bowtieMetadata.svelte.ts` |
| US3 | — | `TreeLeafRow.svelte`, `NewConnectionDialog.svelte` |
| US6 | — | `BowtieCard.svelte`, `TreeLeafRow.svelte`, `BowtieCatalogPanel.svelte` |
| US4 | — | `NewConnectionDialog.svelte`, `BowtieCard.svelte`, `bowties.svelte.ts`, `layout.svelte.ts` |

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story is independently testable at its checkpoint
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- All write operations use the existing `write_config_value` command — no new write protocol needed
- The `pendingEditsStore` is the single source of truth for unsaved event ID changes from both tabs
- `BowtieMetadataStore` handles YAML-only changes (names, tags, classifications) separately
- Save action coordinates both stores: node writes first, then YAML file save
