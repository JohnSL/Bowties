# Implementation Plan: Offline Layout Editing

**Branch**: `010-offline-layout-editing` | **Date**: 2026-04-04 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/010-offline-layout-editing/spec.md`

## Summary

Add offline-first layout capture and editing by moving persistence to a YAML directory format with per-node snapshots, explicit offline change tracking, and a gated sync workflow. Users can capture a live layout, open and edit it without bus connectivity, then reconnect and apply selected changes through a conflict-aware Sync Panel with deterministic match classification and partial-failure handling.

## Technical Context

**Language/Version**: Rust 2021 backend; TypeScript 5.6 + Svelte 5 + SvelteKit 2.9 frontend  
**Primary Dependencies**: Tauri 2, tokio 1.41, serde/serde_json, serde_yaml_ng 0.10, lcc-rs workspace crate, tauri-plugin-dialog 2  
**Storage**: YAML layout directory (`manifest.yaml`, `nodes/*.yaml`, metadata/offline change files), CDI cache references with optional export/import bundle  
**Testing**: `cargo test` (backend), Vitest 4 + Testing Library (frontend), end-to-end sync flow tests via existing Tauri test strategy  
**Target Platform**: Desktop app (Windows/macOS/Linux) via Tauri 2  
**Project Type**: Desktop application (Rust backend + Svelte frontend)  
**Performance Goals**: Meet SC-001..SC-008, especially offline open <3s and bulk apply under scenario thresholds  
**Constraints**: No automatic bus writes; deterministic YAML serialization; atomic multi-file save; explicit mode gating for uncertain bus matches  
**Scale/Scope**: Typical layouts 5-50 nodes, up to low hundreds of pending change rows, one active layout context at a time

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Constitution Principle | Status | Notes |
|---|----------------------|--------|-------|
| I | Rust 2021+ Development | PASS | Backend work remains Rust 2021 with typed error flow and no panic-based production paths |
| II | Cargo-Based Development | PASS | Existing cargo workspace and dependency model unchanged; feature uses existing crates plus current YAML stack |
| III | Test-Driven Development | PASS | Plan includes backend unit/integration tests for save/sync and frontend tests for offline indicators + sync panel flows |
| IV | LCC Protocol Correctness | PASS | Bus writes continue through existing Memory Configuration paths in lcc-rs; no protocol extensions required |
| V | UX-First Design | PASS | Feature is user-facing offline workflow with explicit status banner, conflict clarity, and controlled sync UX |
| VI | TCP-Only Focus | PASS | No new transport; uses current connection stack |
| VII | Event Management Excellence | PASS | Captures producer-identified events, preserves role classifications, and includes conflict-safe event sync |

**Gate Result**: ALL PASS - proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/010-offline-layout-editing/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── tauri-ipc.md
└── tasks.md            # Created later by /speckit.tasks
```

### Source Code (repository root)

```text
app/src-tauri/src/
├── commands/
│   ├── layout_capture.rs        # NEW: capture/open/save/close/new-layout commands
│   ├── sync_panel.rs            # NEW: build/apply sync session commands
│   └── bowties.rs               # EXTEND: offline bowtie edits + staged nodes
├── layout/
│   ├── mod.rs
│   ├── manifest.rs              # NEW: schema + migration handling
│   ├── node_snapshot.rs         # NEW: per-node snapshot structures
│   ├── offline_changes.rs       # NEW: persisted pending rows
│   └── io.rs                    # NEW: deterministic YAML + staging-and-swap writes
├── cdi/
│   └── bundle.rs                # NEW: export/import CDI package support
└── state.rs                     # EXTEND: active layout context, sync mode, pending row cache

app/src/
├── lib/
│   ├── api/
│   │   ├── layout.ts            # NEW: layout open/save/close/capture wrappers
│   │   └── sync.ts              # NEW: match/session/apply wrappers
│   ├── stores/
│   │   ├── layout.svelte.ts     # NEW: active layout + offline status
│   │   ├── offlineChanges.svelte.ts # NEW: baseline/planned row tracking
│   │   └── syncPanel.svelte.ts  # NEW: conflict/clean/already-applied state
│   └── components/
│       ├── Layout/
│       │   ├── OfflineBanner.svelte
│       │   ├── LayoutSwitcher.svelte
│       │   └── MissingCaptureBadge.svelte
│       └── Sync/
│           ├── SyncPanel.svelte
│           ├── ConflictRow.svelte
│           └── CleanSummarySection.svelte
└── routes/
    └── +page.svelte             # EXTEND: startup auto-load, no-layout state, sync entry flow
```

**Structure Decision**: Keep existing Bowties Tauri architecture. Add dedicated backend modules for capture/sync persistence and frontend stores/components for offline status and sync workflows. No new workspace or service boundary required.

## Complexity Tracking

No constitution violations requiring exception handling.

## Post-Design Constitution Re-Check

| # | Constitution Principle | Status | Post-Design Notes |
|---|----------------------|--------|-------------------|
| I | Rust 2021+ Development | PASS | Data model and command contracts use typed Rust structures and error propagation |
| II | Cargo-Based Development | PASS | Plan remains within existing cargo workspace and dependency strategy |
| III | Test-Driven Development | PASS | Quickstart and contracts define verifiable capture/offline/sync acceptance flows and failure handling tests |
| IV | LCC Protocol Correctness | PASS | Sync apply uses established write mechanisms and preserves protocol semantics |
| V | UX-First Design | PASS | Design specifies explicit offline indicators, conflict triage, and low-friction layout switching |
| VI | TCP-Only Focus | PASS | No deviation from existing transport scope |
| VII | Event Management Excellence | PASS | Event capture/sync fidelity and role persistence are first-class in the model |

**Post-Design Gate Result**: ALL PASS.
