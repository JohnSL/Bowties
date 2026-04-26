# Lifecycle And State Ownership

## Purpose

This document names the owner of each major lifecycle transition and state machine in Bowties. It is the authoritative current reference for where each lifecycle decision lives and which module has the right to drive each transition.

These ownership rules prevent regressions that arise when lifecycle decisions are duplicated or driven from the wrong layer (e.g., a component driving a transition that belongs in an orchestrator, or two orchestrators competing to advance the same state).

---

## Lifecycle Machines

### 1. App Startup Bootstrap

**Owner:** `syncSessionOrchestrator.ts` — `bootstrapStartupLifecycle()`

**When it runs:** Once at app mount (`onMount` in `+page.svelte`).

**Decisions it makes:**

| Condition | Transition name | Outcome |
|---|---|---|
| Connected, no layout | `startup_fresh_live` | Reset live session state, probe for nodes |
| Connected, layout present | `startup_preserved_layout` | Keep layout state, probe for nodes |
| Disconnected, layout or no layout | `startup_disconnected_idle` | No probe, restore recent offline layout if available |

**State set by this path:**

- `layout.svelte.ts` → `_connected`
- `syncSessionOrchestrator.ts` → `resetFreshLiveSessionState()` if applicable
- `offlineLayoutOrchestrator.ts` → `restoreRecentOfflineLayout()` if disconnected
- Node tree listening (started unconditionally)
- Bowtie event listening (started unconditionally)

**Source:** `lifecycleTransitionMatrix.ts` — `resolveStartupTransition()`

---

### 2. Connect Transition

**Owner:** `syncSessionOrchestrator.ts` — `connectLiveSession()`

**When it runs:** When the user confirms a connection in the connection dialog.

**Decisions it makes:**

| Condition | Transition name | Outcome |
|---|---|---|
| No layout present | `connect_fresh_live` | Reset live session state, probe for nodes |
| Layout already open | `connect_preserved_layout` | Keep layout state, probe for nodes |

**State set by this path:**

- Connection label set on backend
- `layout.svelte.ts` → `_connected = true`
- `syncSessionOrchestrator.ts` → `resetFreshLiveSessionState()` if `connect_fresh_live`
- Node probe triggered

**Source:** `lifecycleTransitionMatrix.ts` — `resolveConnectTransition()`

---

### 3. Disconnect Transition

**Owner:** `syncSessionOrchestrator.ts` — `disconnectWithOfflineFallback()`

**When it runs:** When the user explicitly disconnects or the connection is lost.

**Decisions it makes:**

| Condition | Transition name | Outcome |
|---|---|---|
| Layout present, has snapshots | `rehydrated_offline` | Rehydrate offline node trees from snapshots |
| Layout present, no snapshots | `preserved_layout` | Preserve current layout state, clear live state |
| No layout, any snapshot state | `cleared_to_connection` | Clear live state entirely |

**State set by this path:**

- Backend `disconnect()` call
- `layout.svelte.ts` → `_connected = false`
- `offlineLayoutOrchestrator.ts` → `rehydrateOffline()` or `clearLiveState()`

**Source:** `lifecycleTransitionMatrix.ts` — `resolveDisconnectTransition()`

---

### 4. Offline Layout Open Machine

**Owner:** `layoutOpenLifecycle.ts` (store phase machine) + `offlineLayoutOrchestrator.ts` (driver)

**Phases (in order):**

```
idle → opening_file → hydrating_snapshots → replaying_offline_changes → ready
                 ↘ error ↙ (from any phase)
```

| Phase | Meaning |
|---|---|
| `idle` | No open in progress |
| `opening_file` | File picker dialog or recent-file restore in progress |
| `hydrating_snapshots` | Loading captured node snapshots from the layout directory |
| `replaying_offline_changes` | Replaying offline-changes.yaml onto tree state |
| `ready` | Layout open and trees hydrated |
| `error` | Open failed; reset to `idle` or retry |

**Phase driver:** `offlineLayoutOrchestrator.ts` calls the transition helpers in `layoutOpenLifecycle.ts`:

- `startLayoutOpen()` → `opening_file`
- `startLayoutHydration()` → `hydrating_snapshots`
- `finishLayoutHydration()` → `replaying_offline_changes`
- `finishOfflineReplay()` → `ready`
- `failLayoutOpen()` → `error`
- `resetLayoutOpenPhase()` → `idle`

**Do not:** advance `layoutOpenPhase` from a route, component, or store. Only `offlineLayoutOrchestrator.ts` drives these transitions.

**Tests:** `app/src/lib/stores/layoutOpenLifecycle.test.ts`, `app/src/lib/orchestration/offlineLayoutOrchestrator.test.ts`

---

### 5. Offline Layout Close / Discard Transition

