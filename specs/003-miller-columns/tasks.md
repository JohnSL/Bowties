# Tasks: Miller Columns Configuration Navigator

**Feature**: 003-miller-columns  
**Input**: Design documents from `/specs/003-miller-columns/`  
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.json

**Tests**: Per Constitution Principle III (Test-Driven Development - MANDATORY), comprehensive tests are required, especially for Rust CDI parsing logic. Test tasks are included in Phase 2b.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Frontend**: `app/src/` (SvelteKit)
- **Backend**: `app/src-tauri/src/` (Rust/Tauri)
- **Library**: `lcc-rs/src/` (Rust LCC protocol library)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and CDI parsing foundation

- [X] T001 Create Miller Columns feature branch `003-miller-columns`
- [X] T002 [P] Add roxmltree dependency to lcc-rs/Cargo.toml (version 0.20 for CDI XML parsing)
- [X] T003 [P] Create directory structure app/src/lib/components/MillerColumns/
- [X] T004 [P] Create directory structure app/src-tauri/src/commands/
- [X] T005 [P] Create directory structure app/src-tauri/src/cdi/
- [X] T006 [P] Create directory structure lcc-rs/src/cdi/

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core CDI parsing and navigation infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### CDI Type Definitions (Rust Backend)

- [X] T007 [P] Define Cdi struct in lcc-rs/src/cdi/mod.rs per data-model.md (identification, acdi, segments)
- [X] T008 [P] Define Segment struct in lcc-rs/src/cdi/mod.rs (name, description, space, origin, elements)
- [X] T009 [P] Define DataElement enum in lcc-rs/src/cdi/mod.rs (Group, Int, String, EventId, Float, Action, Blob)
- [X] T010 [P] Define Group struct in lcc-rs/src/cdi/mod.rs (name, description, offset, replication, repname, elements, hints)
- [X] T011 [P] Define IntElement struct in lcc-rs/src/cdi/mod.rs (name, description, size, offset, min, max, default, map)
- [X] T012 [P] Define EventIdElement struct in lcc-rs/src/cdi/mod.rs (name, description, offset)
- [X] T013 [P] Define StringElement, FloatElement, ActionElement, BlobElement structs in lcc-rs/src/cdi/mod.rs

### CDI XML Parsing (Rust Backend)

- [X] T014 Create parse_cdi function in lcc-rs/src/cdi/parser.rs using roxmltree
- [X] T015 Implement parse_segment function in lcc-rs/src/cdi/parser.rs (extract space, origin, elements)
- [X] T016 Implement parse_data_element function in lcc-rs/src/cdi/parser.rs (recursive, handle all DataElement types)
- [X] T017 Implement parse_group function in lcc-rs/src/cdi/parser.rs (handle replication, nested groups)
- [X] T018 [P] Implement parse_int_element function in lcc-rs/src/cdi/parser.rs (extract size, min, max, default, map)
- [X] T019 [P] Implement parse_eventid_element function in lcc-rs/src/cdi/parser.rs (always 8 bytes)
- [X] T020 [P] Implement parse_string_element, parse_float_element, parse_action_element, parse_blob_element in lcc-rs/src/cdi/parser.rs
- [X] T021 Implement Group::should_render method in lcc-rs/src/cdi/mod.rs (Footnote 4 compliance - filter empty groups)

### CDI Navigation Helpers (Rust Backend)

- [X] T022 Implement Group::expand_replications method in lcc-rs/src/cdi/hierarchy.rs (generate N instances with computed names)
- [X] T023 Implement Group::compute_repname method in lcc-rs/src/cdi/hierarchy.rs (handle numbering per spec)
- [X] T024 Implement calculate_max_depth function in lcc-rs/src/cdi/hierarchy.rs (traverse hierarchy to find deepest level)
- [X] T025 Implement navigate_to_path function in lcc-rs/src/cdi/hierarchy.rs (follow path array to find element)

