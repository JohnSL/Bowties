# Implementation Plan: Panel Facility

**Branch**: `021-panel-facility` | **Date**: 2026-11-25 | **Spec**: [spec.md](spec.md)
**Input**: [Feature specification](spec.md) with the 2026-11-25 clarifications (automatic migration, lost blocks keep their entry, no Flash lamp, clear a pin by clicking it again). Mockup: [mockups.html](mockups.html).

## Summary

Replace the one-lamp Block Indicator facility with a **Panel** facility: one facility per physical panel, holding an ordered list of block entries, each with an optional lamp on a Signal LCC Direct Lamp Control row.

The Panel follows the route Automatic Block Signals (Spec 020) established, not the Block Indicator bowtie route:

- The facility carries a **panel record** (ordered entries, "lit when"), replaced wholesale on Done.
- Each lamp is a user-owned **lamp-indicator channel** (style `single-led-direct-lamp`, binding `lampRow`), attached to the facility's `lamps` slot. Blocks are referred to from the record, not claimed.
- A pure **panel compiler** in `bowties-core` turns the record plus board facts into ordinary configuration edits (Lamp Selection, Lamp On, Lamp Off, Lamp Description) and row resets. `facilityOrchestrator` stages them through `configEditor.applyEdit`. Save and the Sync Panel write them (ADR-0001).
- Claimed pins are derived from the panel's lamp channels, never stored.
- Existing Block Indicator facilities are converted at load by a pure function next to `normalize_facility_channel_refs`. It reuses their lamp channels, so nothing is written to boards.

## Technical Context

**Language/Version**: Rust 2021 (bowties-core, app/src-tauri); TypeScript strict; Svelte 5, SvelteKit 2
**Primary Dependencies**: Existing bowties-core, Tauri 2, serde/YAML. No new dependency.
**Storage**: Layout directory: `facilities.yaml` (panel record), `channels.yaml` (lamp channels), manifest version bump (see research R7). Board changes are Pending Changes / Offline Changes as today.
**Testing**: Rust inline unit tests and `bowties-core/tests/` roundtrip tests; Vitest store, orchestrator, component and integration tests; one bench check on a Signal LCC.
**Target Platform**: Existing Windows/macOS/Linux desktop app
**Project Type**: Desktop frontend plus Tauri backend plus Rust domain library
**Performance Goals**: No new target. Compose for a 16-lamp panel is pure and in-memory; the editor stays responsive.
**Constraints**: Signal LCC Direct Lamp Control is the only lamp style. No track diagram. No new write path. Lamp On and Lamp Off are always both set. Migration creates no board writes.
**Scale/Scope**: A panel can span boards; a Signal LCC has 16 lamp rows. Acceptance covers 1, 3, 12 and 24 lamps, one and two boards.

No NEEDS CLARIFICATION remains. The open technical questions are resolved in [research.md](research.md).

## Constitution Check

*Evaluated before research and again after the design below.*

| Principle / gate | Before research | After design |
|---|---|---|
| I. Rust 2021+, typed errors | Pass | Pass: panel compiler returns typed issues, no panics |
| II. Cargo environment | Pass | Pass: no new crate or dependency |
| III. TDD | Pass: required in build | Pass: owner-level tests listed under Verification |
| IV. Protocol correctness | Pass: no protocol change | Pass: writes are ordinary configuration writes through the existing writer; no new emit site in `lcc-rs` |
| V. UX first | Pass: two-step flow from the mockup | Pass: block-first wiring, pin labels as the board prints them, derived status on the card |
| VI. TCP-only | Pass: no transport change | Pass |
| VII. Event management | Pass | Pass: event IDs come from the block's occupancy channel; no manual event entry; removal resets only claimed pins after confirmation |
| Layer ownership | Pass | Pass: compiler in core, facts in backend, staging in `facilityOrchestrator`, editor state in a store, components render and emit intent |

The constitution's TCP-only principle is stale relative to current transports; this feature does not touch transport, so the gate is not affected.

## Project Structure

### Documentation (this feature)

```text
specs/021-panel-facility/
├── spec.md
├── mockups.html
├── plan.md              # this file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   └── tauri-ipc.md     # Phase 1
└── checklists/requirements.md
```

`tasks.md` / `slices.md` come later from `/design` and `/slices`.

### Source Code (existing owners and planned additions)

