# Tasks: Enhanced Node Discovery with SNIP Data

**Input**: Design documents from `/specs/001-node-snip-data/`  
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.json, quickstart.md

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Rust library**: `lcc-rs/src/`
- **Tauri backend**: `app/src-tauri/src/`
- **SvelteKit frontend**: `app/src/`
- **Tests**: `lcc-rs/tests/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [X] T001 Verify lcc-rs dependencies in lcc-rs/Cargo.toml (tokio, serde, thiserror, async-trait, chrono)
- [X] T002 [P] Add lcc-rs dependency to app/src-tauri/Cargo.toml with path reference
- [X] T003 [P] Create app/src/lib/api/tauri.ts for typed Tauri command wrappers
- [X] T004 [P] Create app/src/lib/stores/nodes.ts for reactive node state management

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Core Types and Enums

- [X] T005 [P] Add SNIPStatus enum to lcc-rs/src/types.rs (Unknown, InProgress, Complete, Partial, NotSupported, Timeout, Error)
- [X] T006 [P] Add ConnectionStatus enum to lcc-rs/src/types.rs (Unknown, Verifying, Connected, NotResponding)
- [X] T007 Update Node struct in lcc-rs/src/types.rs to add snip_status, connection_status, last_verified, last_seen fields
- [X] T008 [P] Implement sanitize method for SNIPData in lcc-rs/src/types.rs to validate and clean string fields

### MTI Protocol Support

- [X] T009 Add SNIP MTI constants to lcc-rs/src/protocol/mti.rs (SNIPRequest=0x19DE8, SNIPResponse=0x19A08)
- [X] T010 [P] Add datagram MTI constants to lcc-rs/src/protocol/mti.rs (DatagramOnly=0x1A000, DatagramFirst=0x1B000, DatagramMiddle=0x1C000, DatagramFinal=0x1D000)
- [X] T011 [P] Add datagram acknowledgment MTIs to lcc-rs/src/protocol/mti.rs (DatagramReceivedOk=0x19A28, DatagramRejected=0x19A48)

### Datagram Assembly Infrastructure

- [X] T012 Create lcc-rs/src/protocol/datagram.rs module for multi-frame datagram reassembly
- [X] T013 Implement DatagramState enum in lcc-rs/src/protocol/datagram.rs (Idle, Receiving, Complete, Error)
- [X] T014 Implement DatagramAssembler struct in lcc-rs/src/protocol/datagram.rs with buffer and state tracking
- [X] T015 Implement handle_frame method in lcc-rs/src/protocol/datagram.rs to process DatagramFirst/Middle/Final frames
- [X] T016 Implement get_payload method in lcc-rs/src/protocol/datagram.rs to extract bytes 2-7 from each frame
- [X] T017 Implement send_acknowledgment method in lcc-rs/src/protocol/datagram.rs for DatagramReceivedOk responses

### SNIP Protocol Module

- [X] T018 Create lcc-rs/src/snip.rs module for SNIP request/response handling
- [X] T019 Implement parse_snip_payload function in lcc-rs/src/snip.rs to extract 6 null-terminated strings from datagram
- [X] T020 Implement parse_section function in lcc-rs/src/snip.rs to handle version byte and field count validation
- [X] T021 Implement query_snip async function in lcc-rs/src/snip.rs with semaphore-based concurrency limiting (max 5)
- [X] T022 Implement timeout handling in lcc-rs/src/snip.rs (5 second max, 25ms silence detection)
- [X] T023 Export snip module from lcc-rs/src/lib.rs

### Tauri Application State

- [X] T024 Create app/src-tauri/src/state.rs module for application state management
- [X] T025 Implement AppState struct in app/src-tauri/src/state.rs with LccConnection and discovered nodes cache
- [X] T026 Initialize AppState in app/src-tauri/src/main.rs with proper Tauri state management

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - View Discovered Nodes with Friendly Names (Priority: P1) 🎯 MVP

**Goal**: Display all discovered LCC nodes with manufacturer, model, version, and user-assigned names instead of cryptic Node IDs

**Independent Test**: Launch application with active LCC network, verify node list displays manufacturer, model, software version, and user names instead of just Node IDs and aliases

### Backend Implementation (User Story 1)

- [X] T027 [P] [US1] Create app/src-tauri/src/commands/mod.rs to organize command modules
- [X] T028 [P] [US1] Create app/src-tauri/src/commands/discovery.rs for node discovery commands
- [X] T029 [US1] Implement discover_nodes Tauri command in app/src-tauri/src/commands/discovery.rs calling LccConnection::discover_nodes
- [X] T030 [US1] Implement query_snip Tauri command in app/src-tauri/src/commands/discovery.rs for single node SNIP retrieval
- [X] T031 [US1] Implement query_snip_batch Tauri command in app/src-tauri/src/commands/discovery.rs for concurrent multi-node SNIP queries
- [X] T032 [US1] Register commands in app/src-tauri/src/main.rs with invoke_handler

### Frontend Implementation (User Story 1)

- [X] T033 [P] [US1] Create app/src/lib/components/NodeList.svelte component for displaying discovered nodes
- [X] T034 [P] [US1] Create app/src/lib/components/NodeStatus.svelte component for status indicators (green/red/gray dots)
- [X] T035 [US1] Implement friendly name formatting in app/src/lib/components/NodeList.svelte (priority: user_name > manufacturer+model > node_id)
- [X] T036 [US1] Implement tooltip display in app/src/lib/components/NodeList.svelte showing full Node ID, alias, software version
- [X] T037 [US1] Add graceful handling for nodes without SNIP support in app/src/lib/components/NodeList.svelte (show "SNIP not supported")
- [X] T038 [US1] Implement duplicate name disambiguation in app/src/lib/components/NodeList.svelte (append partial Node ID)
- [X] T039 [US1] Add truncation with ellipsis for long descriptions in app/src/lib/components/NodeList.svelte
- [X] T040 [US1] Update app/src/routes/+page.svelte to use NodeList component and trigger initial discovery

### Integration (User Story 1)

- [X] T041 [US1] Implement Tauri command wrappers in app/src/lib/api/tauri.ts (discoverNodes, querySnip, querySnipBatch)
- [X] T042 [US1] Wire up nodes store in app/src/lib/stores/nodes.ts to update reactively from Tauri events
- [X] T043 [US1] Test end-to-end: Launch app, verify nodes display with friendly names and SNIP data

**Checkpoint**: At this point, User Story 1 should be fully functional - nodes display with friendly names and SNIP data

---

## Phase 4: User Story 2 - On-Demand Node Status Verification (Priority: P2)

**Goal**: Allow users to manually refresh the node list to verify which nodes are currently online and update their status

**Independent Test**: Connect to network with all nodes online, physically disconnect one node, click "Refresh" button, verify node's status updates to "Not Responding"

### Backend Implementation (User Story 2)

- [X] T044 [P] [US2] Implement refresh_all_nodes Tauri command in app/src-tauri/src/commands/discovery.rs
- [X] T045 [P] [US2] Implement verify_node_status Tauri command in app/src-tauri/src/commands/discovery.rs for single node verification
- [X] T046 [US2] Add cancel support for in-progress refresh operations in app/src-tauri/src/state.rs
- [X] T047 [US2] Implement connection status update logic in lcc-rs/src/discovery.rs (Connected, NotResponding based on Verify Node response)
- [X] T048 [US2] Register new commands in app/src-tauri/src/main.rs

### Frontend Implementation (User Story 2)

- [X] T049 [P] [US2] Create app/src/lib/components/RefreshButton.svelte component with click handler
- [X] T050 [US2] Add "Refreshing..." overlay to app/src/lib/components/NodeList.svelte during refresh operations
- [X] T051 [US2] Implement refresh cancellation in app/src/lib/components/RefreshButton.svelte (cancel previous refresh on new click)
- [X] T052 [US2] Add "Last verified" timestamp display to app/src/lib/components/NodeList.svelte (e.g., "Verified 30 seconds ago")
- [X] T053 [US2] Update status indicators in app/src/lib/components/NodeStatus.svelte to show Connected (green), Not Responding (red), Unknown (gray)
- [X] T054 [US2] Add warning dialog for unresponsive nodes in app/src/lib/components/NodeList.svelte when user attempts configuration

### Integration (User Story 2)

- [X] T055 [US2] Add refreshAllNodes and verifyNodeStatus wrappers to app/src/lib/api/tauri.ts
- [X] T056 [US2] Wire RefreshButton to trigger refresh_all_nodes command and update nodes store in app/src/lib/stores/nodes.ts
- [X] T057 [US2] Test end-to-end: Click refresh, verify status indicators update correctly for online/offline nodes

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently - users can view nodes and manually refresh status

---

## Phase 5: User Story 3 - Automatic Discovery of New Nodes (Priority: P2) ⚠️ DEFERRED

**Goal**: Automatically detect and display newly joined nodes without manual intervention by listening for Verified Node ID broadcasts

**Status**: **DEFERRED** - Architecture refactoring required. See [ARCHITECTURE-NOTES.md](ARCHITECTURE-NOTES.md) for detailed analysis and recommended approach.

**Reason**: Initial implementation attempt revealed that creating a second TCP connection for background listening causes the main connection to hang. Requires transport layer refactoring to support shared frame access with multiple consumers (Frame Broadcasting pattern recommended).

**Independent Test**: Launch application, note initial node count, physically connect new LCC device to network, verify it appears in list within 10 seconds without clicking refresh

### Backend Implementation (User Story 3)

- [ ] T058 [US3] Implement background listener task in lcc-rs/src/discovery.rs to monitor all network traffic
- [ ] T059 [US3] Add MTI filter for Verified Node ID broadcasts (0x19170) in lcc-rs/src/discovery.rs
- [ ] T060 [US3] Implement extract_node_info function in lcc-rs/src/discovery.rs to parse Node ID and alias from Verified Node frames
- [ ] T061 [US3] Add automatic SNIP retrieval trigger in lcc-rs/src/discovery.rs when new node detected
- [ ] T062 [US3] Implement node cache deduplication in app/src-tauri/src/state.rs to prevent duplicate entries

### Frontend Implementation (User Story 3)

- [ ] T063 [US3] Define node_discovered Tauri event in app/src-tauri/src/commands/discovery.rs
- [ ] T064 [US3] Emit node_discovered event when new node joins in app/src-tauri/src/state.rs
- [ ] T065 [US3] Add event listener in app/src/routes/+page.svelte for node_discovered events
- [ ] T066 [US3] Update nodes store in app/src/lib/stores/nodes.ts to append new nodes reactively
- [ ] T067 [US3] Add visual indicator in app/src/lib/components/NodeList.svelte for newly discovered nodes (e.g., brief highlight animation)

### Integration (User Story 3)

- [ ] T068 [US3] Start background listener task on application startup in app/src-tauri/src/main.rs
- [ ] T069 [US3] Test end-to-end: Power on new LCC device, verify it appears automatically within 10 seconds

**Checkpoint**: User Stories 1 & 2 are **fully functional**. User Story 3 is **deferred** pending transport layer refactoring (see ARCHITECTURE-NOTES.md).

**Prerequisite**: Before implementing User Story 3, complete transport layer refactoring to support frame broadcasting pattern.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T070 [P] Add logging statements throughout lcc-rs/src/snip.rs and lcc-rs/src/discovery.rs for debugging
- [ ] T071 [P] Implement string sanitization for SNIP fields in lcc-rs/src/types.rs to handle invalid UTF-8 and control characters
- [ ] T072 [P] Add error handling for malformed SNIP data in lcc-rs/src/snip.rs (invalid version bytes, missing null terminators)
- [ ] T073 Optimize SNIP request queueing in lcc-rs/src/snip.rs with tokio::sync::Semaphore (capacity=5)
- [ ] T074 [P] Add keyboard shortcut (F5 / Ctrl+R) for refresh in app/src/routes/+page.svelte
- [ ] T075 [P] Add accessibility labels to app/src/lib/components/NodeStatus.svelte for screen readers
- [ ] T076 Update app/README.md with SNIP feature documentation and usage instructions
- [ ] T077 Validate implementation against quickstart.md scenarios in specs/001-node-snip-data/quickstart.md
- [ ] T078 [P] Performance testing: Verify SNIP retrieval completes for 95% of nodes within 3 seconds (20 node network)
- [ ] T079 [P] Edge case testing: Test with nodes that don't support SNIP, slow responders, malformed data

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational (Phase 2) completion
- **User Story 2 (Phase 4)**: Depends on Foundational (Phase 2) completion - Builds on US1 but independently testable
- **User Story 3 (Phase 5)**: Depends on Foundational (Phase 2) completion - Builds on US1 but independently testable
- **Polish (Phase 6)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories - **MVP TARGET**
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - May integrate with US1 components but should be independently testable
- **User Story 3 (P2)**: Can start after Foundational (Phase 2) - May integrate with US1 components but should be independently testable

### Within Each User Story

**User Story 1 (View Nodes with Friendly Names):**
1. Backend commands (T027-T032) before frontend integration
2. Frontend components (T033-T034) can be built in parallel
3. Friendly name logic and tooltips (T035-T039) build on components
4. Integration and wiring (T041-T043) requires all pieces complete

**User Story 2 (Manual Refresh):**
1. Backend refresh commands (T044-T048) before frontend integration
2. Frontend components (T049-T054) can be built in parallel
3. Integration (T055-T057) requires both backend and frontend complete

**User Story 3 (Auto-Discovery):**
1. Backend listener and event emission (T058-T062) before frontend
2. Frontend event handling (T063-T067) depends on events defined
3. Integration (T068-T069) requires full event pipeline

### Parallel Opportunities

**Setup Phase (Phase 1):**
- T002, T003, T004 can all run in parallel (different files)

**Foundational Phase (Phase 2):**
- T005, T006, T008 can run in parallel (all in types.rs but different sections)
- T010, T011 can run in parallel with T009 (same file but independent additions)
- T013-T017 must be sequential (datagram.rs state machine)
- T019-T022 must be sequential (snip.rs parsing logic)
- T024, T025 can run in parallel with other modules

**User Story 1:**
- T027, T028 can run in parallel (different modules)
- T033, T034 can run in parallel (different components)
- T035-T039 can run in parallel (different features in same component)

**User Story 2:**
- T044, T045 can run in parallel (different commands)
- T049, T050, T052, T053, T054 can run in parallel (different components/features)

**User Story 3:**
- T058-T061 in lcc-rs can proceed in parallel with T063-T064 (backend event definition)

**Polish Phase:**
- T070, T071, T072, T074, T075, T076, T078, T079 can all run in parallel (different files/concerns)

---

## Parallel Example: User Story 1 Backend

```bash
# These backend tasks can launch together after Phase 2 completes:
Task T027: "Create commands/mod.rs module"
Task T028: "Create commands/discovery.rs for discovery commands"

