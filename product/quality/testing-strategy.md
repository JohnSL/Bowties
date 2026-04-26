# Testing Strategy

## Purpose

This document defines how Bowties uses tests to protect current behavior and reduce regressions.

The goal is not even distribution of tests by file count. The goal is to protect user-visible behavior and high-risk ownership seams with the narrowest effective test.

## Core Rules

- Add or update the narrowest test that can prove the intended contract.
- Keep tests aligned with ownership boundaries: stores for deterministic state, orchestrators for sequencing, components for rendering and emitted intent, routes for cross-component workflow behavior, backend tests for application workflow and persistence, and `lcc-rs` tests for protocol correctness.
- When fixing a regression, encode the failure as a test before or alongside the implementation.
- When intentional behavior changes land, update both tests and the relevant `product/` documents.

## Test Layers

### Store Tests

Use store tests for:

- deterministic state transitions
- derived state rules
- modified/dirty-state logic
- restoring saved state

Current examples include:

- `app/src/lib/stores/offlineChanges.store.test.ts`
- `app/src/lib/stores/layoutOpenLifecycle.test.ts`
- `app/src/lib/stores/syncPanel.store.test.ts`
- `app/src/lib/stores/nodeTree.store.test.ts`

### Orchestrator Tests

Use orchestrator tests for:

- multi-step async workflows
- lifecycle sequencing
- cross-store coordination
- backend call ordering
- workflow guard conditions and branching logic

Current examples include:

- `app/src/lib/orchestration/discoveryOrchestrator.test.ts`
- `app/src/lib/orchestration/configReadOrchestrator.test.ts`
- `app/src/lib/orchestration/offlineLayoutOrchestrator.test.ts`
- `app/src/lib/orchestration/syncApplyOrchestrator.test.ts`
- `app/src/lib/orchestration/syncSessionOrchestrator.test.ts`
- `app/src/lib/orchestration/unsavedChangesGuard.test.ts`

### Component Tests

Use component tests for:

- rendering contracts
- displayed fallback rules
- emitted user intent
- view-only gating behavior

Current examples include:

- `app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts`
- `app/src/lib/components/ElementCardDeck/SaveControls.test.ts`
- `app/src/lib/components/Sync/SyncPanel.lifecycle.test.ts`
- `app/src/lib/components/NodeList.test.ts`

### Route-Level Tests

Use route-level tests only when behavior crosses multiple lower-level seams and cannot be proven at a narrower owner.

Examples include:

- dialog and layout interactions
- connect, discover, and read flows spanning multiple stores and orchestrators
- user-visible workflow behavior in `app/src/routes/+page.svelte`

Current example:

- `app/src/routes/page.route.test.ts`

### Backend Tests

Use Rust backend tests for:

- application workflow and persistence rules
- command behavior and error classification
- snapshot, layout, and sync helpers
- backend state coordination

When the rule is backend-specific, prefer a Rust test over indirectly proving it only through frontend tests.

### `lcc-rs` Tests

Use `lcc-rs` tests for:

- frame, datagram, and protocol parsing or encoding
- alias and discovery semantics
- transport behavior
- reusable protocol helpers used across consumers

Because `lcc-rs` is a reusable protocol library, it should carry stronger low-level test evidence than app glue code.

## Choosing The Right Test

Use this order when choosing a test seam:

1. Can the contract be proven in a pure helper or store test?
2. If not, can it be proven in an orchestrator test?
3. If not, can it be proven in a component test?
4. Use a route-level test only when the behavior depends on cross-component workflow composition.
5. Use backend or library tests when the owning behavior is not truly frontend behavior.

## Regression-Protection Rules

The current regression pass around offline, sync, discovery, and config workflows reinforces these rules:

- protect lifecycle ownership with owner-level tests where possible
- protect display-name fallback and normalization rules with focused helper, component, or orchestrator tests
- protect sync and offline flows with orchestrator and store tests first
- use route-level tests to guard user-visible behavior that spans multiple layers

High-risk seams that should usually get explicit regression tests include:

- discovery naming and SNIP/PIP enrichment
- CDI gating and download prompts
- connect-dialog and layout-open interaction
- offline pending-change save, discard, replay, and apply flows
- sync session auto-trigger, dismissal, and reopen behavior
- pending-value restamping after discard, reconnect, and partial apply

## Test Maintenance Rules

- Prefer reusing existing test files that already cover the owning seam.
- Do not duplicate the same contract at multiple layers without a clear reason.
- If a manual bug report still reproduces, add or extend the narrowest test that can fail red first.
- Keep route tests focused on behavior, not implementation detail.
- Keep component tests focused on rendered output and intent emission, not internal orchestration behavior.

## Done Criteria For Behavior Changes

A behavior change is not complete until:

- the owning seam has focused test coverage
- the relevant higher-level workflow still has enough coverage to protect the user-visible contract
- stale tests that encode the old behavior have been updated intentionally
- the relevant `product/` docs reflect the current intended behavior