# Bowties LCC/OpenLCB Desktop Application Constitution

<!--
Sync Impact Report (Version 1.0.0 - Initial Constitution)
═══════════════════════════════════════════════════════════
Version: 1.0.0
Change Type: Initial ratification
Ratification Date: 2026-02-16

This is the founding constitution for the Bowties LCC/OpenLCB Desktop Application.
It establishes the core principles, technical standards, and governance model
for the project's development and maintenance.

Core Principles Established:
  I.   Rust 2021+ Development (type safety, memory safety, performance)
  II.  Cargo-Based Development Environment (reproducible builds)
  III. Test-Driven Development (protocol correctness through testing)
  IV.  LCC Protocol Correctness (standards compliance)
  V.   UX-First Design (accessibility for hobbyists)
  VI.  TCP-Only Focus (scope constraint)
  VII. Event Management Excellence (core feature set)

Architecture Defined:
  - Frontend: SvelteKit (reactive UI framework)
  - Backend: Rust with Tauri 2 (native desktop framework)
  - Protocol: lcc-rs library (reusable Rust LCC implementation)
  - IPC: Tauri commands (type-safe frontend ↔ backend communication)

Templates Status:
  ✅ plan-template.md - Aligned with Rust/Tauri architecture
  ✅ spec-template.md - User scenario structure supports UX-first principle
  ✅ tasks-template.md - Task categorization reflects TDD and protocol testing
  ⚠️  All command files - Review for constitution principle alignment (ongoing)

Follow-up Actions:
  - Establish CI/CD pipelines with automated test execution
  - Set up protocol compliance validation against LCC standards
  - Create end-to-end test harness for UI workflows
  - Document development environment setup procedures
═══════════════════════════════════════════════════════════
-->

## Core Principles

### I. Rust 2021+ Development (CRITICAL)

**All backend code MUST use Rust 2021 edition or later**. This is non-negotiable.

- Rust code must compile with stable Rust toolchain (1.70+)
- Use modern Rust idioms (async/await, pattern matching, Result types, ? operator)
- Leverage type system for correctness (no unwrap() in production paths)
- All dependencies must support current stable Rust
- Use `tokio` for async runtime (consistency across lcc-rs and Tauri)
- Follow Rust API guidelines and naming conventions
- Prefer explicit error types over panic/unwrap

**Rationale**: Rust provides memory safety, thread safety, and type safety without garbage collection overhead. The ownership model prevents entire classes of bugs common in systems programming. Async/await enables responsive UI while handling blocking network I/O. This combination delivers production-quality implementation with the performance and reliability guarantees essential for desktop applications that interact with real-time control networks.

**Enforcement**: All code changes must pass `cargo check`, `cargo clippy`, and `cargo test`. CI/CD pipeline runs full test suite on every commit. Clippy warnings treated as errors in production builds.

### II. Cargo-Based Development Environment

**Development MUST use Cargo** for Rust dependency management and build orchestration.

- Cargo manages Rust toolchain and crate dependencies
- All setup documentation assumes Rust installed via rustup
- `Cargo.toml` files define dependencies with specific version requirements
- `Cargo.lock` committed to repository for reproducible builds
- Development commands: `cargo build`, `cargo test`, `cargo run`
- Frontend uses npm/pnpm (standard for SvelteKit)

**Rationale**: Cargo is the standard Rust build tool, providing reproducible builds via lockfiles, integrated testing, documentation generation, and dependency resolution. Combined with rustup for toolchain management, it ensures all developers work with identical compiler versions and dependency trees, eliminating "works on my machine" issues.

**Enforcement**: README and contributor documentation must provide rustup installation instructions. All Rust code must build with `cargo build --release` without warnings or errors.

### III. Test-Driven Development (MANDATORY)

**Comprehensive automated testing is REQUIRED**. Protocol correctness depends on it.

**Unit Tests** (Required for all protocol code):
- Every public function in lcc-rs must have unit tests
- Use `#[cfg(test)] mod tests` for inline tests
- Property-based tests (proptest) for protocol parsers and encoders
- Mock transports for testing without network I/O
- Target: >80% code coverage for lcc-rs library

**Integration Tests** (Required for features):
- Test with real LCC hardware when available (Tower-LCC, other nodes)
- Validate protocol messages byte-for-byte against known-good reference implementations
- Test with LCC network simulators and test tools
- Simulate network conditions (timeouts, out-of-order messages, malformed frames)