# These frontend tasks can launch together:
Task T033: "Create NodeList.svelte component"
Task T034: "Create NodeStatus.svelte component"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T026) - CRITICAL foundation
3. Complete Phase 3: User Story 1 (T027-T043) - Basic discovery + SNIP display
4. **STOP and VALIDATE**: Test User Story 1 independently using quickstart.md scenarios
5. Deploy/demo if ready

**Result**: Users can see all discovered nodes with friendly names and SNIP data - core value delivered

### Incremental Delivery

1. Setup + Foundational → Foundation ready (T001-T026)
2. Add User Story 1 → Test independently → Deploy/Demo (T027-T043) **← MVP!**
3. Add User Story 2 → Test independently → Deploy/Demo (T044-T057)
4. Add User Story 3 → Test independently → Deploy/Demo (T058-T069)
5. Polish → Final release (T070-T079)

Each story adds value without breaking previous stories.

### Parallel Team Strategy

With multiple developers:

1. **Together**: Complete Setup + Foundational (T001-T026)
2. **Once Foundational is done**, split work:
   - **Developer A**: User Story 1 (T027-T043)
   - **Developer B**: User Story 2 (T044-T057) - start after checking US1 patterns
   - **Developer C**: User Story 3 (T058-T069) - start after checking US1 patterns
3. Stories complete and integrate independently
4. **Together**: Polish phase (T070-T079)

---

## Notes

- **[P] tasks**: Can run in parallel (different files, no dependencies)
- **[Story] label**: Maps task to specific user story for traceability
- **Each user story**: Independently completable and testable
- **Foundation critical**: Phase 2 must be 100% complete before any user story work begins
- **Testing**: Each user story has independent test criteria defined in spec.md
- **Commits**: Commit after each task or logical group of parallel tasks
- **Checkpoints**: Stop at each checkpoint to validate story works independently
- **Avoid**: Vague tasks, same-file conflicts, cross-story dependencies that break independence

---

## Total Task Count: 79 tasks

- **Setup**: 4 tasks
- **Foundational**: 22 tasks (blocking)
- **User Story 1 (P1)**: 17 tasks (MVP target)
- **User Story 2 (P2)**: 14 tasks
- **User Story 3 (P2)**: 12 tasks
- **Polish**: 10 tasks

---

## Suggested MVP Scope

**Minimum Viable Product**: Complete through User Story 1 (Tasks T001-T043)

This delivers the core value: users can see their LCC nodes with friendly names instead of cryptic IDs. Manual refresh and auto-discovery can be added in subsequent iterations.
