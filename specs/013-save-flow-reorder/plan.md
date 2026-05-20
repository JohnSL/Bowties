# Implementation Plan: Layout-First Model

**Branch**: `bug-fixes` (spec: `013-save-flow-reorder`) | **Date**: 2026-05-17 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/013-save-flow-reorder/spec.md`

## Summary

Adopt a layout-first model that eliminates the 4-state connection×layout matrix by requiring an active layout before any work can begin. Connections become layout properties. Save always writes the layout first, then the bus, then reconciles. This fixes blank bowties during save, stale catalogs after bus writes, cancel inconsistency, and `isOfflineMode` flipping.

## Technical Context

**Language/Version**: Rust 2021 (stable 1.70+) for backend + lcc-rs; TypeScript 5.x + SvelteKit 2.x for frontend  
**Primary Dependencies**: Tauri 2.x (IPC + desktop shell), tokio (async runtime), serde/serde_yaml (layout persistence), Svelte 5 runes (reactive state)  
**Storage**: YAML files — `.layout` base file + `.layout.d/` companion directory (schema v3); `$APPDATA/bowties/connections.json` for connection prefs (moving into layout); `$APPDATA/bowties/` for app preferences and recent layout tracking  
**Testing**: `cargo test` (Rust unit/integration), Vitest (frontend unit), 790+ existing tests  
**Target Platform**: Windows, macOS, Linux desktop (Tauri)  
**Project Type**: Desktop app — Rust backend + SvelteKit frontend via Tauri IPC  
**Performance Goals**: Save completes in <2s for typical layouts (≤20 nodes); layout picker renders in <500ms  
**Constraints**: Offline-capable; atomic file writes (temp→flush→rename); single active connection at a time  
**Scale/Scope**: Typical layout: 5–50 nodes, 1–5 connection definitions, dozens of offline changes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Principle | Status | Notes |
|---|-----------|--------|-------|
| I | Rust 2021+ Development | ✅ PASS | All new backend code in Rust. No unwrap() in production paths. Result types for errors. |
| II | Cargo-Based Development | ✅ PASS | Standard cargo build/test. No new tooling required. |
| III | Test-Driven Development | ✅ PASS | TDD for save-flow reorder, layout picker, connection model. 790+ baseline tests grow. |
| IV | LCC Protocol Correctness | ✅ PASS | No protocol changes. Bus write ordering changes, but write_modified_values command is unchanged. |
| V | UX-First Design | ✅ PASS | Layout picker simplifies startup. Progress feedback during save. Eliminates confusing mode flipping. |
| VI | TCP-Only Focus | ✅ PASS | Connection definitions store adapter type, but this is a data model extension, not a transport change. Existing serial transports already supported. |
| VII | Event Management Excellence | ✅ PASS | Resolves blank-bowtie bugs. Persists resolved event roles through save→close→reopen cycle. |
| — | Separation of Concerns | ✅ PASS | Layout persistence stays in backend. Layout picker is a route-level component. Save orchestration stays in orchestrators. |
| — | No Circular Dependencies | ✅ PASS | Frontend → Tauri commands → backend → lcc-rs chain preserved. |
| — | State Management | ✅ PASS | Backend holds connection state; frontend stores for reactive UI. Known-layout registry in app prefs (backend). |

**Gate result: PASS** — No violations. Proceeding to Phase 0.

**Post-Phase 1 re-evaluation (2026-05-17)**: All gates still pass. The design adds:
- 2 new backend modules (`known_layouts.rs`, `startup.rs` commands) — within existing separation of concerns
- Schema migration (v3→v4) — no protocol changes, standard data migration
- `save_layout_with_bus_writes` command — consolidates the three-phase save into a single backend command, keeping orchestration in the right layer
- `SaveProgressDialog` component — pure rendering, emits no workflow logic
- No new dependencies, no new crates, no architectural violations

## Project Structure

### Documentation (this feature)

```text
specs/013-save-flow-reorder/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (Tauri IPC contracts)
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (affected areas)