**Property-Based Tests** (Required for protocol correctness):
- GridConnect frame encoding/decoding: `parse(encode(x)) == x`
- MTI header manipulation: roundtrip validation
- Datagram assembly: any valid sequence produces correct payload
- Use `proptest` crate for generative testing

**End-to-End Tests** (Required for UI features):
- Tauri test harness for frontend-backend integration
- User workflow validation (discover → view SNIP → refresh)
- Error handling paths (timeouts, network failures)

**Rationale**: Manual testing is insufficient for protocol implementation. LCC/OpenLCB has complex multi-frame protocols (datagrams), timing-sensitive operations (alias allocation), and strict format requirements (GridConnect frames, MTI encoding). Automated tests catch regressions, validate against specification, and serve as executable documentation. This testing rigor ensures reliable operation in production environments.

**Enforcement**: 
- All PRs must include tests for new functionality
- CI/CD runs full test suite; failures block merge
- Coverage reports generated on every build
- Protocol tests validated against LCC standards documents
- Breaking changes to lcc-rs require integration test updates

### IV. LCC Protocol Correctness (CRITICAL)

**Protocol implementation MUST match LCC/OpenLCB standards exactly**. No deviations allowed.

- **Reference Hierarchy** (check in this order):
  1. **Working References** (`docs/technical/`): Curated technical docs for developers
     - protocol-reference.md - Essential LCC protocol concepts and patterns
     - mti-reference.md - MTI values and frame examples
     - gridconnect-format.md, datagram-protocol.md - Protocol details (as created)
     - Must cite authoritative sources for all protocol details
     - Preferred for day-to-day development questions
     - **CHECK HERE FIRST to avoid expensive research**
  2. **Authoritative Standards** (`markdown/standards/`): LCC TN-9.7.x specifications
     - Source of truth for protocol correctness
     - Use when working references insufficient or confirmation needed
     - Reference when implementing new protocol features
  3. **API Documentation** (`docs/technical/`):
     - lcc-rs-api.md - Library API reference (modules, types, functions)
     - tauri-api.md - Frontend commands reference
     - Use before searching codebase
  4. **Reference Implementations**: OpenLCB tools and test harness
     - Validation of wire-format correctness
     - Use for integration testing validation
  5. **External Research**: Only when above sources are insufficient
     - Community documentation, errata, discussions
     - Expensive - avoid if possible

- **Technical Documentation Standards**:
  - All protocol claims must cite authoritative source (e.g., "Per TN-9.7.3.2 §2.5")
  - Include practical examples alongside spec references
  - Update technical docs when implementation reveals spec nuances
  - Keep docs in sync with code (API changes update docs immediately)
- **MTI Values**: All Message Type Identifiers must match standard exactly
- **Frame Format**: GridConnect frames follow `:X[header]N[data];` format precisely
- **Datagram Protocol**: Multi-frame assembly follows TN-9.7.3.2 exactly
- **SNIP Format**: Field order and encoding per TN-9.7.4.3 specification
- **Byte Order**: Network byte order (big-endian) for multi-byte values

**Validation Methods**:
- Property tests verify encoding/decoding invariants
- Integration tests compare output to known-good reference implementations
- Wireshark captures (optional) validate wire protocol
- Cross-reference implementation against standards documentation

**Rationale**: LCC is a standardized protocol for interoperability. Nodes from different manufacturers must communicate reliably. Any deviation from the standard breaks compatibility. The standards are well-documented and implementations exist for validation. Protocol bugs are expensive (user data corruption, network disruption) and hard to debug in deployed systems.

**Enforcement**:
- Protocol code changes require reference to standards section
- Test cases cite specific requirements (e.g., "TN-9.7.4.3 Section 2.5")
- Integration tests validate against known-good implementations
- Wireshark/analyzer validation before releasing protocol features

### V. UX-First Design

**User experience is the PRIMARY metric** for this tool. Every feature must make working with LCC nodes easier than command-line tools or low-level protocol libraries.

- Interactive UI preferred over complex command-line arguments
- Clear, human-readable output (not just raw protocol dumps)
- Error messages must be actionable and explain what went wrong
- Common workflows (discover nodes, read events, modify events) should require minimal steps
- Help text and documentation must be beginner-friendly

