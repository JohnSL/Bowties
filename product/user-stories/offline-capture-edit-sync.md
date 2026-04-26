# Offline Capture, Edit, And Sync

## Purpose

This document captures the current user stories for Bowties' offline layout loop: capture a layout from a live bus, open it offline, make supported offline changes, and later reconnect to sync those changes.

It is validated against the implemented parts of spec 010, current orchestration and backend layout modules, and the current regression-protection tests around offline and sync behavior.

## Validation Sources

- `specs/010-offline-layout-editing/spec.md`
- `specs/010-offline-layout-editing/tasks.md`
- `specs/010-offline-layout-editing/regression-test-plan.md`
- `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`
- `app/src/lib/orchestration/syncSessionOrchestrator.ts`
- `app/src/lib/orchestration/syncApplyOrchestrator.ts`
- `app/src/lib/stores/layout.svelte.ts`
- `app/src/lib/stores/layoutOpenLifecycle.ts`
- `app/src/lib/stores/offlineChanges.svelte.ts`
- `app/src/lib/stores/syncPanel.svelte.ts`
- `app/src-tauri/src/layout/**`
- `app/src-tauri/src/commands/layout_capture.rs`
- `app/src-tauri/src/commands/sync_panel.rs`
- current route, store, orchestrator, component, and backend tests covering layout open, offline changes, sync session, sync apply, and save/discard behavior

## Current User Story 1: Capture A Layout From A Live Bus

A user connects to a live bus, reads the available node/configuration state, and saves that layout for later use.

### Current Behavior

- The user can capture the current layout into the current layout file model.
- The saved layout preserves the information needed for later offline work, including node snapshot data and current related metadata handled by the layout system.
- Saving uses the current persistence behavior instead of relying on a temporary in-memory-only snapshot.

### What The User Gets

- a durable saved layout that can be reopened later
- a baseline for offline work and later sync

## Current User Story 2: Open And Browse A Captured Layout Offline

A user opens a saved layout without connecting to a live bus and continues to browse the captured system state.

### Current Behavior

- Opening a saved layout loads the current layout state into the offline-capable frontend model.
- The app shows offline layout state without requiring a live bus connection.
- Captured tree/configuration state can be hydrated back into the current frontend views.
- The layout-open lifecycle suppresses misleading transient UI states until the open flow is ready.

### What The User Gets

- offline browsing of captured nodes and configuration
- current layout-backed bowtie state available without a live bus
- stable UI behavior during layout open rather than transient dirty/read prompts

## Current User Story 3: Make And Save Offline Changes

A user edits supported configuration and bowtie-related state while offline and saves those changes into the layout.

### Current Behavior

- Offline changes are tracked against a captured baseline rather than being treated as live bus writes.
- The product distinguishes current offline pending changes from the baseline state.
- Save and discard apply to the current layout-backed offline changes model.
- Reverting a planned offline change participates in the same current save/discard lifecycle instead of being auto-written separately.

### What The User Gets

- the ability to prepare configuration and connection changes away from the bus
- visible pending-change state while offline
- durable saved offline changes that are restored when the layout is reopened

## Current User Story 4: Reconnect And Sync Pending Offline Changes

A user reconnects to a bus with a layout that has pending offline changes and needs to compare and apply them safely.

### Current Behavior

- The current product builds a sync session when a layout with pending offline changes is active and the user connects to the bus.
- The sync panel separates rows into current categories such as conflicts, clean changes, and already-applied changes.
- The user can resolve conflicts and apply eligible changes through the current sync workflow.
- Already-applied rows can be auto-cleared by the current implemented sync behavior.
- Dismissed sync UI does not re-open automatically without the current explicit re-entry path.
- Partial failures keep the remaining pending state visible instead of silently losing changes.

### What The User Gets

- a controlled compare-and-apply workflow instead of silent background writes
- visibility into which changes are clean, conflicting, already applied, or still pending
- preservation of unresolved or failed changes for later retry

### Current Limits

- This document does not treat future staged-node and broader bench-preparation workflows as fully current product behavior.

## Supported Outcome

Today, a user can:

1. capture a layout from a live bus
2. reopen and browse that layout offline
3. make supported offline changes and save them
4. reconnect later and use the current sync workflow to compare and apply pending changes

That is the durable current user story for Bowties' offline capture, edit, and sync loop.