```text
# Backend (Rust)
app/src-tauri/src/
├── commands/
│   ├── connection.rs        # MODIFY — move connection defs into layout
│   ├── layout_capture.rs    # MODIFY — save-reorder flow, known-layout registry
│   ├── bowties.rs           # MODIFY — save-layout-first orchestration
│   ├── cdi.rs               # MODIFY — save-reorder: defer write_modified_values
│   └── startup.rs           # NEW — layout picker backend (known-layout registry)
├── layout/
│   ├── types.rs             # MODIFY — add connections section to LayoutFile
│   ├── manifest.rs          # MODIFY — add connections to LayoutManifest
│   ├── io.rs                # MODIFY — migration for existing layouts
│   └── known_layouts.rs     # NEW — known-layout registry persistence
├── state.rs                 # MODIFY — add known-layout registry, layout-required guard
└── lib.rs                   # MODIFY — register new commands

# Frontend (TypeScript/Svelte)
app/src/
├── routes/
│   ├── +page.svelte         # MODIFY — layout picker gate, save-reorder integration
│   └── +layout.svelte       # MODIFY — layout-required guard wrapper
├── lib/
│   ├── components/
│   │   ├── LayoutPicker/          # NEW — layout picker UI
│   │   │   ├── LayoutPicker.svelte
│   │   │   ├── LayoutEntry.svelte
│   │   │   └── NewLayoutDialog.svelte
│   │   ├── ConnectionManager/     # MODIFY — connection defs from layout, multi-connection
│   │   └── SaveProgress/          # NEW — modal save progress UI
│   │       └── SaveProgressDialog.svelte
│   ├── orchestration/
│   │   ├── saveLayoutOrchestrator.ts     # MODIFY — three-phase save flow
│   │   ├── offlineLayoutOrchestrator.ts  # MODIFY — layout-required startup
│   │   └── startupOrchestrator.ts        # NEW — layout picker lifecycle
│   ├── stores/
│   │   ├── layout.svelte.ts              # MODIFY — connection defs, known-layouts
│   │   └── saveProgress.svelte.ts        # NEW — save phase tracking
│   └── api/
│       ├── layout.ts                     # MODIFY — new IPC bindings
│       └── startup.ts                    # NEW — known-layout registry IPC
└── tests/ (colocated with modules)
```

**Structure Decision**: Extends existing Tauri + SvelteKit structure. No new projects or packages. New modules follow existing placement rules (components render, orchestrators sequence, stores hold state, backend owns persistence).

## Complexity Tracking

No constitution violations to justify.

## Architecture Assessment

### Affected Modules

| Module | Layer | Impact | Depth Now | Notes |
|--------|-------|--------|-----------|-------|
| `+page.svelte` | Route | Modified | Shallow (1,942-line god component) | Save flow extraction + layout picker gate |
| `saveLayoutOrchestrator.ts` | Orchestrator | Modified | Good (56 lines, tested, but bypassed) | Becomes canonical save path |
| `offlineLayoutOrchestrator.ts` | Orchestrator | Modified | Good (406 lines, tested) | Layout-required startup |
| `layout.svelte.ts` | Store | Modified | Moderate (mixes store + orchestration) | Connection defs, known-layouts |
| `api/layout.ts` | API | Modified | Thin (has duplicate wrappers) | New IPC bindings, cleanup duplicates |
| `api/bowties.ts` | API | Modified | Thin (naming confusion with layout.ts) | — |
| `commands/layout_capture.rs` | Backend command | Modified | Deep (673 lines, mega-function) | Three-phase save calls existing function |
| `commands/bowties.rs` | Backend command | Modified | Very deep (1,962 lines, 0 tests, mixed concerns) | Catalog rebuild in save flow |
| `commands/connection.rs` | Backend command | Modified | Shallow (98 lines, clean) | Connections move into layout |
| `layout/types.rs` | Backend domain | Modified | Good (tested) | Connections field |
| `layout/manifest.rs` | Backend domain | Modified | Thin | Connections + schema bump |
| `layout/io.rs` | Backend domain | Modified | Deep (699 lines, tested) | v3→v4 migration chain |
| `state.rs` | Backend domain | Modified | Moderate (god-struct) | Known-layout registry state |
| `lib.rs` | Backend | Modified | Moderate | Register new commands |
| `layout/known_layouts.rs` | Backend domain | **New** | — | Known-layout registry persistence |
| `startupOrchestrator.ts` | Orchestrator | **New** | — | Layout picker lifecycle |
| `saveProgress.svelte.ts` | Store | **New** | — | Save phase tracking |
| `LayoutPicker.svelte` | Component | **New** | — | Layout picker UI |
| `SaveProgressDialog.svelte` | Component | **New** | — | Modal save progress |
| `api/startup.ts` | API | **New** | — | Known-layout registry IPC |