```text
bowties-core/src/
  behavior_templates.rs          PANEL template replaces BLOCK_INDICATOR in REGISTRY
  layout/facilities.rs           PanelRecord on Facility; SetPanelRecord delta; schema bump
  layout/manifest.rs             LAYOUT_SCHEMA_VERSION 6
  layout/io.rs                   call migrate_block_indicators after load
  layout/panel_migration.rs      NEW pure Block Indicator → Panel conversion
  signaling/outputs.rs           reuse signal_lcc_output_label, owner detection
  signaling/facts.rs, slots.rs   claimed pins include panel lamps; overlay_releases reused
  panel/                         NEW pure panel compiler (compose, removal, status inputs)
  facility_bowties/mod.rs        Block Indicator composition removed after migration lands
app/src-tauri/src/
  panel/ (or signaling/panel.rs) NEW fact gathering + compose/removal commands
  commands/layout_capture.rs     open result carries migration summary
app/src/lib/
  api/panels.ts                  NEW typed IPC
  stores/panelEditor.svelte.ts   NEW editor state (steps, entries, wiring, lit when)
  orchestration/panelEditorOrchestrator.ts   NEW editor workflow (facts, compose check)
  orchestration/facilityOrchestrator.ts      create/update/remove panel; stage edits
  layout/effectiveLayoutStore.svelte.ts      panel status; channel usage from panel records
  components/Panel/              NEW PanelEditor, BlocksStep, WiringStep (pin grid), PanelCard, PanelRemoveDialog
  components/Facilities/         Add facility offers Panel, not Block Indicator
  routes/+page.svelte            surface migration notice, mark layout changed
```

**Structure Decision**: Extend the facilities aggregate with an optional panel record, the same shape as `block_signals`. Reuse the ABS compile/stage/remove route, output owner detection, release overlay and the Wiring pin interaction. Do not add a second write path, a stored claimed-slot list or a generic row-type framework. Module names marked NEW are placements, to be confirmed in `/design`.

## Phase 0: Research

See [research.md](research.md). Decisions:

- R1 Compile to configuration edits (ABS route), not Block Indicator bowties.
- R2 Lamps are lamp-indicator channels in a `lamps` slot; blocks are record references.
- R3 Claimed pins are derived from panel lamp channels.
- R4 The pin grid reuses ABS output facts and the Wiring step's pin interaction.
- R5 "Lit when" picks which occupancy event goes to Lamp On.
- R6 Migration is a pure load-time conversion that reuses lamp channels.
- R7 Manifest version 6 and facilities schema "3.0".
- R8 Status and live lamp state are derived in the frontend facade.
- R9 Block Indicator is removed from the registry; its id stays only for migration.
- R10 Channel usage reads panel and ABS records, so FR-028 holds.

## Phase 1: Design

[data-model.md](data-model.md) defines the panel record, entries, lamps, derived status and the migration mapping. [contracts/tauri-ipc.md](contracts/tauri-ipc.md) defines the wiring-facts, compose and removal commands and the open-result migration summary. [quickstart.md](quickstart.md) is the walkthrough for manual acceptance.

### Flow

1. **Add facility → Panel** (name required) opens `PanelEditor` on Blocks with an empty list.
2. **Blocks** edits the editor store only: add one, add all from an ABS facility (travel order) or detector board (input order), reorder, remove, re-point an entry.
3. **Wiring** asks the backend for wiring facts once per board shown: pins with labels and owners (free, this panel, Bowties elsewhere, outside Bowties with Lamp Description). Pin clicks follow the ABS `seatLamp` rule: a free pin seats, the same pin clears, another entry's pin moves with a message, an outside pin needs Release.
4. **Done** calls `facilityOrchestrator.createPanelFacility` / `updatePanelFacility`:
   - sync drafts, call `compose_panel` with the record and releases,
   - refuse with nothing staged if there are blocking issues,
   - create, rebind, rename or remove lamp channels; set the record; stage edits with `{ facilityId }`; roll back on failure.
5. **Save** writes the layout, then the bus when connected; offline, the edits become Offline Changes.
6. **Remove** shows the pins per board, then `compose_panel_removal` resets those rows; channels and the facility are deleted; blocks remain.

### Migration

