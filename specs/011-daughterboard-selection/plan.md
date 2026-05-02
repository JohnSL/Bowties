# Implementation Plan: Connector Daughterboard Selection

**Branch**: `011-daughterboard-selection` | **Date**: 2026-04-29 | **Spec**: `D:\src\github\LCC\Bowties.worktrees\011-daughterboard-selection\specs\011-daughterboard-selection\spec.md`
**Input**: Feature specification from `D:\src\github\LCC\Bowties.worktrees\011-daughterboard-selection\specs\011-daughterboard-selection\spec.md`

## Summary

Extend the existing structure-profile system so RR-CirKits Tower and Signal LCC carrier boards can declare connector slots, reference reusable daughterboard profiles, and compose carrier-specific overrides per slot. Persist per-node connector selections in saved layout or project state, resolve connector-governed constraints from authored profile data, surface per-slot selection controls in the configuration workflow, and automatically stage compatible follow-up config changes when a connector selection invalidates existing values.

## Technical Context

**Language/Version**: TypeScript 5.6 + Svelte 5 frontend, Rust 2021 Tauri backend, YAML profile and layout files  
**Primary Dependencies**: SvelteKit 2, Tauri 2, `@tauri-apps/api`, `tokio`, `serde`, `serde_yaml_ng`, `lcc-rs`, Vitest, Testing Library for Svelte  
**Storage**: Bundled and user-editable `.profile.yaml` files under the existing profile loader, plus saved layout or offline-layout YAML state for per-node connector selections and staged pending changes  
**Testing**: `vitest run`, targeted store/orchestrator/component tests, `svelte-check`, Rust unit tests via `cargo test` for profile parsing, layout persistence, and backend command behavior  
**Target Platform**: Tauri desktop app on Windows, macOS, and Linux  
**Project Type**: Desktop application with Svelte frontend, Tauri backend, and reusable Rust protocol library  
**Performance Goals**: Connector-aware tree annotations load with the existing `get_node_tree` first-render flow, connector selection changes recompute filtering and staged repairs locally without extra LCC network reads, and supported RR-CirKits node workflows remain responsive during single-node config editing  
**Constraints**: Profile-authored rules are the only compatibility source, no hardcoded board-specific decision tables outside profiles, per-node selections must persist in saved layout/project context, unsupported nodes must retain current behavior, SPROG IO-LCC remains deferred until connector compatibility is confirmed  
**Scale/Scope**: Initial scope covers RR-CirKits Tower-LCC and Signal LCC-32H/Signal LCC-S/Signal LCC-P carrier families, a shared reusable daughterboard set for RR-CirKits aux-port cards, and a small number of connector slots per node with dozens of affected lines/sections per supported profile

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Rust 2021+ Development**: PASS. Backend work stays in `app/src-tauri/src/**` with Rust 2021 types, `serde_yaml_ng`, and existing Tauri command patterns. No new protocol logic is planned for `lcc-rs` unless a reusable CDI/path rule emerges.
- **Cargo-Based Development Environment**: PASS. Backend changes remain in the existing Tauri crate and use current Cargo-managed dependencies.
- **Test-Driven Development**: PASS. The feature plan targets focused tests at the owning seams: Rust tests for profile schema and layout persistence, Vitest store/orchestrator/component tests for connector selection state, filtering, and staged repairs.
- **LCC Protocol Correctness**: PASS. This feature does not change wire-level protocol semantics; it constrains UI/config workflows using profile-authored metadata and existing config write paths.
- **UX-First Design**: PASS. The design centers on per-slot hardware selection, narrower valid choices, and automatic staged compatible follow-up edits rather than exposing users to manual repair logic.
- **TCP-Only Focus**: PASS. Transport support is unaffected.
- **Event Management Excellence**: PASS. Existing profile loading and config-tree annotation remain the enrichment seam; no bowtie/event regression is introduced by design.

**Post-Design Re-check**: PASS. The generated research, data model, contracts, and quickstart keep protocol behavior unchanged, preserve existing ownership boundaries, and commit to owner-level tests rather than broad end-to-end-only coverage.

## Project Structure

### Documentation (this feature)

```text
specs/011-daughterboard-selection/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── connector-daughterboard.openapi.yaml
│   └── structure-profile.schema.json
└── tasks.md
```

### Source Code (repository root)

```text
app/
├── src/
│   ├── lib/
│   │   ├── api/
│   │   ├── components/
│   │   ├── orchestration/
│   │   ├── stores/
│   │   ├── types/
│   │   └── utils/
│   └── routes/
└── src-tauri/
    ├── profiles/
    └── src/
        ├── commands/
        ├── layout/
        ├── node_tree.rs
        ├── profile/
        └── state.rs

lcc-rs/
└── src/

product/
├── architecture/
└── quality/
```

**Structure Decision**: Keep profile parsing, CDI-path resolution, saved layout persistence, and authoritative connector metadata in the Tauri backend; keep multi-step selection and auto-repair workflows in frontend orchestrators/stores; keep rendering of connector controls and constrained choices in Svelte components. Extend existing `app/src-tauri/src/profile/**`, `app/src-tauri/src/layout/**`, `app/src/lib/stores/**`, `app/src/lib/orchestration/**`, and `app/src/lib/components/**` instead of introducing new top-level modules.

## Complexity Tracking

No constitution violations currently require justification.