### Tauri Command Scaffolding (Rust Backend)

- [X] T026 Create app/src-tauri/src/commands/cdi.rs with get_discovered_nodes command stub
- [X] T027 Add get_cdi_structure command to app/src-tauri/src/commands/cdi.rs per contracts/tauri-commands.json
- [X] T028 Add get_column_items command to app/src-tauri/src/commands/cdi.rs per contracts/tauri-commands.json
- [X] T029 Add get_element_details command to app/src-tauri/src/commands/cdi.rs per contracts/tauri-commands.json
- [X] T030 Add expand_replicated_group command to app/src-tauri/src/commands/cdi.rs per contracts/tauri-commands.json
- [X] T031 Register all CDI commands in app/src-tauri/src/main.rs invoke_handler

### Frontend State Management

- [X] T032 Create MillerColumnsState interface in app/src/lib/stores/millerColumns.ts (selectedNode, columns, breadcrumb)
- [X] T033 Create millerColumnsStore writable store in app/src/lib/stores/millerColumns.ts
- [X] T034 Implement selectNode action in app/src/lib/stores/millerColumns.ts (reset columns, trigger segment load)
- [X] T035 Implement addColumn action in app/src/lib/stores/millerColumns.ts (dynamic column injection)
- [X] T036 Implement removeColumnsAfter action in app/src/lib/stores/millerColumns.ts (navigation back support)
- [X] T037 Implement updateBreadcrumb action in app/src/lib/stores/millerColumns.ts (track selection path)

### TypeScript API Wrappers

- [X] T038 [P] Create TypeScript types in app/src/lib/api/cdi.ts matching contracts/tauri-commands.json definitions
- [X] T039 [P] Implement getDiscoveredNodes wrapper in app/src/lib/api/cdi.ts (calls Tauri invoke)
- [X] T040 [P] Implement getCdiStructure wrapper in app/src/lib/api/cdi.ts
- [X] T041 [P] Implement getColumnItems wrapper in app/src/lib/api/cdi.ts
- [X] T042 [P] Implement getElementDetails wrapper in app/src/lib/api/cdi.ts
- [X] T043 [P] Implement expandReplicatedGroup wrapper in app/src/lib/api/cdi.ts

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 2b: Test Infrastructure (TDD Compliance)

**Purpose**: Implement comprehensive tests per Constitution Principle III (Test-Driven Development - MANDATORY)

**⚠️ CRITICAL**: These tests ensure protocol correctness and prevent regressions. Property-based tests validate CDI parsing invariants.

### CDI Parsing Unit Tests (Rust)

- [X] T043a [P] Create tests module in lcc-rs/src/cdi/mod.rs for type validation
- [X] T043b [P] Add unit tests for Group::should_render in lcc-rs/src/cdi/mod.rs (Footnote 4 compliance - test empty groups filtered)
- [X] T043c [P] Add unit tests for Group::expand_replications in lcc-rs/src/cdi/hierarchy.rs (test replication=1, replication=16, replication=100)
- [X] T043d [P] Add unit tests for Group::compute_repname in lcc-rs/src/cdi/hierarchy.rs (test numbering with/without repname template)
- [X] T043e Create property-based tests in lcc-rs/src/cdi/parser.rs using proptest (parse valid CDI → roundtrip validation)
- [X] T043f [P] Add unit tests for parse_segment in lcc-rs/src/cdi/parser.rs (test space, origin, elements extraction)
- [X] T043g [P] Add unit tests for parse_group in lcc-rs/src/cdi/parser.rs (test nested groups, replication parsing)
- [X] T043h [P] Add unit tests for parse_int_element in lcc-rs/src/cdi/parser.rs (test size validation, min/max, map values)
- [X] T043i [P] Add unit tests for parse_eventid_element in lcc-rs/src/cdi/parser.rs (test 8-byte validation)
- [X] T043j Add integration tests in lcc-rs/tests/cdi_parsing.rs with real CDI XML samples from Tower-LCC, I/O nodes
- [X] T043k Add malformed XML tests in lcc-rs/tests/cdi_parsing.rs (missing tags, invalid attributes, graceful degradation)

