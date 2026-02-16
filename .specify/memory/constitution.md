# Bowtie LCC/OpenLCB UX Tool Constitution

<!--
Sync Impact Report (Version 2.0.0 - Python 3.12 Migration Amendment)
═══════════════════════════════════════════════════════════
Version Change: 1.0.0 → 2.0.0
Change Type: MAJOR (backward-incompatible principle redefinition)
Amendment Date: 2026-02-14

Modified Principles:
  1. Python 2.7 Compatibility → Python 3.12+ Compatibility (REDEFINED)
     - Old: All code MUST run on Python 2.7
     - New: All code MUST run on Python 3.12+
     - Reason: OpenLCB_Python library successfully migrated to Python 3.12 (spec 001-python3-migration)
  
  2. UV-Based Development Environment (UPDATED)
     - Updated references from Python 2.7 to Python 3.12
     - Core principle unchanged (UV still required)

Unchanged Principles:
  3. UX-First Design
  4. TCP-Only Focus
  5. Event Management Excellence

Sections Modified:
  - Technical Constraints: Updated for Python 3.12 features and dependencies
  - Development Workflow: Updated Quality Gates for Python 3.12

Templates Requiring Updates:
  ✅ plan-template.md - No updates required (constitution check logic agnostic to Python version)
  ✅ spec-template.md - No updates required (scope/requirements compatible)
  ✅ tasks-template.md - No updates required (task categorization compatible)
  ⚠️ Integration testing: Verify OpenLCB_Python migration complete before Bowtie adoption

Follow-up TODOs:
  - Complete OpenLCB_Python migration (spec 001-python3-migration)
  - Plan Bowtie codebase Python 3 adoption after library migration
  - Update any quickstart/setup guides to reference Python 3.12

Next Steps:
  - Monitor spec 001-python3-migration completion
  - Plan Bowtie Python 3 adoption timeline
  - Update CI/CD pipelines to Python 3.12
═══════════════════════════════════════════════════════════
-->

## Core Principles

### I. Python 3.12+ Compatibility (CRITICAL)

**All code MUST run on Python 3.12 or later**. This is non-negotiable.

