# Tasks: Profile Schema, Event Roles, and Conditional Relevance

**Input**: `specs/008-guided-configuration/` — plan.md, spec-phase2.md, data-model-phase2.md, contracts/
**Branch**: `008-guided-configuration`
**Spec**: [spec-phase2.md](spec-phase2.md) | **Plan**: [plan.md](plan.md)

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no incomplete-task dependencies)
- **[Story]**: User story label (US1–US5) — present only on user story phase tasks
- Tests are included per the TDD MANDATORY gate in the constitution check

---

## Phase 1: Setup

**Purpose**: Add the new dependency, declare the module, create directory skeleton. Unblocks all subsequent Rust work.

- [X] T001 Add `serde_yaml_ng = "0.10"` to `[dependencies]` in `app/src-tauri/Cargo.toml` and verify `cargo check` passes
- [X] T002 [P] Create empty module files `app/src-tauri/src/profile/mod.rs`, `types.rs`, `loader.rs`, `resolver.rs`
- [X] T003 [P] Add `mod profile;` declaration to `app/src-tauri/src/lib.rs`

**Checkpoint**: `cargo check` passes with no errors.

---

## Phase 2: Foundational — Profile Module Core

**Purpose**: Implement the full Rust profile module (types, resolver, loader, public API), hook it into `get_node_tree`, and configure Tauri resource bundling. MUST be complete before any user story work begins.

⚠️ **CRITICAL**: No user story work can begin until this phase is complete.

- [X] T004 Implement all profile types per data-model-phase2.md §1–3 in `app/src-tauri/src/profile/types.rs`: `StructureProfile`, `ProfileNodeType`, `FirmwareVersionRange`, `EventRoleDecl`, `ProfileEventRole`, `RelevanceRule`, `RelevanceCondition`, `RelevanceAnnotation`, and `impl From<ProfileEventRole> for lcc_rs::cdi::EventRole`
- [X] T005 [P] Implement `profile/resolver.rs`: `resolve_profile_paths(profile, cdi) -> ProfilePathMap` converting `#N`-ordinal name paths to index-based tree paths per data-model-phase2.md §5
- [X] T006 Implement `profile/loader.rs`: `load_profile` async fn — check user data dir first (`{app_data_dir}/profiles/{Manufacturer}_{Model}.profile.yaml`), fall back to bundled resource, parse with `serde_yaml_ng::from_str`, log warning and return `None` on parse failure (FR-006), validate `schemaVersion == "1.0"` per contracts/backend-profile-module.md
- [X] T007 [P] Add `profiles: ProfileCache` (`Arc<RwLock<HashMap<ProfileKey, Option<StructureProfile>>>>`) to `AppState` in `app/src-tauri/src/state.rs` and initialize as empty `HashMap` in `AppState::new()`
- [X] T008 Implement `profile/mod.rs` public API: re-export `load_profile`, `annotate_tree` stub (returns empty `AnnotationReport`), `ProfileCache`, `ProfileKey`, `make_profile_key`, `AnnotationReport` struct
- [X] T009 Wire `load_profile` + `annotate_tree` call into `get_node_tree` in `app/src-tauri/src/commands/cdi.rs` after the existing `merge_event_roles` call, using the integration snippet from contracts/backend-profile-module.md; call `annotate_tree` AFTER `merge_event_roles` so profile roles take precedence
- [X] T010 [P] Add `profiles/` directory to `bundle.resources` in `app/src-tauri/tauri.conf.json` per contracts/backend-profile-module.md Tauri Config section; create `app/src-tauri/profiles/` directory with a placeholder `.gitkeep`

**Checkpoint**: `cargo build` succeeds. `get_node_tree` compiles with the new hook (annotate_tree is a no-op stub until US1/US2 phases complete). Profile loading infrastructure is in place.

---

## Phase 3: User Story 1 — Accurate Event Role Labels (Priority: P1) 🎯 MVP

**Goal**: Profile-declared event roles override heuristic role assignments on Tower-LCC event group leaves, so PRODUCER/CONSUMER badges in the configuration view match manufacturer documentation.