### Tauri Command Integration Tests

- [X] T043l [P] Create integration test module in app/src-tauri/src/commands/cdi.rs for get_discovered_nodes
- [X] T043m [P] Add integration tests for get_cdi_structure command (mock CDI cache, verify segment extraction)
- [X] T043n [P] Add integration tests for get_column_items command (test path navigation, replication expansion)
- [X] T043o [P] Add integration tests for get_element_details command (test metadata extraction, breadcrumb generation)

### Frontend Component Tests (Optional but Recommended)

- [X] T043p [P] Create Vitest test for NodesColumn.svelte in app/src/lib/components/MillerColumns/NodesColumn.test.ts
- [X] T043q [P] Create Vitest test for NavigationColumn.svelte selection handling
- [X] T043r [P] Create Vitest test for DetailsPanel.svelte metadata rendering
- [X] T043s Create end-to-end test for full navigation workflow (node → segment → group → element)

**Checkpoint**: Test infrastructure complete - tests will drive implementation quality

---

## Phase 3: User Story 1 - Navigate to Event ID Elements (Priority: P1) 🎯 MVP

**Goal**: Enable users to navigate from discovered nodes through CDI hierarchy (segments → groups → elements) to identify Event ID elements for producer/consumer linking.

**Independent Test**: Select any discovered node with CDI data, navigate through segments → groups to elements, select an Event ID element, and verify its metadata (name, description, type="Event ID (8 bytes)") appears in Details Panel.

### Implementation for User Story 1

#### Nodes Column (Leftmost - Fixed)

- [X] T044 [US1] Implement get_discovered_nodes command in app/src-tauri/src/commands/cdi.rs (query node cache, return node list with CDI status)
- [X] T045 [US1] Create NodesColumn.svelte in app/src/lib/components/MillerColumns/ (display nodes list, handle selection)
- [X] T046 [US1] Add node display logic to NodesColumn.svelte (show user name or SNIP manufacturer + model, fallback to Node ID)
- [X] T047 [US1] Add CDI unavailable indicator in NodesColumn.svelte (grayed out with ⚠️ icon if no CDI)
- [X] T048 [US1] Implement node selection handler in NodesColumn.svelte (call selectNode action, trigger segments load)

#### Segments Column (Fixed - Second Position)

- [X] T049 [US1] Implement get_cdi_structure command logic in app/src-tauri/src/commands/cdi.rs (parse CDI XML, return segments)
- [X] T050 [US1] Add segment extraction logic in get_cdi_structure (call lcc-rs parse_cdi, iterate segments)
- [X] T051 [US1] Create SegmentsColumn component in app/src/lib/components/MillerColumns/NavigationColumn.svelte (reusable for segments, groups, elements)
- [X] T052 [US1] Add segments rendering in NavigationColumn.svelte (display segment names, handle selection)
- [X] T053 [US1] Implement segment selection handler in NavigationColumn.svelte (analyze contents, determine next column type)

#### Groups Columns (Dynamic - Variable Count)

- [X] T054 [US1] Implement get_column_items command logic in app/src-tauri/src/commands/cdi.rs (navigate to parent path, return child items)
- [X] T055 [US1] Add group navigation logic in get_column_items (handle nested groups, filter empty groups per Footnote 4)
- [X] T056 [US1] Implement group expansion in get_column_items (call Group::expand_replications for replicated groups)
- [X] T057 [US1] Add groups rendering in NavigationColumn.svelte (display group names, show "Group" columnType)
- [X] T058 [US1] Implement group selection handler in NavigationColumn.svelte (check if contains nested groups, add column or show elements)
- [X] T059 [US1] Add dynamic column injection in MillerColumnsNav.svelte (call addColumn action when navigating deeper)

