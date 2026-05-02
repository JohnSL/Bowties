# Tasks: Connector Daughterboard Selection

**Input**: Design documents from `/specs/011-daughterboard-selection/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `quickstart.md`, `contracts/`

**Tests**: Included. The plan, constitution, and product testing strategy require focused automated coverage at the owning seam for backend profile parsing/persistence plus frontend store, orchestrator, and component behavior.

**Organization**: Tasks are grouped by user story so each story can be implemented and validated independently once the foundational work is complete.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish the shared connector-daughterboard surfaces used by all stories.

- [X] T001 Create shared connector daughterboard frontend types in `app/src/lib/types/connectorProfile.ts`
- [X] T002 [P] Create frontend IPC wrappers for connector profile and selection operations in `app/src/lib/api/connectorProfiles.ts`
- [X] T003 [P] Create backend command module scaffold for connector daughterboard operations in `app/src-tauri/src/commands/connector_profiles.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core profile-schema, persistence, and command infrastructure that all user stories depend on.

**⚠️ CRITICAL**: No user story work should begin until this phase is complete.

- [X] T004 [P] Extend the structure-profile schema for connector slots, reusable daughterboards, overrides, and repair rules in `specs/011-daughterboard-selection/contracts/structure-profile.schema.json` and `app/src-tauri/src/profile/types.rs`
- [X] T005 [P] Extend backend profile resolution scaffolding for connector-slot paths, reusable daughterboard references, and carrier overrides in `app/src-tauri/src/profile/resolver.rs` and `app/src-tauri/src/profile/mod.rs`
- [X] T006 [P] Create the shared reusable RR-CirKits daughterboard definition source in `app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml` and add loader support in `app/src-tauri/src/profile/loader.rs` and `app/src-tauri/src/profile/mod.rs`
- [X] T007 [P] Extend layout persistence types for per-node connector selections in `app/src-tauri/src/layout/types.rs` and `app/src/lib/api/layout.ts`
- [X] T008 Register connector daughterboard commands in `app/src-tauri/src/commands/mod.rs`, `app/src-tauri/src/main.rs`, and `app/src-tauri/src/commands/connector_profiles.rs`
- [X] T009 [P] Add initial RR-CirKits Tower and Signal carrier profile fixture files in `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml`, `app/src-tauri/profiles/RR-CirKits_Signal-LCC-P.profile.yaml`, `app/src-tauri/profiles/RR-CirKits_Signal-LCC-S.profile.yaml`, and `app/src-tauri/profiles/RR-CirKits_Signal-LCC-32H.profile.yaml`
- [X] T010 [P] Add foundational Rust coverage for extended profile parsing and layout metadata round-trip in `app/src-tauri/src/profile/types.rs`, `app/src-tauri/src/profile/loader.rs`, and `app/src-tauri/src/layout/types.rs`

**Checkpoint**: Schema, persistence, command registration, and fixture carrier profiles are ready for story work.

---

## Phase 3: User Story 1 - Choose Installed Daughterboards Per Connector (Priority: P1) 🎯 MVP

**Goal**: Let operators choose and persist one daughterboard selection per connector slot for supported RR-CirKits carrier boards.

**Independent Test**: Open a supported node, pick different daughterboards for two connector slots, save the layout/project, reopen it, and confirm both selections restore for that node.

### Tests for User Story 1

- [X] T011 [P] [US1] Add backend command tests for connector profile loading and connector-selection persistence in `app/src-tauri/src/commands/connector_profiles.rs`
- [X] T012 [P] [US1] Add store and layout orchestration tests for connector-selection load/save behavior in `app/src/lib/stores/layout.svelte.test.ts` and `app/src/lib/orchestration/offlineLayoutOrchestrator.test.ts`
- [X] T013 [P] [US1] Add ConfigSidebar rendering tests for connector slot selectors in `app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts` and `app/src/lib/components/ConfigSidebar/configSidebarPresenter.test.ts`

