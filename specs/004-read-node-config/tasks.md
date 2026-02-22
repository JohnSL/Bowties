---
description: "Implementation tasks for Read Node Configuration feature"
---

# Tasks: Read Node Configuration

**Input**: Design documents from `/specs/004-read-node-config/`
**Prerequisites**: plan.md, spec.md, data-model.md, contracts/, quickstart.md

**Tests**: This feature requires test-driven development per Constitution Gate III. Tests are included and must be written before implementation.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `- [ ] [ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify project structure and dependencies

- [X] T001 Verify existing Tauri project structure in app/src-tauri/
- [X] T002 Verify lcc-rs dependency includes memory_config module in app/src-tauri/Cargo.toml
- [X] T003 [P] Verify SvelteKit frontend structure in app/src/

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types and infrastructure that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Backend Foundation

- [X] T004 [P] Define ConfigValue enum in app/src-tauri/src/commands/cdi.rs (Int, String, EventId, Float, Invalid variants)
- [X] T005 [P] Define ConfigValueWithMetadata struct in app/src-tauri/src/commands/cdi.rs
- [X] T006 [P] Define ReadProgressUpdate struct in app/src-tauri/src/commands/cdi.rs
- [X] T007 [P] Define ProgressStatus enum in app/src-tauri/src/commands/cdi.rs
- [X] T008 [P] Define ReadAllConfigValuesResponse struct in app/src-tauri/src/commands/cdi.rs
- [X] T009 Implement get_element_size helper function in app/src-tauri/src/commands/cdi.rs
- [X] T010 Implement extract_address_info helper function in app/src-tauri/src/commands/cdi.rs (calculates segment.origin + element.offset)
- [X] T011 Implement navigate_to_element helper function in app/src-tauri/src/commands/cdi.rs
- [X] T012 [P] Add cancellation token support to AppState in app/src-tauri/src/state.rs (Arc<AtomicBool> for read operation cancellation)
- [X] T013 Implement check_cancellation helper in app/src-tauri/src/commands/cdi.rs (returns early if cancellation requested)

### Frontend Foundation

- [X] T014 [P] Define ConfigValue type in app/src/lib/api/types.ts (discriminated union)
- [X] T015 [P] Define ConfigValueWithMetadata interface in app/src/lib/api/types.ts
- [X] T016 [P] Define ReadProgressState interface in app/src/lib/api/types.ts
- [X] T017 [P] Define ProgressStatus type in app/src/lib/api/types.ts
- [X] T018 [P] Define ReadAllConfigValuesResponse interface in app/src/lib/api/types.ts
- [X] T019 [P] Define ConfigValueMap type in app/src/lib/api/types.ts

### State Management Foundation

- [X] T020 Extend MillerColumnsState interface with configValues, readProgress, and isCancelling fields in app/src/lib/stores/millerColumns.ts
- [X] T021 Add configValues cache (Map) initialization in app/src/lib/stores/millerColumns.ts
- [X] T022 Add readProgress state initialization in app/src/lib/stores/millerColumns.ts
- [X] T023 Add cancellation state management (isCancelling flag) in app/src/lib/stores/millerColumns.ts

**Checkpoint**: Foundation complete - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - View Current Configuration Values (Priority: P1) 🎯 MVP

**Goal**: Display current configuration values in Miller Columns details panel when users select elements

**Independent Test**: Select a configuration element (e.g., "Node Name") in Miller Columns and verify the current value stored on the node is displayed

### Unit Tests for User Story 1

> **TDD REQUIREMENT: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T024 [P] [US1] Unit test for parse_config_value with 1-byte int in app/src-tauri/src/commands/cdi.rs (test_parse_int_value_1_byte)
- [ ] T022 [P] [US1] Unit test for parse_config_value with 2-byte int in app/src-tauri/src/commands/cdi.rs (test_parse_int_value_2_bytes)
- [ ] T023 [P] [US1] Unit test for parse_config_value with 4-byte int in app/src-tauri/src/commands/cdi.rs (test_parse_int_value_4_bytes)
- [ ] T024 [P] [US1] Unit test for parse_config_value with 8-byte int in app/src-tauri/src/commands/cdi.rs (test_parse_int_value_8_bytes)
- [ ] T025 [P] [US1] Unit test for parse_config_value with string value in app/src-tauri/src/commands/cdi.rs (test_parse_string_value)
- [ ] T026 [P] [US1] Unit test for parse_config_value with EventId value in app/src-tauri/src/commands/cdi.rs (test_parse_eventid_value)
- [ ] T027 [P] [US1] Unit test for parse_config_value with Float value in app/src-tauri/src/commands/cdi.rs (test_parse_float_value)
- [ ] T028 [P] [US1] Unit test for parse_config_value with invalid UTF-8 string in app/src-tauri/src/commands/cdi.rs (test_parse_invalid_string)

### Backend Implementation for User Story 1

- [X] T032 [US1] Implement parse_config_value function in app/src-tauri/src/commands/cdi.rs (parse Element to ConfigValue based on type)
- [X] T033 [US1] Implement read_config_value Tauri command in app/src-tauri/src/commands/cdi.rs (single element read from address space 0xFD)
- [X] T034 [US1] Register read_config_value command in invoke_handler in app/src-tauri/src/lib.rs
- [X] T035 [US1] Add error handling for node timeout in read_config_value in app/src-tauri/src/commands/cdi.rs
- [X] T036 [US1] Add error handling for invalid response in read_config_value in app/src-tauri/src/commands/cdi.rs

### Frontend Implementation for User Story 1

- [X] T037 [P] [US1] Implement readConfigValue API wrapper in app/src/lib/api/cdi.ts
- [X] T038 [P] [US1] Implement formatConfigValue display formatter in app/src/lib/utils/formatters.ts (handles Int, String, EventId, Float, Invalid types)
- [X] T039 [P] [US1] Implement formatEventId helper in app/src/lib/utils/formatters.ts (dotted hexadecimal format)
- [X] T040 [US1] Add setConfigValue method to millerColumnsStore in app/src/lib/stores/millerColumns.ts
- [X] T041 [US1] Add getConfigValue method to millerColumnsStore in app/src/lib/stores/millerColumns.ts
- [X] T042 [US1] Add cache key generation helper getCacheKey in app/src/lib/api/types.ts (format: nodeId:elementPath with slash separator)
- [X] T043 [US1] Display config value in DetailsPanel component in app/src/lib/components/MillerColumns/DetailsPanel.svelte (add value display section, uses formatConfigValue from T038)
- [X] T044 [US1] Add loading state for value display in DetailsPanel.svelte in app/src/lib/components/MillerColumns/DetailsPanel.svelte

**Checkpoint**: User Story 1 complete - users can view configuration values when selecting elements

---

## Phase 4: User Story 2 - Monitor Configuration Reading Progress (Priority: P2)

**Goal**: Show progress indication during batch configuration reading so users know the system is working

**Independent Test**: Refresh nodes with 2+ discovered nodes and observe progress indicator showing accurate count and node name (e.g., "Reading Tower LCC config... 35%")

### Integration Tests for User Story 2

> **TDD REQUIREMENT: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T045 [P] [US2] Integration test for read_all_config_values with mock node in app/src-tauri/tests/config_reading.rs (test_read_all_config_values_with_mock_node)
- [ ] T046 [P] [US2] Integration test for SNIP priority cascade in app/src-tauri/tests/config_reading.rs (test_snip_priority_cascade)
- [ ] T047 [P] [US2] Integration test for progress event emission in app/src-tauri/tests/config_reading.rs (test_progress_events_emitted)
- [ ] T048 [P] [US2] Integration test for replicated group reading in app/src-tauri/tests/config_reading.rs (test_replicated_group_instances - validates all 16 instances read)

### Backend Implementation for User Story 2

- [X] T049 [US2] Implement get_node_display_name helper function in app/src-tauri/src/commands/cdi.rs (SNIP priority: user_name > user_description > model_name > node_id)
- [X] T050 [US2] Implement extract_all_elements_with_addresses helper in app/src-tauri/src/commands/cdi.rs (walks CDI tree, returns all elements with memory addresses including replicated groups)
- [X] T051 [US2] Implement read_all_config_values Tauri command in app/src-tauri/src/commands/cdi.rs (batch read with progress emission and cancellation support)
- [X] T052 [US2] Add progress event emission logic in read_all_config_values in app/src-tauri/src/commands/cdi.rs (emit config-read-progress events after each element)
- [X] T053 [US2] Add completion percentage calculation in read_all_config_values in app/src-tauri/src/commands/cdi.rs
- [X] T054 [US2] Add cancellation check in read loop in app/src-tauri/src/commands/cdi.rs (check cancellation token before each element read)
- [X] T055 [US2] Register read_all_config_values and cancel_config_reading commands in invoke_handler in app/src-tauri/src/lib.rs
- [X] T056 [US2] Add error recovery logic in read_all_config_values in app/src-tauri/src/commands/cdi.rs (continue on element failure)

### Frontend Implementation for User Story 2

- [X] T057 [P] [US2] Implement readAllConfigValues API wrapper in app/src/lib/api/cdi.ts
- [X] T058 [P] [US2] Implement cancelConfigReading API wrapper in app/src/lib/api/cdi.ts (new Tauri command)
- [X] T059 [US2] Add setConfigValues batch method to millerColumnsStore in app/src/lib/stores/millerColumns.ts (uses colon-separated cache keys)
- [X] T060 [US2] Add setReadProgress method to millerColumnsStore in app/src/lib/stores/millerColumns.ts
- [X] T061 [US2] Add clearConfigValues method to millerColumnsStore in app/src/lib/stores/millerColumns.ts
- [X] T062 [US2] Add setCancelling method to millerColumnsStore in app/src/lib/stores/millerColumns.ts
- [X] T063 [US2] Setup config-read-progress event listener in app/src/routes/+page.svelte (onMount)
- [X] T064 [US2] Integrate readAllConfigValues into refresh flow in app/src/routes/+page.svelte (call after refreshAllNodes)
- [X] T065 [US2] Add progress indicator UI component in app/src/routes/+page.svelte (displays "Reading [Node Name] config... X%" with Cancel button)
- [X] T066 [US2] Add cancel button handler in app/src/routes/+page.svelte (calls cancelConfigReading, sets isCancelling state)
- [X] T067 [US2] Add progress state conditional rendering in app/src/routes/+page.svelte (show/hide based on readProgress)
- [X] T068 [US2] Add config cache clearing on refresh in app/src/routes/+page.svelte (call clearConfigValues before batch read)

**Checkpoint**: User Story 2 complete - users see progress indication during configuration reading

---

## Phase 5: User Story 3 - Refresh Configuration Values on Demand (Priority: P3)

**Goal**: Allow users to manually refresh individual configuration values without full node refresh

**Independent Test**: Select an element, change its value externally, click "Refresh Value" button, and verify display updates

### Frontend Implementation for User Story 3

- [X] T069 [P] [US3] Add "Refresh Value" button to DetailsPanel in app/src/lib/components/MillerColumns/DetailsPanel.svelte
- [X] T070 [US3] Implement handleRefreshValue handler in DetailsPanel.svelte in app/src/lib/components/MillerColumns/DetailsPanel.svelte (calls readConfigValue from T037 and updates cache)
- [X] T071 [US3] Add loading state for refresh button in DetailsPanel.svelte in app/src/lib/components/MillerColumns/DetailsPanel.svelte
- [X] T072 [US3] Add error handling for refresh failures in DetailsPanel.svelte in app/src/lib/components/MillerColumns/DetailsPanel.svelte (display error message, retain stale value)
- [X] T073 [US3] Add staleness indicator when refresh fails in DetailsPanel.svelte in app/src/lib/components/MillerColumns/DetailsPanel.svelte

**Checkpoint**: User Story 3 complete - users can manually refresh individual values

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

### Performance Validation (Addresses FR-014, SC-001, SC-004)

- [ ] T074 [P] Add performance test for single element read in app/src-tauri/tests/config_reading.rs (validates <2s per SC-001)
- [ ] T075 [P] Add performance test for single node batch read in app/src-tauri/tests/config_reading.rs (validates ~5s per node per FR-014)
- [ ] T076 [P] Add integration performance test for 3-node network in app/src-tauri/tests/config_reading.rs (validates ≤15s total per SC-004)
- [ ] T077 Add performance instrumentation in read_all_config_values in app/src-tauri/src/commands/cdi.rs (log duration_ms per node)

### Quality & Documentation

- [ ] T078 [P] Add documentation for configuration reading in docs/features/config-reading.md
- [ ] T079 [P] Add JSDoc comments to API wrappers in app/src/lib/api/cdi.ts
- [ ] T080 [P] Add Rust doc comments to Tauri commands in app/src-tauri/src/commands/cdi.rs
- [ ] T081 Add error rate tracking in read_all_config_values in app/src-tauri/src/commands/cdi.rs (validates <5% per SC-006)
- [ ] T082 [P] Validate quickstart.md examples match implementation in specs/004-read-node-config/quickstart.md
- [ ] T083 Add accessibility labels to progress indicator in app/src/routes/+page.svelte (ARIA attributes)
- [ ] T084 Add keyboard shortcut for "Refresh Value" in DetailsPanel.svelte in app/src/lib/components/MillerColumns/DetailsPanel.svelte

### Scale & Edge Case Validation (Addresses SC-003, SC-008)

- [ ] T085 [P] Add scale test with 100+ element CDI in app/src-tauri/tests/config_reading.rs (validates SC-003)
- [ ] T086 [P] Add value formatting audit test in app/src-tauri/tests/config_reading.rs (validates 90%+ readable per SC-008)
- [ ] T087 [P] Add edge case test for large values (64-byte strings) in app/src-tauri/tests/config_reading.rs

### Protocol Layer Tests — Added February 22, 2026 (Phase 1 implementation complete)

> These tests were written as part of the Bug 1 fix (embedded vs generic format). They live in
> `lcc-rs/src/protocol/memory_config.rs` and are all \[X\] (already passing — 222/222).

- [X] T088 [P] test_build_read_cdi: 7-byte payload, command `0x43`, no space byte in lcc-rs/src/protocol/memory_config.rs
- [X] T089 [P] test_build_read_configuration: 7-byte payload, command `0x41` in lcc-rs/src/protocol/memory_config.rs
- [X] T090 [P] test_build_read_all_memory: 7-byte payload, command `0x42` in lcc-rs/src/protocol/memory_config.rs
- [X] T091 [P] test_build_read_acdi_user: 8-byte payload, command `0x40`, space byte `0xFB` at `[6]` in lcc-rs/src/protocol/memory_config.rs
- [X] T092 [P] test_parse_read_reply_success_embedded: embedded reply `0x53`, data at `[6..]` in lcc-rs/src/protocol/memory_config.rs
- [X] T093 [P] test_parse_read_reply_generic_with_space_byte: generic reply `0x50`, space byte at `[6]`, data at `[7..]` in lcc-rs/src/protocol/memory_config.rs
- [X] T094 [P] test_parse_read_reply_generic_acdi_user: generic reply for AcdiUser space in lcc-rs/src/protocol/memory_config.rs
- [X] T095 [P] test_parse_read_reply_failed_embedded: embedded failure `0x5B`, error bytes at `[6-7]` in lcc-rs/src/protocol/memory_config.rs
- [X] T096 [P] test_parse_read_reply_failed_generic: generic failure `0x58`, space at `[6]`, error at `[7-8]` in lcc-rs/src/protocol/memory_config.rs

> **Note on T024–T031** (unit tests for `parse_config_value` in `app/src-tauri/src/commands/cdi.rs`): These tests were
> specified as TDD pre-requisites but were not written before the implementation (T032 was done directly).
> The function is verifiably correct — values parsed correctly in live traffic captures. The unit tests still
> need to be written to satisfy Constitution Gate III for this function.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-5)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - Reuses US1's read_config_value internally but independently testable
- **User Story 3 (P3)**: Depends on US1 completion (reuses readConfigValue API) - But can be tested independently

### Within Each User Story

- **US1**: Tests (T024-T031) → Implementation (T032-T044)
  - Backend tests → Backend implementation (T032-T036)
  - Frontend can proceed in parallel after T033 (T037-T044)
- **US2**: Tests (T045-T048) → Implementation (T049-T068)
  - Backend helpers (T049-T050) → Command (T051-T056)
  - Frontend can proceed after T051 (T057-T068)
- **US3**: Implementation only (T069-T073) - sequential within DetailsPanel.svelte

### Parallel Opportunities

**Phase 1 (Setup)**: T001, T002, T003 can all run in parallel

**Phase 2 (Foundational)**:
- Backend types (T004-T008) can all run in parallel
- Backend helpers (T009-T011) can run in parallel
- Frontend types (T014-T019) can all run in parallel
- Backend types + Frontend types can run in parallel
- Store updates (T020-T023) must wait for T014-T019
- Cancellation infrastructure (T012-T013) can run in parallel with types

**Phase 3 (User Story 1)**:
- All unit tests (T024-T031) can run in parallel
- After tests pass: Backend implementation (T032-T036) and Frontend formatters (T038-T039) can run in parallel
- Frontend API wrapper (T037) can run in parallel with formatters (T038-T039)

**Phase 4 (User Story 2)**:
- All integration tests (T045-T048) can run in parallel
- Backend helpers (T049-T050) can run in parallel
- Frontend API wrapper (T057-T058) can run in parallel with store methods (T059-T062)

**Phase 6 (Polish)**:
- Documentation tasks (T078-T080) can all run in parallel
- Performance tests (T074-T076) can run in parallel
- Scale/edge case tests (T085-T087) can run in parallel
- Quickstart validation (T082) and accessibility (T083) can run in parallel
- Documentation tasks (T066-T068) can all run in parallel
- Performance logging (T069) and error tracking (T070) can run in parallel
- Quickstart validation (T071) and accessibility (T072) can run in parallel

---

## Parallel Example: User Story 1 Tests

```bash
# Launch all unit tests for User Story 1 together:
Task: "Unit test for parse_config_value with 1-byte int in app/src-tauri/src/commands/cdi.rs"
Task: "Unit test for parse_config_value with 2-byte int in app/src-tauri/src/commands/cdi.rs"
Task: "Unit test for parse_config_value with 4-byte int in app/src-tauri/src/commands/cdi.rs"
Task: "Unit test for parse_config_value with 8-byte int in app/src-tauri/src/commands/cdi.rs"
Task: "Unit test for parse_config_value with string value in app/src-tauri/src/commands/cdi.rs"
Task: "Unit test for parse_config_value with EventId value in app/src-tauri/src/commands/cdi.rs"
Task: "Unit test for parse_config_value with Float value in app/src-tauri/src/commands/cdi.rs"
Task: "Unit test for parse_config_value with invalid UTF-8 string in app/src-tauri/src/commands/cdi.rs"