### Assessment Summary

**Scale: Full** — 3+ layers affected, new modules needed, existing debt in touched modules, design trade-offs.

The feature touches modules across every layer of the stack. The riskiest seam is the save flow: `+page.svelte` currently inlines save-and-rebuild logic that bypasses `saveLayoutOrchestrator`, and the three-phase reorder must go through the orchestrator to be testable and maintainable. This extraction (S1) is the prerequisite for the core fix (S2).

The schema migration (v3→v4) is the first migration the codebase has needed — it sets the pattern for all future version bumps, making it HITL.

New modules (`known_layouts.rs`, `startupOrchestrator.ts`, `saveProgress.svelte.ts`) follow established patterns and carry low risk. The layout picker gate in `+page.svelte` changes startup flow and introduces a screen-level conditional render, which is HITL but well-scoped.

Four deepening opportunities were identified in touched modules but deferred to avoid scope creep: bowties.rs decomposition, bowties.rs test coverage, layout_capture.rs mega-function decomposition, and layout.svelte.ts orchestration extraction.

### Findings

**F1: `+page.svelte` god component blocks clean feature delivery** (depth/placement)
- **Affected**: `+page.svelte` (1,942 lines, ~40 `$state` variables)
- **Concern**: The route inlines `saveCurrentCaptureToFile` as a multi-step async workflow, bypassing `saveLayoutOrchestrator`. Adding the three-phase save here would deepen the god component rather than fixing the seam.
- **Decision**: Include — extract save flow to orchestrator as S1 prerequisite

**F2: `saveLayoutOrchestrator` bypassed — orphaned seam** (seam/leverage)
- **Affected**: `saveLayoutOrchestrator.ts`, `+page.svelte`
- **Concern**: The orchestrator exists and is tested but `+page.svelte` duplicates its logic. The three-phase save must unify through the orchestrator.
- **Decision**: Include — wire orchestrator as canonical save path in S1

**F3: `layout.svelte.ts` mixes store and orchestration** (placement)
- **Affected**: `layout.svelte.ts`
- **Concern**: The store owns dialog opening, IPC sequencing, and recent-layout orchestration — concerns that belong in orchestrators. Legacy file-mode methods appear to be dead code.
- **Decision**: Defer — not in critical path; captured as `specs/ideas/layout-store-orchestration-extraction.md`

**F4: `api/layout.ts` duplicate IPC wrappers** (duplication)
- **Affected**: `api/layout.ts`
- **Concern**: `saveLayoutFile` ≡ `saveLayoutDirectory` and `openLayoutFile` ≡ `openLayoutDirectory` call identical backend commands.
- **Decision**: Include — clean up as part of S11 when adding `saveLayoutWithBusWrites`

**F5: Connection definitions need schema migration** (depth)
- **Affected**: `layout/io.rs`, `layout/manifest.rs`
- **Concern**: First schema migration — establishes the pattern for all future migrations. Migration chain should be private in io.rs.
- **Decision**: Include as S4 (HITL)