### Implementation for User Story 1

- [X] T014 [US1] Implement connector-slot profile loading and node payload assembly in `app/src-tauri/src/profile/mod.rs`, `app/src-tauri/src/profile/resolver.rs`, `app/src-tauri/src/commands/cdi.rs`, and `app/src-tauri/src/node_tree.rs`
- [X] T015 [US1] Implement saved layout/project persistence for per-node connector selections in `app/src-tauri/src/layout/types.rs`, `app/src-tauri/src/commands/connector_profiles.rs`, and `app/src/lib/api/connectorProfiles.ts`
- [X] T016 [US1] Implement connector-selection store and layout rehydrate/save flow in `app/src/lib/stores/connectorSelections.svelte.ts`, `app/src/lib/stores/layout.svelte.ts`, and `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`
- [X] T017 [US1] Implement connector-slot presenter logic and selector UI in `app/src/lib/components/ConfigSidebar/configSidebarPresenter.ts`, `app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte`, and `app/src/lib/components/ConfigSidebar/ConnectorSlotSelector.svelte`
- [X] T018 [US1] Wire active-node connector selection loading into the configuration workflow in `app/src/routes/+page.svelte`, `app/src/lib/stores/configSidebar.ts`, and `app/src/lib/stores/nodeTree.svelte.ts`
- [X] T019 [US1] Author connector-slot declarations and supported daughterboard references for the initial RR-CirKits Tower and Signal carrier boards in `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml`, `app/src-tauri/profiles/RR-CirKits_Signal-LCC-P.profile.yaml`, `app/src-tauri/profiles/RR-CirKits_Signal-LCC-S.profile.yaml`, and `app/src-tauri/profiles/RR-CirKits_Signal-LCC-32H.profile.yaml`

**Checkpoint**: Connector slots can be selected per supported node and selections persist with the saved layout/project context.

---

## Phase 4: User Story 2 - See Only Valid Line Options (Priority: P1)

**Goal**: Filter each affected line, group, and field so the UI shows only options valid for the selected daughterboard on that connector slot.

**Independent Test**: Select a daughterboard that supports only a subset of line modes, open a governed line, and confirm unsupported sections/options are hidden or narrowed appropriately. Then set the slot to `None installed` and confirm the governed line returns to the base carrier-board options unless the profile explicitly authors empty-slot behavior.

### Tests for User Story 2

- [X] T020 [P] [US2] Add pure constraint-evaluation tests for connector filtering rules, including default `None installed` no-op behavior and explicit empty-slot overrides, in `app/src/lib/utils/connectorConstraints.test.ts`
- [X] T021 [P] [US2] Add UI tests for connector-governed filtering behavior and live line refresh in `app/src/lib/components/ElementCardDeck/SegmentView.test.ts` and `app/src/lib/components/ElementCardDeck/TreeGroupAccordion.test.ts`

### Implementation for User Story 2

- [X] T022 [US2] Implement frontend constraint-evaluation helpers for slot selections and daughterboard rules in `app/src/lib/utils/connectorConstraints.ts` and `app/src/lib/types/connectorProfile.ts`
- [X] T023 [US2] Extend backend payloads with resolved connector-governed target metadata, reusable daughterboard references, and repair-rule inputs needed by frontend evaluation in `app/src-tauri/src/profile/mod.rs`, `app/src-tauri/src/node_tree.rs`, and `app/src-tauri/src/commands/cdi.rs`
- [X] T024 [US2] Thread connector-based filtering inputs and option narrowing through the active segment render flow in `app/src/lib/components/ElementCardDeck/SegmentView.svelte`, `app/src/lib/components/ElementCardDeck/TreeGroupAccordion.svelte`, and `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`
- [X] T025 [US2] Update config rendering to hide, disable, and narrow governed sections and fields in `app/src/lib/components/ElementCardDeck/SegmentView.svelte`, `app/src/lib/components/ElementCardDeck/TreeGroupAccordion.svelte`, and `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`
- [X] T026 [US2] Populate the shared reusable RR-CirKits daughterboard definitions in `app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml` and land Tower-LCC-specific slot mappings in `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml` while leaving Signal carrier overrides empty until concrete path evidence exists

