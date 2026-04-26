# Frontend Boundaries

## Purpose

This document defines the current ownership boundaries inside the Bowties frontend.

Use it together with `product/architecture/code-placement-and-ownership.md` when deciding where frontend logic belongs and when reviewing changes that touch routes, components, orchestrators, stores, or shared utilities.

## Current Frontend Pattern

Bowties uses a route plus orchestrator plus store plus component split.

- Routes compose screens, own visible page-level state, and hand user intent to lower layers.
- Components render state and emit intent.
- Orchestrators own multi-step async workflows and lifecycle-sensitive transitions.
- Stores own durable frontend state and deterministic transitions.
- Utilities own pure normalization, formatting, comparison, and translation helpers.

When a behavior needs multiple async steps, cross-store coordination, or lifecycle branching, it should move out of `.svelte` view code and into an orchestrator, store, or pure helper with focused tests.

## Current Owners By Area

### Routes

Current route surfaces include:

- `app/src/routes/+page.svelte`
- `app/src/routes/config/+page.svelte`
- `app/src/routes/traffic/+page.svelte`

Routes own:

- top-level page composition
- tab, dialog, and visible page-state coordination
- wiring user actions to orchestrators and stores
- cross-feature UI entry points

Routes should not become the owner of:

- multi-step discovery, config-read, offline, or sync workflows
- backend call sequencing
- duplicated normalization or fallback rules

### Components

Current component areas include:

- `app/src/lib/components/ConfigSidebar/**`
- `app/src/lib/components/ElementCardDeck/**`
- `app/src/lib/components/Sync/**`
- `app/src/lib/components/Layout/**`
- `app/src/lib/components/Bowtie/**`

Components own:

- rendering
- local UI interaction
- emitted user intent
- small display-only derivations

Components should not own:

- retry loops
- lifecycle orchestration
- backend workflow sequencing
- app-wide state transitions

### Orchestrators

Current orchestrators include:

- `discoveryOrchestrator.ts`
- `configReadOrchestrator.ts`
- `configReadSessionOrchestrator.ts`
- `offlineLayoutOrchestrator.ts`
- `syncSessionOrchestrator.ts`
- `syncApplyOrchestrator.ts`
- `syncPanelViewOrchestrator.ts`
- `cdiDialogOrchestrator.ts`
- `unsavedChangesGuard.ts`
- `lifecycleTransitionMatrix.ts`

Orchestrators own:

- discovery and enrichment sequencing
- configuration read sessions
- offline layout capture/open/save/discard/apply workflows
- sync-session creation and application
- lifecycle-sensitive transitions and UI guards
- ordering backend calls and store updates

Orchestrators should not own:

- long-lived frontend state that belongs in a store
- pure normalization or formatting rules that belong in utilities
- rendering concerns that belong in components

### Stores

Current stores include:

- `layout.svelte.ts`
- `layoutOpenLifecycle.ts`
- `offlineChanges.svelte.ts`
- `syncPanel.svelte.ts`
- `bowtieMetadata.svelte.ts`
- `bowties.svelte.ts`
- `configReadStatus.ts`
- `configSidebar.ts`
- `configFocus.svelte.ts`
- `connectionRequest.svelte.ts`
- `nodes.ts`
- `nodeInfo.ts`
- `nodeTree.svelte.ts`
- `pillSelection.ts`

Stores own:

- durable frontend state
- deterministic transitions
- derived state needed by routes and components
- explicit state APIs used by orchestrators

Stores should not own:

- broad multi-step async sequencing unless the store is explicitly documented as that workflow owner
- duplicated helper logic that belongs in utilities

### Utilities

Current utility areas include:

- `nodeId.ts`
- `nodeDisplayName.ts`
- `formatters.ts`
- `eventIds.ts`
- `serialize.ts`
- `cardTitle.ts`
- `xmlFormatter.ts`

Utilities own:

- Node ID normalization
- display-name fallback rules
- formatting and comparison helpers
- pure translation and serialization rules

Utilities should not own:

- store mutation
- backend calls
- hidden workflow sequencing

## High-Risk Ownership Seams

These seams have a higher regression risk and should remain explicit in code, tests, and documentation.

### Discovery And Naming

- Discovery sequencing belongs in `discoveryOrchestrator.ts`.
- Durable node and tree state belongs in stores such as `nodes.ts`, `nodeInfo.ts`, and `nodeTree.svelte.ts`.
- Display-name fallback and Node ID normalization belong in shared utilities, not local route or component code.

### Config Read And CDI Gating

- Read-session sequencing and preflight checks belong in config-read orchestrators.
- Visible prompts and call-to-action rendering belong in routes and components.
- Gating rules should not be duplicated between route code, sidebar code, and helper code.

### Offline Layout And Sync

- Layout capture, reopen, discard, replay, sync-session build, and apply sequencing belong in orchestrators.
- Pending changes, layout-open state, and sync-panel state belong in stores.
- Visible confirmation and conflict presentation belong in components.

### Lifecycle Ownership

- Lifecycle transitions should have one named owner.
- Route files may initiate a workflow, but they should not silently become the owner of complex transition logic.
- If a transition touches multiple stores and backend calls, it should usually be orchestrator-owned.

## Review Rules

Re-check placement when a change:

- adds multi-step async logic to a route or component
- duplicates a naming, normalization, or fallback rule in more than one frontend layer
- moves durable state into orchestration code or view code
- moves sequencing into stores without naming the store as the explicit owner
- adds frontend behavior that clearly belongs in the backend or `lcc-rs`

## Testing Expectations

- Store-owned rules should be protected by store tests.
- Orchestrator-owned workflows should be protected by orchestrator tests.
- Component-owned rendering and emitted intent should be protected by component tests.
- Route-level tests should protect cross-component workflow behavior that cannot be proven at a narrower seam.

When a regression exposes a missing ownership rule, update this document and the relevant tests together.