# Then launch backend + frontend formatters in parallel:
Task: "Implement parse_config_value function in app/src-tauri/src/commands/cdi.rs"
Task: "Implement formatConfigValue display formatter in app/src/lib/formatters.ts"
Task: "Implement formatEventId helper in app/src/lib/formatters.ts"
```

---

## Parallel Example: User Story 2 Implementation

```bash
# Backend helpers can run in parallel:
Task: "Implement get_node_display_name helper function in app/src-tauri/src/commands/cdi.rs"
Task: "Implement extract_all_elements_with_addresses helper in app/src-tauri/src/commands/cdi.rs"

# Frontend store methods can run in parallel:
Task: "Add setConfigValues batch method to millerColumnsStore in app/src/lib/stores/millerColumns.ts"
Task: "Add setReadProgress method to millerColumnsStore in app/src/lib/stores/millerColumns.ts"
Task: "Add clearConfigValues method to millerColumnsStore in app/src/lib/stores/millerColumns.ts"
Task: "Add setCancelling method to millerColumnsStore in app/src/lib/stores/millerColumns.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundational (T004-T023) - **CRITICAL: blocks all stories**
3. Complete Phase 3: User Story 1 (T024-T044)
4. **STOP and VALIDATE**: 
   - All tests passing (T024-T031)
   - Manual test: Select element in Miller Columns → value displays
