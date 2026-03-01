# Implementation Plan: Editable Node Configuration with Save

**Branch**: `007-edit-node-config` | **Date**: 2026-02-28 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/007-edit-node-config/spec.md`

## Summary

Add write support to the LCC node configuration workflow. Users will be able to edit configuration fields (strings, integers, event IDs, floats, dropdowns) inline in the existing card-deck UI, see clear visual feedback of unsaved/invalid/error states, and save all changes via Memory Configuration Protocol write datagrams. This requires: (1) adding write commands to the `lcc-rs` protocol library, (2) new Tauri backend commands for writing, (3) frontend dirty-tracking store and editable input components, and (4) Save/Discard controls with progress indication.

## Technical Context

**Language/Version**: Rust 2021 edition (stable 1.70+), TypeScript (strict), SvelteKit 2.x  
**Primary Dependencies**: lcc-rs (path dep), Tauri 2, tokio 1.41, serde 1.0, roxmltree 0.20, thiserror (2.0 in lcc-rs, 1.0 in app)  
**Storage**: N/A (values written directly to LCC node memory via protocol)  
**Testing**: `cargo test` + proptest (Rust), Vitest + jsdom + @testing-library (Frontend)  
**Target Platform**: Windows, macOS, Linux (Tauri desktop)  
**Project Type**: Desktop app (Rust backend + SvelteKit frontend via Tauri 2)  
**Performance Goals**: Sequential field writes with <3s timeout per write attempt; UI remains responsive during saves  
**Constraints**: Writes ≤64 bytes per datagram chunk; sequential writes (not parallel) to avoid overwhelming nodes; TCP-only transport  
**Scale/Scope**: Single-node editing at a time; typical CDI has 10-50 editable fields per segment

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Rust 2021+ Development** | PASS | All new lcc-rs code uses Rust 2021 edition, Result types, async/await. No unwrap() in production paths. |
| **II. Cargo-Based Development** | PASS | All Rust code managed via Cargo.toml workspaces. No new build tooling needed. |
| **III. Test-Driven Development** | PASS | Plan includes: unit tests for `build_write`/`parse_write_reply`, MockTransport tests for `write_memory`, proptest for write encoding roundtrips, Vitest tests for editable components and dirty-tracking store. |
| **IV. LCC Protocol Correctness** | PASS | Write commands follow Memory Configuration Protocol per TN-9.7.4.1. Command bytes, address encoding, space encoding mirror existing read implementation. Update Complete (0xA8) per spec. |
| **V. UX-First Design** | PASS | Inline editing with visual dirty/invalid/error states, progress indicator during save, navigation guards, discard with confirmation — all improve over raw protocol tools. |
| **VI. TCP-Only Focus** | PASS | No new transport types. Uses existing TcpTransport for write datagrams. |
| **VII. Event Management Excellence** | PASS | Event ID fields editable with dotted-hex validation. Event IDs written as 8 raw bytes. Supports event reconfiguration workflow. |
| **Separation of Concerns** | PASS | Write protocol logic in lcc-rs (pure library), Tauri commands bridge to frontend, edit state in frontend stores. No circular dependencies. |
| **No unwrap() in production** | PASS | All write paths use Result<T, Error> with explicit error handling and retries. |
| **Cross-Platform** | PASS | No OS-specific code. TCP writes, Tauri commands, and Svelte components are platform-independent. |

**Post-Design Re-evaluation (Phase 1 complete)**: All gates still PASS. Design confirmed against OpenLCB_Java reference implementation. Value serialization matches `ConfigRepresentation.java` patterns. Write acknowledgment uses `RequestWithNoReply` pattern (Datagram Received OK = success). No new dependencies required. Edit state kept in frontend stores (transient UI state), maintaining separation of concerns. String writes use minimal-length encoding (NOT full-padded), matching Java reference behavior.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
lcc-rs/src/
├── protocol/
│   └── memory_config.rs    # ADD: build_write(), build_update_complete()
├── discovery.rs             # ADD: write_memory(), send_update_complete()
└── lib.rs                   # EXPORT: new public types

app/src-tauri/src/
├── commands/
│   └── cdi.rs               # ADD: write_config_value, send_update_complete Tauri commands
├── state.rs                 # No changes needed (edit state lives in frontend)
└── node_tree.rs             # No changes needed (serialization done in frontend)

app/src/
├── lib/
│   ├── api/
│   │   ├── config.ts        # NEW: writeConfigValue(), sendUpdateComplete() wrappers
│   │   └── types.ts         # ADD: PendingEdit, WriteResult, SaveProgress types
│   ├── stores/
│   │   └── pendingEdits.svelte.ts  # NEW: PendingEditsStore (dirty tracking, validation)
│   ├── components/
│   │   └── ElementCardDeck/
│   │       ├── TreeLeafRow.svelte        # MODIFY: add editable inputs (inline, no wrapper component)
│   │       ├── SaveControls.svelte       # NEW: Save/Discard buttons + progress
│   │       └── SegmentView.svelte        # MODIFY: add SaveControls
│   │   └── ConfigSidebar/
│   │       ├── NodeEntry.svelte          # MODIFY: add unsaved-changes badge
│   │       └── SegmentEntry.svelte       # MODIFY: add unsaved-changes badge
│   └── types/
│       └── nodeTree.ts      # ADD: PendingEdit, WriteResult interfaces
└── routes/
    └── config/
        └── +page.svelte     # MODIFY: add navigation guard logic
```

**Structure Decision**: Follows existing Bowties architecture (web application variant with Rust backend + SvelteKit frontend). New code slots into established module boundaries. Edit state managed in frontend Svelte stores (not backend) since it's transient UI state. Protocol write logic added to lcc-rs for reusability per constitution separation-of-concerns principle.

## Complexity Tracking

> No constitution violations detected. All design decisions align with established principles.
