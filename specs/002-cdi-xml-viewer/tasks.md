---
description: "Implementation tasks for CDI XML Viewer feature"
---

# Tasks: CDI XML Viewer

**Input**: Design documents from `specs/001-cdi-xml-viewer/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Tests are OPTIONAL for this feature - none explicitly requested in specification

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Path Conventions

- **Tauri Backend**: `app/src-tauri/src/`
- **SvelteKit Frontend**: `app/src/`
- **Tests**: `app/src-tauri/tests/` (Rust), `app/src/__tests__/` (TypeScript)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify existing project structure supports the feature

- [X] T001 Verify Tauri 2.x project structure in app/src-tauri/
- [X] T002 Verify SvelteKit project structure in app/src/
- [X] T003 [P] Create commands module directory app/src-tauri/src/commands/ if not exists

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Add cdi field to Node struct in app/src-tauri/src/node.rs (if not exists): `cdi: Option<CdiData>`
- [X] T005 Define CdiData struct in app/src-tauri/src/node.rs with xml_content and retrieved_at fields
- [X] T006 Create contracts module in frontend: app/src/lib/types/cdi.ts with ViewerStatus and CdiViewerState types
- [X] T007 [P] Define CdiError enum in app/src-tauri/src/commands/cdi.rs with error variants
- [X] T008 [P] Define GetCdiXmlResponse struct in app/src-tauri/src/commands/cdi.rs per contracts/rust-signatures.md

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - View Formatted CDI XML (Priority: P1) 🎯 MVP

**Goal**: Enable developers to view formatted CDI XML for any node with retrieved CDI data by right-clicking on the node

**Independent Test**: Right-click on any node with CDI data, select "View CDI XML", and confirm properly indented XML displays in modal with copy functionality

### Backend Implementation for User Story 1

- [X] T009 [P] [US1] Implement get_cdi_xml Tauri command in app/src-tauri/src/commands/cdi.rs
- [X] T010 [US1] Add node_id parsing logic to get_cdi_xml command in app/src-tauri/src/commands/cdi.rs
- [X] T011 [US1] Add CDI retrieval from node cache in get_cdi_xml command in app/src-tauri/src/commands/cdi.rs
- [X] T012 [US1] Build GetCdiXmlResponse with xml_content, size_bytes, retrieved_at in app/src-tauri/src/commands/cdi.rs
- [X] T013 [US1] Register get_cdi_xml command in Tauri builder in app/src-tauri/src/lib.rs
- [X] T014 [US1] Export cdi module in app/src-tauri/src/lib.rs

### Frontend Utilities for User Story 1

- [X] T015 [P] [US1] Create XML formatter utility in app/src/lib/utils/xmlFormatter.ts with formatXml function
- [X] T016 [P] [US1] Implement DOMParser-based XML indentation in app/src/lib/utils/xmlFormatter.ts
- [X] T017 [P] [US1] Add XML parse error handling (return raw XML on parse failure) in app/src/lib/utils/xmlFormatter.ts
- [X] T018 [P] [US1] Create Tauri command wrapper in app/src/lib/api/cdi.ts with getCdiXml function

### Frontend UI Components for User Story 1

- [X] T019 [US1] Create CdiXmlViewer modal component in app/src/lib/components/CdiXmlViewer.svelte
- [X] T020 [US1] Add modal overlay and click-outside-to-close behavior in app/src/lib/components/CdiXmlViewer.svelte
- [X] T021 [US1] Add modal header with node ID display in app/src/lib/components/CdiXmlViewer.svelte
- [X] T022 [US1] Add formatted XML display area with pre/code styling in app/src/lib/components/CdiXmlViewer.svelte
- [X] T023 [US1] Implement copy-to-clipboard button using Clipboard API in app/src/lib/components/CdiXmlViewer.svelte
- [X] T024 [US1] Add close button and Escape key handler in app/src/lib/components/CdiXmlViewer.svelte
- [X] T025 [US1] Apply monospaced font and scrollable styling to XML content in app/src/lib/components/CdiXmlViewer.svelte

### Frontend Integration for User Story 1

- [X] T026 [US1] Add context menu to node list in app/src/routes/(nodes)/+page.svelte (or equivalent node display page)
- [X] T027 [US1] Implement contextmenu event handler with menu positioning in node list component
- [X] T028 [US1] Add "View CDI XML" menu item to context menu in node list component
- [X] T029 [US1] Wire context menu selection to open CdiXmlViewer modal with node ID
- [X] T030 [US1] Implement loading state during Tauri command invocation in node list component

**Checkpoint**: At this point, User Story 1 should be fully functional - developers can view formatted CDI XML via right-click

---

## Phase 4: User Story 2 - Handle Missing or Invalid CDI (Priority: P2)

**Goal**: Provide clear feedback when CDI data is not available or invalid so developers understand whether the issue is with retrieval or data quality

**Independent Test**: Attempt to view CDI XML on nodes without CDI data, with corrupted data, or non-existent nodes, and verify appropriate error messages display

### Backend Error Handling for User Story 2

- [X] T031 [P] [US2] Implement CdiNotRetrieved error case in app/src-tauri/src/commands/cdi.rs when node.cdi is None
- [X] T032 [P] [US2] Implement NodeNotFound error case in app/src-tauri/src/commands/cdi.rs when node not in cache
- [X] T033 [P] [US2] Implement CdiUnavailable error case in app/src-tauri/src/commands/cdi.rs for nodes without CDI support
- [X] T034 [US2] Add error message formatting with node ID in all error cases in app/src-tauri/src/commands/cdi.rs

### Frontend Error Handling for User Story 2

- [X] T035 [P] [US2] Add error state to CdiViewerState in app/src/lib/types/cdi.ts
- [X] T036 [P] [US2] Implement getCdiErrorMessage helper in app/src/lib/api/cdi.ts per contracts/tauri-commands.ts
- [X] T037 [US2] Add error display area in CdiXmlViewer modal in app/src/lib/components/CdiXmlViewer.svelte
- [X] T038 [US2] Map backend error types to user-friendly messages in app/src/lib/components/CdiXmlViewer.svelte
- [X] T039 [US2] Handle Promise rejection from getCdiXml and display error in modal
- [X] T040 [US2] Add fallback to display raw XML when formatting fails in app/src/lib/components/CdiXmlViewer.svelte

### User Experience Polish for User Story 2

- [ ] T041 [US2] Disable "View CDI XML" menu item when node has no CDI data in node list component (Not implemented: menu shows for all nodes, error displayed on click)
- [ ] T042 [US2] Add tooltip to disabled menu item explaining why it's disabled in node list component (Not implemented: see T041)

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently - full error handling implemented

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [-] T043 [P] Add keyboard shortcut Ctrl+I (Cmd+I) to open CDI viewer in node list component (Skipped: requires node selection model)
- [X] T044 [P] Add performance warning when CDI size exceeds 1MB in app/src/lib/components/CdiXmlViewer.svelte
- [X] T045 Add modal focus trap (tab cycles within modal) in app/src/lib/components/CdiXmlViewer.svelte
- [X] T046 Add aria-labels for accessibility in app/src/lib/components/CdiXmlViewer.svelte
- [ ] T047 [P] Update user documentation with CDI viewer usage per specs/001-cdi-xml-viewer/quickstart.md
- [ ] T048 Run quickstart.md validation with actual feature implementation

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User Story 1 can start after Foundational (Phase 2)
  - User Story 2 depends on User Story 1 modal component (T019) being created
- **Polish (Phase 5)**: Depends on User Stories 1 and 2 being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after User Story 1 modal component exists (T019) - Enhances US1 with error handling

### Within Each User Story

**User Story 1 flow:**
1. Backend tasks (T009-T014) can proceed in parallel with Utilities tasks (T015-T018)
2. UI Components (T019-T025) depend on Utilities (T015-T018) being complete
3. Integration tasks (T026-T030) depend on both Backend and UI Components

**User Story 2 flow:**
1. Backend error handling (T031-T034) can run in parallel with Frontend error handling (T035-T040)
2. UX polish tasks (T041-T042) depend on both backend and frontend error handling

### Parallel Opportunities

- **Setup**: T003 can run in parallel with T001-T002
- **Foundational**: T004-T005 (backend) can run in parallel with T006 (frontend types)
- **Foundational**: T007 and T008 are both backend types and can run in parallel
- **User Story 1 Backend**: T009 is the main implementation, T010-T012 are parts of it (sequential)
- **User Story 1 Frontend Utilities**: T015, T016, T017 are parts of xmlFormatter (sequential), but T018 (API wrapper) can run in parallel once T015-T017 complete
- **User Story 1 UI Components**: T019 creates the component, T020-T025 enhance it (mostly sequential)
- **User Story 2 Backend**: T031, T032, T033 can all run in parallel (different error cases)
- **User Story 2 Frontend**: T035, T036 can run in parallel
- **Polish**: T043, T044, T047 can all run in parallel

---

## Parallel Example: User Story 1

```bash
# After Foundational Phase completes, launch in parallel:

