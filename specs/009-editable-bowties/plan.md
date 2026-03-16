# Implementation Plan: Editable Bowties

**Branch**: `009-editable-bowties` | **Date**: 2026-03-15 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/009-editable-bowties/spec.md`

## Summary

Make the Bowties tab editable with bidirectional config sync, YAML persistence for connection names/tags, and multiple creation modes (intent-first and config-first). Users will be able to create, modify, and manage connections between LCC node event slots visually — without needing to know or type event IDs. A New Connection dialog provides dual element pickers; a context action in the Configuration tab offers config-first entry. All bowtie metadata persists in a user-managed YAML layout file. Unsaved changes are tracked and saved/discarded together with config edits.

## Technical Context

**Language/Version**: Rust 2021 (stable 1.70+) backend; TypeScript 5.6 / Svelte 5 / SvelteKit 2.9 frontend  
**Primary Dependencies**: Tauri 2, tokio 1.41, serde_yaml_ng 0.10, lcc-rs (workspace crate), TailwindCSS 4.2  
**Storage**: User-managed YAML layout file (serde_yaml_ng); in-memory bowtie catalog (AppState); pending edits (frontend Svelte store)  
**Testing**: Vitest 4.0 + @testing-library/svelte (frontend); cargo test (backend); jsdom test environment  
**Target Platform**: Desktop (Windows, macOS, Linux) via Tauri 2  
**Project Type**: Desktop application (Tauri: Rust backend + SvelteKit frontend)  
**Performance Goals**: Bowtie catalog rebuild <1s for typical layouts (50-100 bowties); bidirectional sync latency <1s (SC-002); connection creation <60s user workflow (SC-001)  
**Constraints**: No partial writes to nodes (FR-029); sequential multi-node writes with rollback (FR-029a); YAML file human-readable (FR-025)  
**Scale/Scope**: Typical layout: 5-50 nodes, 10-200 bowties, 1 active layout file at a time

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Constitution Principle | Status | Notes |
|---|----------------------|--------|-------|
| I | Rust 2021+ Development | PASS | All backend code uses Rust 2021; new bowtie edit commands use `Result<T, Error>`, no `unwrap()` in production paths |
| II | Cargo-Based Development | PASS | New dependencies (if any) added to existing `Cargo.toml`; `serde_yaml_ng` already present for profile YAML |
| III | Test-Driven Development | PASS | Each new Tauri command requires integration tests; each new Svelte component requires Vitest tests; store mutations require unit tests |
| IV | LCC Protocol Correctness | PASS | Write operations use existing Memory Configuration protocol via lcc-rs; event ID format follows dotted hex standard; Identify Events exchange already validated |
| V | UX-First Design | PASS | Core feature: visual connection creation replaces manual event ID entry; inline naming; unsaved indicators; element pickers with search |
| VI | TCP-Only Focus | PASS | No new transport dependencies; all writes go through existing TCP transport layer |
| VII | Event Management Excellence | PASS | This feature IS the event management excellence goal — creating, modifying, and organizing event connections visually |

**Gate Result**: ALL PASS — proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/009-editable-bowties/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
app/src-tauri/src/
├── commands/
│   └── bowties.rs           # Extended: bowtie edit commands (create, add/remove element, write)
├── state.rs                 # Extended: BowtieEditState, LayoutFile types
├── layout/                  # NEW: YAML layout file persistence
│   ├── mod.rs               #   module root
│   ├── types.rs             #   LayoutFile, BowtieMetadata, RoleClassification serde types
│   └── io.rs                #   save/load/merge with node state
└── profile/                 # Existing: event role annotations

app/src/
├── lib/
│   ├── api/
│   │   └── bowties.ts       # NEW: Tauri IPC wrappers for bowtie edit commands
│   ├── stores/
│   │   ├── bowties.svelte.ts    # Extended: editable bowtie catalog, unsaved tracking
│   │   ├── pendingEdits.svelte.ts # Extended: unified save/discard lifecycle
│   │   └── layout.svelte.ts     # NEW: layout file state (path, dirty, recent)
│   ├── components/
│   │   ├── Bowtie/
│   │   │   ├── BowtieCard.svelte           # Extended: edit actions, unsaved indicators
│   │   │   ├── BowtieCatalogPanel.svelte   # Extended: + New Connection, filter
│   │   │   ├── NewConnectionDialog.svelte  # NEW: dual element picker dialog
│   │   │   ├── ElementPicker.svelte        # NEW: browsable node/element tree
│   │   │   ├── RoleClassifyPrompt.svelte   # NEW: ambiguous role prompt
│   │   │   └── *.test.ts                   # Tests for all new/modified components
│   │   └── ElementCardDeck/
│   │       └── TreeLeafRow.svelte          # Extended: "Create Connection from Here" context action
│   └── types/
│       ├── bowtie.ts        # NEW: editable bowtie types, layout file types
│       └── nodeTree.ts      # Existing: tree helpers
└── routes/
    └── +page.svelte         # Extended: layout file open/save controls
```

**Structure Decision**: Follows existing Tauri desktop app structure (`app/src-tauri/` backend, `app/src/` frontend). New layout persistence module in backend; new dialog components and store extensions in frontend. No new workspaces or crates needed.

## Complexity Tracking

> No constitution violations to justify — all gates pass.

## Post-Design Constitution Re-Check

*Re-evaluated after Phase 1 design artifacts were completed.*

| # | Constitution Principle | Status | Post-Design Notes |
|---|----------------------|--------|-------------------|
| I | Rust 2021+ Development | PASS | New `layout/` module uses `Result<T, Error>`, `BTreeMap`, `serde` derives. No `unwrap()` in production paths. |
| II | Cargo-Based Development | PASS | One new dependency: `tauri-plugin-dialog` v2 (actively maintained, Tauri ecosystem). `serde_yaml_ng` already present. |
| III | Test-Driven Development | PASS | Plan includes: Vitest for all new components/stores, cargo tests for layout I/O and catalog merge, rollback property tests. |
| IV | LCC Protocol Correctness | PASS | All writes use existing Memory Configuration protocol. No new protocol extensions. Event ID format unchanged. |
| V | UX-First Design | PASS | Visual picker replaces manual event ID entry. Intent-first mode, unsaved indicators, inline naming all improve UX. |
| VI | TCP-Only Focus | PASS | No transport changes. |
| VII | Event Management Excellence | PASS | This feature delivers the core event management vision: visual connection creation, modification, and organization. |

**Post-Design Gate Result**: ALL PASS.
