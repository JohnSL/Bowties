---

description: "Task list for Configuration Tab — Sidebar and Element Card Deck"

---

# Tasks: Configuration Tab — Sidebar and Element Card Deck

**Input**: Design documents from `/specs/005-config-sidebar-view/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅, quickstart.md ✅

**Tests**: Tests are REQUIRED per Constitution §III (TDD — all new Svelte components, `get_card_elements` Rust command, and `resolveCardTitle()` utility must have tests).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no blocking dependencies)
- **[Story]**: Which user story this task belongs to ([US1], [US2])
- Exact file paths are included in all task descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create directory scaffolding for all new components before implementation begins

- [X] T001 Create component subdirectories `app/src/lib/components/ConfigSidebar/` and `app/src/lib/components/ElementCardDeck/`

---

## Phase 2: Foundational (Blocking Prerequisite)

**Purpose**: `configSidebarStore` is required by all sidebar and card deck components. No US1 or US2 work can begin until this store exists.

**⚠️ CRITICAL**: Both user stories read from and write to this store. US1 (sidebar) depends on `expandedNodeIds`, `selectedSegment`, `nodeLoadingStates`, and `nodeErrors`. US2 (card deck) depends on `cardDeck` and `expandedCardIds`.

- [X] T002 Implement `configSidebarStore` with full `ConfigSidebarState` interface, `toggleNodeExpanded`, `setNodeSegments`, `setNodeLoading`, `selectSegment`, `setCardDeck`, `updateCard`, `toggleCardExpanded`, and `reset` actions in `app/src/lib/stores/configSidebar.ts`

**Checkpoint**: `configSidebarStore` is importable — user story implementation can now begin

---

## Phase 3: User Story 1 — Browse Nodes and Segments via the Sidebar (Priority: P1) 🎯 MVP

**Goal**: A fixed-width left sidebar lists all discovered nodes, each expandable to show CDI segment names. Clicking a segment sets the selection state. The page replaces `MillerColumnsNav` entirely.

**Independent Test**: Verify nodes appear in the sidebar on tab open; clicking a node expands its segment list; clicking a segment highlights it as selected in the sidebar — without any element cards rendered. Also verify empty-state message when no nodes are discovered and that a node refresh (FR-018) clears all sidebar state.

### Tests for User Story 1 ⚠️ Write first — must FAIL before implementation

- [X] T003 [P] [US1] Write Vitest unit tests for `ConfigSidebar.svelte` covering: node list render, expand/collapse toggle (FR-002, FR-015), segment list render on expand, segment selection highlight, empty-state message (FR-002 edge case), offline indicator render (RQ-006) in `app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts`

### Implementation for User Story 1

- [X] T004 [P] [US1] Create `NodeEntry.svelte`: expandable node row showing `nodeName`, secondary `manufacturer`/`model` detail for duplicate names, offline indicator when `connectionStatus === 'NotResponding'` (RQ-006), right-click context menu with "View CDI XML" (`openCdiViewer(nodeId, false)`) and "Download CDI from Node" (`openCdiViewer(nodeId, true)`) actions reusing `CdiXmlViewer` (FR-016), CDI-not-loaded inline prompt when `hasCdi === false` in `app/src/lib/components/ConfigSidebar/NodeEntry.svelte`
- [X] T005 [P] [US1] Create `SegmentEntry.svelte`: selectable segment item displaying `segmentName` verbatim from CDI (FR-004), `isSelected` highlight style, fallback label `"Segment {space}"` when name absent in `app/src/lib/components/ConfigSidebar/SegmentEntry.svelte`
- [X] T006 [US1] Create `ConfigSidebar.svelte`: main sidebar container consuming `configSidebarStore` and `nodeInfoStore`; renders `NodeEntry` per discovered node and `SegmentEntry` per segment when expanded; calls `configSidebarStore.toggleNodeExpanded` and `getCdiStructure(nodeId)` on first expansion; calls `configSidebarStore.selectSegment` on segment click; FR-015 multi-node expansion preserved in `app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte`
- [X] T007 [US1] Update `app/src/routes/config/+page.svelte`: replace `MillerColumnsNav` import and usage with `ConfigSidebar` in a two-panel layout (fixed-width sidebar + scrollable main area per FR-001); subscribe to `nodeInfoStore` and call `configSidebarStore.reset()` reactively on node list change (FR-018, RQ-005); show `ElementCardDeck` placeholder (empty `<div>`) in main area pending US2
  - Also updated `app/src/routes/+page.svelte` (root page) to replace `MillerColumnsNav` with the same two-panel layout, since that is the app entry point.

**Checkpoint**: User Story 1 is fully functional — sidebar navigates independently without card deck content; `cargo test` and `vitest` pass

---

## Phase 4: User Story 2 — Inspect Element Configuration Values in the Card Deck (Priority: P2)

**Goal**: Selecting a segment in the sidebar populates the main area with one accordion card per top-level CDI group. Expanding a card calls `get_card_elements` and renders all leaf fields and sub-groups inline. Fields show cached values with [R] and [?] actions. Event slots show dotted-hex IDs or `"(free)"`.

**Independent Test**: Select a segment containing at least one named element (e.g., "Yard Button" assigned via `User Name` CDI string field); expand its card; verify the card header reads `"Yard Button (Line 3)"` per FR-007; verify field values match `millerColumnsStore.configValues` cache; verify [R] re-reads the value; verify event slots show `"(free)"` for unset IDs (FR-014).

### Tests for User Story 2 ⚠️ Write first — must FAIL before implementation

- [X] T008 [P] [US2] Write Vitest unit tests for `resolveCardTitle()` covering: replicated+named (`"Yard Button (Line 3)"`), replicated+unnamed (`"Line 3 (unnamed)"`), non-replicated+named (`"Yard Button (Port I/O)"`), non-replicated+unnamed (CDI group name only), null-byte and whitespace-only user name treated as unnamed (RQ-002, FR-007) in `app/src/lib/utils/cardTitle.test.ts`
- [X] T009 [P] [US2] Write Rust unit tests for `get_card_elements` command covering: valid `(nodeId, groupPath)` returns correct `CardElementTree` struct with `fields` and `subGroups` in CDI document order; `NodeNotFound` error; `InvalidPath` error; recursive sub-group nesting returns correct depth; replicated group instance path resolves correctly in `app/src-tauri/src/commands/cdi.rs` (test module inside the same file)
- [X] T010 [P] [US2] Write Vitest unit tests for `ElementCard.svelte` covering: collapsed-by-default render (FR-008), card title from `cardTitle` prop, expand reveals `CardElementTree` fields and sub-groups inline (FR-011), `isLoading` spinner, `loadError` error state, `"(no configurable fields)"` when `fields` and `subGroups` are empty in `app/src/lib/components/ElementCardDeck/ElementCard.test.ts`
- [X] T011 [P] [US2] Write Vitest unit tests for `ElementCardDeck.svelte` covering: one card per top-level CDI group (FR-006), all cards collapsed on segment load (FR-008), `isLoading` deck-level spinner, `error` deck-level error state, segment change replaces all cards in `app/src/lib/components/ElementCardDeck/ElementCardDeck.test.ts`

### Implementation for User Story 2

- [X] T012 [P] [US2] Implement `resolveCardTitle(groupInfo, nodeId, configValues)` pure function: searches `CardElementTree.fields` for a `StringElement` whose `name` (case-insensitive) matches `["user name", "name", "description"]`; validates against null-byte and whitespace-only; computes final title per FR-007 naming algorithm (RQ-002) in `app/src/lib/utils/cardTitle.ts`
- [X] T013 [US2] Implement `get_card_elements(node_id: String, group_path: Vec<String>)` Tauri command in `app/src-tauri/src/commands/cdi.rs`: look up node in `CDI_PARSE_CACHE`; navigate CDI tree to `group_path`; expand replications where path contains instance selectors; recursively collect all leaf `CardField`s (with absolute `memory_address` computed from `calculate_size()` and `expand_replications()`) and `CardSubGroup`s in CDI document order; return `CardElementTree`; error variants: `NodeNotFound`, `CdiNotRetrieved`, `InvalidPath`, `ParseError`; no `unwrap()` in production path (Constitution §I); unit tests added in T009 must now pass
- [X] T014 [US2] Register `get_card_elements` in the Tauri invoke handler in `app/src-tauri/src/lib.rs` (add to `.invoke_handler(tauri::generate_handler![...])` list alongside existing CDI commands)
- [X] T015 [P] [US2] Create `FieldRow.svelte`: read-only field row displaying `label`, `currentValue` from `millerColumnsStore.configValues` (null shown as `"—"`), [R] button calling `readConfigValue(nodeId, elementPath)` (FR-009, FR-010), [?] toggle revealing `description` text (FR-012, hidden by default) in `app/src/lib/components/ElementCardDeck/FieldRow.svelte`
- [X] T016 [P] [US2] Create `EventSlotRow.svelte`: specialised field row for `dataType === 'eventid'`; shows raw dotted-hex event ID or `"(free)"` when value matches all-zeros/unset default (FR-013, FR-014); [R] and [?] actions same as `FieldRow`; [?] hidden by default in `app/src/lib/components/ElementCardDeck/EventSlotRow.svelte`
- [X] T017 [US2] Create `ElementCard.svelte`: single accordion card; header shows `cardTitle` (computed via `resolveCardTitle`); collapsed by default (FR-008); on expand calls `get_card_elements(nodeId, groupPath)` via Tauri invoke and stores result via `configSidebarStore.updateCard`; renders `fields` as `FieldRow` or `EventSlotRow` (by `dataType`) and `subGroups` recursively inline with no collapse toggles (FR-011); shows `"(no configurable fields)"` when both `fields` and `subGroups` are empty; shows `isLoading` spinner and `loadError` error state in `app/src/lib/components/ElementCardDeck/ElementCard.svelte`
- [X] T018 [US2] Create `ElementCardDeck.svelte`: scrollable accordion container; one `ElementCard` per `cards` entry from `configSidebarStore.cardDeck`; deck-level `isLoading` and `error` states; all cards collapsed on mount (FR-008); calls `getCdiGroupList(nodeId, segmentId)` to populate top-level card list when `selectedSegment` changes (FR-005, FR-006); uses `resolveCardTitle` to pre-compute card titles (FR-007) in `app/src/lib/components/ElementCardDeck/ElementCardDeck.svelte`
- [X] T019 [US2] Update `app/src/routes/config/+page.svelte`: replace the empty main-area placeholder from T007 with `ElementCardDeck`; pass `selectedSegment` from `configSidebarStore`; ensure SC-002 is met by reading from `millerColumnsStore.configValues` cache (not from node) on first render (RQ-003)

**Checkpoint**: Both user stories are fully functional — navigate sidebar, select segment, expand cards, see cached field values, [R] refreshes single fields; all Vitest and Rust tests pass

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Validate end-to-end correctness, FR-017 non-regression, and quickstart scenarios

- [X] T020 [P] Validate FR-017 non-regression: run `cargo test` and `vitest run` with full test suite; confirm feature 004 config caching (`millerColumnsStore.configValues`, `readAllConfigValues`, `readConfigValue`) is unchanged and all existing tests still pass
- [X] T021 Run quickstart.md validation scenarios manually: 3-click navigation (SC-001), card deck loads within 500 ms on cached values (SC-002), named vs unnamed visible without expansion (SC-003), descriptions and event slots hidden by default (SC-004); confirm offline node indicator and empty-state message

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — **BLOCKS both user stories**
- **User Story 1 (Phase 3)**: Depends on Phase 2 completion; no dependency on US2
- **User Story 2 (Phase 4)**: Depends on Phase 2 completion; integrates with US1's `+page.svelte` but is independently testable
- **Polish (Phase 5)**: Depends on all desired user stories complete

### User Story Dependencies

- **US1 (P1)**: Depends only on `configSidebarStore` (T002); no dependency on US2 files
- **US2 (P2)**: Depends on `configSidebarStore` (T002); integrates into `+page.svelte` after US1 wires the layout (T007); `ElementCard` depends on `FieldRow` (T015) and `EventSlotRow` (T016); `ElementCardDeck` depends on `ElementCard` (T017)

### Within Each User Story

- Tests (T003, T008–T011) MUST be written and **FAIL** before their paired implementation tasks run (TDD — Constitution §III)
- Models/utilities before components: `cardTitle.ts` (T012) before `ElementCard.svelte` (T017)
- Leaf components before containers: `NodeEntry` + `SegmentEntry` (T004, T005) before `ConfigSidebar` (T006); `FieldRow` + `EventSlotRow` (T015, T016) before `ElementCard` (T017); `ElementCard` (T017) before `ElementCardDeck` (T018)
- Backend command (T013) + registration (T014) before `ElementCard` can invoke it

### Parallel Opportunities

#### User Story 1 — can run in parallel after T002

```
T003 ─────> T006 ─> T007
T004 ──┘
T005 ──┘
```

#### User Story 2 — can run in parallel after T007 (layout ready) and T002

```
T008 ──> T012 ──────────────────────────────────────> T017 ─> T018 ─> T019
T009 ──> T013 ─> T014 ──────────────────────────────┘
T010 ──> (ElementCard impl waits on T015 + T016) ──┘
T011 ──> (ElementCardDeck impl waits on T017) ──────────────────────────────┘
T015 ─────────────────────────────────────────────┘
T016 ─────────────────────────────────────────────┘
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001)
2. Complete Phase 2: Foundational (T002) — **CRITICAL**
3. Complete Phase 3: User Story 1 (T003–T007)
4. **STOP and VALIDATE**: Sidebar navigates nodes and segments independently
5. Demo/merge — Sidebar MVP shipped; Miller Columns removed

### Incremental Delivery

1. T001–T002 → Foundation ready
2. T003–T007 → US1 sidebar shipped (MVP ✅)
3. T008–T019 → US2 card deck shipped (full feature ✅)
4. T020–T021 → Polish and regression confirmed

### Parallel Execution (two developers after T002)

- **Developer A**: T003 → T004 → T005 → T006 → T007 (full US1 sidebar)
- **Developer B**: T008 → T009 → T010 → T011 → T012 → T013 → T014 → T015 → T016 (US2 backend + utilities + leaf components)
- Merge: T017 → T018 → T019 (container components + page wiring)

---

## Notes

- All `[P]` tasks target different files — no write conflicts
- `resolveCardTitle()` is a pure function; test coverage via T008 before implementation T012
- `get_card_elements` must not call `unwrap()` in any production code path (Constitution §I)
- `millerColumnsStore.configValues` is the single source of truth for cached values (RQ-003); `configSidebarStore` holds navigation state only
- Event IDs must use dotted-hex format throughout (Constitution §VII, FR-013)
- Commit after each task group; validate each story checkpoint independently before proceeding