#### Elements Column (Dynamic - Appears When Needed)

- [X] T060 [US1] Add elements extraction logic in get_column_items (filter DataElement enum for primitives)
- [X] T061 [US1] Add elements rendering in NavigationColumn.svelte (display element names, show "Element" columnType)
- [X] T062 [US1] Add element type indicators in NavigationColumn.svelte (🎯 for EventId, 123 for Int, abc for String)
- [X] T063 [US1] Implement element selection handler in NavigationColumn.svelte (trigger element details load)

#### Details Panel (Right Panel - Fixed)

- [X] T064 [US1] Implement get_element_details command logic in app/src-tauri/src/commands/cdi.rs (navigate to element path, extract metadata)
- [X] T065 [US1] Add metadata extraction in get_element_details (name, description, dataType, constraints, defaultValue, memoryAddress)
- [X] T066 [US1] Create DetailsPanel.svelte in app/src/lib/components/MillerColumns/ (right-side panel layout)
- [X] T067 [US1] Add element name and description display in DetailsPanel.svelte (handle null descriptions)
- [X] T068 [US1] Add data type display in DetailsPanel.svelte (format as "Event ID (8 bytes)", "Integer (2 bytes)", etc.)
- [X] T069 [US1] Add constraints display in DetailsPanel.svelte (show min/max for Integer, map values if available)
- [X] T070 [US1] Add default value display in DetailsPanel.svelte (show "Default: <value>" if specified in CDI)
- [X] T071 [US1] Add full path breadcrumb in DetailsPanel.svelte (display element path with › separators)

#### Main Container Component

- [X] T072 [US1] Create MillerColumnsNav.svelte in app/src/lib/components/MillerColumns/ (main container with flexbox layout)
- [X] T073 [US1] Add flexbox column layout in MillerColumnsNav.svelte (overflow-x auto for horizontal scrolling)
- [X] T074 [US1] Add NodesColumn component to MillerColumnsNav.svelte (leftmost, always visible)
- [X] T075 [US1] Add dynamic column rendering in MillerColumnsNav.svelte (iterate columns from store, render NavigationColumn)
- [X] T076 [US1] Add DetailsPanel component to MillerColumnsNav.svelte (rightmost, always visible)
- [X] T077 [US1] Add selection highlighting in NavigationColumn.svelte (show active item with background color)

#### Route Integration

- [X] T078 [US1] Create +page.svelte in app/src/routes/config/ (Miller Columns view page)
- [X] T079 [US1] Import and render MillerColumnsNav component in app/src/routes/config/+page.svelte
- [X] T080 [US1] Add page title and navigation breadcrumb in app/src/routes/config/+page.svelte

**Checkpoint**: At this point, User Story 1 should be fully functional - users can navigate from nodes through segments, groups, and elements to view Event ID metadata in the Details Panel.

---

## Phase 4: User Story 2 - Navigate Replicated Configuration Groups (Priority: P1)

**Goal**: Enable users to navigate through replicated groups (e.g., "Line 1" through "Line 16") with clear instance numbering so each instance can be configured individually.

**Independent Test**: Select a node with replicated groups (e.g., I/O node with 16 input lines), verify that each instance is listed separately with numbering ("Line 1", "Line 2", ..., "Line 16"), and confirm selecting an instance shows its specific configuration elements.

### Implementation for User Story 2

- [X] T081 [US2] Add replication count display in NavigationColumn.svelte (show "16 instances" next to replicated group)
- [X] T082 [US2] Implement instance numbering in Group::compute_repname (handle repname template from CDI)
- [X] T083 [US2] Add instance index to breadcrumb path in app/src/lib/stores/millerColumns.ts (show "Line #7" not "Line")
- [X] T084 [US2] Add instance-specific address calculation in get_column_items (offset = base + index * group_size)
- [X] T085 [US2] Add replicated group visual indicator in NavigationColumn.svelte (icon or badge for groups with replication > 1)

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently - users can navigate both flat and replicated CDI structures.