**Independent Test**: Connect a Tower-LCC node. Without any hardware changes, verify that every event group in the Port I/O segment displays the correct PRODUCER or CONSUMER badge as declared in the Tower-LCC profile. Connect a non-Tower-LCC node and verify its badges are unchanged (heuristic still runs).

### Tests for User Story 1

- [X] T011 [P] [US1] Write unit test `annotate_tree_applies_event_roles` in `app/src-tauri/src/profile/mod.rs`: build a minimal `NodeConfigTree` with two event leaf groups, call `annotate_tree` with a profile declaring opposite roles, assert both leaves have the declared role
- [X] T012 [P] [US1] Write unit tests `load_profile_parses_valid_yaml`, `load_profile_returns_none_for_invalid_yaml`, `load_profile_returns_none_for_missing_file` in `app/src-tauri/src/profile/loader.rs` using fixture YAML strings / temp file paths
- [X] T013 [P] [US1] Write unit tests `resolve_profile_paths_basic` and `resolve_profile_paths_ordinal_suffix` in `app/src-tauri/src/profile/resolver.rs`: assert name-path `"Port I/O/Line/Event#1"` maps to the correct index-based path from a Tower-LCC CDI fixture

### Implementation for User Story 1

- [X] T014 [US1] Implement event role application loop in `annotate_tree` in `app/src-tauri/src/profile/mod.rs`: for each `EventRoleDecl`, resolve path via `ProfilePathMap`, walk matching `GroupNode`s and all replicated instances, set `leaf.event_role = Some(decl.role.into())` on every `LeafType::EventId` leaf; log warning and skip on unresolved path (FR-012)
- [X] T015 [US1] Author Tower-LCC event role declarations from `profiles/tower-lcc/event-roles.json` into `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml`: create the file with `schemaVersion: "1.0"`, `nodeType` block, and all `eventRoles` entries (leave `relevanceRules: []` for now); run `cargo test` to confirm `load_profile_parses_valid_yaml` passes with this file

**Checkpoint**: Event role badges on Tower-LCC event groups show PRODUCER/CONSUMER per the profile. Non-Tower-LCC nodes are unaffected. All US1 unit tests pass.

---

## Phase 4: User Story 2 — Irrelevant Configuration Sections Are Visually Suppressed (Priority: P1)

**Goal**: Profile relevance rules annotate `GroupNode`s. The frontend evaluates those annotations reactively against the controlling field's current value and marks the affected groups (or picker items) as "not applicable" with the profile's verbatim explanation banner.

**Independent Test**: Load a Tower-LCC node, set Output Function on Line 1 to "No Function" (value 0). Verify consumer event items in the Event picker are muted/not-applicable, and that selecting one shows the exact explanation text from the profile. Set Output Function to "Pulse Active Hi" and verify the banner disappears and items are no longer marked. Verify producer items are never affected by the Output Function rule.

### Tests for User Story 2

- [ ] T016 [P] [US2] Write unit test `annotate_tree_skips_multi_condition_rules` in `app/src-tauri/src/profile/mod.rs`: provide a rule with `allOf` length 2, assert it is skipped (zero rules applied) and a warning string is returned
- [ ] T017 [P] [US2] Write unit test `annotate_tree_skips_unknown_path` in `app/src-tauri/src/profile/mod.rs`: provide a rule whose `affectedGroupPath` does not resolve in the CDI, assert it is skipped with a warning
- [ ] T018 [P] [US2] Write unit test `resolve_profile_paths_roundtrip` (property test) in `app/src-tauri/src/profile/resolver.rs`: for every named group in the Tower-LCC CDI fixture, assert that the name-path round-trips through resolve and back to the same leaf count- [ ] T018a [P] [US2] Write Vitest test `TreeGroupAccordion_relevance_reactive` in `app/src/lib/components/ElementCardDeck/TreeGroupAccordion.test.ts`: mount a `TreeGroupAccordion` with a `GroupConfigNode` carrying a `relevanceAnnotation`; set the controlling field value in `pendingEditsStore` to a value in `irrelevantWhen`; assert the group is rendered with `isIrrelevant === true`, the explanation banner DOM node is present (both when collapsed AND when manually expanded), and the CSS transition class is applied; then change the store value outside `irrelevantWhen` and assert `isIrrelevant === false` and banner is absent (FR-009, FR-010, FR-011)
- [ ] T018b [P] [US2] Write Vitest test `TreeGroupAccordion_picker_item_muted` in `app/src/lib/components/ElementCardDeck/TreeGroupAccordion.test.ts`: mount a `TreeGroupAccordion` in pill-selector mode with a replicated group where instances 0–5 carry a `relevanceAnnotation` and instances 6–11 do not; fire the relevance condition; assert that only instances 0–5 receive muted styling and that selecting a muted item renders the explanation banner beneath the picker; assert that instances 6–11 have neither muted class nor banner (FR-009)
### Implementation for User Story 2