**Rationale**: LCC protocol libraries exist but are low-level tools designed for protocol implementation. Bowties exists to make LCC node interaction accessible to model railroad hobbyists who are not protocol experts. If Bowties is as hard to use as raw protocol commands, it has failed its purpose. Visual presentation, intuitive workflows, and clear feedback are essential to serving the target audience.

**Enforcement**: Feature acceptance requires demonstration that UX has improved over the equivalent raw library approach. User testing feedback should drive design decisions.

### VI. TCP-Only Focus

**Bowties supports TCP connections ONLY**. Serial, CAN-over-Ethernet (GridConnect), and local pipe connections are out of scope.

- Backend uses `lcc-rs` TCP transport (`TcpTransport`)
- Configuration and discovery assume TCP/IP networking
- Documentation and examples use TCP connection scenarios
- Features requiring other transports must be explicitly justified and approved

**Rationale**: Scope constraint. TCP is the most modern and network-friendly transport, suitable for IP-based LCC nodes and bridges. Supporting multiple transports adds complexity without clear value for the initial release. The underlying library architecture supports other modes if future expansion is needed.

**Enforcement**: Pull requests adding non-TCP transport support require constitution amendment discussion before implementation begins.

### VII. Event Management Excellence

**Event discovery, inspection, and modification are core competencies.** These features must be robust, well-tested, and user-friendly.

- Must support listing all events produced/consumed by a node
- Must support modifying event configurations
- Event IDs must be displayed in human-readable format (dotted hex: `01.02.03.04.05.06.07.08`)
- Event operations must include validation and confirmation prompts
- Changes must be reversible or provide "dry-run" modes

**Rationale**: Event management is the stated goal of Bowties. This is the feature set that justifies the tool's existence. If event operations are buggy, confusing, or incomplete, the project has not met its objectives. This focus drives prioritization and quality standards.

**Enforcement**: Event-related code requires integration tests against real or simulated LCC nodes. UX review is mandatory for event workflows.

## Technical Constraints

### Dependency Management

**Backend (Rust)**:
- **lcc-rs library** is the reusable LCC protocol implementation
  - Lives in `lcc-rs/` workspace directory
  - Standalone crate, can be used in other Rust projects
  - No reimplementation of protocol logic outside lcc-rs
- **Essential dependencies only**: tokio, serde, thiserror, async-trait
- All dependencies must:
  - Support current stable Rust
  - Be actively maintained (no abandoned crates)
  - Pass `cargo audit` security checks
- Prefer std library over external crates when reasonable
- Document rationale for each dependency in Cargo.toml comments

**Frontend (TypeScript/SvelteKit)**:
- SvelteKit 2.x for reactive UI framework
- Tauri 2.x API for frontend-backend communication
- Minimal external dependencies (avoid bloat)
- Type safety via TypeScript (strict mode enabled)
- All dependencies audited for security (npm audit)

**Cross-Platform**:
- Code must compile and run on Windows, macOS, Linux
- No OS-specific hacks unless absolutely necessary (and documented)
- Test on at least two platforms before major releases

### Architecture Constraints

**Separation of Concerns**:
- `lcc-rs/` - Pure Rust protocol library (no UI dependencies)
- `app/src-tauri/` - Tauri backend (bridges lcc-rs to frontend)
- `app/src/` - SvelteKit frontend (UI components, stores)

**No Circular Dependencies**:
- Frontend depends on Tauri commands (backend API)
- Backend depends on lcc-rs library
- lcc-rs is fully independent (can be tested standalone)

**State Management**:
- Backend holds connection state and node cache
- Frontend uses Svelte stores for reactive UI state
- Tauri events for backend → frontend notifications (e.g., new node discovered)

### Code Organization

**Rust Backend**:
- Modules organized by feature (discovery, snip, events)
- Public API minimal and well-documented
- Internal functions private by default
- Use Result<T, Error> for error handling (no panics in production paths)
- Comprehensive rustdoc comments on public items

**Frontend**:
- Components organized by feature area
- Reusable components in `lib/components/`
- API wrappers in `lib/api/` (typed Tauri command calls)
- Stores in `lib/stores/` (reactive state management)
- Prefer composition over inheritance

