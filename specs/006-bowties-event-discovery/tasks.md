# Tasks: Bowties Tab — Discover Existing Connections

**Feature**: `006-bowties-event-discovery`
**Input**: `specs/006-bowties-event-discovery/` — plan.md, spec.md, data-model.md, research.md, contracts/
**Branch**: `006-bowties-event-discovery`

---

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no incomplete-task dependencies)
- **[US1/US2/US3]**: User story this task belongs to
- **No label**: Setup or foundational phase task

---

## Phase 2: Foundational — lcc-rs CDI Infrastructure

**Purpose**: Core Rust types and CDI tree changes that block all user story implementation. Must be complete before any Phase 3+ work.

**⚠️ All three user stories depend on the EventRole enum and AppState additions.**

- [X] T001 [P] Write unit tests for `classify_event_slot` in `lcc-rs/src/cdi/role.rs` — **write first, verify they fail before T002**: test Tier 1 keyword matches ("Producers" parent → Producer, "Consumers" parent → Consumer), Tier 2 description phrase matches ("Generated when…" → Producer, "When this event…" → Consumer), no-match case → Ambiguous, empty inputs → Ambiguous (constituting III: unit test every public lcc-rs function)
- [X] T002 [P] Create `lcc-rs/src/cdi/role.rs`: `EventRole` enum (Producer/Consumer/Ambiguous) + `classify_event_slot(element: &EventIdElement, parent_group_names: &[&str]) -> EventRole` with two-tier heuristic (Tier 1: parent group name keywords; Tier 2: `<description>` phrase patterns — see research.md RQ-3) — make T001 tests pass
- [X] T003 [P] Update `lcc-rs/src/cdi/hierarchy.rs` to collect ancestor `<group><name>` strings during EventId slot traversal and pass them to the caller (needed to supply `parent_group_names` to `classify_event_slot`)
- [X] T004 Update `lcc-rs/src/cdi/mod.rs` to re-export `EventRole` and expose the updated walk API that includes ancestor group names (depends on T002 + T003)
- [X] T005 [P] Update `app/src-tauri/src/state.rs`: add `NodeRoles` struct (`producers: HashSet<String>`, `consumers: HashSet<String>`), and add `bowties_catalog: Arc<RwLock<Option<BowtieCatalog>>>` and `event_roles: Arc<RwLock<HashMap<String, NodeRoles>>>` fields to `AppState` with `Default` init

**Checkpoint**: `cargo check` passes on lcc-rs; AppState compiles with new fields.

---

## Phase 3: User Story 1 — View Existing Connections (Priority: P1) 🎯 MVP

**Goal**: After CDI reads complete, user sees a bowtie card for every event ID shared across at least one producer slot and one consumer slot on any discovered nodes.

**Independent Test**: Connect to a live LCC network with ≥2 nodes sharing an event ID. Wait for the Bowties tab to become enabled. Open it. Verify a bowtie card appears with the correct producer and consumer element entries (node name + element label).