# Group A: Backend implementation
Task T009: "Implement get_cdi_xml Tauri command in app/src-tauri/src/commands/cdi.rs"
  └─> T010-T012 (sequential within command implementation)
  └─> T013: "Register command in Tauri builder"
  └─> T014: "Export cdi module"

# Group B: Frontend utilities (parallel with Group A)
Task T015: "Create XML formatter utility"
  └─> T016-T017 (implement formatting logic)
Task T018: "Create Tauri command wrapper" (depends on T015-T017)

# Group C: UI Components (after Group B completes)
Task T019: "Create CdiXmlViewer modal component"
  └─> T020-T025 (enhance modal features sequentially)

# Group D: Integration (after Groups A, B, C complete)
Task T026-T030: Wire up context menu and modal
```

---

## Parallel Example: User Story 2

```bash
# Launch all error cases in parallel:
Task T031: "Implement CdiNotRetrieved error case"
Task T032: "Implement NodeNotFound error case"
Task T033: "Implement CdiUnavailable error case"
  └─> T034: "Format error messages"

# In parallel with backend errors:
Task T035: "Add error state to CdiViewerState"
Task T036: "Implement getCdiErrorMessage helper"
  └─> T037-T040: Display errors in modal
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (verify structure)
2. Complete Phase 2: Foundational (CRITICAL - types and base structures)
3. Complete Phase 3: User Story 1 (full viewing functionality)
4. **STOP and VALIDATE**: Test User Story 1 independently
   - Right-click on node with CDI
   - Verify formatted XML displays
   - Test copy functionality
   - Close modal and repeat
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → **Deploy/Demo (MVP!)**
   - Developers can now view CDI XML for debugging