---

## Phase 5: User Story 3 - Preview Element Details (Priority: P2)

**Goal**: Show comprehensive CDI metadata in Details Panel with full breadcrumb path for quick confirmation without disrupting navigation.

**Independent Test**: Select any element in Elements column and verify Details Panel shows complete metadata (name, description, data type, constraints, default value, full breadcrumb path).

**Note**: This story is largely implemented in Phase 3 (US1), this phase adds enhancements.

### Implementation for User Story 4

- [X] T091 [P] [US4] Add full element name on hover tooltip in NavigationColumn.svelte (for truncated names)
- [X] T092 [P] [US4] Add constraint formatting in DetailsPanel.svelte (e.g., "Range: 0-255", "Map: 0=Inactive, 1=Active")
- [X] T093 [US4] Add memory address display in DetailsPanel.svelte (show absolute address in config space)
- [X] T094 [US4] Add read-only note in DetailsPanel.svelte ("Structure metadata only - values not retrieved")

**Checkpoint**: Details Panel now shows comprehensive metadata with all constraints and addressing information.

---

## Phase 6: User Story 4 - Navigate Back Through Hierarchy (Priority: P3)

**Goal**: Enable users to click on previous column selections or breadcrumb segments to navigate back up the hierarchy without restarting from the beginning.

**Independent Test**: Navigate deep into hierarchy (Node → Segment → Group → Element), then click on a segment in Segments column to jump back, verify subsequent columns clear and update correctly.

### Implementation for User Story 4

- [X] T090 [US4] Create Breadcrumb.svelte component in app/src/lib/components/MillerColumns/ (display path with separators)
- [X] T091 [US4] Implement breadcrumb segment click handler in Breadcrumb.svelte (call removeColumnsAfter action)
- [X] T092 [US4] Add breadcrumb rendering in MillerColumnsNav.svelte (position above columns or below)
- [X] T093 [US4] Implement column click-to-navigate in NavigationColumn.svelte (re-select item in earlier column)
- [X] T094 [US4] Add column removal animation in MillerColumnsNav.svelte (smooth transition <150ms)
- [X] T095 [US4] Add breadcrumb path truncation in Breadcrumb.svelte (show first + last 2-3 segments if > 6 levels)
- [X] T096 [US4] Add full path tooltip in Breadcrumb.svelte (show complete path on hover)

**Checkpoint**: All user stories should now be independently functional with full bidirectional navigation support.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories and overall quality

### Error Handling & Edge Cases

- [X] T097 [P] Add "No CDI data available" message in DetailsPanel.svelte (when node has no CDI)
- [X] T098 [P] Add "Parsing issue" indicator in NavigationColumn.svelte (when CDI XML has errors)
- [X] T099 [P] Add loading indicators in NavigationColumn.svelte (show spinner while column populates)
- [X] T100 [P] Add error boundary in MillerColumnsNav.svelte (catch and display parsing errors)
- [X] T101 Add debouncing for rapid navigation clicks in app/src/lib/stores/millerColumns.ts (50ms debounce)
- [X] T102 Add request cancellation in app/src/lib/api/cdi.ts (abort previous pending requests)

### Performance Optimization

- [X] T103 [P] Add column population performance tracking in app/src-tauri/src/commands/cdi.rs (log if > 500ms)
- [X] T104 Add CDI parsing caching in app/src-tauri/src/commands/cdi.rs (cache parsed Cdi structs by node ID)
- [X] T105 Add column item memoization in get_column_items (avoid re-computing on re-renders)

### Accessibility & UX