**Checkpoint**: Connector selections actively constrain the visible configuration choices for governed lines while unaffected nodes and sections remain unchanged.

---

## Phase 5: User Story 3 - Resolve Incompatible Existing Settings (Priority: P2)

**Goal**: Automatically stage compatible repairs when a connector daughterboard change invalidates current settings, and surface those staged changes before apply.

**Independent Test**: Start with a compatible value, change the connector to a different daughterboard that invalidates it, and confirm Bowties stages compatible replacements/resets and blocks newly invalid values.

### Tests for User Story 3

- [ ] T027 [P] [US3] Add orchestrator and store tests for auto-staged compatibility repairs in `app/src/lib/orchestration/syncApplyOrchestrator.test.ts`, `app/src/lib/stores/offlineChanges.store.test.ts`, and `app/src/lib/stores/nodeTree.store.test.ts`
- [ ] T028 [P] [US3] Add Rust tests for repair-rule selection and unknown daughterboard preservation in `app/src-tauri/src/profile/mod.rs` and `app/src-tauri/src/layout/types.rs`

### Implementation for User Story 3

- [ ] T029 [US3] Implement backend validation helpers for connector selections, unknown-daughterboard preservation, and resolved repair metadata exposure in `app/src-tauri/src/profile/mod.rs`, `app/src-tauri/src/profile/resolver.rs`, and `app/src-tauri/src/commands/connector_profiles.rs`
- [ ] T030 [US3] Implement frontend connector-selection orchestration to compute compatibility previews and stage generated config changes in `app/src/lib/orchestration/connectorSelectionOrchestrator.ts`, `app/src/lib/stores/connectorSelections.svelte.ts`, and `app/src/lib/stores/offlineChanges.svelte.ts`
- [ ] T031 [US3] Integrate auto-staged repairs into pending tree values and sync/apply flows in `app/src/lib/orchestration/syncApplyOrchestrator.ts`, `app/src/lib/orchestration/configReadOrchestrator.ts`, and `app/src/lib/stores/nodeTree.svelte.ts`
- [ ] T032 [US3] Surface staged repair summaries and unknown daughterboard warnings in `app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte`, `app/src/lib/components/ElementCardDeck/SaveControls.svelte`, and `app/src/lib/components/ElementCardDeck/saveControlsPresenter.ts`

**Checkpoint**: Connector changes automatically stage compatible follow-up edits and show them before apply without requiring manual repair knowledge.

---

## Phase 6: User Story 4 - Preserve Current Behavior For Non-Modular Boards (Priority: P3)

**Goal**: Ensure nodes without connector-slot metadata behave exactly as they do today.

**Independent Test**: Open a node with no connector-slot profile metadata and confirm there is no connector-selection UI or altered configuration behavior.

### Tests for User Story 4

- [ ] T033 [P] [US4] Add regression tests for nodes without connector-slot metadata in `app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts`, `app/src/lib/stores/nodeTree.store.test.ts`, and `app/src-tauri/src/commands/cdi.rs`

### Implementation for User Story 4

- [ ] T034 [US4] Guard backend and frontend connector flows so non-modular nodes keep pre-feature behavior in `app/src-tauri/src/commands/cdi.rs`, `app/src-tauri/src/profile/mod.rs`, `app/src/lib/components/ConfigSidebar/configSidebarPresenter.ts`, and `app/src/lib/stores/connectorSelections.svelte.ts`

**Checkpoint**: Nodes without connector metadata remain unchanged while modular nodes keep the new behavior.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final docs, validation, and shared cleanup across the completed stories.