- [ ] T019 [US2] Add `relevance_annotation: Option<RelevanceAnnotation>` field to `GroupNode` struct in `app/src-tauri/src/node_tree.rs`; add `pub use crate::profile::types::RelevanceAnnotation;` at the top of `node_tree.rs`; confirm `cargo check` passes (backward-compatible addition)
- [ ] T020 [US2] Implement relevance rule evaluation loop in `annotate_tree` in `app/src-tauri/src/profile/mod.rs`: skip rules with `all_of.len() != 1` with log warning (FR-009a); resolve `affected_group_path`; locate the controlling sibling leaf; build `RelevanceAnnotation` with field path, address, space, `irrelevant_when` values, and verbatim `explanation`; set `group.relevance_annotation` on all matching groups
- [ ] T021 [US2] Add `RelevanceAnnotation` TypeScript interface and update `GroupConfigNode` with `relevanceAnnotation: RelevanceAnnotation | null` in `app/src/lib/types/nodeTree.ts` per contracts/group-node-updated.md
- [ ] T022 [US2] Add reactive relevance evaluation to `TreeGroupAccordion.svelte` in `app/src/lib/components/ElementCardDeck/`: derive `controllingValue` from `pendingEditsStore` keyed by the annotation's `controllingFieldAddress + space`; derive `isIrrelevant: boolean` from `controllingValue ∈ irrelevantWhen`; apply collapsed-by-default state and explanation banner when `isIrrelevant` is true; use ≈200ms CSS transition; banner text MUST use `relevanceAnnotation.explanation` verbatim (FR-009, FR-010, FR-011)
- [ ] T023 [US2] Add "not applicable" muted treatment for picker items in `TreeGroupAccordion.svelte`: when in pill-selector mode, mark individual picker items that belong to an `isIrrelevant` group with muted styling; show explanation banner beneath picker when user selects a marked item; if ALL picker items are covered by the same fired rule, collapse the entire picker section as a standalone section (FR-009)
- [ ] T024 [US2] Author Tower-LCC relevance rules from `profiles/tower-lcc/relevance-rules.json` into `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml`: add all three rules (consumer events controlled by Output Function, producer events controlled by Input Function, Delay group controlled by Output Function) with verbatim `explanation` text from extraction output

**Checkpoint**: Relevance suppression works end-to-end for Tower-LCC Lines. Consumer event picker items are muted when Output Function = 0. Producer items are unaffected. Explanation banner text matches the profile exactly. Reactive update ≤ 200ms. All US2 unit tests pass.

---

## Phase 5: User Story 3 — Profile Loads Automatically by Node Type (Priority: P2)

**Goal**: Confirm the acceptance scenarios for automatic profile discovery: user-placed profiles take precedence over built-in, duplicate built-in profiles are detected, firmware version mismatch is advisory-only.

**Independent Test**: Connect a Tower-LCC on a fresh session — profile is applied immediately without any user action. Place a second (overriding) profile in the user data directory and confirm it takes precedence. Connect a node type with no profile and verify zero profile-related behavior.

- [ ] T025 [US3] Extend `profile/loader.rs` to detect duplicate manifest conflict: if two built-in profiles claim identical `manufacturer + model`, log a warning and store `None` for that key — neither profile is applied (edge case from spec)
- [ ] T026 [US3] Extend `profile/loader.rs` to log an advisory note when loaded profile specifies `firmwareVersionRange` and the connected node's reported firmware falls outside that range (FR-003); note is `eprintln!` only, never blocks profile application or surfaces in UI