- [X] T106 [P] Add keyboard navigation support in NavigationColumn.svelte (arrow keys, Enter to select)
- [X] T107 [P] Add ARIA labels and roles in MillerColumnsNav.svelte (screen reader support)
- [X] T108 Add focus management in NavigationColumn.svelte (focus newly created columns)
- [X] T109 Add horizontal scroll indicators in MillerColumnsNav.svelte (show when more columns exist off-screen)

### Documentation

- [X] T110 [P] Update README.md in app/src/lib/components/MillerColumns/ (component usage guide)
- [X] T111 [P] Add inline code comments in lcc-rs/src/cdi/parser.rs (CDI XML parsing logic)
- [ ] T112 Validate all quickstart.md test scenarios (manual testing)

### Code Quality

- [X] T113 Run Rust linter on lcc-rs/src/cdi/ and app/src-tauri/src/ (cargo clippy --fix)
- [X] T114 Run TypeScript linter on app/src/lib/ (npm run lint --fix)
- [X] T115 Run Svelte formatter on app/src/lib/components/MillerColumns/ (npm run format)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **Test Infrastructure (Phase 2b)**: Can proceed in parallel with early Foundational tasks - TDD compliance
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User Story 1 (P1): Can start after Foundational (Phase 2) - No dependencies on other stories
  - User Story 2 (P1): Can start after Foundational (Phase 2) - Builds on US1 components but independently testable
  - User Story 3 (P2): Can start after Foundational (Phase 2) - Enhances US1 with status indicators
  - User Story 3b (P2): Can start after US1 (Phase 3) - Enhances Details Panel
  - User Story 4 (P3): Can start after US1 (Phase 3) - Adds navigation enhancements
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Foundation only - No dependencies on other stories
- **User Story 2 (P1)**: Foundation + US1 components (reuses NavigationColumn.svelte) - Independently testable
- **User Story 3 (P2)**: US1 complete (enhances DetailsPanel.svelte) - Independently testable
- **User Story 4 (P3)**: US1 complete (adds Breadcrumb navigation) - Independently testable

### Within Each User Story

- **User Story 1**: Backend commands → TypeScript wrappers → Svelte components → Route integration
- **User Story 2**: Replication helpers → Rendering enhancements
- **User Story 3**: Metadata enhancements → Details Panel updates
- **User Story 4**: Breadcrumb component → Navigation handlers

### Parallel Opportunities

**Setup Phase (Phase 1)**: All tasks marked [P] can run in parallel (T002-T006)

**Foundational Phase (Phase 2)**:
- Type definitions: T007-T013 (all parallel)
- Parsing functions: T018-T020 (parallel, after T014-T017)
- TypeScript wrappers: T038-T043 (all parallel, after types defined)

**Test Infrastructure (Phase 2b)**:
- CDI unit tests: T043a-T043i (all parallel)
- Integration tests: T043j-T043o (parallel after unit tests)
- Frontend tests: T043p-T043s (parallel, optional)

**User Story 1 (Phase 3)** - Parallel batches:
```bash
Batch 1 (Backend commands):
  T044 (get_discovered_nodes)
  T049-T050 (get_cdi_structure)
  T054-T056 (get_column_items)
  T064-T065 (get_element_details)

Batch 2 (Svelte components):
  T045-T048 (NodesColumn.svelte)
  T066-T071 (DetailsPanel.svelte)
  T051-T053 (NavigationColumn.svelte for Segments)
```

**Polish Phase (Phase 8)**:
- Error handling: T102-T105 (all parallel)
- Documentation: T115-T116 (parallel)
- Linting: T118-T120 (parallel)

---

## Parallel Example: User Story 1