At open, `migrate_block_indicators(facilities, channels)` converts every `block-indicator` facility. The open result reports `{ blockIndicators, panels }`. The route shows the notice and marks the layout changed through the facade, so the next Save writes the new form. The converted lamps already hold the right event IDs, so no edits are staged.

### Retired

After migration lands, Block Indicator composition in `facility_bowties` and the slot pickers used only by it (`selectChannelForSlot`, `addChannelForSlot` paths for lamp/block slots) lose their callers. Remove dead code in the slice that makes them dead. Keep `normalize_facility_channel_refs` for slot bindings; it must not drop panel record block references (lost blocks are kept, per FR-029).

## Phase 2: Sequencing and Verification

Order of deliverables; `/design` and `/slices` set the slices.

1. **One-board panel end to end (US1)**: template, record, compiler, wiring facts, editor, Done, Save, bench check with non-sequential pins.
2. **Railroad card (US2)**: one card per panel, entries in order, derived status, live lamp state, waiting changes per board.
3. **Edit and remove (US3)**: rewire, remove entry, re-point entry, remove panel with confirmation; only claimed pins reset.
4. **Migration (US4)**: load-time conversion, notice, marked changed, Add facility no longer offers Block Indicator, retire dead Block Indicator code.
5. **Docs**: glossary (Panel; the "at most one facility slot" rule), `docs/user/using.md`, ADR-0012/0013 dated extensions, aiwiki owners/flows/seams, backlog.

### Automated verification

| Owner | Behavior contracts |
|---|---|
| Core panel compiler | Lamp On/Off from the block's events by "lit when" and override; both always set; description from lamp name; pin moved or dropped is reset; pin claimed elsewhere is a blocking issue; release of outside pins only when named and unclaimed; multi-board edits grouped per node |
| Core migration | One panel per lamp board, named after the board; facility order kept; lamp-less indicators in one separate panel; block-less ones dropped; lamp channels reused and renamed to the former facility name; idempotent; no-indicator layouts unchanged |
| Core layout | SetPanelRecord delta; save/open roundtrip; manifest 6 accepted, newer rejected, version 4/5 opened and migrated; lost block references survive load repair |
| Backend | Wiring facts list 16 labelled pins with owners and Lamp Description; compose performs no writes and no draft mutation |
| Frontend store/orchestrator | Steps and counts; add-all ordering and skip; reorder; seat/clear/move/release pin rules; Done stages nothing on issues; rollback; edit resets only dropped pins; remove resets claimed pins only |
| Frontend facade/components | Status Empty/Incomplete/Wired incl. lost block and missing board; card order and pins; lit/unlit/unknown; Channels "Used by" lists panel and ABS; Add facility offers Panel only; migration notice and dirty flag |

Existing tests to keep green or update deliberately: `facilityOrchestrator.test.ts`, `facilities.svelte.test.ts`, `FacilityCard.test.ts`, `AddFacilityDialog.test.ts`, `channelUsage.test.ts`, `facilityStatus.test.ts`, `RailroadPanel.integration.test.ts`, `WiringStep.test.ts`, `block_signals_layout_roundtrip.rs`. Block Indicator–only tests are rewritten for Panel or migration, not deleted blindly.

Validation during build, per [testing strategy](../../product/quality/testing-strategy.md):

```powershell
cargo test -p bowties-core
cargo test -p app
Push-Location .\app; npm test; npm run check; Pop-Location
cargo clippy -p bowties-core -p app -- -D warnings
```

No write-mode formatter.

### Bench acceptance

- Three blocks wired to #9, #3, #14: each lamp follows its own block (SC-003).
- One block with "lit when" Clear.
- Rewire #6 → #12 and remove a block: old pins reset, ABS facility sharing the blocks unaffected (SC-004).
- Open a layout saved with Block Indicators on two boards: panels appear, no pending board writes, lamps still follow occupancy (SC-005).

## Complexity Tracking

No constitution violation. The manifest bump is required so older Bowties builds refuse layouts whose panel records they would drop.

## Architecture Assessment

**Reviewed**: 2026-11-25. The user accepted every recommendation and added the repository-wide facade cleanup (F8) as the final slice.

### Affected Modules

