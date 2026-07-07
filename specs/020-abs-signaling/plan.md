# Implementation Plan: ABS Signaling

**Branch**: `020-abs-signaling` | **Date**: 2026-07-07 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/020-abs-signaling/spec.md`

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
