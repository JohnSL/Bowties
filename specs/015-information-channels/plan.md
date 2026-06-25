# Implementation Plan: Information Channels — Auto-Create & Inventory

**Branch**: `015-information-channels` | **Date**: 2026-06-24 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/015-information-channels/spec.md`

## Summary

Introduce the **information channel** abstraction — a typed, named representation of a single piece of layout-meaningful information (e.g., "Block 7 Occupancy"). Channels are auto-created when a BOD-family daughter board is selected for a TowerLCC connector, displayed in a new **Railroad tab** as a channel inventory, and renamable by the user. Channel data persists in `channels.yaml` in the layout folder. This is the foundational data layer; no live state, behavior, or wiring is in scope.

**Technical approach**: Add a `channels.yaml` persistence file to the layout folder (backend, `bowties-core`), expose channel CRUD via new Tauri IPC commands, add a `channelsStore` (frontend store), hook channel auto-creation into the existing `connectorSelectionOrchestrator` workflow, and render the inventory in a new Railroad tab on `+page.svelte`.

## Technical Context

**Language/Version**: Rust 2021 (stable 1.70+) backend; TypeScript 5.6 / Svelte 5 / SvelteKit 2.9 frontend  
**Primary Dependencies**: Tauri 2, tokio, serde/serde_yaml (backend); @tauri-apps/api, @testing-library/svelte, Vitest 4 (frontend)  
**Storage**: YAML files in layout folder (`channels.yaml` alongside `bowties.yaml`, `manifest.yaml`)  
**Testing**: `cargo test` (Rust unit/integration); Vitest + @testing-library/svelte (frontend unit/component)  
**Target Platform**: Windows, macOS, Linux (Tauri desktop)  
**Project Type**: Desktop app (Tauri 2 — Rust backend + SvelteKit frontend)  
**Performance Goals**: Channel creation/rename must be instant (<50ms user-perceived); Railroad tab must render smoothly for up to 100 channels  
**Constraints**: Offline-capable (channels are layout-level abstractions, never written to nodes); additive to existing layout folder (no existing files modified)  
**Scale/Scope**: Typical layout: 1–10 nodes × 2 connectors × up to 8 inputs = up to ~160 channels maximum

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Rust 2021+ Development | ✅ PASS | Backend channel types, persistence, and IPC commands implemented in Rust. No `unwrap()` in production paths; `Result<T, E>` throughout. |
| II. Cargo-Based Development | ✅ PASS | New types in `bowties-core` crate; `Cargo.toml` unchanged (serde_yaml already a dependency). |
| III. Test-Driven Development | ✅ PASS | Unit tests for channel types/persistence in `bowties-core`; Vitest tests for `channelsStore`, Railroad tab component, and orchestrator integration. |
| IV. LCC Protocol Correctness | ✅ N/A | Channels are layout-level abstractions — no protocol messages involved. Hardware reference uses node ID but no wire-level encoding. |
| V. UX-First Design | ✅ PASS | Auto-creation eliminates manual setup; inline rename is minimal-friction; Railroad tab provides at-a-glance inventory. |
| VI. TCP-Only Focus | ✅ N/A | No transport involvement — channels are metadata stored in layout files. |
| VII. Event Management Excellence | ✅ PASS | Channels are the foundation for the next layer of event-to-layout mapping. This feature does not modify event management but extends the information model it will build on. |
| Architecture Constraints (separation) | ✅ PASS | `bowties-core`: types + persistence. `app/src-tauri`: IPC commands. `app/src/lib/stores`: `channelsStore`. `app/src/lib/orchestration`: channel auto-creation hook in `connectorSelectionOrchestrator`. `app/src/routes`: Railroad tab rendering. |
| No Circular Dependencies | ✅ PASS | `bowties-core` → independent. `app/src-tauri` → depends on `bowties-core`. Frontend → depends on Tauri IPC. |

## Project Structure

### Documentation (this feature)

```text
specs/015-information-channels/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (Tauri IPC contracts)
└── tasks.md             # Phase 2 output (/speckit.tasks command — NOT created by /speckit.plan)
```

### Source Code (existing repository structure)

```text
bowties-core/
├── src/
│   └── layout/
│       ├── types.rs           # + InformationChannel, ChannelType, HardwareReference structs
│       ├── channels.rs        # NEW: channels.yaml read/write, channel CRUD logic
│       ├── mod.rs             # + channels.yaml in read_capture / save_capture
│       └── io.rs              # + channels.yaml file I/O