- [ ] T035 [P] Document connector-slot profile authoring and saved hardware-selection behavior in `docs/technical/profile-extraction-guide.md` and `docs/user/using.md`
- [ ] T036 [P] Add quickstart-aligned regression coverage for one Tower carrier and one Signal carrier in `app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts`, `app/src/lib/components/ElementCardDeck/ElementCardDeck.test.ts`, and `app/src-tauri/src/profile/mod.rs`
- [ ] T037 Run the feature quickstart validation and update follow-up work in `specs/011-daughterboard-selection/quickstart.md` and `specs/backlog.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies; can start immediately.
- **Foundational (Phase 2)**: Depends on Setup completion and blocks all user story work.
- **User Story 1 (Phase 3)**: Starts after Foundational and establishes the selection/persistence base used by later stories.
- **User Story 2 (Phase 4)**: Depends on User Story 1 selection plumbing plus Foundational profile metadata support.
- **User Story 3 (Phase 5)**: Depends on User Story 2 filtering and the User Story 1 persisted selection flow.
- **User Story 4 (Phase 6)**: Depends on Foundational work and should be completed after the modular-board behavior is in place so regressions are proven explicitly.
- **Polish (Phase 7)**: Depends on the user stories you intend to ship.

### User Story Dependencies

- **US1**: No dependency on other stories after Foundational; this is the MVP.
- **US2**: Depends on US1 because filtering needs the persisted active connector-selection flow.
- **US3**: Depends on US1 and US2 because repair staging needs both selection state and constraint evaluation.
- **US4**: Independent of story-specific functionality after Foundational, but best validated after US1-US3 changes land.

### Parallel Opportunities

- T002 and T003 can run in parallel during Setup.
- T004, T005, T006, T007, T009, and T010 can run in parallel during Foundational work once command and type ownership is agreed.
- In US1, T011, T012, and T013 can run in parallel before implementation; T015 and T017 can proceed in parallel after T014 establishes the payload shape.
- In US2, T020 and T021 can run in parallel; T022 and T023 can proceed in parallel before T024 and T025 integrate filtering into stores and UI.
- In US3, T027 and T028 can run in parallel; T029 and T030 can proceed in parallel before T031 and T032 integrate the staged repair flow.
- T035 and T036 can run in parallel during Polish.

---

## Parallel Example: User Story 1

```bash
# Parallel test work before US1 implementation
Task: T011 Add backend command tests for connector profile loading and connector-selection persistence in app/src-tauri/src/commands/connector_profiles.rs
Task: T012 Add store and layout orchestration tests for connector-selection load/save behavior in app/src/lib/stores/layout.svelte.test.ts and app/src/lib/orchestration/offlineLayoutOrchestrator.test.ts
Task: T013 Add ConfigSidebar rendering tests for connector slot selectors in app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts and app/src/lib/components/ConfigSidebar/configSidebarPresenter.test.ts

# Parallel implementation work after the backend payload shape exists
Task: T015 Implement saved layout/project persistence for per-node connector selections in app/src-tauri/src/layout/types.rs, app/src-tauri/src/commands/connector_profiles.rs, and app/src/lib/api/connectorProfiles.ts
Task: T017 Implement connector-slot presenter logic and selector UI in app/src/lib/components/ConfigSidebar/configSidebarPresenter.ts, app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte, and app/src/lib/components/ConfigSidebar/ConnectorSlotSelector.svelte
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational.
3. Complete Phase 3: User Story 1.
4. Validate the US1 independent test before expanding scope.

### Incremental Delivery

1. Ship US1 to establish connector selection and persistence.
2. Add US2 to enforce daughterboard-aware filtering once the selection flow is stable.
3. Add US3 to auto-stage compatible repairs after the filtering rules are trustworthy.
4. Finish with US4 regression protection and polish.

### Suggested Task Count Summary

- **Setup**: 3 tasks
- **Foundational**: 7 tasks
- **US1**: 9 tasks
- **US2**: 7 tasks
- **US3**: 6 tasks
- **US4**: 2 tasks
- **Polish**: 3 tasks
- **Total**: 37 tasks