| Module | Layer | Impact | Notes |
|---|---|---|---|
| `bowties-core/src/behavior_templates.rs` | Core | Modified | `PANEL` replaces `BLOCK_INDICATOR` in `REGISTRY`. The `"block-indicator"` id is kept only as migration input. |
| `bowties-core/src/layout/facilities.rs` | Core | Modified | Optional `panel` record and `SetPanelRecord` delta. Facilities `SCHEMA_VERSION` becomes "3.0". |
| `bowties-core/src/layout/manifest.rs` | Core | Modified | `LAYOUT_SCHEMA_VERSION` 6 |
| `bowties-core/src/layout/mod.rs`, `io.rs` | Core | Modified | One facilities read that normalizes and migrates (F1) |
| `bowties-core/src/layout/panel_migration.rs` | Core | New | Pure conversion from Block Indicator to Panel |
| `bowties-core/src/panel/` | Core | New | Pure compiler: record and board facts in, lamp-row edits, resets and issues out |
| `bowties-core/src/signaling/outputs.rs`, `slots.rs`, `facts.rs` | Core | Modified | Pin label visible within the crate; claimed slots and output attribution include panel lamps (F6) |
| `bowties-core/src/facility_bowties/` + `commands/facility_bowties.rs` | Core / Backend | Deleted in S5 | Only Block Indicator composes bowties (F4) |
| Backend panel commands | Backend | New | `panel_wiring_facts`, `compose_panel`, `compose_panel_removal` |
| `commands/facilities.rs` (`list_facilities`), `layout_capture.rs` | Backend | Modified | Use the single read (F1); the open result carries the migration summary |
| `app/src/lib/orchestration/facilityOrchestrator.ts` | Orchestrator | Modified | Panel create, update and remove next to ABS. Slot pickers and bowtie composition deleted in S5. |
| `app/src/lib/stores/panelEditor.svelte.ts`, `orchestration/panelEditorOrchestrator.ts` | Store / Orchestrator | New | Editor steps, entries, wiring, "lit when" |
| `app/src/lib/utils/` pin-seat rule | Utility | New | Pure rule shared by the Panel and ABS editors (F5) |
| `blockSignalsEditor.svelte.ts`, `BlockSignals/WiringStep.svelte` | Store / Component | Modified | Use the shared rule; share the pin grid only if the markup matches |
| `app/src/lib/layout/effectiveLayoutStore.svelte.ts` | Facade | Modified | Panel status; channel usage from slots and ABS and panel records (F3) |
| `facilities.svelte.ts` | Store | Modified | Panel record draft; "needs save" flag after migration (F2) |
| `components/Panel/*` | Component | New | PanelEditor, BlocksStep, WiringStep, PanelCard, PanelRemoveDialog |
| `components/Facilities/FacilitySlot.svelte` + pickers, `routes/+page.svelte` slot handlers | Component / Route | Deleted in S5 | Block Indicator editing only |
| Components and routes that import edit stores directly | Component / Route | Modified in S6 | Imports go through the `$lib/layout` facade (ADR-0004) |

### Assessment Summary

The panel follows the route Automatic Block Signals established. The panel record is the intent. A pure core compiler turns it into ordinary configuration edits, `facilityOrchestrator` stages them, and Save or the Sync Panel writes them. Lamps are persisted lamp-indicator channels in the `lamps` slot, blocks are referenced from the record, and claimed pins are derived. The Block Indicator bowtie route is retired. Depth comes from one compile call that hides "lit when" mapping, pin ownership, resets and releases. The one owner of "who owns this pin" stays in `signaling/`.

Patterns: facility composes into configuration edits; functional core, imperative shell; derived claims; load-time conversion.

### Findings

**F1: Facilities are read two ways**
- Category: locality / debt
- Affected: `list_facilities`, `read_facilities`, `io.rs` open, `facilityCascadeOrchestrator.reconcileDanglingChannelRefsOnLoad`
- Concern: open normalizes `facilities.yaml`, but the frontend store loads the raw file, so a migration in `io.rs` alone would never reach the UI, and the repair is repeated in the frontend.
- Decision: include in S5. One core read normalizes and migrates; both paths use it. Remove the frontend copy of the repair if it becomes redundant.

**F2: Marking the migrated layout changed**
- Category: ADR-0012
- Affected: facilities store, dirty aggregate, route
- Concern: load repair never sets dirty; `layoutStore.markDirty` has no non-test callers.
- Decision: include option B in S5. Convert on read so the converted form is the baseline. The open result sets a "needs save" flag on the facilities store that feeds the existing aggregate. Option A, staging the conversion as deltas, is rejected: Discard would restore Block Indicators the app can no longer show.

