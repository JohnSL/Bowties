---
applyTo: "app/src-tauri/src/**"
description: "Use when editing the Bowties Tauri Rust backend. Keep IPC boundaries, backend domain logic, state ownership, and protocol integration clearly separated and test-driven."
---

# Tauri Backend

- Use `product/architecture/code-placement-and-ownership.md` when deciding whether logic belongs in the Bowties backend or should instead live in the frontend layers or `lcc-rs`.
- Command modules own the IPC boundary, request validation, and error translation for frontend callers.
- Keep deeper workflow sequencing, state coordination, and domain behavior in focused backend modules instead of in large command handlers.
- Treat backend state as an authoritative application model, not as a mirror of incidental UI structure.
- Keep protocol-specific rules in `lcc-rs` or focused backend adapters when they are not truly app-specific.
- Apply SOLID by giving each module one clear responsibility: command boundary, domain workflow, state coordination, or integration adapter.
- Apply DRY by reusing shared parsing, normalization, and error-conversion helpers instead of open-coding them in each command.
- Apply YAGNI by avoiding new backend layers unless they remove a current duplication, bug, or ownership ambiguity.
- Apply TDD by adding or updating focused Rust tests around command behavior, domain transitions, and regression seams before or alongside implementation.