**Configuration**:
- Use TOML for Rust configuration (Cargo.toml, Tauri config)
- Use JSON for data interchange (Tauri IPC, API contracts)
- Environment-specific config via Tauri config (dev vs prod)

### Testing Requirements

**Test Organization**:
- Unit tests: Inline in source files (`#[cfg(test)] mod tests`)
- Integration tests: `tests/` directory in each crate
- Property tests: Use proptest for protocol correctness
- Frontend tests: Vitest for component testing

**Test Coverage**:
- lcc-rs library: Minimum 80% coverage
- Tauri commands: All commands have integration tests
- Frontend: Critical user flows have end-to-end tests
- Protocol code: 100% coverage for parsers/encoders

**Test Execution**:
- `cargo test` runs all Rust tests
- `cargo test --all-features` validates feature flags
- `npm test` runs frontend tests
- CI runs tests on Windows, macOS, Linux

**Test Quality**:
- Tests are deterministic (no flaky tests)
- Tests are independent (can run in any order)
- Tests are fast (unit tests <1s total, integration tests <10s)
- Mock external dependencies (network, filesystem) in unit tests

### Documentation Standards

- README must include rustup installation instructions
- Common workflows must be documented with examples and screenshots
- Protocol concepts (events, nodes, aliases, SNIP, CDI) explained for non-experts
- LCC standards referenced for protocol features (cite TN-9.7.x sections)
- Rustdoc comments on all public APIs (use `cargo doc` to generate)
- Feature specifications in `specs/` directory (feature-driven development)
- Quickstart guides for end users (not just developers)

## Development Workflow

### Feature Development Process

1. **Specification** (`/speckit.plan`): Write feature spec with user scenarios
2. **Planning**: Generate research, data model, contracts, quickstart docs
3. **Design Review**: Review plan for UX, protocol correctness, testing approach
4. **Implementation**: TDD - write tests first, then implement
5. **Integration**: Test against real LCC hardware and network simulators
6. **Documentation**: Update README, rustdoc, user guides
7. **Review**: Code review + UX review + protocol correctness check
8. **Merge**: Integration into main branch after all checks pass

### Contribution Guidelines

- Changes must compile with stable Rust toolchain
- UX changes should include before/after screenshots or videos
- Breaking changes to lcc-rs API require MAJOR version bump
- Dependencies must be justified and approved before addition
- All code must pass `cargo clippy` without warnings
- Format code with `cargo fmt` before committing

### Quality Gates

Before merging to main:
- ✅ Compiles with stable Rust (no warnings with clippy)
- ✅ All tests pass (`cargo test --all-features`)
- ✅ Code formatted (`cargo fmt --check`)
- ✅ No security vulnerabilities (`cargo audit`)
- ✅ Documentation updated (README, rustdoc, specs)
- ✅ Tested on at least one platform (Windows/macOS/Linux)
- ✅ Protocol changes validated against LCC standards
- ✅ UX changes validated with user testing or design review

### Version Control Practices

- Main branch should always be in working state
- Feature branches for non-trivial changes
- Commit messages should reference issues/features
- Tag releases with semantic versioning

## Governance

### Authority of This Constitution

This constitution supersedes informal practices, undocumented conventions, and individual preferences. When development decisions conflict with these principles, the constitution takes precedence.

### Amendment Process

1. **Proposal**: Document proposed change with rationale
2. **Discussion**: Review impact on existing code and workflows
3. **Approval**: Require consensus or maintainer approval
4. **Migration Plan**: If existing code violates new principle, create migration plan
5. **Update Version**: Increment constitution version per semantic versioning rules:
   - **MAJOR**: Principle removed, redefined, or backward-incompatible governance change
   - **MINOR**: New principle added or material expansion of guidance
   - **PATCH**: Clarifications, wording improvements, typo fixes

### Compliance Review

- All pull requests should reference constitution principles where relevant
- Constitution violations must be explicitly justified or corrected
- Regular review (quarterly or per release) to ensure constitution remains aligned with project goals

### Versioning Policy

Constitution uses **MAJOR.MINOR.PATCH** semantic versioning:
- **MAJOR**: Changing core principles or removing guarantees
- **MINOR**: Adding new principles or sections
- **PATCH**: Clarifications, corrections, formatting

**Version**: 1.0.0 | **Ratified**: 2026-02-16 | **Last Amended**: 2026-02-16