**F6: `save_layout_directory` mega-function** (depth/locality)
- **Affected**: `commands/layout_capture.rs`
- **Concern**: ~150-line function mixing 5 concerns. The new `save_layout_with_bus_writes` calls it as a step rather than extending it.
- **Decision**: Include (implicitly) — new command preserves existing seam. Mega-function decomposition deferred as `specs/ideas/layout-capture-decomposition.md`

**F7: `bowties.rs` mixed concerns — untested core algorithm** (placement/testability)
- **Affected**: `commands/bowties.rs` (1,962 lines, 0 tests)
- **Concern**: Layout YAML commands mixed with catalog builder. Core algorithm has zero test coverage. Three-phase save calls `build_bowtie_catalog_command` without needing to add logic here.
- **Decision**: Defer — captured as `specs/ideas/bowties-rs-decomposition.md`

**F8: Event role classification semantics extension** (depth)
- **Affected**: `layout/types.rs`, `commands/bowties.rs`
- **Concern**: Semantic expansion with no structural change — same type, same merge function, more entries.
- **Decision**: Include as S9 (AFK)

**F9: Known-layout registry as new backend module** (new module)
- **Affected**: `layout/known_layouts.rs` (new)
- **Concern**: Good depth potential. Follows `connection.rs` pattern. Passes deletion test.
- **Decision**: Include as S5 (AFK)

**F10: Layout picker gate** (placement/seam)
- **Affected**: `+page.svelte`, `LayoutPicker.svelte`, `startupOrchestrator.ts`
- **Concern**: Screen-level gate. Must be maximally decoupled from the existing god component — standalone component + orchestrator, route does only conditional render.
- **Decision**: Include as S6 (HITL)

**F11: Layout-required guard placement** (depth)
- **Affected**: `+page.svelte`, `+layout.svelte`
- **Concern**: Guard is better as a conditional render in `+page.svelte` (picker vs main UI) than a wrapper in `+layout.svelte` (currently 33-line shell).
- **Decision**: Include in S6 — place guard in `+page.svelte` conditional render

### Vertical Slices

**S1: Extract save flow to orchestrator** (HITL)
- **Layers**: Route → Orchestrator
- **Blocked by**: None
- **Test**: Save triggers `saveLayoutOrchestrated()` not inline code; existing save tests pass
- **Acceptance**: `saveCurrentCaptureToFile` in `+page.svelte` delegates to `saveLayoutOrchestrator`; no inline save workflow remains in the route
- **Why HITL**: Establishes the orchestrator as the canonical save seam — pattern choice that affects all subsequent slices

**S2: Three-phase save — layout first, bus second, reconcile** (HITL)
- **Layers**: Orchestrator → API → Backend command → Backend domain
- **Blocked by**: S1
- **Test**: Online save writes layout before bus; bowties never show stale catalog during save; cancel before bus writes sends nothing to bus
- **Acceptance**: `save_layout_with_bus_writes` backend command implements three-phase flow; orchestrator calls it; ADR-0001 ordering enforced
- **Why HITL**: Creates a new backend command pattern (multi-phase with progress events), core architectural fix

**S3: Save progress tracking** (AFK)
- **Layers**: Store → Component → (Tauri event listener)
- **Blocked by**: S2
- **Test**: Progress store transitions through phases during save; dialog displays correct phase labels
- **Acceptance**: Modal `SaveProgressDialog` renders during save; progress updates per-field during bus writes

**S4: Schema migration v3→v4 + connections field** (HITL)
- **Layers**: Backend domain (io.rs, manifest.rs, types.rs)
- **Blocked by**: None
- **Test**: Load v3 manifest → migrates to v4 with empty connections; save → reopen → connections preserved; v2 rejected
- **Acceptance**: Migration chain in io.rs; `LayoutManifest` has `connections: Vec<ConnectionConfig>`
- **Why HITL**: First schema migration — establishes the migration pattern for all future versions

