# Implementation Plan: Configuration Modes & Placeholder Boards

**Branch**: `014-config-modes-placeholders` | **Date**: 2026-05-24 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/014-config-modes-placeholders/spec.md`

## Summary

Generalize the structure-profile schema with a first-class **Configuration Mode** concept (selector + named variants, each variant declaring overlays for event roles, relevance, and structural constraints), and add **placeholder boards** to layouts so users can preview and pre-configure any bundled board profile without connected hardware. The TurnoutBoss profile (issue #8) is shipped as the validation case; Tower-LCC's existing connector/daughterboard shape is re-expressed under the unified schema (no aliases — daughterboards haven't shipped). The schema also relaxes v1 limits: event-role overrides at the eventid-leaf level, and relevance rules whose controlling field/affected target can reference any CDI path (cross-segment, leaf-targeted).

Technical approach: bump `schemaVersion` to `"2.0"`, add `configurationModes` + relax `eventRoles`/`relevanceRules` in `app/src-tauri/src/profile/types.rs`, layer a deterministic overlay-composition pass in `profile::annotate_tree` (declaration order, last-write-wins per target), re-express Tower-LCC under the new shape, assemble + bundle the TurnoutBoss profile, extend `LayoutFile` with a `placeholderBoards` map (id = `placeholder:<uuidv4>`, key = profile filename stem), and run placeholder boards through the existing CDI/profile rendering pipeline by feeding a bundled CDI XML in place of a live-node CDI fetch.

## Technical Context

**Language/Version**: Rust 2021 (stable 1.70+) backend; TypeScript 5.x + Svelte 5 / SvelteKit 2.x frontend
**Primary Dependencies**: Tauri 2.x, `lcc-rs` (workspace crate), `serde`, `serde_yaml_ng`, `tokio`, Vitest, `cargo test`
**Storage**: YAML layout files (`.bowties.yaml`) on disk; bundled `.profile.yaml` + CDI XML resources under `app/src-tauri/profiles/`
**Testing**: `cargo test` (unit + integration for profile schema, overlay composition, Tower-LCC parity, TurnoutBoss load); Vitest for placeholder store/orchestrator + UI gating; Playwright not required for this slice
**Target Platform**: Windows / macOS / Linux desktop (Tauri)
**Project Type**: Desktop application (Tauri backend + SvelteKit frontend + `lcc-rs` protocol library)
**Performance Goals**: Profile load + annotation per node remains under existing budgets (no measurable regression vs. v1); adding a placeholder must not block UI thread (annotation runs in the same backend path as a real node)
**Constraints**: Bundled CDI XML must be available offline (no network round-trip for placeholders); no breaking change to existing on-disk layout YAML (new field is additive and defaulted); Tower-LCC behavior must remain identical for every supported connector + daughterboard combination
**Scale/Scope**: ~2 bundled profiles touched (Tower-LCC migration + TurnoutBoss new); 1 schema change (v1 → v2); 1 new layout field (`placeholderBoards`); ~3 new frontend modules (placeholder store, placeholder orchestrator, "add placeholder" UI surface in the existing layout view)

## Constitution Check

Evaluated against `.specify/memory/constitution.md` v1.0.0:

- **I. Rust 2021+**: ✅ Backend changes stay in Rust 2021 with `Result` error handling; no new panics.
- **II. Cargo-Based Dev**: ✅ No new toolchain; profile YAML stays under `app/src-tauri/profiles/` per existing convention.
- **III. TDD (MANDATORY)**: ✅ Each slice opens with a failing test (schema-accept, overlay-compose, Tower-LCC parity, TurnoutBoss load, placeholder round-trip, placeholder excluded from binding enumeration).
- **IV. LCC Protocol Correctness**: ✅ No protocol semantics changed. Placeholder eventids are explicitly non-bindable and never enter event traffic; the profile schema is app-layer only and stays out of `lcc-rs`.
- **V. UX-First**: ✅ Placeholder boards demonstrably remove the "must own the hardware" barrier; clarified Q&A pinned the unknown-variant UX to a single, actionable warning.
- **VI. TCP-Only**: ✅ No transport changes.
- **VII. Event Management Excellence**: ✅ Existing event flows are unchanged; placeholder eventids are visibly marked and refused from any cross-node binding flow (FR-014, FR-015).

**Gate result: PASS.** No deviations require justification. Complexity Tracking section omitted intentionally.

Re-check after Phase 1 design: PASS (no new violations introduced by `data-model.md` or `contracts/`; placeholder identity (`placeholder:<uuidv4>` + profile filename stem) keeps the data model additive and avoids touching `lcc-rs`).

## Project Structure

### Documentation (this feature)

```text
specs/014-config-modes-placeholders/
├── plan.md                                # This file
├── research.md                            # Phase 0 output
├── data-model.md                          # Phase 1 output
├── quickstart.md                          # Phase 1 output
├── contracts/
│   ├── profile-yaml-schema-v2.json        # New schema (configurationModes + relaxed rules)
│   └── layout-yaml-schema-additions.md    # Additive placeholderBoards shape
├── proposal-original.md                   # Pre-spec problem framing
├── spec.md                                # Feature specification
└── checklists/
```

### Source Code (repository root)

```text
app/
├── src-tauri/
│   ├── profiles/
│   │   ├── RR-CirKits_Tower-LCC.profile.yaml                  # Re-expressed under v2 schema
│   │   ├── Mustangpeak-Engineering_TurnoutBoss.profile.yaml   # NEW (assembled from profile-extractions/turnout-boss/)
│   │   ├── Mustangpeak-Engineering_TurnoutBoss.cdi.xml        # NEW (bundled CDI for placeholders)
│   │   └── RR-CirKits_Tower-LCC.cdi.xml                       # NEW (backfilled for placeholders)
│   └── src/
│       ├── profile/
│       │   ├── types.rs                  # +ConfigurationMode, +Selector, +Variant, +Overlay
│       │   ├── mod.rs                    # +overlay composition in annotate_tree
│       │   ├── loader.rs                 # Accept schemaVersion "2.0"; reject legacy connector* fields
│       │   └── resolver.rs               # Path resolution for cross-segment + leaf-targeted rules
│       ├── layout/
│       │   └── types.rs                  # schemaVersion bumped to "2.0"; remove connector_selections;
│       │                                 # +placeholder_boards: BTreeMap<PlaceholderId, PlaceholderBoard>;
│       │                                 # +node_mode_selections: BTreeMap<NodeKey, BTreeMap<ModeId, VariantId>>;
│       │                                 # +LayoutEditDelta::{AddPlaceholderBoard, DeletePlaceholderBoard,
│       │                                 #                    SetPlaceholderConfigValue, SetNodeModeSelection,
│       │                                 #                    RenamePlaceholderBoard}
│       └── commands/
│           ├── cdi.rs                    # +load_bundled_cdi(profile_stem) for placeholders
│           ├── placeholders.rs           # NEW — 5 placeholder commands
│           └── bowties.rs                # Binding enumerations gain exclude_placeholders gate keyed on node-key prefix
└── src/
    └── lib/
        ├── stores/
        │   ├── placeholderBoardsStore.svelte.ts        # NEW — durable placeholder state
        │   └── connectorSelections.svelte.ts            # Re-targeted to nodeModeSelectionsStore (unified key)
        ├── orchestration/
        │   └── placeholderBoardOrchestrator.ts         # NEW — add/delete/configure lifecycle
        └── routes/+page.svelte                          # "Add board" entry; wire placeholders into layout view