```bash
# Backend Tauri commands can be implemented in parallel:
Task T044: "Implement get_discovered_nodes command in app/src-tauri/src/commands/cdi.rs"
Task T049: "Implement get_cdi_structure command logic in app/src-tauri/src/commands/cdi.rs"
Task T054: "Implement get_column_items command logic in app/src-tauri/src/commands/cdi.rs"
Task T064: "Implement get_element_details command logic in app/src-tauri/src/commands/cdi.rs"

# Frontend components can be built in parallel after TypeScript wrappers:
Task T045: "Create NodesColumn.svelte in app/src/lib/components/MillerColumns/"
Task T066: "Create DetailsPanel.svelte in app/src/lib/components/MillerColumns/"
Task T051: "Create SegmentsColumn component in app/src/lib/components/MillerColumns/NavigationColumn.svelte"
```

---

## Implementation Strategy

### MVP First (User Stories 1 & 2 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 2b: Test Infrastructure (TDD compliance - write tests FIRST)
4. Complete Phase 3: User Story 1 (Navigate to Event ID Elements)
5. Complete Phase 4: User Story 2 (Navigate Replicated Groups)
6. **STOP and VALIDATE**: Test both stories independently
7. Deploy/demo if ready

**Rationale**: US1 + US2 are both P1 and cover the critical navigation workflow. This provides immediate value for Event Bowties feature prerequisite (discovering Event IDs).

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Write tests (Phase 2b) → TDD compliance → Tests FAIL (expected)
3. Add User Story 1 → Tests PASS → Test independently → Deploy/Demo (Basic navigation!)
4. Add User Story 2 → Test independently → Deploy/Demo (Replication support!)
5. Add User Story 3 → Test independently → Deploy/Demo (Enhanced details!)
6. Add User Story 4 → Test independently → Deploy/Demo (Back navigation!)
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. **Team completes Setup + Foundational together** (critical path, ~40 tasks)
2. **Write tests together (Phase 2b)** (TDD - tests written FIRST, ~18 tasks)
3. **Once Foundational is done**:
   - Developer A: User Story 1 (T044-T080) - Core navigation
   - Developer B: User Story 2 (T081-T085) - Replication support
   - Developer C: Foundational optimizations & documentation
4. **After US1 complete**:
   - Developer A: User Story 3 (T086-T089) - Details enhancements
   - Developer B: User Story 4 (T090-T096) - Back navigation
   - Developer C: Polish phase tasks
5. **All developers**: Polish phase (T097-T115) in parallel

---

## Summary

- **Total Tasks**: 115 tasks (97 implementation + 18 test tasks)
- **MVP Scope**: Phase 1 (6 tasks) + Phase 2 (37 tasks) + Phase 2b (18 tests) + Phase 3 (37 tasks) + Phase 4 (5 tasks) = **103 tasks**
- **Task Distribution**:
  - Setup: 6 tasks
  - Foundational: 37 tasks (BLOCKS all stories)
  - Test Infrastructure: 18 tasks (TDD compliance - WRITE FIRST)
  - User Story 1 (P1): 37 tasks
  - User Story 2 (P1): 5 tasks
  - User Story 3 (P2): 4 tasks
  - User Story 4 (P3): 7 tasks
  - Polish: 19 tasks
- **Parallel Opportunities**: 
  - Setup: 5 tasks parallel
  - Foundational: ~15 tasks parallel (in batches)
  - Test Infrastructure: ~15 tasks parallel (CDI unit tests + integration tests)
  - User Story 1: ~10 tasks parallel (backend + frontend separation)
  - Polish: ~10 tasks parallel
- **Independent Test Criteria**: Each user story has clear acceptance criteria
- **Format Validation**: ✅ All tasks follow checklist format (checkbox, ID, labels, file paths)
- **TDD Compliance**: ✅ Test tasks added per Constitution Principle III

---

## Notes

- [P] tasks = different files, no dependencies within same batch
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- **TDD Workflow**: Write tests in Phase 2b FIRST (they will FAIL), then implement to make them PASS
- Tests especially critical for Rust CDI parsing (property-based tests validate protocol correctness)
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Foundational phase is extensive but critical - establishes all CDI parsing infrastructure