app/src-tauri/
├── src/
│   └── commands/
│       └── channels.rs        # NEW: Tauri IPC commands (list, create, rename, delete)

app/src/
├── lib/
│   ├── stores/
│   │   └── channels.svelte.ts          # NEW: channelsStore — channel inventory state
│   ├── orchestration/
│   │   └── connectorSelectionOrchestrator.ts  # MODIFIED: hook channel auto-create/remove
│   ├── api/
│   │   └── channels.ts                 # NEW: typed Tauri invoke wrappers
│   ├── components/
│   │   └── RailroadTab/                # NEW: Railroad tab components
│   │       ├── RailroadTab.svelte      # Channel inventory view
│   │       ├── ChannelGroup.svelte     # Group-by-type rendering
│   │       └── ChannelRow.svelte       # Single channel with inline rename
│   └── utils/
│       └── channelDefaults.ts          # NEW: default name generation logic
└── routes/
    └── +page.svelte                    # MODIFIED: add Railroad tab to tab bar
```

**Structure Decision**: Follows existing layered architecture. New `channels.rs` in `bowties-core/src/layout/` for domain logic. New `channels.rs` Tauri command module for IPC. New `channelsStore` for frontend state. New `RailroadTab/` component directory for rendering. Channel auto-creation hooks into the existing `connectorSelectionOrchestrator` workflow rather than creating a new orchestrator.

## Complexity Tracking

No constitution violations — no justification needed.

## Architecture Assessment

### Affected Modules

| Module | Layer | Impact | Notes |
|--------|-------|--------|-------|
| `+page.svelte` | Route | Modified | Extends tab union to `config \| bowties \| railroad`; adds Railroad tab rendering |
| `connectorSelectionOrchestrator.ts` | Orchestrator | Modified | 4th step: channel auto-create/remove after compatibility recompute |
| `channelsStore.svelte.ts` | Store | New | Channel inventory state; createChannels / renameChannel / deleteChannels mutations |
| `channels.ts` (API) | API | New | Typed Tauri invoke wrappers for channel CRUD |
| `RailroadTab/` | Component | New | Channel inventory grouped by type; inline rename |
| `channelDefaults.ts` | Utils | New | Pure default name generation helper |
| `channels.rs` (commands) | Backend command | New | IPC boundary for channel CRUD; error translation |
| `channels.rs` (core) | Backend domain | New | `channels.yaml` read/write; domain types; CRUD logic |
| `bowties-core/layout/mod.rs` | Backend domain | Modified | Reads/writes `channels.yaml` in save_capture / read_capture |
| `bowties-core/layout/types.rs` | Backend domain | Modified | `CreateChannel` / `RenameChannel` / `DeleteChannel` delta variants |
| `layoutLifecycleOrchestrator.ts` | Orchestrator | Modified | Registers `channelsStore.reset()` in layout close path |
| Shared-daughterboards profile YAML | Profile | Modified | Adds `channelCount` metadata for BOD variants |

### Assessment Summary

Architecture confirmed with no concerns. The feature maps cleanly to established patterns: companion-file persistence (ADR-0005/0006), delta-based mutations (ADR-0002), orchestrator-owned workflows, and store-based frontend state. All new modules earn their depth — `channels.rs` (core) hides YAML schema, validation, and CRUD behind a small read/write interface; `channelsStore` hides IPC + state reconciliation behind create/rename/delete calls; `connectorSelectionOrchestrator` extension keeps "what happens when a board changes" in one owner.

Six findings evaluated; all included in slices. No deferrals, no rejected approaches, no new ADRs required.

### Findings

**F1: Orchestrator extension vs. new orchestrator**
- Category: depth / SOLID
- Affected: `connectorSelectionOrchestrator`
- Concern: Extending the existing 3-step flow with a 4th step (channel auto-create/remove) increases depth proportionally. A separate orchestrator would create two owners of one user action — shallow duplication.
- Decision: include — extend existing orchestrator

**F2: Lifecycle reset coverage**
- Category: locality
- Affected: `layoutLifecycleOrchestrator`, `channelsStore`
- Concern: Every layout-scoped store must be registered in the lifecycle reset path (ADR-0011). Missing `channelsStore.reset()` would leave stale channels after layout close — same class of bug recently fixed for connection state.
- Decision: include — S6 explicitly covers lifecycle integration with regression test

**F3: Read/write capture integration**
- Category: ADR compliance (ADR-0005)
- Affected: `bowties-core/layout/mod.rs`, `io.rs`
- Concern: `channels.yaml` must flow through `read_capture()` / `save_capture()` with `default()` fallback for pre-015 layouts. Standard companion-file pattern.
- Decision: include — covered in S2

**F4: LayoutEditDelta extension**
- Category: ADR compliance (ADR-0002)
- Affected: `bowties-core/layout/types.rs`
- Concern: Adding `CreateChannel`, `RenameChannel`, `DeleteChannel` to the delta enum follows the established mutation pattern. Three new match arms in `apply_layout_deltas()`.
- Decision: include — covered across S2–S5

**F5: Profile metadata extension — channelCount**
- Category: seam placement
- Affected: shared-daughterboards profile YAML, `connectorSelectionOrchestrator`
- Concern: Profile YAML is the single source of truth for board capabilities. `channelCount` is a real seam — two adapters already read it. Hardcoding in frontend would scatter board knowledge.
- Decision: include — covered in S3

**F6: No duplication found**
- Category: duplication (DRY)
- Affected: none
- Concern: No existing channel-like abstractions. Default name pattern is distinct from `nodeRoster` display name fallback.
- Decision: include — no action needed

### Vertical Slices

**S1: Railroad tab with stubbed channels** 
- Type: HITL
- Layers: Route → Component → Store → API → Backend command → Core (stub)
- Blocked by: None
- Test: Open layout → click Railroad tab → stubbed channel entries render grouped by type
- Acceptance: User sees Railroad tab button, clicks it, sees hardcoded channel entries
- Architecture note: Establishes IPC contract shape, store pattern, component hierarchy. Backend returns hardcoded data.

**S2: Backend channel persistence**
- Type: AFK
- Layers: Backend command → Core (`channels.rs`, `mod.rs`, `io.rs`)
- Blocked by: S1
- Test: Place `channels.yaml` in layout folder → open layout → Railroad tab shows those channels. Save layout → `channels.yaml` written.
- Acceptance: Channels survive layout close/reopen with full fidelity

**S3: Auto-create channels on BOD board selection**
- Type: HITL
- Layers: Orchestrator → Store → API → Backend command → Core, Profile YAML
- Blocked by: S2
- Test: Select BOD-8 on connector-a → 8 block-occupancy channels appear with correct default names
- Acceptance: Selecting any BOD variant creates the correct number of channels in Railroad tab
- Architecture note: Extends connectorSelectionOrchestrator step chain. Introduces CreateChannel delta and profile channelCount metadata.

**S4: Channel rename**
- Type: AFK
- Layers: Component → Store → API → Backend command → Core
- Blocked by: S1
- Test: Click channel name → edit inline → Enter → new name displayed. Close/reopen → name persists.
- Acceptance: Renamed channels persist across sessions with 100% fidelity

**S5: Channel removal on board change**
- Type: AFK
- Layers: Orchestrator → Store → API → Backend command → Core
- Blocked by: S3
- Test: Change BOD-8 to BOD4 → confirmation dialog → confirm → 8 removed, 4 created. Cancel → no change.
- Acceptance: Board change with confirmation removes old channels and creates new ones

**S6: Lifecycle integration + empty state**
- Type: AFK
- Layers: Orchestrator (lifecycle) → Store → Component
- Blocked by: S1
- Test: Close layout → channels cleared. Open layout with no `channels.yaml` → empty state guidance shown.
- Acceptance: After layout close, channelsStore is empty; Railroad tab shows empty state guidance

### Deferred Improvements

None — all findings included in slices.

### Architecture Decisions

No new ADRs required. Feature is ADR-compliant with ADR-0002, ADR-0005, ADR-0006, and ADR-0011.