3. Add User Story 2 → Test independently → Deploy/Demo
   - Error handling complete, production-ready
4. Add Polish → Test completely → Deploy/Demo
   - Full feature with keyboard shortcuts and accessibility

### Parallel Team Strategy

With multiple developers:

1. **Team completes Setup + Foundational together** (small, quick)
2. **Once Foundational is done:**
   - **Developer A**: User Story 1 - Backend (T009-T014)
   - **Developer B**: User Story 1 - Frontend Utilities (T015-T018)
   - After both complete, either can do UI Components (T019-T025)
   - Work together on Integration (T026-T030)
3. **After User Story 1 complete:**
   - **Developer A**: User Story 2 - Backend (T031-T034)
   - **Developer B**: User Story 2 - Frontend (T035-T042)
4. **Polish tasks** can be split between developers

---

## Summary

**Total Tasks**: 48
- **Phase 1 (Setup)**: 3 tasks
- **Phase 2 (Foundational)**: 5 tasks (BLOCKS all stories)
- **Phase 3 (User Story 1)**: 22 tasks
  - Backend: 6 tasks
  - Frontend Utilities: 4 tasks
  - Frontend UI: 7 tasks
  - Integration: 5 tasks
- **Phase 4 (User Story 2)**: 12 tasks
  - Backend: 4 tasks
  - Frontend: 6 tasks
  - UX Polish: 2 tasks
- **Phase 5 (Polish)**: 6 tasks

**Parallel Opportunities**: 15+ tasks can run in parallel across different files and user stories

**Independent Test Criteria**:
- **User Story 1**: Can right-click node → View formatted XML → Copy content → Close modal
- **User Story 2**: Can handle missing CDI with clear error messages

**Suggested MVP Scope**: Complete through Phase 3 (User Story 1) - delivers core debugging functionality

**Implementation Notes**:
- All tasks follow checklist format with Task IDs, parallel markers [P], and story labels [US1]/[US2]
- File paths specified for every implementation task
- Clear dependencies identified between phases and within user stories
- No new external dependencies required (uses existing Tauri + SvelteKit stack)
- Zero tests included (tests not explicitly requested in specification)