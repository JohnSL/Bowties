# Implementation Plan: ABS Signaling

**Branch**: `020-abs-signaling` | **Date**: 2026-07-07 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/020-abs-signaling/spec.md`

## Related refactors

- **Composer event-wiring unification** ([plan-event-wiring.md](plan-event-wiring.md), tracker [slices-event-wiring.md](slices-event-wiring.md)) — prerequisite to closing the S6 residual-catalog-card bug on Un-Wire. Retires the compile-side event-ID authority in `logic_adapter::compile_facility`; consolidates all event-ID wiring (composed and compiled templates) under `facility_bowties::compose_bowtie_ops`. Supersedes ADR-0015 §"2026-07-25 extension" by eliminating the two-owner situation that extension addressed.

## Summary

ABS Signaling introduces behavior templates that compile abstract railroad signaling rules into Tower LCC conditional line CDI writes, signal-aspect channels with hardware style mappings, and logic allocation tracking with capacity enforcement. The core value proposition: users select a template, map inputs and outputs, choose a target node, and Bowties compiles the entire conditional-line configuration — eliminating manual Tower LCC configuration for ABS signals.

The implementation extends the existing channel/facility/slot architecture (spec 018) with: a new `signal-aspect` channel role and `2-led-bicolor-aspect` style, a YAML-defined ABS 3-Aspect Signal behavior template, a Tower LCC Logic Adapter that compiles template rules to mast-grouped conditional lines, logic allocation records for resource tracking, and same-node cascade wiring via Track Circuits.

## Technical Context

**Language/Version**: Rust 2021+ (stable 1.70+) for backend/core; TypeScript + Svelte 5 (runes) for frontend  
**Primary Dependencies**: Tauri 2 (desktop framework), SvelteKit (UI), bowties-core (domain logic), lcc-rs (LCC protocol)  
**Storage**: Layout YAML files (backend-owned, ADR-0002/0015); logic allocation records persisted as part of layout data  
**Testing**: `cargo test` (Rust backend/core), `vitest` (frontend stores/utils/orchestration)  
**Target Platform**: Windows/macOS/Linux desktop (Tauri)  
**Project Type**: Desktop application with Tauri backend + SvelteKit frontend  
**Performance Goals**: Compilation of a 3-aspect signal template <100ms; apply workflow <3 min user time (SC-001)  
**Constraints**: Tower LCC hardware limits: 32 conditional lines/node, 8 Track Circuits/node, 4 actions/line; contiguous line allocation required  
**Scale/Scope**: Typical layout: 4-8 signal facilities per Tower LCC node; first slice supports same-node cascade only

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Rust 2021+ Development | **PASS** | Template compiler, logic adapter, allocation records all in Rust (bowties-core). No unwrap() in production paths. |
| II. Cargo-Based Development | **PASS** | All Rust code managed via Cargo.toml in bowties-core and app/src-tauri. |
| III. Test-Driven Development | **PASS** | Template compilation is a pure function — ideal for unit tests. Allocation tracking testable with mock node state. Frontend stores tested via vitest. |
| IV. LCC Protocol Correctness | **PASS** | CDI field writes must match Tower LCC CDI structure exactly. Conditional line fields, Track Circuit sources, and mast group flags validated against profile extraction data. |
| V. UX-First Design | **PASS** | Eliminates manual Tower LCC conditional config. User selects template + maps I/O + applies; compilation is invisible. Capacity displayed before apply. |
| VI. TCP-Only Focus | **PASS** | No transport changes. CDI writes use existing sync/write infrastructure. |
| VII. Event Management Excellence | **PASS** | Signal-aspect channel events displayed in human-readable format. Aspect-to-event map validated at bind time. |

No violations. All gates pass.

## Project Structure

### Documentation (this feature)

```text
specs/020-abs-signaling/
├── spec.md              # Feature specification (done)
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
bowties-core/src/
├── behavior_templates.rs          # Template registry + ABS template definition (extend existing)
├── logic_adapter/                 # NEW: Tower LCC logic adapter
│   ├── mod.rs                     # Adapter trait + Tower LCC implementation
│   ├── compiler.rs                # Template → conditional line compilation
│   ├── allocation.rs              # Logic allocation record + capacity tracking
│   └── tests/                     # Unit tests for compilation + allocation
└── layout/
    └── facilities.rs              # Extend with logic allocation persistence (existing)

app/src-tauri/src/
└── commands/
    ├── behavior_templates.rs      # Extend: ABS template + signal-aspect role (existing)
    └── facilities.rs              # Extend: apply-with-compilation + delete-with-cleanup (existing)