**Owner:** `offlineLayoutOrchestrator.ts` — `closeOfflineLayout()` / `discardOfflineLayout()`

**When it runs:** When the user explicitly closes or discards an offline layout.

**It resets:**

- `layoutOpenPhase` → `idle`
- `layout.svelte.ts` layout state cleared
- `offlineChangesStore` rows cleared
- Node tree cleared

**Do not:** drive close/discard from a route or component. The route emits intent (user pressed Close); the orchestrator executes the transition.

---

### 6. Sync Session Lifecycle

**Owner:** `syncSessionOrchestrator.ts` — `maybeTriggerSync()`, `scheduleAutoSync()`

**Sync Panel State Owner:** `syncPanel.svelte.ts`

**When sync-panel auto-show triggers:**

1. User reconnects to the bus with a layout open (`connect_preserved_layout` transition).
2. Discovery settles (all SNIP/PIP enrichment complete) and `syncPanel.svelte.ts.isDismissed` is still false.
3. Layout has pending offline changes (`pendingCount > 0`).
4. Node match status is `full_match`.

**When sync-panel is NOT re-triggered:**

- `isDismissed` is true. The user must manually re-open.
- Layout has no pending changes.
- Match status is not `full_match` (nodes missing or extra).

**Source:** `syncSessionOrchestrator.ts` — `resetSyncSessionAutoTrigger()` on fresh-live or connect-fresh transitions.

---

## State Ownership Summary

| State | Owner module | Layer |
|---|---|---|
| Connection status (`_connected`) | `layout.svelte.ts` | Store |
| Layout file and path | `layout.svelte.ts` | Store |
| Active layout context | `layout.svelte.ts` | Store |
| Layout open phase machine | `layoutOpenLifecycle.ts` | Store |
| Offline change rows (persisted + draft) | `offlineChanges.svelte.ts` | Store |
| Sync panel session and resolutions | `syncPanel.svelte.ts` | Store |
| Node tree | `nodeTree.svelte.ts` | Store |
| Node SNIP/PIP info | `nodeInfo.ts` | Store |
| Config read status per node | `configReadStatus.ts` | Store |
| Config sidebar selection | `configSidebar.ts` | Store |
| Bowtie metadata | `bowtieMetadata.svelte.ts` | Store |
| Bowties list | `bowties.svelte.ts` | Store |
| Startup/connect/disconnect transitions | `syncSessionOrchestrator.ts` | Orchestrator |
| Offline open/replay/close/discard | `offlineLayoutOrchestrator.ts` | Orchestrator |
| Sync session auto-trigger | `syncSessionOrchestrator.ts` | Orchestrator |
| Post-apply tree reconciliation | `syncApplyOrchestrator.ts` | Orchestrator |
| Discovery enrichment | `discoveryOrchestrator.ts` | Orchestrator |
| Config read execution and sessions | `configReadOrchestrator.ts`, `configReadSessionOrchestrator.ts` | Orchestrator |
| CDI dialog state | `cdiDialogOrchestrator.ts` | Orchestrator |
| Sync panel view flow | `syncPanelViewOrchestrator.ts` | Orchestrator |
| Lifecycle transition decisions | `lifecycleTransitionMatrix.ts` | Shared logic |

---

## High-Risk Seams

### Lifecycle Ordering: Open/Apply/Disconnect

Layout `open`, `apply`, and `disconnect` must not run concurrently or interleave. Each path drives `layoutOpenPhase` transitions or clears layout state, and concurrent execution produces undefined store state.

Rule: The route serializes these user-driven actions (disabled/busy-gated UI). The orchestrators assert expected phases before advancing.

### Sync-Session Auto-Trigger Suppression

The sync panel must not re-open automatically after the user dismisses it. `isDismissed` on `syncPanel.svelte.ts` gates the auto-trigger in `syncSessionOrchestrator.ts`. This flag is only reset when `resetSyncSessionAutoTrigger()` is called during a `connect_fresh_live` or `startup_fresh_live` transition (i.e., when there is no existing layout to sync).

### Post-Apply Snapshot Refresh

After `syncApplyOrchestrator.ts` completes, it must refresh offline changes from the backend and rebuild the affected trees — not use the stale in-memory state. The backend prunes already-applied rows from `offline-changes.yaml` before returning the apply result.

---

## Sources

- `app/src/lib/orchestration/lifecycleTransitionMatrix.ts`
- `app/src/lib/stores/layoutOpenLifecycle.ts`
- `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`
- `app/src/lib/orchestration/syncSessionOrchestrator.ts`
- `app/src/lib/orchestration/syncApplyOrchestrator.ts`
- `app/src/lib/stores/layout.svelte.ts`
- `app/src/lib/stores/syncPanel.svelte.ts`
- `specs/010-offline-layout-editing/refactoring-roadmap.md` (Track A, current-progress section)