- [X] T006 [P] [US1] Write unit tests for `build_bowtie_catalog` in `app/src-tauri/src/commands/bowties.rs` — **write first**: (a) two nodes share event ID, one producer + one consumer → 1 BowtieCard, ambiguous_entries empty; (b) two producers + one consumer on three nodes → 1 card with 2 producers; (c) node replies both ProducerIdentified + ConsumerIdentified, heuristic resolves → classified correctly; (d) node replies both, heuristic inconclusive → entry in ambiguous_entries; (e) event ID only on producers → excluded; (f) zero nodes → empty catalog; (g) same event ID = exactly 1 card (SC-002)
- [X] T007 [P] [US1] Write integration test skeleton for `get_bowties` Tauri command in `app/src-tauri/tests/bowties_integration.rs` — **write first**: test returns `Ok(None)` before any CDI read; returns `Ok(Some(BowtieCatalog))` after build (constitution III: all Tauri commands must have integration tests)
- [X] T008 [P] [US1] Create `app/src-tauri/src/commands/bowties.rs` with `#[derive(Serialize, Deserialize)]` structs: `EventSlotEntry` (node_id, node_name, element_path, element_label, event_id: [u8;8], role: EventRole — MUST be Producer or Consumer only; Ambiguous entries go to ambiguous_entries on BowtieCard, never here) and `BowtieCard` (event_id_hex, event_id_bytes, producers, consumers, ambiguous_entries, name: Option\<String\>) and `BowtieCatalog` (bowties sorted by event_id_bytes, built_at, source_node_count, total_slots_scanned)
- [X] T009 [US1] Implement `build_bowtie_catalog(nodes: &[DiscoveredNode], event_roles: &HashMap<[u8;8], NodeRoles>) -> BowtieCatalog` in `app/src-tauri/src/commands/bowties.rs` — make T006 tests pass: walk each node's CDI event slots; apply NodeRoles to classify cross-node slots as Producer/Consumer; apply `classify_event_slot` for same-node cases; group entries by event_id_bytes; add `debug_assert!(entry.role != EventRole::Ambiguous)` before pushing to producers/consumers; emit BowtieCard only when ≥1 confirmed producer AND ≥1 confirmed consumer; sort by event_id_bytes; guarantee one card per unique event_id_bytes (FR-002, FR-011, SC-002, FR-010)
- [X] T010 [US1] Implement `query_event_roles(node_ids: &[[u8;8]], send_delay_ms: u64, collect_window_ms: u64, state: &AppState) -> HashMap<[u8;8], NodeRoles>` in `app/src-tauri/src/commands/bowties.rs`: send `IdentifyEventsAddressed` (MTI 0x0488) to each node with 125 ms between sends; collect `ProducerIdentified` (MTI 0x0544/0x0545/0x0547) and `ConsumerIdentified` (MTI 0x04C4/0x04C5/0x04C7) replies for 500 ms after last send; ignore EventState field in replies (research.md RQ-11)
- [X] T011 [US1] Implement `get_bowties` Tauri command in `app/src-tauri/src/commands/bowties.rs` (returns `Ok(Option<BowtieCatalog>)` from AppState); update `app/src-tauri/src/commands/cdi.rs` to call `query_event_roles` → `build_bowtie_catalog` → store on AppState → emit `cdi-read-complete` event with `CdiReadCompletePayload { catalog, node_count }` when `node_index + 1 == total_nodes` — make T007 integration tests pass (contracts/tauri-commands.md)
- [X] T012 [US1] Register `get_bowties` in `app/src-tauri/src/commands/mod.rs` and add to the Tauri builder's `invoke_handler`
- [X] T013 [P] [US1] Add `getBowties(): Promise<BowtieCatalog | null>` wrapper and `EventSlotEntry`, `BowtieCard`, `BowtieCatalog`, `CdiReadCompletePayload` TypeScript types to `app/src/lib/api/tauri.ts` (mirror of contracts/frontend-types.ts)
- [X] T014 [P] [US1] Create `app/src/lib/stores/bowties.ts`: `bowtieCatalogStore: Writable<BowtieCatalog | null>` and `cdiReadCompleteStore: Writable<boolean>`; register `listen<CdiReadCompletePayload>('cdi-read-complete', ...)` listener that sets both stores on receipt
- [X] T015 [P] [US1] Write Vitest tests for `BowtieCard.svelte` in `app/src/lib/components/Bowtie/BowtieCard.test.ts` — **write first**: renders correct card header (name if present, event_id_hex if not); renders producer column entries; renders consumer column entries; renders ambiguous_entries section when present; hides ambiguous_entries section when empty (FR-014, FR-004, FR-002)
- [X] T016 [P] [US1] Create `app/src/lib/components/Bowtie/ElementEntry.svelte`: accepts `entry: EventSlotEntry` prop; renders node_name (bold) and element_label (secondary text); used in both producer column and consumer column
- [X] T017 [P] [US1] Create `app/src/lib/components/Bowtie/ConnectorArrow.svelte`: accepts `eventIdHex: string` prop; renders a right-pointing arrow (→) with the event ID displayed beneath it (FR-005)
- [X] T018 [US1] Create `app/src/lib/components/Bowtie/BowtieCard.svelte` — make T015 tests pass: accepts `card: BowtieCard` prop; renders card header (`card.name ?? card.event_id_hex`, FR-014), three-column layout (producers column of ElementEntry components | ConnectorArrow | consumers column of ElementEntry components, FR-004); render ambiguous_entries section if non-empty ("Unknown role — needs clarification")
- [X] T019 [US1] Create `app/src/routes/bowties/+page.svelte`: subscribe to bowtieCatalogStore and cdiReadCompleteStore; render scrollable list of BowtieCard components when catalog data present (FR-003, FR-010); register the Bowties route in the main layout nav with tab disabled (non-navigable, greyed label) until cdiReadCompleteStore is true (FR-013)

**Checkpoint**: With a live network connection and completed CDI reads, the Bowties tab becomes enabled and shows bowtie cards. User Story 1 is independently functional.

---