**Checkpoint**: `cargo test` passes. On fresh session, Tower-LCC profile applies without user action. User data dir override works. Malformed profiles are silently skipped (SC-005).

---

## Phase 6: User Story 4 — Ambiguous Bowtie Entries Resolved by Profile (Priority: P2)

**Goal**: Profile-declared roles feed `build_bowtie_catalog`'s same-node ambiguity resolution step, moving Tower-LCC slots from `ambiguous_entries` into the correct `producers` / `consumers` list.

**Independent Test**: Open the Bowties tab with a Tower-LCC on the network. Verify zero Tower-LCC slots appear in "Unknown role — needs clarification." Verify a node type with no profile still produces `ambiguous_entries` entries as before.

### Tests for User Story 4

- [X] T027 [P] [US4] Write unit test `build_bowtie_catalog_uses_profile_roles` in `app/src-tauri/src/commands/bowties.rs`: build a minimal catalog with a same-node event ID where one slot has a profile-declared Producer role; assert the slot appears in `producers`, not in `ambiguous_entries`

### Implementation for User Story 4

- [X] T028 [US4] Add optional `profile_group_roles: Option<&HashMap<String, lcc_rs::EventRole>>` parameter to `build_bowtie_catalog` in `app/src-tauri/src/commands/bowties.rs`; in the same-node ambiguity resolution block, check `profile_group_roles` for `"{node_id}:{slot.element_path.join("/")}"` before falling back to heuristic tier; pass profile roles derived from annotated tree when calling `build_bowtie_catalog` from the Tauri command (FR-016, FR-017); **also** update every existing call site of `build_bowtie_catalog` in the codebase to pass `None` as the new `profile_group_roles` argument, and update all existing unit tests in `commands/bowties.rs` (T006/T009 from spec 006) to supply `None` so they continue to compile and pass

**Checkpoint**: Zero Tower-LCC entries in `ambiguous_entries` when profile is active. Non-Tower-LCC nodes unchanged. `build_bowtie_catalog_uses_profile_roles` test passes.

---

## Phase 7: User Story 5 — Profile File Format Enables Community Authoring (Priority: P3)

**Goal**: Deliver the CDI template generator script and verify the profile-7-assemble skill produces a profile that passes schema validation, enabling community members to author profiles without code knowledge.

**Independent Test**: Run the generator script against the Tower-LCC CDI XML and confirm it outputs a valid (empty) `.profile.yaml` skeleton. Manually fill in two event role entries and place the file in the user data directory; confirm Bowties applies those roles on next launch.

- [ ] T029 [P] [US5] Write `scripts/cdi-template-generator/generate-profile-template.py` (note: subdirectory per plan.md §Project Structure): reads a CDI XML file path as argument, walks all CDI `<group>` elements containing `<eventid>` leaves, outputs an empty `.profile.yaml` skeleton with `schemaVersion: "1.0"`, `nodeType` (manufacturer+model from `<identification>`), and one commented-out `eventRoles` entry per event group (using `#N` ordinal notation); outputs to stdout or `--output` path; also create `scripts/cdi-template-generator/README.md` documenting usage (FR-001 Phase 2A tooling)
- [ ] T030a [US5] Author `.github/skills/profile-7-assemble/SKILL.md`: follow the structure of existing `profile-N` skills (`profile-1-event-roles`, etc.) — define inputs (event-roles.json + relevance-rules.json + CDI XML), step-by-step assembly instructions for producing a `.profile.yaml`, reference to `contracts/profile-yaml-schema.json` for output validation, and example invocation. This skill file must exist before T030 can be verified (FR-001 Phase 2A tooling)
- [ ] T030 [US5] *(after T030a and T015+T024)* Verify `.github/skills/profile-7-assemble/SKILL.md` produces valid output: use the skill against `profiles/tower-lcc/event-roles.json` + `profiles/tower-lcc/relevance-rules.json` + `temp/Tower LCC CDI.xml`, confirm the assembled YAML is complete and passes schema validation against `contracts/profile-yaml-schema.json`; confirm the output is consistent with `RR-CirKits_Tower-LCC.profile.yaml` authored in T015+T024 (validation of skill vs manually-authored artifact — the manually-authored T015+T024 file is canonical)