- All syntax and features must be Python 3.12+ compatible
- Modern Python 3 idioms are encouraged (f-strings, type hints, pathlib, etc.)
- All dependencies must support Python 3.12+
- Use `print()` function calls exclusively
- String handling uses Python 3's unified str type (UTF-8 by default)
- Use `range()` for iteration (Python 3's efficient implementation)
- Leverage modern standard library features (asyncio, dataclasses, typing)

**Rationale**: The OpenLCB_Python library has been successfully migrated to Python 3.12 (spec 001-python3-migration), removing the constitutional barrier to Python 3 adoption. Python 3.12 provides modern language features, better performance, active security support, and compatibility with current frameworks like Flask. This enables Bowtie to leverage the full Python ecosystem while maintaining protocol compatibility.

**Enforcement**: All code changes must be tested with Python 3.12+. CI/CD pipeline MUST use Python 3.12 as the minimum supported version. Type hints should be validated with mypy or similar tools.

### II. UV-Based Development Environment

**Development MUST use UV** to manage Python 3.12+ environments without requiring system-wide Python installation.

- UV manages Python 3.12+ installation and virtual environments
- All setup documentation assumes UV is available
- `pyproject.toml` and UV configuration files define dependencies
- No instructions requiring manual Python installation
- Development scripts should invoke Python through UV (`uv run python ...`)

**Rationale**: UV provides isolated, reproducible environments that protect the host system while enabling precise Python version control. This ensures all developers work with identical Python 3.12 environments regardless of their system configuration, eliminating "works on my machine" issues.

**Enforcement**: README and contributor documentation must provide UV-based setup instructions only. Scripts that assume system Python must be refactored or documented as exceptions.

### III. UX-First Design

**User experience is the PRIMARY metric** for this tool. Every feature must make working with LCC nodes easier than raw OpenLCB_Python scripts.

- Interactive prompts preferred over complex command-line arguments
- Clear, human-readable output (not just raw protocol dumps)
- Error messages must be actionable and explain what went wrong
- Common workflows (discover nodes, read events, modify events) should require minimal steps
- Help text and documentation must be beginner-friendly

**Rationale**: OpenLCB_Python is a powerful but low-level library designed for protocol testing. Bowtie exists to make LCC node interaction accessible to users who are not protocol experts. If Bowtie is as hard to use as the raw library, it has failed its purpose.

**Enforcement**: Feature acceptance requires demonstration that UX has improved over the equivalent raw library approach. User testing feedback should drive design decisions.

### IV. TCP-Only Focus

**Bowtie supports TCP connections ONLY**. Serial, CAN-over-Ethernet (GridConnect), and local pipe connections are out of scope.

- All connection code targets `tcpolcblink.py` from the OpenLCB_Python library
- Configuration and discovery assume TCP/IP networking
- Documentation and examples use TCP connection scenarios
- Features requiring other transports must be explicitly justified and approved

**Rationale**: Scope constraint. TCP is the most modern and network-friendly transport, suitable for IP-based LCC nodes. Supporting multiple transports adds complexity without clear POC value. The underlying library supports other modes if future expansion is needed.

**Enforcement**: Pull requests adding non-TCP transport support require constitution amendment discussion before implementation begins.

### V. Event Management Excellence

**Event discovery, inspection, and modification are core competencies.** These features must be robust, well-tested, and user-friendly.

- Must support listing all events produced/consumed by a node
- Must support modifying event configurations
- Event IDs must be displayed in human-readable format (dotted hex: `01.02.03.04.05.06.07.08`)
- Event operations must include validation and confirmation prompts
- Changes must be reversible or provide "dry-run" modes

**Rationale**: Event management is the stated goal of Bowtie. This is the feature set that justifies the tool's existence. If event operations are buggy, confusing, or incomplete, the project has not met its objectives.

**Enforcement**: Event-related code requires integration tests against real or simulated LCC nodes. UX review is mandatory for event workflows.

## Technical Constraints

### Dependency Management

- **OpenLCB_Python library** is the sole protocol implementation dependency
- OpenLCB_Python must be vendored or referenced from `../OpenLCB_Python` directory
- No reimplementation of protocol logic—always delegate to the library
- Additional dependencies (UI frameworks, CLI helpers) allowed if Python 3.12+ compatible
- Leverage modern Python 3 packages (rich, click, pydantic, etc.) where appropriate

### Code Organization

- Entry points should be simple scripts or a single CLI tool
- Business logic separated from UI/interaction logic where practical
- Configuration should use modern formats (TOML, JSON, or YAML with appropriate parsers)
- Type hints encouraged for better IDE support and static analysis

### Testing Requirements

- Manual testing against real LCC nodes is acceptable for POC phase
- Event modification operations MUST be tested before release
- Discovery and read operations should have basic smoke tests
- Test documentation should specify required hardware/simulation setup

### Documentation Standards

- README must include UV-based setup instructions
- Common workflows must be documented with examples
- Protocol concepts (events, nodes, aliases) should be explained for non-experts
- OpenLCB_Python library functions used should be referenced with file/line info for maintainability

## Development Workflow

### Feature Development Process

1. **Design Review**: Discuss UX approach before implementation
2. **Prototype**: Build basic working version
3. **Test with Real Nodes**: Validate against LCC hardware or simulator
4. **Document**: Update README with examples
5. **Review**: Code review + UX review
6. **Merge**: Integration into main branch

### Contribution Guidelines

- Changes must maintain Python 3.12+ compatibility (verified by testing)
- UX changes should include before/after comparison
- Breaking changes to CLI/API require discussion and version bump
- Dependencies must be justified and approved before addition

### Quality Gates

Before merging to main:
- ✅ Runs with Python 3.12+ via UV
- ✅ No syntax errors when parsed by Python 3.12
- ✅ Type hints validated (if present)
- ✅ Documented in README or help text
- ✅ Tested against at least one LCC node (real or simulated)

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

**Version**: 2.0.0 | **Ratified**: 2026-02-14 | **Last Amended**: 2026-02-14