## Phase 4: User Story 2 — Empty Tab Guidance (Priority: P2)

**Goal**: When no bowties are found, the tab shows an illustration + message + CTA instead of a blank screen.

**Independent Test**: Connect with nodes that have no shared event IDs (factory defaults / no matching pairs). Wait for CDI reads to complete. Open Bowties tab. Verify empty-state illustration and "No connections yet" message are shown.

- [X] T020 [P] [US2] Write Vitest tests for `EmptyState.svelte` in `app/src/lib/components/Bowtie/EmptyState.test.ts` — **write first**: illustration placeholder renders; message text "No connections yet" is present; "+ New Connection" button is disabled/inert (FR-006, FR-012)
- [X] T021 [P] [US2] Create `app/src/lib/components/Bowtie/EmptyState.svelte` — make T020 tests pass: centred illustration placeholder, message "No connections yet — click + New Connection to link a producer to a consumer", disabled "+ New Connection" button (button inert in this phase per FR-012)
- [X] T022 [US2] Update `app/src/routes/bowties/+page.svelte` to branch on `catalog.bowties.length === 0`: render `<EmptyState />` when empty, card list when non-empty (FR-006, SC-004)

**Checkpoint**: Bowties tab shows EmptyState when no connections exist and shows cards when connections exist. User Story 2 is independently functional.

---

## Phase 5: User Story 3 — Configuration Cross-Reference (Priority: P3)

**Goal**: Event slots in the Configuration tab show "Used in: [name]" links that navigate to the corresponding bowtie card.

**Independent Test**: Navigate to a node event slot in the Configuration tab whose event ID is part of a discovered bowtie. Verify "Used in: [event_id_hex]" link is present. Click it. Verify Bowties tab opens and scrolls to/highlights the matching card.