**F3: Channel usage misses record-referenced blocks**
- Category: locality (FR-028)
- Affected: `channelUsageMap`
- Concern: it reads slot bindings only, so blocks used by ABS and panels don't appear.
- Decision: include in S2. One usage function covers slots, ABS records and panel records. Add the missing owners.md entry.

**F4: Retire Block Indicator code**
- Category: YAGNI
- Affected:
  - `compose_bowtie_ops` and its Tauri command
  - `composeBowtiesIfWired` and `tearDownFacilityBowties`
  - `selectChannelForSlot`, `addChannelForSlot` and `removeFromSlot`
  - `FacilitySlot` and its pickers
  - the three route handlers
- Decision: include in S5. Keep the bowtie names Block Indicators created ("{facility} — lit"); their event IDs are unchanged, and they are harmless labels.

**F5: Shared pin rule**
- Category: DRY
- Affected: `blockSignalsEditor.seatLamp`, both Wiring steps
- Decision: include in S1. A pure helper in `utils/` used by both editors. Share the pin grid component only if the markup matches.

**F6: One owner for pin ownership**
- Category: locality / seam
- Affected: `claimed_slots_from_facilities`, `attribute_output_facilities`, `overlay_releases`, `signal_lcc_output_label`
- Decision: include in S1. Panel lamps count as claimed, so ABS won't release them and the reverse. No second owner map.

**F7: Docs disagree with code**
- Category: ADR drift
- Affected:
  - ADR-0012 (L305, 398, 431) and the glossary's Claimed Resource entry say claimed slots are recorded; the code derives them
  - the glossary's "at most one facility slot" rule (L144)
  - ADR-0015 and the Layout In-Memory Persistence seam name `node_registry.saved_trees`; the cache is `saved_trees_for_state`
- Decision: include as wording fixes in the first slice that touches each one. Add a Panel glossary term in S1.

**F8: Direct store imports bypass the layout facade**
- Category: ADR-0004 drift
- Affected: `+page.svelte`, `config/+page.svelte`, `BowtieCatalogPanel`, `ElementPicker`, `ElementCard`, `TreeGroupAccordion`, `SegmentView`, `SaveControls`, `TreeLeafRow`, `EventSlotRow`, `ConfigSidebar`, `configSidebarPresenter`; route reads of raw dirty flags (`+page.svelte:200-201`)
- Decision: include. New Panel code uses only the facade from S1. The repository-wide cleanup is S6.

### ADR invariant audit (2026-11-25)

Unknown is not a pass.

| ADR | Invariant | Status | Evidence |
|---|---|---|---|
| 0004 | `$lib/layout` is the only import surface for components and routes | Drift | `+page.svelte:30-32,86`; `BowtieCatalogPanel.svelte:10-14`; `TreeLeafRow.svelte:18-24`; others in F8 |
| 0004 | Write entry points re-exported by the facade | OK | `layout/index.ts:183-205` |
| 0011 | Dirty breakdown includes every participant | OK | `effectiveNodeStore.svelte.ts:199,224,245` |
| 0011 | Participant dispatch through the registry | OK | `layoutLifecycleOrchestrator.ts:79`; `collectSaveDeltas.ts:37` |
| 0011 | Consumers use only the aggregate dirty flag | Drift | `+page.svelte:200-201` |
| 0012 | Store contract: isDirty, discard, hydrateBaseline, collectDeltas | OK | `facilities.svelte.ts:99,163,173,185` |
| 0012 | Saves go through the save orchestrator only | Unknown | `+page.svelte:586,703` not confirmed |
| 0012 | No-op edits suppressed | Unknown | not checked |
| 0012 | Facility record carries claimed slots (2026-11-22) | Drift (text) | `facilities.rs:86-97,1054` |
| 0015 | No CDI cache in LayoutState; config values only on captured nodes | OK | `state.rs:51` |
| 0015 | Seeding cache `node_registry.saved_trees` | Drift (name) | `layout_capture.rs:833` |
| 0020 | Logic placement planned in one place | OK | `compiler.rs:631` |

The S6 slice resolves the ADR-0004 and ADR-0011 consumer drift. S1 confirms the ADR-0012 Unknown rows for the panel paths it adds.

### Vertical Slices

