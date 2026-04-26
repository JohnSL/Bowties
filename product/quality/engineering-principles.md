# Engineering Principles

## Purpose

This document explains how Bowties applies SOLID, DRY, YAGNI, and TDD in the current codebase.

These principles are not slogans. They are concrete ownership and change-discipline rules used to reduce regressions.

## SOLID In Bowties

Apply SOLID as ownership discipline.

### Single Responsibility

- Route files own screen composition, visible page state, and UI entry points.
- Components own rendering, small interaction handling, and emitted intent.
- Orchestrators own multi-step async workflows and lifecycle sequencing.
- Stores own durable frontend state and deterministic transitions.
- Utility modules own pure normalization, formatting, comparison, and translation helpers.
- Tauri command modules own IPC boundaries and error translation.
- Deeper backend modules own application workflow and state coordination.
- `lcc-rs` owns reusable LCC/OpenLCB protocol behavior.

If a file mixes two or more of those concerns, the change should be reconsidered.

### Open/Closed

- Prefer extending existing owners with one more explicit rule over creating parallel implementations of the same behavior.
- Add behavior by extending the current store, orchestrator, backend module, or helper that already owns the seam.
- Do not add a second normalization path, fallback path, or lifecycle path when one already exists.

### Liskov Substitution

- Keep shared contracts stable when replacing implementations behind them.
- Frontend stores, orchestrators, and backend commands should preserve their observable contracts unless the product docs and tests are updated intentionally.

### Interface Segregation

- Keep store and orchestrator APIs focused on the workflow they own.
- Avoid broad “manager” APIs that expose unrelated capabilities to many callers.
- Keep backend command surfaces small and purpose-specific.

### Dependency Inversion

- Depend on clear boundaries instead of view-layer details.
- Routes and components should depend on stores, orchestrators, and explicit helper APIs rather than embedding backend or protocol details.
- Backend application code should depend on focused adapters and library APIs rather than scattering wire-level logic.

## DRY In Bowties

DRY means one canonical owner for each shared rule.

Current high-value examples:

- one Node ID normalization rule
- one display-name fallback rule
- one lifecycle transition owner per workflow
- one canonical comparison or formatting helper per repeated concept
- one documented acceptance rule per user-visible behavior

When a change repeats a rule in a second place, prefer extracting or reusing the existing owner instead.

## YAGNI In Bowties

Bowties should prefer the smallest explicit abstraction that solves the current problem.

- Prefer focused helpers over general frameworks.
- Introduce a shared abstraction only when at least two real call sites need the same concept.
- Do not add generic orchestration layers, store wrappers, backend managers, or library convenience APIs without a concrete current need.
- Keep product docs focused on current truth, not speculative future architecture.

## TDD In Bowties

Use TDD as behavior protection at the owning seam.

- For production behavior changes, add or update the narrowest test that can prove the contract before or alongside the implementation.
- For regressions, encode the observed failure as a repeatable behavior contract in tests.
- Prefer owner-level tests over broad route-level tests when the behavior can be proven at the extracted seam.
- Use route-level tests for cross-component workflow behavior that cannot be proven elsewhere.

## Change Rules

When changing behavior:

1. Identify the owning layer.
2. Add or update the closest focused test.
3. Make the smallest change that satisfies the behavior contract.
4. Update the relevant `product/` document if the behavior or ownership rule changed intentionally.

## High-Risk Seams

Apply these principles especially carefully around:

- lifecycle ownership
- Node ID normalization
- display-name fallback
- sync-session trigger and dismiss behavior
- offline capture, replay, discard, and apply flows
- frontend/backend/library placement boundaries