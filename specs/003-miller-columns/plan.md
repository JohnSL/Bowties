# Implementation Plan: Miller Columns Configuration Navigator

**Branch**: `003-miller-columns` | **Date**: 2026-02-17 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/003-miller-columns/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Implement dynamic Miller Columns navigation for browsing OpenLCB node CDI structure hierarchy. The navigator displays discovered nodes, CDI segments, groups (with replication support), and configuration elements with a focus on discovering Event ID elements for producer/consumer linking. Uses SvelteKit frontend components with Tauri backend for CDI XML parsing and traversal. Follows macOS Finder pattern with variable column count based on CDI structure depth (3 to 8+ levels).

## Technical Context

**Language/Version**: TypeScript 5.x (frontend), Rust 2021+ edition (backend via Tauri 2)  
**Primary Dependencies**: SvelteKit 2.x, Tauri 2.x, lcc-rs (existing LCC protocol library), Tauri events  
**Storage**: In-memory CDI cache (already retrieved by Feature F2 - CDI Caching dependency)  
**Testing**: Vitest (frontend component tests), cargo test (Rust backend), end-to-end Tauri integration tests  
**Target Platform**: Desktop (Windows, macOS, Linux) via Tauri desktop application
**Project Type**: Web application (Tauri hybrid - SvelteKit frontend + Rust backend)  
**Performance Goals**: Column population <500ms for typical CDI (1000 elements), column add/remove <200ms, navigation response <100ms
**Constraints**: Render up to 100 replicated groups without UI freeze, support 8-level hierarchy depth, no virtual scrolling (simple rendering)
**Scale/Scope**: Handle CDI structures with 1000+ total elements across 8 hierarchy levels, up to 100 replicated group instances per level

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Evidence/Notes |
|-----------|--------|----------------|
| I. Rust 2021+ Development | ✅ PASS | Backend CDI parsing in Rust via Tauri, uses lcc-rs library |
| II. Cargo-Based Development | ✅ PASS | Tauri backend uses Cargo, existing lcc-rs library |
| III. Test-Driven Development | ⚠️ VERIFY | Must ensure tests written for CDI parsing logic and UI components (Vitest + cargo test) |
| IV. LCC Protocol Correctness | ✅ PASS | Uses CDI specification S-9.7.4.1, references technical docs, reads CDI XML per standard |
| V. UX-First Design | ✅ PASS | Entire feature spec is UX-focused with user scenarios, macOS Finder pattern for familiarity |
| VI. TCP-Only Focus | ✅ PASS | Assumes CDI already retrieved via Feature F2, no new transport requirements |
| VII. Event Management Excellence | ✅ PASS | Primary goal is discovering Event ID elements for producer/consumer linking |

**Gate Result**: ✅ **PASS** (with TDD verification required during Phase 2 implementation)

**Rationale**: This feature aligns with all constitution principles. It uses the established Tauri + SvelteKit stack, leverages existing lcc-rs library, follows LCC standards for CDI parsing, and focuses on UX for Event ID discovery (core mission). No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/003-miller-columns/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   └── tauri-commands.json  # TypeScript API contracts for Tauri commands
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
app/
├── src/                         # SvelteKit frontend
│   ├── lib/
│   │   ├── components/
│   │   │   ├── MillerColumns/
│   │   │   │   ├── MillerColumnsNav.svelte    # Main container
│   │   │   │   ├── Column.svelte              # Reusable column component
│   │   │   │   ├── NodesColumn.svelte         # Nodes column (leftmost)
│   │   │   │   ├── NavigationColumn.svelte    # Segments/Groups/Elements columns
│   │   │   │   ├── DetailsPanel.svelte        # Details panel (rightmost)
│   │   │   │   └── Breadcrumb.svelte          # Navigation breadcrumb
│   │   │   └── ...
│   │   ├── stores/
│   │   │   └── millerColumns.ts               # Navigation state management
│   │   └── api/
│   │       └── cdi.ts                         # Tauri command wrappers (typed)
│   └── routes/
│       └── config/
│           └── +page.svelte                   # Miller Columns view page
│
└── src-tauri/                   # Rust Tauri backend
    ├── src/
    │   ├── commands/
    │   │   └── cdi.rs                         # Tauri commands for CDI operations
    │   ├── cdi/
    │   │   ├── parser.rs                      # CDI XML parsing logic
    │   │   ├── hierarchy.rs                   # Hierarchy navigation/traversal
    │   │   └── mod.rs
    │   └── main.rs
    └── Cargo.toml

lcc-rs/                          # Existing LCC protocol library (dependency)
└── src/
    └── cdi/
        └── ...                                # CDI types may already exist

tests/                           # Integration tests
├── miller-columns-ui.test.ts                  # Frontend component tests (Vitest)
└── integration/
    └── cdi_navigation.rs                      # Rust integration tests (cargo test)
```

**Structure Decision**: Tauri hybrid application with clear frontend/backend separation. Frontend uses SvelteKit with feature-based component organization under `lib/components/MillerColumns/`. Backend Tauri commands in `src-tauri/src/commands/cdi.rs` call CDI parsing logic. Navigation state managed in Svelte stores. Follows constitution principle of separation between lcc-rs (protocol library), Tauri backend (bridge), and SvelteKit frontend (UI).

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

**No violations detected.** All constitution principles are satisfied by this feature design.