app/src/lib/
├── api/
│   ├── behaviorTemplates.ts       # Extend: ABS template type (existing)
│   └── facilities.ts              # Extend: apply/delete with logic allocation (existing)
├── stores/
│   ├── facilitiesStore.svelte.ts  # Extend: logic allocation state (existing)
│   └── effectiveLayoutStore.svelte.ts  # Extend: capacity queries (existing)
├── orchestration/
│   └── facilityOrchestrator.ts    # Extend: apply workflow with target selection (existing)
├── components/Facilities/
│   ├── LogicTargetSelector.svelte # NEW: target node selection + capacity display
│   └── FacilityCard.svelte        # Extend: signal facility rendering (existing)
└── utils/
    └── channelStyles.ts           # Extend: signal-aspect styles (existing)
```

**Structure Decision**: This feature extends existing modules at every layer rather than creating parallel infrastructure. The only new module is `bowties-core/src/logic_adapter/` which encapsulates the compilation and allocation domain. All other changes extend existing files following the established facility/channel patterns from spec 018.

## Complexity Tracking

No constitution violations to justify.

## Constitution Check — Post-Design Re-evaluation

*Re-checked after Phase 1 design completion.*

All gates still **PASS**. Key alignment notes:

- **Principle I (Rust)**: Template compiler and allocation tracking are pure Rust in `bowties-core/src/logic_adapter/`. All CDI enum discriminants match hardware values exactly.
- **Principle III (TDD)**: Compiler is a pure function (template + bindings + style → `Vec<CompiledConditionalLine>`) — ideal for deterministic unit tests. Allocation is testable state tracking with known limits.
- **Principle IV (Protocol)**: All enum values for conditional line fields (Function, Logic Operation, Variable Source/Trigger, Action Condition/Destination, Track Speed) validated against Tower LCC profile extraction data.
- **Principle V (UX)**: User flow is 3 steps (select template → bind slots → apply). Capacity displayed before apply. No manual CDI configuration required.

No new violations introduced by Phase 1 design.

## Architecture Assessment

**Assessment date**: 2026-07-07
**Scaling level**: Full (3+ layers affected, new module proposed, design trade-offs)

### Affected Modules

| Module | Layer | Impact | Notes |
|--------|-------|--------|-------|
| `bowties-core/src/behavior_templates.rs` | Backend domain | Modified | Gains ABS 3-Aspect template with condition→action rules + compilation target marker |
| `bowties-core/src/logic_adapter/` | Backend domain | New | Tower LCC compiler: template rules → conditional lines, Track Circuit allocation, aspect-to-event expansion |
| `bowties-core/src/facility_bowties/mod.rs` | Backend domain | Touched | Reused unchanged — ABS signal-aspect bowties use the same `compose_bowtie_ops` interface |
| `bowties-core/src/layout/types.rs` | Backend domain | Modified | `LayoutEditDelta` gains `AllocateLogic` / `FreeLogic` variants |
| `bowties-core/src/layout/state.rs` | Backend domain | Modified | `LayoutState` gains logic allocation persistence layer |
| `bowties-core/src/layout/facilities.rs` | Backend domain | Modified | `apply_facility_deltas` extended with logic allocation fields |
| `app/src-tauri/src/commands/behavior_templates.rs` | Backend command | Modified | ABS template metadata served via IPC |
| `app/src-tauri/src/commands/facility_bowties.rs` | Backend command | Modified | Compile-before-compose IPC path |
| `app/src/lib/orchestration/facilityOrchestrator.ts` | Orchestrator | Modified | Logic-target selection step, compile-before-compose workflow, allocation tracking |
| `app/src/lib/stores/facilitiesStore.svelte.ts` | Store | Modified | Logic-target-node field per facility + allocation state |
| `app/src/lib/stores/channels.svelte.ts` | Store | Modified | Signal-aspect role + 2-LED bicolor style support |
| `app/src/lib/layout/effectiveLayoutStore.svelte.ts` | Facade | Modified | `allocationCapacity(nodeKey)` derivation |
| `app/src/lib/utils/channelStyles.ts` | Utils | Modified | `2-led-bicolor-aspect` style entry in `STYLE_EVENT_MAPPINGS` |
| `app/src/lib/components/Facilities/LogicTargetSelector.svelte` | Component | New | Node picker + capacity display |
| `app/src/lib/components/Facilities/FacilityCard.svelte` | Component | Modified | Signal-aspect facility rendering |
| `app/src/routes/+page.svelte` | Route | Modified | Logic-target selection dialog orchestration |

### Assessment Summary

ABS Signaling is a Full-scale feature touching 4+ layers with one new module (`logic_adapter/`). The feature follows established patterns from Spec 018 — draft-layer atomicity (ADR-0012), single-merge derivation (ADR-0004), registry-driven lifecycle (ADR-0011), and backend layout state ownership (ADR-0015). The new seam (`logic_adapter/`) is a legitimate variation point: Tower LCC conditionals are the first adapter, with STL and LogixNG deferred. All touched seam invariants (ADR-0004, ADR-0009, ADR-0011, ADR-0012, ADR-0015) audit clean — no drift detected. Logic allocation state lives as a per-facility field (not a new store), avoiding a new `LayoutScopedParticipant` registration and keeping the `dirtyBreakdown` bucket count stable. Capacity enforcement is compiler-owned (post-validate), concentrating allocation knowledge in one module.

### Findings

**F1: New seam — Logic Adapter introduces a real variation point**
- Category: seam
- Affected: `bowties-core/src/logic_adapter/` (new)
- Concern: The adapter seam is legitimate (Tower LCC first, STL/LogixNG deferred). A function-level interface suffices — no trait/dynamic dispatch until the second adapter arrives (YAGNI).
- Decision: include — build as a concrete module with function-level interface

**F2: Logic allocation persistence — new LayoutEditDelta variants must join existing seams**
- Category: placement / ADR compliance
- Affected: `facilitiesStore`, `effectiveNodeStore`, `LayoutState`
- Concern: Logic allocation is per-facility state. A separate store would be a shallow pass-through. Extending `facilitiesStore` avoids a new participant registration, a new dirty bucket, and a new `collectDeltas()` implementor.
- Decision: include — logic allocation state lives as a field on the facility record in `facilitiesStore`

**F3: Capacity enforcement at compile time — compiler-owned**
- Category: depth / leverage
- Affected: `logic_adapter/`, `facilityOrchestrator.ts`
- Concern: Compiler returns `Result<..., CompileError::InsufficientCapacity>`. Frontend surfaces the error. No pre-guard in the orchestrator — concentrates capacity knowledge in one place.
- Decision: include — compiler owns the capacity check

**F4: Signal-aspect channel extends ADR-0013**
- Category: ADR compliance
- Affected: `channelsStore`, `channelStyles.ts`, `channel_events.rs`
- Concern: New `ChannelRole` value + new style entry, same `lampRow` binding kind. No structural change to the channel seam.
- Decision: include — extend existing ADR-0013 vocabulary

**F5: Existing debt — STYLE_EVENT_MAPPINGS hard-codes leaf ordinals with no validation**
- Category: existing debt / deepening
- Affected: `channelStyles.ts`
- Concern: Adding a 4-state style increases ordinal mismatch risk. A validation test would deepen this module.
- Decision: defer — capture as GitHub idea issue

### Vertical Slices

**S1: Standalone ABS signal — stub compile, full vertical path**
- Type: HITL
- Layers: Route → Component → Orchestrator → Store → API → Backend command → Backend domain (stubbed compiler)
- Blocked by: None
- Test: User creates ABS facility, binds channels, selects logic target, applies → Wired status
- Acceptance: Full UI flow exercisable with stubbed compilation. Establishes compile-before-compose workflow, logic-target selection, and `logic_adapter/` seam.

**S2: Real Tower LCC compiler**
- Type: HITL
- Layers: Backend domain (`logic_adapter/`)
- Blocked by: S1
- Test: Compiled conditional lines match expected Tower LCC CDI values for standalone 3-aspect signal
- Acceptance: Save + bus write produces correct CDI state on target node. Mast-group ordering, aspect-to-event expansion, and capacity enforcement validated.

**S3: Signal-aspect channel style + event resolution**
- Type: AFK
- Layers: Component → Store → API → Backend (`channel_events.rs`)
- Blocked by: S1
- Test: Signal-aspect channel rows on Railroad panel show lit/unlit state dots
- Acceptance: `2-led-bicolor-aspect` style resolves event IDs. Composed bowties drive correct lamp events.

**S4: ABS cascade (same-node Track Circuits)**
- Type: HITL
- Layers: Orchestrator → Backend (`logic_adapter/` compiler extension)
- Blocked by: S2
- Test: 3-signal cascade produces correct Stop→Approach→Clear propagation
- Acceptance: Track Circuit allocation for inter-signal aspect communication. Downstream-signal rules compile to TC-read variables.

**S5: Facility deletion + resource reclamation**
- Type: AFK
- Layers: Orchestrator → Backend (`logic_adapter/` + `LayoutState`)
- Blocked by: S2
- Test: Deleting a compiled facility resets conditional lines and frees allocations
- Acceptance: Capacity display updates after delete. No stale event IDs remain in CDI.

**S6: Capacity display + logic target selection UX**
- Type: AFK
- Layers: Route → Component → Store → Backend
- Blocked by: S1
- Test: LogicTargetSelector shows capacity numbers; over-allocation shows error
- Acceptance: "X/32 conditional lines, Y/8 Track Circuits" visible per candidate node.

### Deferred Improvements

- F5 (STYLE_EVENT_MAPPINGS validation test): pending GitHub issue creation — see below.

### Architecture Decisions

No new ADRs required. The `logic_adapter/` module follows existing patterns and does not reverse any prior commitment. ADR-0012 and ADR-0013 are extended within their existing scope.