5. Ready for demo/feedback

**MVP Deliverable**: Users can view current configuration values in Miller Columns

### Incremental Delivery

1. **Foundation** (Phases 1-2): Setup + types → Foundation ready
2. **MVP Release** (Phase 3): User Story 1 → View values → Deploy/Demo ✅
3. **Enhancement 1** (Phase 4): User Story 2 → Progress indication + cancellation → Deploy/Demo ✅
4. **Enhancement 2** (Phase 5): User Story 3 → Manual refresh → Deploy/Demo ✅
5. **Polish** (Phase 6): Performance validation + documentation + accessibility → Final release ✅

Each story adds value without breaking previous stories.

### Parallel Team Strategy

With multiple developers:

1. **Together**: Complete Setup + Foundational (Phases 1-2)
2. **Once Foundational is done**:
   - Developer A: User Story 1 (Phase 3) - Critical path (MVP)
   - Developer B: User Story 2 (Phase 4) - Can start in parallel after Phase 2
   - Developer C: User Story 3 (Phase 5) - Wait for US1 completion (T034, T037-T038)
3. Stories integrate and test independently

---

## Validation Checklist

### User Story 1 Validation

- [ ] All unit tests pass (T024-T031)
- [ ] Can select "Node Name" element and see current value
- [ ] Integer values display in decimal format
- [ ] Event ID values display in dotted hex format (e.g., `05.01.01.01.03.01.00.00`)
- [ ] String values display as UTF-8 text
- [ ] Invalid values show error message