**S1: One-board panel, end to end**
- Type: HITL
- Layers: components → panel editor store/orchestrator → facilityOrchestrator → facilities/channels/config drafts → API → backend commands → core panel compiler and signaling outputs → Save → Signal LCC
- Blocked by: None
- Test: an integration test from Add facility through Done shows the facility, one lamp channel per wired entry and the expected unsaved edits on non-sequential rows. Core compiler tests cover "lit when", both events set, the description, claimed elsewhere as a blocking issue, and multi-board grouping. Shared pin rule tests cover seat, clear, move and outside. Version tests: 6 accepted, newer refused, 4 and 5 open.
- Acceptance: name a panel; add blocks one at a time; wire them to #9, #3 and #14; clicking a block's pin again clears it; Done and Save; each LED follows its own block. Pins used by ABS can't be chosen, and ABS can't take panel pins. A minimal card shows the panel.
- Architecture note: sets up the panel compile-and-stage pattern and the new IPC contracts (F5, F6, F7 as touched). New code imports only through `$lib/layout`.

**S2: Panel card and channel usage**
- Type: AFK
- Layers: components → effectiveLayoutStore → eventState
- Blocked by: S1
- Test: card and facade tests cover:
  - entries in order with their pins;
  - lit, unlit and unknown;
  - Empty, Incomplete (no lamp, lost block, missing board) and Wired;
  - changes waiting per board;
  - Channels "Used by" lists both the panel and the ABS facility (F3).
- Acceptance: SC-002 and SC-006 on a layout with two panels.

**S3: Blocks step and "lit when"**
- Type: AFK
- Layers: components → panel editor store/orchestrator → facilityOrchestrator → compiler
- Blocked by: S1
- Test:
  - add all from an ABS facility (travel order) and from a detector board (input order), skipping duplicates;
  - reorder;
  - point an entry at another block, keeping its lamp;
  - panel "lit when" and per-block override swap Lamp On and Lamp Off.
- Acceptance: US1 scenarios 3, 4, 7 and 8.

**S4: Edit, release and remove**
- Type: HITL
- Layers: components → orchestrators → compiler removal and resets → drafts → Save
- Blocked by: S1
- Test:
  - editing resets exactly the dropped pins;
  - moving a pin between blocks shows the message;
  - releasing an outside pin is staged with the panel's changes;
  - removal resets exactly the claimed pins after the confirmation that names them;
  - an ABS facility sharing the blocks is unchanged.
- Acceptance: US3 on the bench; SC-004.
- Architecture note: reset recipe for lamp rows; release through `overlay_releases`.

**S5: Migrate Block Indicators and retire them**
- Type: HITL
- Layers: route → facilities store → API → backend open/list → core single read and migration
- Blocked by: S1, S2
- Test: migration unit tests:
  - one panel per board;
  - facility order kept;
  - lamp-less indicators go into one panel;
  - block-less ones are dropped;
  - channels are reused and renamed;
  - the conversion is idempotent.

  Open and list return the same converted document (F1). The "needs save" flag makes the layout dirty, and Discard keeps the converted form (F2). No configuration edits are staged. Add facility offers only Panel.
- Acceptance: US4; SC-005 on a layout saved by 0.4.x. The Block Indicator code in F4 is deleted, and the tests that depended on it are rewritten for Panel or migration.

**S6: Route all layout imports through the facade**
- Type: REFACTOR
- Layers: routes, components → `$lib/layout`
- Blocked by: S5
- Test: a guard test (or lint rule) that fails when a route or component imports an edit-layer store directly; existing route and component tests stay green.
- Acceptance: no route or component imports `bowties`, `layout`, `bowtieMetadata`, `configChanges` or the other edit stores outside the facade. The route reads only the aggregate dirty flag. Behavior unchanged. ADR-0004 and ADR-0011 audit rows become OK.

The roadmap is acyclic: S1 → (S2, S3, S4) → S5 → S6.

### Deferred Improvements

None. No issue proposed.

### Architecture Decisions

- No new ADR.
- During build, add dated extensions:
  - ADR-0012: panel compiled intent; migration as a converted baseline with a "needs save" flag, and why staging deltas was rejected.
  - ADR-0013: panel lamps persisted as lamp-indicator channels; blocks referenced from records.
  - ADR-0005: manifest version 6.
- Correct the ADR-0012 and ADR-0015 wording under F7.