- [X] T023 [US3] Add `usedInMap: Readable<Map<string, BowtieCard>>` derived store to `app/src/lib/stores/bowties.ts`: derives from `bowtieCatalogStore`; maps each `event_id_hex` (and all 8-byte key representations) to its `BowtieCard` for O(1) lookup (FR-008, research.md RQ-10)
- [X] T024 [P] [US3] Update `app/src/lib/components/ElementCardDeck/EventSlotRow.svelte`: add optional `usedIn: BowtieCard | undefined` prop; when present, render "Used in: [usedIn.name ?? usedIn.event_id_hex]" as a navigable anchor that calls `goto('/bowties?highlight=' + usedIn.event_id_hex)` (FR-008, FR-009)
- [X] T025 [US3] Update the parent component that renders `EventSlotRow` (in the config/segment view) to subscribe to `usedInMap` and pass the matching `BowtieCard` (keyed on the slot's current event ID value) as the `usedIn` prop (SC-005)
- [X] T026 [US3] Update `app/src/routes/bowties/+page.svelte` to read the `highlight` query parameter on mount; scroll to and visually highlight the matching `BowtieCard` component (e.g., CSS ring/outline) when the parameter is present (FR-009)

**Checkpoint**: "Used in" link appears on matching event slots; clicking navigates and highlights. User Story 3 is independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T027 [P] Audit `build_bowtie_catalog` for completeness: confirm element_label priority (CDI `<name>` → `<description>` first sentence → slash-joined path per research.md RQ-12); confirm node_name priority (SNIP user_name → "{mfg} — {model}" → node_id per data-model.md); confirm no card emitted without ≥1 confirmed producer AND ≥1 consumer (FR-002); log catalog build duration at INFO level to facilitate SC-001 manual verification
- [X] T028 [P] Verify FR-013 tab disabled styling in the SvelteKit layout nav: confirm the Bowties tab has a visually distinct disabled style (greyed label, `aria-disabled`, no pointer events) until `cdiReadCompleteStore` is true; confirm it enables after the first `cdi-read-complete` event; manually smoke-test SC-001 (≤5 s build) and SC-004 (≤1 s empty state) on a representative network to confirm timing thresholds are met

---

## Dependencies

```
T001 (tests) ──▶ T002 (role.rs impl)
               ──────────┬
T003 (hierarchy) ─────────┼──▶ T004 (mod.rs) ──▶ T009 (catalog impl, makes T006 pass)
T005 (state.rs)  ─────────┘     T006 (catalog tests) ──▶ T009
                                T007 (integration test) ──▶ T011 (get_bowties impl)
                          T008 (structs) ──▶ T009
                          T009 ──▶ T010 ──▶ T011 ──▶ T012  (US1 backend complete)

T013 ──┬
T014 ──┤  (start after T008 types exist)
T015 ──┬  (tests for BowtieCard)
T016 ──┤
T017 ──┴──▶ T018 (BowtieCard impl, makes T015 pass) ──▶ T019 (page)  (US1 frontend complete)

T019 exists ──▶ T020 (EmptyState tests) ──▶ T021 ──▶ T022  (US2 complete)

T014 (stores) ──▶ T023 (usedInMap)
T023 ──┬
T024 ──┴ ──▶ T025 ──▶ T026  (US3 complete)

All US1–US3 done ──▶ T027, T028
```

**US2 unblocks after T019** (needs the Bowties route page to exist).  
**US3 unblocks after T014** (needs bowties.ts store file to add usedInMap) and after T019 (needs the Bowties route for highlight navigation).  
**T009 is the hardest task** — it integrates lcc-rs hierarchy walk (T002–T004), EventRole classification (T002), and NodeRoles protocol map (T010 output) in one builder function.

---

## Parallel Execution Examples

### Phase 2 — T001/T003 parallelisable:
```
T001 (classify tests)  ──▶ T002 (role.rs impl)  ──┬
T003 (hierarchy)                                  ├──▶ T004 (mod.rs)
T005 (state.rs)                                   ┘  (independent)
```

### US1 backend — after T004 is done:
```
T006 (catalog tests)    ──┬
T007 (integration test) ──┤
T008 (structs)          ──┘──▶ T009 (catalog impl, makes T006 pass)
T009 ──▶ T010 (query_event_roles) ──▶ T011 (get_bowties, makes T007 pass) ──▶ T012
```
T006, T007, T008 can all be written in parallel (different functions/files).

### US1 frontend — after T008 types are defined:
```
T013 (tauri.ts types)   ──┬
T014 (stores)           ──┤  all parallel, different files
T015 (BowtieCard tests) ──┤
T016 (ElementEntry)     ──┤
T017 (ConnectorArrow)   ──┘
```
Then T018 (BowtieCard.svelte, makes T015 pass) → T019 (page).

### US2 — after T019 page file exists:
```
T020 (EmptyState tests) ──▶ T021 (EmptyState.svelte, makes T020 pass)
T022 (page branch)          edit to existing file, parallel with T021
```

### US3 — after T014 and T019:
```
T023 (usedInMap store)   ──┬
T024 (EventSlotRow prop) ──┘ parallel, different files
```
Then T025 (pass prop) → T026 (highlight navigation).

---

## Implementation Strategy

**MVP = Phase 2 + Phase 3 (US1 only)**: The foundational CDI role infrastructure plus the main bowtie discovery pipeline delivers the entire "inspect what I have" value. Users with a live configured layout will see their connections immediately.

**Increment 2 = Phase 4 (US2)**: EmptyState guidance — small but important UX completeness for new users.

**Increment 3 = Phase 5 (US3)**: Cross-reference links — closes the Configuration ↔ Bowties tab gap; depends on US1 catalog being populated.

**Hardest task**: T009 (`build_bowtie_catalog`) — integrates lcc-rs CDI walk (T002–T004), EventRole classification (T002), and NodeRoles protocol map (T010 output). Write T006 unit tests first against a mock `NodeRoles` map to verify grouping logic before wiring in the real query.

**Protocol implementation note (T010)**: Pattern from JMRI `EventTablePane.sendRequestEvents` — send `IdentifyEventsAddressed` per node (not per event ID), 125 ms between sends. The Rust async equivalent is `tokio::time::sleep(Duration::from_millis(125))` between each addressed send in a loop over `AppState.nodes`.

---

## Task Count Summary

| Phase | Tasks | Tests | User Story |
|---|---|---|---|
| Phase 2: Foundational | 5 (T001–T005) | T001 | — |
| Phase 3: US1 View Connections | 14 (T006–T019) | T006, T007, T015 | US1 (P1) |
| Phase 4: US2 Empty Guidance | 3 (T020–T022) | T020 | US2 (P2) |
| Phase 5: US3 Cross-Reference | 4 (T023–T026) | — | US3 (P3) |
| Phase 6: Polish | 2 (T027–T028) | — | — |
| **Total** | **28** | **5 test tasks** | |

**Parallel opportunities identified**: 14 tasks marked [P].  
**MVP scope**: T001–T019 (19 tasks — Phases 2 + 3).  
**TDD compliance**: T001 → T002, T006 → T009, T007 → T011, T015 → T018, T020 → T021 (write test first, verify it fails, then implement).