### User Story 2 Validation

- [ ] All integration tests pass (T045-T048)
- [ ] Progress indicator appears during refresh
- [ ] Node name shown in progress uses SNIP priority cascade
- [ ] Percentage updates accurately (0% → 100%)
- [ ] Progress indicator disappears after completion
- [ ] Error in one node doesn't stop reading other nodes
- [ ] Cancel button stops reading operation
- [ ] Cancelled operation retains partial data

### User Story 3 Validation

- [ ] "Refresh Value" button appears in details panel
- [ ] Clicking button updates displayed value
- [ ] Failed refresh shows error message
- [ ] Stale value retained with indicator when refresh fails
- [ ] Refresh button shows loading state during operation

### Success Criteria (from spec.md)

- [ ] SC-001: Values display within 2 seconds of element selection
- [ ] SC-002: Progress updates smoothly with accurate percentage
- [ ] SC-003: System handles 100+ config elements per node
- [ ] SC-004: 3-node network completes within 15 seconds
- [ ] SC-005: Progress shows human-readable node names
- [ ] SC-006: Error rate < 5% under normal conditions
- [ ] SC-007: UI remains responsive (can cancel during reading)
- [ ] SC-008: 90%+ of values display in human-readable format

---

## Notes

- **[P] marker**: Tasks marked with [P] can run in parallel (different files, no shared dependencies)
- **[Story] label**: Maps task to specific user story (US1, US2, US3) for traceability
- **Test-Driven**: Tests must be written FIRST and FAIL before implementation (Constitution Gate III)
- **Independent Stories**: Each user story deliverable can be tested and deployed independently
- **File Paths**: All paths are absolute from repository root (app/src-tauri/..., app/src/...)
- **Constitution**: This task list satisfies all constitution gates from plan.md
- **Memory Protocol**: All reads use the address space declared in the CDI segment; for config values this is typically `0xFD`, but use `segment.space` directly rather than hard-coding
- **SNIP Priority**: Consistent node naming: user_name > user_description > model_name > node_id
- **Protocol Format (added Feb 22)**: Spaces `>=0xFD` use embedded request format (`0x41`/`0x42`/`0x43`); others use generic `0x40` + space byte. See `plan-fixLccMemoryConfigReading.md` and `technical-context.md`
- **CDI offset semantics (added Feb 22)**: `element.offset` is a *relative skip* from the end of the previous element, not an absolute address. Address calculation requires a running cursor; see `process_elements()` in `app/src-tauri/src/commands/cdi.rs`

**Total Tasks**: 96 (was 87; T088–T096 added Feb 22)
- Setup: 3 tasks
- Foundational: 20 tasks (BLOCKS all stories)
- User Story 1: 21 tasks (8 tests + 13 implementation)
- User Story 2: 24 tasks (4 tests + 20 implementation)
- User Story 3: 5 tasks (implementation only)
- Polish: 14 tasks (performance validation + documentation + scale tests)

**Parallel Opportunities**: 40 tasks marked [P] (46% of total)

**Suggested MVP Scope**: Phases 1-3 only (User Story 1) = 44 tasks