**S5: Known-layout registry backend** (AFK)
- **Layers**: API → Backend command → Backend domain
- **Blocked by**: None
- **Test**: CRUD on known-layouts.json; filters stale paths; atomic writes
- **Acceptance**: `known_layouts.rs` module with `get_known_layouts`, `add_known_layout`, `remove_known_layout`

**S6: Layout picker gate** (HITL)
- **Layers**: Route → Component → Orchestrator → Store → API
- **Blocked by**: S4, S5
- **Test**: App with no active layout shows picker; selecting a known layout opens it; "New Layout" creates and opens; picker disappears after selection
- **Acceptance**: `LayoutPicker.svelte` renders when `activeContext == null`; `startupOrchestrator.ts` manages lifecycle
- **Why HITL**: New screen-level gate pattern, changes startup flow

**S7: Connection definitions in layout** (AFK)
- **Layers**: API → Backend command → Backend domain
- **Blocked by**: S4
- **Test**: Add connection to layout manifest, persist, reopen, connection available
- **Acceptance**: `get_layout_connections` and `save_layout_connections` commands work; connections round-trip through save/open

**S8: Connect from within layout** (HITL)
- **Layers**: Route → Component → Orchestrator
- **Blocked by**: S6, S7
- **Test**: Connect using layout-stored connection; disconnect preserves layout; reconnect uses same settings; multi-connection selector appears when >1 defined
- **Acceptance**: Connection dialog uses layout connections; `lib.rs` connect path reads from active layout
- **Why HITL**: Changes the connection initiation flow; trade-off between reusing existing dialog vs new layout-aware dialog

**S9: Event role persistence** (AFK)
- **Layers**: Backend domain (bowties.rs, layout_capture.rs)
- **Blocked by**: S2
- **Test**: Protocol-resolved roles persist through save → close → reopen; ambiguous roles omitted
- **Acceptance**: `merge_layout_metadata` includes all resolved (non-ambiguous) roles from live catalog

**S10: Node visibility when connected** (AFK)
- **Layers**: Route → Component → Store
- **Blocked by**: S6
- **Test**: Connected with layout nodes not on bus shows badged entries; discovered nodes auto-added to layout
- **Acceptance**: Node list shows all layout nodes with "not on bus" badge for absent nodes; new bus nodes auto-included

**S11: API layer cleanup** (AFK)
- **Layers**: API
- **Blocked by**: S2
- **Test**: All callers compile; no duplicate wrappers remain
- **Acceptance**: Remove `saveLayoutFile`/`openLayoutFile` duplicates; clarify layout.ts vs bowties.ts boundary

### Slice Dependency Graph

```
S1 (extract save)    S4 (migration)     S5 (registry)
      │                   │  │                │
      ▼                   │  │                │
S2 (3-phase save)         │  ▼                │
      │                   │  S7 (conn defs)   │
      ├──────┐            │       │           │
      ▼      ▼            ▼       ▼           ▼
S3 (progress) S9 (roles)  S6 (picker gate)◄───┘
      S11 (cleanup)            │    │
                               ▼    ▼
                         S8 (connect) S10 (node vis)
```

### Deferred Improvements

- [`specs/ideas/layout-store-orchestration-extraction.md`](../ideas/layout-store-orchestration-extraction.md) — Extract dialog + IPC sequencing from `layout.svelte.ts` to orchestrators (F3)
- [`specs/ideas/layout-capture-decomposition.md`](../ideas/layout-capture-decomposition.md) — Decompose `save_layout_directory` mega-function in `layout_capture.rs` (F6)
- [`specs/ideas/bowties-rs-decomposition.md`](../ideas/bowties-rs-decomposition.md) — Decompose `bowties.rs` mixed concerns + add test coverage for `build_bowtie_catalog` (F7)

### Architecture Decisions

- [ADR 0001 — Save layout before bus writes](../../product/architecture/adr/0001-save-layout-before-bus-writes.md) (existing, reaffirmed)
