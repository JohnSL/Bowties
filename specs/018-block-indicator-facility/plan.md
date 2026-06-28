# Implementation Plan: Block Indicator Facility — Channels, LED Indicators, and the First Facility

**Branch**: `018-block-indicator-facility` | **Date**: 2026-06-27 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/018-block-indicator-facility/spec.md`

## Summary

Introduce a layered **pin → channel → facility** model in which channels are the only first-class binding entity. Each channel carries a **role** (state vocabulary + slot-binding contract — e.g., `block-occupancy`, `lamp-indicator`) and a **style** (the hardware shape that realises the role — e.g., `bod-block-detector-input`, `single-led-direct-lamp`). Channels are either **hardware-owned** (auto-created by hardware-config choices like BOD daughter-board selection) or **user-owned** (created via a facility slot's *Add channel* action).

A **facility** is a named instance of a **behavior template** with slots that bind by role. The first template is **Block Indicator** — a pass-through (`occupied → lit`, `clear → unlit`) from a `block-occupancy` input to a `lamp-indicator` output. When all slots are filled, the facility becomes **Wired** and Bowties creates the underlying bowtie(s) using the existing bowtie creation mechanism; when a slot empties, the affected bowtie(s) are freed via the existing slot-detach pipeline. Spec 018 introduces **no new sync, persistence, or deployment machinery** — facilities are a UI veneer that drives bowtie creation.

Technical approach: extend the existing channel system (specs 015/016/017) with `role`, `style`, and `ownership` fields; add a new draft-layer **facilities store** and **facility persistence** (`facilities.yaml`); reuse the existing event store + eager-resolve pipeline for live channel state on both producers and consumers; add a **Channels panel** (hardware-organised list) and **Facilities section** to the Railroad route; migrate BOD-family `validityRules` from daughterboards into a style-owned **constraint contract**; declare a `single-led-direct-lamp` style on Signal LCC's Direct Lamp Control subsystem. Eight independently demoable slices, with the last two (channel-inventory retirement and the top-level tab-style refresh) optional and isolated.

## Technical Context

**Language/Version**: Rust 2021 (stable 1.70+) for backend and `lcc-rs`; TypeScript 5.x (strict) + Svelte 5 (runes) for frontend.
**Primary Dependencies**:
- Backend: Tauri 2, `tokio`, `serde`, `serde_yaml`, `thiserror`, `uuid` (v4); `lcc-rs` workspace crate.
- Frontend: SvelteKit 2, `@tauri-apps/api`, Vitest + `@testing-library/svelte`.
**Storage**: Per-layout folder (filesystem) — extends the current layout shape with a new `facilities.yaml` file alongside `bowties.yaml`, `channels.yaml`, `manifest.yaml`, `nodes/*.snapshot.yaml`, `offline-changes.yaml`. Writes route through the existing journaled in-place writer (ADR-0006). No database.
**Testing**: `cargo test` (unit + integration, proptest where applicable) for Rust; `vitest` for stores/orchestrators/components; Tauri integration tests under `app/src-tauri/tests/`.
**Target Platform**: Desktop (Windows / macOS / Linux) via Tauri.
**Project Type**: Multi-package desktop app — `lcc-rs/` (protocol library) + `bowties-core/` (backend domain crate) + `app/src-tauri/` (Tauri backend) + `app/src/` (SvelteKit frontend).
**Performance Goals**: UI interactions remain at 60 fps; live channel state for a 8-channel BOD-8 reflects bus events within the existing event-store response window (spec 016 baseline preserved at the channel layer). End-to-end physical observation (block → LED) under 1 second once Wired and synced to bus.
**Constraints**: TCP-only LCC transport (Constitution VI). All layout mutations flow through the ADR-0012 draft layer; no write-through. ADR-0004 single-merge-derivation rule applies to facility status computation. `lcc-rs` MUST NOT learn about facilities, channels, roles, or styles — those concepts live in `bowties-core` and the frontend stores. Constraint contract declared in profile YAML, applied through the existing relevance/validity rendering surface (no new Config-tab UI).
**Scale/Scope**: Single user, single layout per session. Up to ~32 BOD channels (4 connectors × 8) and a handful of Signal LCC nodes per realistic layout. Tens of facilities. 8 implementation slices; ~36 functional requirements; ~11 measurable success criteria.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| **I — Rust 2021+ Development** | All backend additions (facility persistence, channel schema migration, behavior template registry, slot-binding IPC) implemented in Rust 2021 on stable; explicit error types via `thiserror`; no `unwrap` in production paths. | PASS |
| **II — Cargo-Based Development Environment** | No new toolchains. `Cargo.toml` in `bowties-core` and `app/src-tauri` gets the existing `serde_yaml` / `uuid` deps reused; lockfile commits as usual. | PASS |
| **III — Test-Driven Development** | Every slice's red-green-refactor lands store/orchestrator/component tests before implementation, per the existing test-mapping convention in `aiwiki/owners.md`. New behavior template registry, facility store, slot-binding logic, and style constraint contract each get unit tests; end-to-end test for the Wired-state bowtie creation. | PASS |
| **IV — LCC Protocol Correctness** | No new protocol behavior. Facility wiring uses the existing bowtie creation mechanism (`bowties-core` + `query_event_roles`), which already encodes correct LCC producer/consumer event semantics. Direct Lamp Control consumer events use the existing `eventRoles` declaration in the Signal LCC profile. | PASS |
| **V — UX-First Design** | The headline workflow (scaffold facility → bind input from BOD channel → add LED channel → watch the LED follow the block) is single-page, single-tab, with no required round-trip to the Config tab. Empty slots are first-class. Release notes + user docs deliver expectation-setting out-of-product; the Facilities section itself ships as plain functional UI with no in-product chrome (FR-035). | PASS |
| **VI — TCP-Only Focus** | No transport changes. | PASS |
| **VII — Event Management Excellence** | Facility wiring is *exactly* event-id co-configuration on producer + consumer sides via existing bowties — Bowties' core competency. Adds no new event-management surface, only a higher-level orchestration on top. Direct Lamp Control consumer events become first-class targets of facility binding. | PASS |

**Result**: All gates pass with no violations. **Complexity Tracking** section is empty.

## Project Structure

### Documentation (this feature)

```text
specs/018-block-indicator-facility/
├── plan.md              # This file
├── spec.md              # Feature specification (input)
├── mockups.html         # Existing UX mockups
├── research.md          # Phase 0 output — design decisions for unresolved questions
├── data-model.md        # Phase 1 output — entity shapes (Channel, Facility, Slot, Role, Style, BehaviorTemplate)
├── quickstart.md        # Phase 1 output — end-to-end demo path
├── contracts/           # Phase 1 output — IPC command contracts + profile YAML schema deltas
│   ├── ipc-commands.md
│   └── profile-schema-deltas.md
├── checklists/
│   └── requirements.md  # Existing requirements checklist
└── tasks.md             # Phase 2 output (created by /speckit.tasks, not by this command)
```

### Source Code (repository root)

```text
lcc-rs/                                # NO CHANGES — protocol library stays facility-agnostic
└── src/                               # alias_allocation, cdi, datagram_reader, discovery, dispatcher,
                                       # pip, protocol, snip, transport, transport_actor, types

bowties-core/                          # Backend domain crate
└── src/
    ├── layout/                        # EXTEND: add facilities.yaml read/write through existing journal
    │   ├── facilities.rs              # NEW — facility persistence (CRUD, slot bindings, serde)
    │   └── ...                        # existing capture, snapshot, manifest, journal
    ├── channels/                      # EXTEND: schema migration adds role/style/ownership fields
    │   └── ...
    └── behavior_templates/            # NEW — hardcoded Block Indicator template registry
        └── mod.rs                     #   (one template in this slice; declarative loader is future)

app/src-tauri/                         # Tauri backend
├── src/
│   ├── commands/
│   │   ├── channels.rs                # EXTEND: list_channels returns new schema; new create_user_owned_channel
│   │   ├── facilities.rs              # NEW — list/create/rename/delete facilities; bind/unbind slots
│   │   ├── behavior_templates.rs      # NEW — list_behavior_templates (returns Block Indicator)
│   │   ├── connector_profiles.rs      # EXTEND: BOD-family channel auto-create now produces role+style
│   │   └── bowties.rs                 # REUSE: existing bowtie creation mechanism, called by facility flow
│   └── ...                            # diagnostics, events, layout, node_registry, node_proxy, node_tree, profile, traffic
└── profiles/
    ├── RR-CirKits.shared-daughterboards.yaml   # EXTEND: BOD-* channelInputs gain explicit style id;
                                                #         existing validityRules migrate to style.constraints
    └── RR-CirKits_Inc._Signal-LCC.profile.yaml # EXTEND: declare lamp-indicator role + single-led-direct-lamp
                                                #         style + constraint contract on Direct Lamp Control rows

app/src/                               # SvelteKit frontend
├── routes/
│   └── +page.svelte                   # EXTEND: Railroad tab gains Facilities section + restructured Channels panel
│                                      # (FR-036 chrome refresh — segmented buttons → tab strip — is Slice 8, isolated)
├── lib/
│   ├── components/
│   │   ├── Railroad/                  # EXTEND: ChannelGroup/ChannelCard get role/style/ownership/binding columns;
│   │   │                              #         hardware-organised grouping replaces channelType grouping
│   │   ├── Facilities/                # NEW directory
│   │   │   ├── FacilitiesSection.svelte
│   │   │   ├── FacilityCard.svelte
│   │   │   ├── FacilitySlot.svelte
│   │   │   ├── AddFacilityDialog.svelte
│   │   │   ├── SelectChannelPicker.svelte
│   │   │   └── AddChannelPicker.svelte     # Lamp-row sub-picker for single-led-direct-lamp
│   │   └── ...                        # Bowtie, ConfigSidebar, ElementCardDeck, Layout, LayoutPicker (no changes)
│   ├── stores/
│   │   ├── channels.svelte.ts         # EXTEND: schema adds role/style/ownership; draft layer unchanged
│   │   ├── facilities.svelte.ts       # NEW — facility CRUD + slot bindings (draft layer per ADR-0012)
│   │   ├── behaviorTemplates.svelte.ts# NEW — read-only registry mirror (loaded from backend on app start)
│   │   ├── effectiveLayoutStore...    # EXTEND: facility status derivation (Incomplete vs Wired) joins the merge
│   │   └── ...                        # all other stores untouched
│   ├── orchestration/
│   │   ├── connectorSelectionOrchestrator.ts  # EXTEND: BOD channel auto-create populates role/style/ownership
│   │   ├── facilityOrchestrator.ts            # NEW — Add facility, bind/unbind slot, Add-channel atomic flow,
│   │   │                                      #       Incomplete↔Wired transition (creates/frees bowties via
│   │   │                                      #       existing bowtie creation mechanism + slot-detach pipeline)
│   │   ├── saveLayoutOrchestrator.ts          # EXTEND: collect facility deltas alongside channels/connectors
│   │   └── layoutLifecycleOrchestrator.ts     # EXTEND: facilities/templates stores join the reset enumeration
│   ├── utils/
│   │   ├── facilityStatus.ts          # NEW — pure derivation: slots → Incomplete | Wired
│   │   ├── styleConstraints.ts        # NEW — apply style constraint contract over CDI field render decisions
│   │   └── ...
│   └── api/                           # EXTEND: typed wrappers for new Tauri commands
└── ...
```

**Structure Decision**: Multi-package Tauri desktop app (existing layout retained). New code adds two **store modules** (`facilities`, `behaviorTemplates`), one **orchestrator** (`facilityOrchestrator`), one **component directory** (`Facilities/`), two **utils** (`facilityStatus`, `styleConstraints`), one **backend domain module** (`bowties-core/src/behavior_templates`), one **backend layout module** (`bowties-core/src/layout/facilities.rs`), and two **command modules** (`facilities.rs`, `behavior_templates.rs`). All other surfaces are *extensions* of existing files. `lcc-rs/` gets **zero** changes — it stays a pure protocol library per Constitution principle IV and the lcc-rs instructions file.

## Complexity Tracking

> No constitution violations. No entries required.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| *(none)* | | |

## Post-Design Constitution Re-check

After Phase 1 (data-model.md, contracts/, quickstart.md, agent context update) all seven constitutional principles still pass:

- The data model places **roles and styles in profile YAML + a tiny in-code registry**, not in `lcc-rs` (Principle IV preserved).
- The contracts add **eight Tauri commands** in two new command modules — all backed by typed Rust functions with `Result<T, AppError>` returns (Principles I, III).
- The quickstart workflow is **a single user journey across three tabs** with no CLI fallback required, validating Principle V.
- No new transport, no new protocol surface (Principles VI, VII).

**Result**: Re-check PASS. Proceed to `/speckit.tasks`.

## Architecture Assessment

*Produced by `/design` on 2026-06-27 and accepted by the architect. Findings F1–F7 included; F8 (idea capture) deferred to the existing ux-vision documents rather than `kind/idea` issues.*

### Patterns introduced

- **Pin → Channel → Facility layered binding model** — channels are the only first-class binding entity; facilities only see channels; pins only see hardware.
- **Role / Style as Interface / Implementation duality** — Role is the state-vocabulary contract (Rust enums, exhaustive match); Style is the hardware-shape realisation (profile YAML). See ADR-0013.
- **Ownership-driven Lifecycle** — `hardware-owned` channels follow their backing hardware-config; `user-owned` channels follow their single binding slot. No ref-counting in this slice.
- **Behavior Template as Declarative Composition** — facility = template + slot bindings + name. Hardcoded `Block Indicator` registry; future declarative loader.
- **Constraint Contract owned by Style** — the existing profile-driven relevance/validity renderer is unchanged; only the source of truth moves from daughter-board entries onto styles (ADR-0013).
- **Wired / Incomplete as Derived Status** — pure function over slot fullness, joined in `effectiveLayoutStore` per ADR-0004; never persisted.
- **Facilities as UI Veneer over Bowties** — `facilityOrchestrator` composes the existing bowtie-creation pipeline + slot-detach pipeline; bowties carry a `createdByFacility` back-reference for cleanup.

### Module-level change shape

The full Before/After diagrams and module-level change table from the assessment are recorded in the post-decision summary at the bottom of this section; the dispatch into source code follows the file-tree from "Project Structure" above. Net change vs. the original plan:

- `facilitiesStore` and `behaviorTemplatesStore` join `layoutLifecycleOrchestrator.resetForNewLayout()` **in the slice that introduces them**, not in a deferred lifecycle slice — per ADR-0011 and Finding F3.
- The BOD-family `validityRules` migration to the `bod-block-detector-input` style's `constraints:` block is a **replacement** (legacy entries removed) in the same slice that lands the style — per ADR-0013 and Finding F4. No transition period with two parallel constraint sources.

### Findings and dispositions

| # | Principle | Finding | Disposition |
|---|---|---|---|
| **F1** | YAGNI / Locality | Channel schema (`role`/`style`/`ownership`/`binding`) shipped in Slice 1 with no consumers | **Include in slices** — move channel-schema fields to Slice 2 alongside the BOD retrofit so the schema lands with its first read path. Slice 1 keeps facility persistence + behavior templates + pre-018-shaped channels. |
| **F2** | Slice complexity / TDD cycle health | Original Slice 2 bundled schema + retrofit + panel restructure + constraint migration + new Channels panel | **Include in slices** — Option B: split into two demoable slices. New Slice 2 = channel schema + BOD retrofit (existing panel grouping unchanged). New Slice 3 = hardware-organised Channels panel + constraint-contract migration. |
| **F3** | Locality / ADR-0011 | Lifecycle reset enumeration deferred to "Iteration ergonomics" risks cross-layout state bleed | **Include in slices** — each slice that introduces a layout-scoped store adds it to `layoutLifecycleOrchestrator.resetForNewLayout()` + tests in the same slice. No separate lifecycle slice. |
| **F4** | DRY / Single source of truth | If style `constraints:` are additive to legacy `validityRules`, the resolver gets two parallel sources | **Include in slices** — the YAML migration is a replacement, not additive. Legacy `validityRules` on BOD daughter-board entries are removed in the same change that introduces the style `constraints:` block. Captured in ADR-0013. |
| **F5** | Label honesty | Slice that retires the Spec 015 BOD-8 channel inventory ships no new behavior | **Include in slices** — explicitly labelled `[REFACTOR]` in the slice roadmap so `/build` runs a refactor-only TDD pass rather than red→green→refactor. |
| **F6** | Terminology consistency | Spec 018 introduces ~8 new canonical terms not yet in `product/glossary.md` | **Done in this design pass** — added a `## Facilities System` section to glossary covering `Facility`, `Behavior Template`, `Facility Slot`, `Role`, `Style`, `Channel Ownership`, `Binding`, `Facility Status`, `Style Constraint Contract`. Updated the existing `Information Channel`, `Channel Type`, `Hardware Reference`, and `Railroad Tab` entries. Added `Channels Panel`. Updated Relationships block. |
| **F7** | Architectural seam recording | Two load-bearing decisions (channel schema; facility persistence) needed ADR coverage | **Done in this design pass** — new **ADR-0013** records the channel role/style/ownership/binding schema and the style-owned constraint contract. ADR-0012 extended with a 2026-06-27 section adding `facilities.yaml` to the draft-layer family. ADR-0005 extended with a 2026-06-27 section adding `read_facilities` / `update_facilities` to the intent-shaped layout API. |
| **F8** | Future-work cache | ~13 Future Considerations items in spec.md not captured as `kind/idea` issues | **Rejected** — the ux-vision documents under `specs/proposals/app-ux-vision/` are the canonical home for these deferred items in this feature family; no `kind/idea` issues will be proposed for them at this time. |

### Accepted slice roadmap

Risk-first ordering. Each slice cuts all needed layers (vertical-slice gate) and yields a user-demoable outcome. Numbering changed from spec.md's "Suggested implementation slicing" per F1, F2, and F5.

| # | Slice title | User-visible change | Demoable? | Notes |
|---|---|---|---|---|
| 1 | **Facility CRUD with empty slots (+ lifecycle reset wiring)** | Add / rename / delete a Block Indicator facility; layout round-trip with empty slots. Glossary updated. | YES | First slice. `facilitiesStore` + `behaviorTemplatesStore` added to `layoutLifecycleOrchestrator.resetForNewLayout()` + tests in this slice (F3). Channel schema not yet extended (F1). |
| 2 | **Channel schema + BOD retrofit (existing panel grouping unchanged)** | BOD-8 inputs still appear in the existing inventory but now carry `role` / `style` / `ownership` / `binding` end-to-end; rename still works; persistence is the new shape. | YES | Schema lands with its first read path (F1). RailroadPanel grouping/styling unchanged; new fields are populated but not yet surfaced as columns. |
| 3 | **Hardware-organised Channels panel + style-owned constraint contract** | Restructured Channels panel groups by node + subsystem with role/style/ownership/binding columns; daughter-board constraints now sourced from the `bod-block-detector-input` style with legacy `validityRules` **removed** in the same change. | YES | Single source of truth for constraint contract (F4). Channels panel is a discoverable hardware-verification surface (US2 lands here). |
| 4 | **Select channel — bind a BOD channel to the facility input slot** | User selects a BOD channel into a Block Indicator's input slot; slot fills; Channels panel binding column updates; Remove-from-slot returns it to "unbound". | YES | Producer half of US3. Facility still Incomplete (output empty). |
| 5 | **Lamp-indicator role + `single-led-direct-lamp` style + Add channel on output** | Add channel on output slot → lamp-row sub-picker (constraint-filtered, ineligible rows hidden) → atomic create + claim row + bind. LED channel appears in Channels panel; live state is "last commanded on bus". | YES | Consumer half lands; FR-032 discoverability via tooltip + release notes + user docs (no per-row inline label). |
| 6 | **Facility becomes Wired + end-to-end + Rebind** | Last slot filled → Wired → orchestrator creates bowtie(s) via the existing bowtie creation mechanism → physical block toggles physical LED. Remove-from-slot / Delete facility / clear daughter board all return to Incomplete via the existing slot-detach pipeline; Rebind swaps cleanly. | YES (headline) | US3 fully lands. End-to-end physical observation. SC-004, SC-005, SC-006, SC-007, SC-008. |
| 7 | **Retire the Spec 015 BOD-8 channel inventory** `[REFACTOR]` | Duplicated surface disappears; BOD inputs visible only via the Channels panel and (when bound) via facility slots. | YES `[REFACTOR]` | F5. Refactor-only TDD pass. Pre-existing renamed-channel names lost (acceptable per FR-009). |
| 8 | **Iteration ergonomics** (optional) | Polish across mixed-state save/reopen scenarios that aren't already covered by FR-005 in slices 1–7; rebinding ergonomics across nodes; rename + restore behavior across daughter-board reselects. | YES (optional) | Safely deferrable. |
| 9 | **Top-level tab chrome refresh** (optional, orthogonal) | Standard tabs with underline-on-active replace the segmented-button group; labels and ordering unchanged. | YES (optional, orthogonal) | FR-036. Pure visual change. Fully isolated from the rest of the feature (R12). |

### Slice HITL/AFK classification

To be assigned by `/slices` from these defaults; recorded here for reference:

- **Slice 1** — **HITL** (introduces the Facilities surface and naming patterns; user feedback wanted on first contact with the Facilities concept).
- **Slice 2** — **AFK** (schema retrofit + orchestrator change; behaviorally identical to today from a user's chair until Slice 3 surfaces the new columns).
- **Slice 3** — **HITL** (significant surface change: hardware-organised grouping + new columns + constraint contract repositioning).
- **Slice 4** — **HITL** (first slot-binding workflow; Select-channel picker UX).
- **Slice 5** — **HITL** (atomic Add-channel sub-picker workflow; first user-owned channel creation path).
- **Slice 6** — **HITL** (headline end-to-end demo; physical hardware verification).
- **Slice 7** — `[REFACTOR]` — **AFK** (one duplicated surface disappears).
- **Slice 8** — **HITL** (optional polish; surface depends on what carries over).
- **Slice 9** — **HITL** (chrome change; visual review).

### ADRs / glossary updates landed in this design pass

- **New**: `product/architecture/adr/0013-channel-role-style-ownership.md` — channel schema (`role` / `style` / `ownership` / discriminated `binding`) + style-owned **Style Constraint Contract**, with rejected alternatives recorded.
- **Extended**: `product/architecture/adr/0012-all-layout-edits-draft-layer.md` — 2026-06-27 extension adding `facilities.yaml` to the draft-layer family; spells out the `facilitiesStore` four-method contract and the per-slice lifecycle-reset enumeration rule.
- **Extended**: `product/architecture/adr/0005-layout-module-owns-file-structure.md` — 2026-06-27 extension adding `read_facilities` / `update_facilities` to the intent-shaped public API.
- **Updated**: `product/glossary.md` — new `## Facilities System` section (Facility, Behavior Template, Facility Slot, Role, Style, Channel Ownership, Binding, Facility Status, Style Constraint Contract). Existing Information Channel / Channel Type / Hardware Reference / Railroad Tab entries updated for the Spec 018 schema and surface changes. Added Channels Panel entry. Relationships block updated.

### Deferred items disposition

- **F8 (Future-Considerations capture as `kind/idea` issues)** — **rejected by architect**: the ux-vision documents under `specs/proposals/app-ux-vision/` are the canonical home for the placeholder-nodes, channel fan-out + ref-counting, multi-pin styles, test events for consumer channels, node-removal cascade, and the other ~10 deferred items listed in spec.md's *Future Considerations*. No `kind/idea` issues are created or proposed for these items.

### Handoff

Architecture assessment complete. Run `/slices` to generate the slice task file from the accepted slice roadmap above.