lcc-rs/                                                   # UNCHANGED — no protocol library impact

specs/014-config-modes-placeholders/                      # See Documentation tree above
```

**Structure Decision**: Existing three-layer Bowties topology (frontend / Tauri backend / `lcc-rs` library) is preserved. All new logic lands in the **Tauri backend** (`profile/`, `layout/`, `commands/`) and the **frontend** (`stores/`, `orchestration/`, `components/`, route wiring). `lcc-rs/` is untouched — Configuration Modes and placeholders are Bowties-app concerns per `product/architecture/code-placement-and-ownership.md`. Bundled CDI XML moves alongside `.profile.yaml` under `app/src-tauri/profiles/` so placeholders work fully offline.

## Complexity Tracking

> Constitution Check passed with no deviations. This section is intentionally left empty.

## Architecture Assessment

Reviewed against `product/architecture/code-placement-and-ownership.md`, the ADR set, `aiwiki/owners.md`, and `aiwiki/flows.md`. The feature is multi-layer (profile, layout, commands, stores, orchestration, routes, components) and introduces new modules, so a **Full** assessment was warranted. Findings + decisions below; ADR-0008 captures the one load-bearing rule the rest of the slices depend on.

### Affected Modules

| Module | Layer | Impact | Notes |
|--------|-------|--------|-------|
| `app/src-tauri/src/profile/types.rs` | Backend domain | Modified | +`ConfigurationMode` / `Selector` / `Variant` / `Overlay`; relax `EventRoleDecl` + `RelevanceRule`; remove `connector_slots`, `connector_constraint_variants`, `daughterboard_references`, `carrier_overrides`. |
| `app/src-tauri/src/profile/mod.rs` | Backend domain | Modified | Overlay composition in `annotate_tree` (declaration order, last-write-wins); fold `build_connector_profile` into the generic applier. |
| `app/src-tauri/src/profile/resolver.rs` | Backend domain | Modified | Drop sibling-only check (cross-segment + leaf-targeted paths). |
| `app/src-tauri/src/profile/loader.rs` | Backend domain | Modified | Accept `schemaVersion: "2.0"`; reject leftover v1 connector fields. |
| `app/src-tauri/src/layout/types.rs` | Backend domain | Modified | Bump `schemaVersion: "2.0"`; remove `connector_selections`; +`placeholder_boards`; +`node_mode_selections` (top-level, keyed by NodeKey); +5 `LayoutEditDelta` variants; +validation. |
| `app/src-tauri/src/commands/placeholders.rs` | Backend command | New | 5 placeholder commands (add / delete / setConfigValue / setNodeModeSelection / rename). Keeps `bowties.rs` from crowding. |
| `app/src-tauri/src/commands/cdi.rs` | Backend command | Modified | +`load_bundled_cdi(profile_stem)`. |
| `app/src-tauri/src/commands/bowties.rs` | Backend command | Modified | Binding enumerations gain `exclude_placeholders` gated on `node_key.starts_with("placeholder:")`. |
| `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml` + `.cdi.xml` | Profile bundle | Rewritten / backfilled | Re-expressed under v2; bundled CDI added. |
| `Mustangpeak-Engineering_TurnoutBoss.profile.yaml` + `.cdi.xml` | Profile bundle | New | Assembled from `profile-extractions/turnout-boss/`. |
| `app/src/lib/stores/placeholderBoardsStore.svelte.ts` | Store | New | Durable placeholder state (identity, configValues, name). |
| `app/src/lib/stores/connectorSelections.svelte.ts` | Store | Re-targeted | Reads/writes through the unified `nodeModeSelections` field via the same `NodeKey` accepted everywhere else. |
| `app/src/lib/orchestration/placeholderBoardOrchestrator.ts` | Orchestrator | New | Add / delete / rename / configure lifecycle; wraps deletion in confirmation per FR-017a. |
| `app/src/lib/orchestration/connectorSelectionOrchestrator.ts` | Orchestrator | Modified | Drives the unified mode-selection seam. |
| `app/src/lib/stores/nodeTree.svelte.ts`, `configChanges.svelte.ts`, `configEditor.svelte.ts` | Store | Touched | Accept `placeholder:<uuidv4>` as a `NodeKey` everywhere a `NodeID` is accepted today (ADR-0008). |
| `app/src/routes/+page.svelte`, `routes/config/+page.svelte` | Route | Modified | "Add board" entry; placeholder-aware composition. |
| `app/src/lib/components/ConfigSidebar/NodeEntry.svelte` | Component | Modified | Inline "placeholder" marker (no separate badge component — single call site per F7). |

### Assessment Summary

The schema work is a real depth win: collapsing Tower-LCC's bespoke `connectorSlots` / `connectorConstraintVariants` / `daughterboardReferences` / `carrierOverrides` shape into one declarative `ConfigurationMode` mechanism removes ~150 LoC of parallel construction (`build_connector_profile`) and gives placeholder boards and real boards a single annotation path. The placeholder concept itself piggy-backs on existing seams — the editor pipeline, the layout-file delta protocol, the CDI rendering path — instead of building parallel infrastructure, by treating `placeholder:<uuidv4>` as a first-class `NodeKey` (ADR-0008). Per-leaf placeholder annotation is unnecessary: "placeholderness" is a property of the node, not its individual leaves, so a single `node_key.starts_with("placeholder:")` check at the binding-enumeration seam fully covers FR-014 + FR-015. Because daughterboards have not shipped, we consolidate the duplicate "selected variant" home (`connector_selections` + per-placeholder `modeSelections`) into one top-level `nodeModeSelections` map and bump `LayoutFile.schemaVersion` to `"2.0"` with no migration code — F2 (ii) + F6 land together.

### Findings

**F1: One editor pipeline for real and placeholder boards (load-bearing)**
- Category: depth / seam placement
- Affected: `nodeTree`, `configChanges`, `configEditor`, `effectiveLayoutStore`, backend `node_tree`
- Concern: The editor pipeline today is keyed by `NodeID`. Placeholders need the same pipeline.
- Decision: **include** — captured as **ADR-0008 — Unified node-key for real and placeholder boards**. Every `NodeID` slot becomes a `NodeKey` (`NodeID | "placeholder:<uuidv4>"`). One `is_placeholder(node_key)` check at the binding-enumeration seam.

**F2: Consolidate `connector_selections` + placeholder `modeSelections` into one top-level `nodeModeSelections` (Shape A)**
- Category: duplication / depth
- Affected: `LayoutFile`
- Concern: After Tower-LCC's migration, the same "which variant is selected" concept would live in two layout fields with two shapes.
- Decision: **include — F2 (ii) + Shape A**. Single top-level `nodeModeSelections: BTreeMap<NodeKey, BTreeMap<ModeId, VariantId>>`. Daughterboards have not shipped, so no migration code is needed.

**F3: "Placeholder" is a node-level property, not a per-leaf annotation**
- Category: seam placement
- Affected: `commands/bowties.rs` binding enumeration, frontend `eventIds.ts`
- Concern: Original draft proposed a per-leaf `is_placeholder: true` annotation and risked colliding with the existing all-zeros "unassigned" sentinel meaning of `isPlaceholderEventId`.
- Decision: **include simplified**. `node_key.starts_with("placeholder:")` is the single seam. No per-leaf annotation; no rename of `isPlaceholderEventId` (it keeps meaning "unassigned"). Issue B (rename) withdrawn.

**F4: Placeholder IPCs live in a dedicated `commands/placeholders.rs`**
- Category: placement compliance
- Affected: `commands/`
- Decision: **include**.

**F5: Fold `build_connector_profile` into the generic overlay applier**
- Category: deepening
- Affected: `profile/mod.rs`
- Decision: **include** as part of the Tower-LCC migration slice (S4). Only the profile is migrated — user data on disk is not (daughterboards never shipped, so there is none to carry forward).

**F6: Bump `LayoutFile.schemaVersion` to `"2.0"`**
- Category: placement compliance
- Affected: `layout/types.rs::validate`
- Concern: Removing `connector_selections` makes the change non-additive.
- Decision: **include**. `validate()` rejects `"1.0"` with a clear "created by an older Bowties build that did not ship daughterboard support" message. No migration code.

**F7: Inline the placeholder marker; no `PlaceholderBoardBadge` component**
- Category: depth
- Affected: `ConfigSidebar/NodeEntry.svelte`
- Decision: **include simplified**. Sidebar is the only call site; inline marker keeps the component count honest.

### Vertical Slices

**S1: v2 profile schema + overlay composition + path resolver relaxation**
- Type: HITL
- Layers: Backend domain (`profile/types`, `mod`, `resolver`, `loader`)
- Blocked by: None
- Test: round-trip a minimal v2 profile with one ConfigurationMode + two variants + one cross-segment relevance rule + per-leaf event role; `annotate_tree` produces deterministic last-write-wins output; unknown-variant warning surfaces; v1 connector fields rejected.
- Acceptance: profile-only Rust test suite green; no frontend changes.

**S2: Layout file v2 — placeholders + unified nodeModeSelections + new deltas + commands**
- Type: HITL
- Layers: Backend domain (`layout/types`), Backend command (`commands/placeholders.rs`)
- Blocked by: S1
- Test: `schemaVersion: "2.0"`; old `"1.0"` rejected with the documented message; AddPlaceholderBoard → SetPlaceholderConfigValue → SetNodeModeSelection → RenamePlaceholderBoard → DeletePlaceholderBoard round-trip; `DeletePlaceholderBoard` also clears the node's entry from `nodeModeSelections`; invalid id rejected with `InvalidPlaceholderId`.
- Acceptance: backend can hold placeholder state end-to-end.

**S3: Unified NodeKey — placeholder-aware editor pipeline (ADR-0008)**
- Type: HITL
- Layers: Store (`nodeTree`, `configChanges`, `configEditor`), Backend domain (tree assembly for placeholder), API, Backend command (`load_bundled_cdi`)
- Blocked by: S2
- Test: frontend opens a placeholder, renders the bundled CDI through the same tree-rendering path as a real node, edits a non-event field, configChanges store records the edit keyed by `placeholder:<uuidv4>`; the binding-enumeration seam excludes any node-key with the `placeholder:` prefix.
- Acceptance: editor works for a placeholder end-to-end (no Configuration Mode interaction yet); no parallel placeholder editor stores exist.

**S4: Tower-LCC migration (re-express + behavior parity + delete connector* fields + fold build_connector_profile)**
- Type: HITL
- Layers: Backend domain (`profile/types`, `mod`); shipped `RR-CirKits_Tower-LCC.profile.yaml` rewritten; backfilled CDI XML
- Blocked by: S1
- Test: every supported connector + daughterboard combo produces identical relevance / role / structural outcomes vs. the pre-migration snapshot (existing connector test suite rewritten to target the unified schema).
- Acceptance: FR-023 / SC-003 green; zero remaining `connectorSlots` / `connectorConstraintVariants` / `daughterboardReferences` / `carrierOverrides` in the repo; `build_connector_profile` deleted.

**S5: ConfigurationMode UI — selector reshapes the tree**
- Type: AFK
- Layers: Route, Component (`ConfigSidebar`, `ElementCardDeck`), Store (`connectorSelections.svelte.ts` re-targeted to unified `nodeModeSelections`), Orchestrator (`connectorSelectionOrchestrator.ts` re-targeted)
- Blocked by: S3, S4
- Test: changing a Configuration Mode selector in the editor triggers re-annotation and the rendered tree re-shapes (relevance + roles); unknown-variant warning surfaces in the UI; the Tower-LCC connector picker still works against the migrated profile.
- Acceptance: real Tower-LCC nodes and placeholder Tower-LCC boards exhibit identical re-shape behavior through the same store/orchestrator pair.

**S6: TurnoutBoss profile assembled & bundled**
- Type: AFK
- Layers: Profile bundle only
- Blocked by: S1
- Test: assembled profile loads + validates; Left vs Right reshape (Detector 3 relevance, Occupancy role flip) produces the documented outputs.
- Acceptance: FR-009 / FR-010 / SC-002 green.

**S7: Placeholder boards UI — picker, sidebar marker, binding exclusion**
- Type: HITL
- Layers: Component ("Add board" picker; inline marker in `NodeEntry.svelte`), Store (`placeholderBoardsStore`), Orchestrator (`placeholderBoardOrchestrator`), Route wiring
- Blocked by: S2, S3, S6
- Test: add TurnoutBoss placeholder from the picker → flip Left/Right → edit fields → save → reopen → state restored exactly; every binding-enumeration flow excludes placeholder eventids; deleting a placeholder requires confirmation and does not touch other layout entries.
- Acceptance: full quickstart steps 1–7 pass.

**S8: Tower-LCC placeholder demo + Unknown-Model resilience**
- Type: AFK
- Layers: Backend domain (FR-022 unknown-model handling), Route
- Blocked by: S4, S7
- Test: add a Tower-LCC placeholder, exercise daughterboard variants, save, hand-edit YAML to reference a nonexistent profile stem, reopen → placeholder loads as "Unknown model" without crash.
- Acceptance: FR-022 green; quickstart step 8 passes.

### Deferred Improvements

None. The two findings flagged for possible deferral (F2 → consolidated `nodeModeSelections`, F3 → rename `isPlaceholderEventId`) were both resolved in-scope: F2 lands as Shape A in S2, and F3 was simplified away (no rename needed). No `kind/idea` issues created.

### Architecture Decisions

- **ADR-0008 — Unified node-key for real and placeholder boards.** See `product/architecture/adr/0008-unified-node-key-for-real-and-placeholder-boards.md`.

