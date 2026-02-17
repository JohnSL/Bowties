# Implementation Plan: CDI XML Viewer

**Branch**: `001-cdi-xml-viewer` | **Date**: February 16, 2026 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/001-cdi-xml-viewer/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Provide developers with a debugging tool to view formatted CDI (Configuration Description Information) XML data retrieved from LCC nodes. The feature adds a right-click context menu option on nodes to display the raw CDI XML with proper indentation and formatting in a dedicated viewer window. This enables developers to verify CDI retrieval correctness and troubleshoot configuration structure issues without external tools.

## Technical Context

**Language/Version**: Rust 2021+ (backend), TypeScript 5.x with SvelteKit 2.x (frontend)  
**Primary Dependencies**: Tauri 2.x, lcc-rs (protocol library), tokio (async runtime)  
**Storage**: In-memory CDI data cache (already retrieved by node discovery/management)  
**Testing**: cargo test (Rust backend), Vitest (SvelteKit frontend), integration tests with mock nodes  
**Target Platform**: Windows, macOS, Linux desktop (cross-platform via Tauri)
**Project Type**: Desktop application (Tauri-based hybrid architecture)  
**Performance Goals**: Display formatted XML within 3 seconds, handle CDI documents up to 10MB  
**Constraints**: Must preserve all XML content exactly, must use monospaced font for readability  
**Scale/Scope**: Single-node focus (one viewer window at a time), debugging/development tool (not production-critical)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Principle Compliance

✅ **I. Rust 2021+ Development**: Backend uses Rust 2021 edition via Tauri, frontend uses TypeScript/SvelteKit  
✅ **II. Cargo-Based Development**: No new dependencies required, uses existing Cargo/npm toolchain  
✅ **III. Test-Driven Development**: Will include tests for XML formatting logic, UI component tests  
⚠️  **IV. LCC Protocol Correctness**: Not applicable - feature displays existing CDI data, doesn't implement protocol  
✅ **V. UX-First Design**: Right-click context menu is intuitive, formatted view improves debugging UX  
✅ **VI. TCP-Only Focus**: No transport changes, works with existing TCP-based CDI retrieval  
⚠️  **VII. Event Management Excellence**: Not applicable - CDI viewer is separate from event management  

### Quality Gates Status

- ✅ Uses existing Tauri/Rust/SvelteKit stack (no new languages)
- ✅ No new external dependencies required
- ✅ Cross-platform compatible (Tauri handles platform abstraction)
- ✅ Testable (XML formatting, error handling, UI components)
- ✅ Follows existing architectural patterns (Tauri commands, Svelte components)

**Gate Result**: ✅ **PASS** - No architectural changes, fits within existing patterns, no constitution violations

## Project Structure

### Documentation (this feature)

```text
specs/001-cdi-xml-viewer/
├── spec.md              # Feature specification (completed)
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (to be generated)
├── data-model.md        # Phase 1 output (to be generated)
├── quickstart.md        # Phase 1 output (to be generated)
├── contracts/           # Phase 1 output (to be generated)
│   └── tauri-commands.ts  # TypeScript interface definitions
├── checklists/
│   └── requirements.md  # Specification quality checklist (completed)
└── tasks.md             # Phase 2 output (/speckit.tasks command - not created by /speckit.plan)
```

### Source Code (repository root)

```text
app/
├── src-tauri/          # Rust backend
│   ├── src/
│   │   ├── lib.rs      # Update: Export CDI viewer command
│   │   └── commands/
│   │       └── cdi.rs  # New: CDI XML viewer Tauri command
│   └── Cargo.toml      # No dependency changes needed
│
└── src/                # SvelteKit frontend
    ├── lib/
    │   ├── components/
    │   │   └── CdiXmlViewer.svelte  # New: XML viewer modal component
    │   ├── api/
    │   │   └── cdi.ts               # New: Tauri command wrapper
    │   └── utils/
    │       └── xmlFormatter.ts      # New: XML formatting utilities
    └── routes/
        └── (nodes page with context menu - existing, to be enhanced)
```

**Structure Decision**: Web application architecture (Tauri desktop app with SvelteKit frontend and Rust backend). This feature adds:
- **Backend**: 1 new Tauri command module for CDI retrieval formatting
- **Frontend**: 1 new Svelte component (modal viewer), 1 API wrapper, 1 utility module
- No changes to existing lcc-rs library (CDI data already available via node management)

## Complexity Tracking

**No violations requiring justification.** This feature aligns with all constitution principles and requires no architectural deviations.

---

## Post-Design Constitution Re-Check

*Completed after Phase 1 design (research, data model, contracts, quickstart)*

### Design Compliance Review

✅ **I. Rust 2021+ Development**
- Backend: Tauri command in Rust (single module: `commands/cdi.rs`)
- Frontend: TypeScript/SvelteKit components
- No deviation from constitution

✅ **II. Cargo-Based Development**
- **Zero new dependencies** added in research phase
- Uses existing: serde, tauri, tokio, chrono
- Frontend: No new npm packages required (DOMParser is browser-native)

✅ **III. Test-Driven Development**
- Unit tests defined in contracts/rust-signatures.md
- Frontend component tests planned
- Integration tests for Tauri command (mock nodes)
- XML formatter edge case tests

✅ **IV. LCC Protocol Correctness**
- Not applicable (confirmed post-design)
- Feature only displays data, doesn't implement protocol

✅ **V. UX-First Design**
- Quickstart guide demonstrates user-friendly workflow
- Right-click context menu (standard pattern)
- Clear error messages (4 scenarios documented)
- Copy-to-clipboard for developer convenience

✅ **VI. TCP-Only Focus**
- Confirmed: No transport changes
- Works with existing node management system

✅ **VII. Event Management Excellence**
- Not applicable (confirmed post-design)

### Architectural Analysis

**Decisions Made**:
1. **XML formatting in frontend** (JavaScript) not backend (Rust)
   - Rationale: Presentation logic, avoids Rust XML dependencies
   - Constitution compliance: ✅ Minimizes dependencies

2. **Svelte modal component** not native OS dialog
   - Rationale: Better UX control, copy functionality
   - Constitution compliance: ✅ UX-first design

3. **In-house XML formatting** not external library
   - Rationale: Simple indentation, avoid bloat
   - Constitution compliance: ✅ Cargo-based (no new deps)

4. **Read-only viewer** not editor
   - Rationale: Debugging tool scope constraint
   - Constitution compliance: ✅ Simplicity principle

**No New Risks Identified**

**Gate Result**: ✅ **PASS** - Design phase maintains full constitution compliance, zero violations introduced

---

## Implementation Readiness

✅ **Phase 0 (Research)**: Complete - All decisions documented in [research.md](research.md)  
✅ **Phase 1 (Design)**: Complete - Data model, contracts, quickstart generated  
✅ **Constitution**: Compliant - No violations before or after design  
✅ **Next Phase**: Ready for `/speckit.tasks` to generate implementation tasks

**Planning Artifacts**:
- [x] plan.md (this file)
- [x] research.md (6 research topics, all resolved)
- [x] data-model.md (entities, state management, testing)
- [x] contracts/ (TypeScript + Rust signatures)
- [x] quickstart.md (user guide with examples)
- [x] Agent context updated (copilot-instructions.md)

**Ready for implementation task generation** ✅