**Checkpoint**: Generator script runs without errors on the Tower-LCC CDI. profile-7-assemble skill output is valid YAML matching the schema. A community member reading the schema and generator output can author a profile without reading application code.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T031 Write integration test `tower_lcc_profile_parses_without_warnings` in `app/src-tauri/tests/profile_integration.rs`: load `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml` via `load_profile` against the Tower-LCC CDI fixture; assert `AnnotationReport.warnings` is empty (SC-007); assert event role count and rules applied count match expected values from the profile
- [ ] T032 [P] Verify SC-004 manually: connect at least two non-Tower-LCC node types and confirm zero profile-related UI elements are visible (no muted sections, no explanation banners, no badge changes from profile)
- [ ] T033 [P] Verify SC-005 manually: place a malformed `.profile.yaml` in the user data directory, launch the app, confirm normal operation and a warning entry in the application log

**Checkpoint**: All `cargo test` tests pass. All Vitest frontend tests pass. Integration test produces zero warnings against the bundled Tower-LCC profile. Acceptance criteria SC-001 through SC-008 verified.

---

## Dependencies (Story Completion Order)

```
Phase 1 (Setup)
    └─► Phase 2 (Foundational: module core, get_node_tree hook)
            ├─► Phase 3 (US1: event roles) ──────────────────► Phase 8 (Polish)
            ├─► Phase 4 (US2: conditional relevance) ────────► Phase 8 (Polish)
            ├─► Phase 5 (US3: auto loading acceptance) ──────► Phase 8 (Polish)
            ├─► Phase 6 (US4: bowtie resolution) ────────────► Phase 8 (Polish)
            └─► Phase 7 (US5: community authoring) ──────────► Phase 8 (Polish)
```

Phase 3 and Phase 4 are both P1 and can be implemented in parallel once Phase 2 completes (different files: `mod.rs` event role loop vs `node_tree.rs` + `mod.rs` relevance loop + frontend).

Phases 5, 6, 7 are independent of each other and can proceed in parallel after Phase 2.

---

## Parallel Execution Examples

**US1 parallel slots** (after T008 completes):
- T011 (test: annotate event roles) + T012 (test: loader) + T013 (test: resolver) — all different files

**US2 parallel slots** (after T014 completes):
- T016 (test: multi-condition skip) + T017 (test: unknown path skip) + T018 (test: roundtrip) + T018a (Vitest: relevance reactive) + T018b (Vitest: picker muted) — all different files
- T021 (TypeScript types) + T022 (accordion relevance state) — different files, can overlap

**After Phase 2**:
- Phase 5 (T025–T026, Rust only) + Phase 6 (T027–T028, Rust only) + Phase 7 (T029–T030a sequential, then T030) — T030a must precede T030

---

## Implementation Strategy

**Suggested MVP scope**: Phases 1–3 (Setup + Foundational + US1). Delivers correct event role labels for Tower-LCC — the highest-confidence, lowest-risk Phase 2 improvement. US1 is independently verifiable without any frontend changes beyond what already exists (event role field is already present on leaf nodes).

**Incremental delivery**:
1. Phase 1–2: Infrastructure and loading plumbing
2. Phase 3: Event role badges correct on Tower-LCC (observable improvement, no new UI components)
3. Phase 4: Relevance suppression (new UI patterns in TreeGroupAccordion)
4. Phases 5–7: Remaining acceptance scenarios and tooling

---

## Task Count Summary

| Phase | Story | Tasks | Parallel |
|-------|-------|-------|---------|
| 1 Setup | — | 3 | 2 |
| 2 Foundational | — | 7 | 3 |
| 3 US1 Event Role Labels | P1 | 5 | 3 |
| 4 US2 Conditional Relevance | P1 | 11 | 6 |
| 5 US3 Auto Loading | P2 | 2 | 0 |
| 6 US4 Bowtie Resolution | P2 | 2 | 1 |
| 7 US5 Community Authoring | P3 | 3 | 1 |
| 8 Polish | — | 3 | 2 |
| **Total** | | **36** | **18** |
