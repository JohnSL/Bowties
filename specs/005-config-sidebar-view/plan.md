# Implementation Plan: Configuration Tab — Sidebar and Element Card Deck

**Branch**: `005-config-sidebar-view` | **Date**: 2026-02-22 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/005-config-sidebar-view/spec.md`

## Summary

Replace the Miller Columns CDI navigation layout with a two-panel design: a fixed-width left sidebar listing discovered nodes and their CDI segments, and a scrollable main area displaying an accordion card deck showing all configuration fields for the selected segment. All existing config reading and caching (feature 004) continues unchanged; this feature is primarily a layout and navigation change, with one new Tauri backend command (`get_card_elements`) added to support efficient recursive card-body rendering.

## Technical Context

**Language/Version**: Rust 2021 (backend, `app/src-tauri/src/`), TypeScript 5.x strict mode (frontend, `app/src/`)  
**Primary Dependencies**: SvelteKit 2.x, Tauri 2.x, lcc-rs (internal library), tokio, serde, uuid  
**Storage**: In-memory config value cache (`millerColumns.ts` Svelte store, `ConfigValueMap`); backend CDI parse cache (`CDI_PARSE_CACHE` lazy_static `Arc<RwLock<HashMap<String, lcc_rs::cdi::Cdi>>>`)  
**Testing**: `cargo test` (Rust unit + integration), Vitest (SvelteKit frontend component tests)  
**Target Platform**: Desktop (Windows, macOS, Linux) via Tauri 2  
**Project Type**: Web application (SvelteKit frontend + Tauri/Rust backend)  
**Performance Goals**: All element cards visible within 500ms of segment selection using cached values (SC-002)  
**Constraints**: Fixed-width sidebar (non-resizable in this iteration); read-only fields (no [W] action, FR-010); navigation reachable in ≤3 clicks (SC-001)  
**Scale/Scope**: Desktop app, single-user, real-time LCC network; typically 1–20 nodes, 1–5 segments each, 10–100 elements per segment

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Rust 2021+ | ✅ PASS | New `get_card_elements` Tauri command uses Rust 2021 idioms; `Result<T, String>`, `?` operator, no `unwrap()` in production paths |
| II. Cargo-Based | ✅ PASS | No Cargo toolchain changes; new command added to existing `cdi.rs` module |
| III. TDD | ✅ REQUIRED | New Svelte components need Vitest tests; `get_card_elements` needs Rust unit tests; `resolveCardTitle()` utility needs unit tests |
| IV. LCC Protocol Correctness | ✅ PASS | No new protocol code; memory addresses computed from existing CDI hierarchy (`calculate_size()`, `expand_replications()`); no new wire-format handling |
| V. UX-First | ✅ PASS | SC-001 (3 clicks max), SC-002 (<500ms), SC-003 (named vs unnamed visible without expansion), SC-004 (descriptions and sub-groups hidden until requested) |
| VI. TCP-Only | ✅ PASS | No transport changes |
| VII. Event Management Excellence | ✅ PASS | FR-013 (event slot raw ID shown), FR-014 ("(free)" for unset slots), dotted-hex format per Constitution §VII |

**Post-Phase-1 re-check**: All gates still pass. No violations.

## Project Structure

### Documentation (this feature)

```text
specs/005-config-sidebar-view/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   ├── get_card_elements.json      # New Tauri command contract (RQ-001)
│   └── configSidebar-store.ts      # Store type contract
└── tasks.md             # Phase 2 output (/speckit.tasks command — NOT created here)
```

### Source Code (repository root)

```text
app/src-tauri/src/commands/
└── cdi.rs                          # ADD: get_card_elements() command (~100 lines Rust)

app/src/lib/
├── components/
│   ├── ConfigSidebar/
│   │   ├── ConfigSidebar.svelte        # Main sidebar: node list + segment navigation
│   │   ├── NodeEntry.svelte            # Expandable node row (with CDI context menu)
│   │   ├── SegmentEntry.svelte         # Selectable segment item within expanded node
│   │   └── ConfigSidebar.test.ts       # Vitest unit tests
│   └── ElementCardDeck/
│       ├── ElementCardDeck.svelte      # Scrollable accordion card deck container
│       ├── ElementCard.svelte          # Single accordion card (one top-level CDI group)
│       ├── FieldRow.svelte             # Read-only config field with [R] and [?] actions
│       ├── EventSlotRow.svelte         # Event ID field row (dotted-hex or "(free)")
│       ├── ElementCardDeck.test.ts     # Vitest unit tests
│       └── ElementCard.test.ts        # Vitest unit tests
├── stores/
│   └── configSidebar.ts               # Sidebar + card deck state (new Svelte store)
└── utils/
    └── cardTitle.ts                   # resolveCardTitle() utility (FR-007 naming, RQ-002)

app/src/routes/config/
└── +page.svelte                       # REPLACE MillerColumnsNav with ConfigSidebar + ElementCardDeck
```

**Structure Decision**: Web Application layout (SvelteKit frontend + Tauri backend). New components follow the established feature-folder convention (`MillerColumns/` → `ConfigSidebar/`, `ElementCardDeck/`). New store and utility follow existing `lib/stores/` and `lib/utils/` patterns.

## Complexity Tracking

No constitution violations requiring justification.
