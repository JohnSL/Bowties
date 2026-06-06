# Handoff — Spec 014 / S8.5 Phase D continuation (T8–T12)

Branch: `014-config-modes-placeholders`. Use TDD. Test commands at the bottom.

## What's already done (do not redo)

- **Phases A, B, C** of S8.5: NodeSnapshot widened (`node_key` + `node_id: Option`), backend layout types reshaped (`AddPlaceholderBoard { node_key, profile_stem, config_values }`), per-edit placeholder IPCs deleted, save-flush synthesizes snapshots, on-disk round-trip test green.
- **Implicit-naming pivot**: `AddPlaceholderBoard` delta has **no** `display_name` field; synthesis sets `snip.user_name = String::new()`. Sidebar uses existing `resolveNodeDisplayName()` fallback to `"{manufacturer} — {model}"`.
- **Add-time tree IPC (Option A)**: backend exposes `build_placeholder_tree(node_key, profile_stem)` (registered in `lib.rs`). It runs the same CDI → tree-build → profile-overlay → active-mode-selection pipeline as `get_node_tree`'s placeholder branch (which now delegates to the shared `build_placeholder_tree_from_stem` helper in `commands/cdi.rs`).
- **T6 + T7** (frontend orchestrator):
  - New store: `app/src/lib/stores/inMemoryPlaceholders.svelte.ts` — `Map<NodeKey, {profileStem}>`. API: `register(key, stem)`, `unregister(key)`, `has(key)`, `profileStem(key)`, `list()`, `reset()`.
  - `nodeTreeStore.removeTree(nodeId)` added.
  - `placeholderBoardOrchestrator.ts` fully rewritten — `addPlaceholderBoard({profileStem})` (no name) seeds `nodeInfoStore`, `nodeTreeStore`, `configReadNodesStore`, `inMemoryPlaceholdersStore`. `deletePlaceholderBoard({nodeKey, confirm})` removes from all four. No rename, no per-field IPC.
  - `placeholderBoardOrchestrator.test.ts` rewritten (6 tests).
  - `AddBoardDialog.svelte` dropped the name input; `onAdded(nodeKey)`.
  - `lib/api/layout.ts` — deleted 4 legacy placeholder wrappers; added `buildPlaceholderTree(nodeKey, profileStem)`.
- **373/373 backend lib tests** and **950/950 frontend vitest tests** green at handoff start.

## Synthesized DiscoveredNode shape (reference)

`addPlaceholderBoard` synthesizes:
```ts
{
  node_id: [],                  // empty — gate identity logic on isPlaceholderKey(key)
  alias: 0,
  snip_data: { manufacturer, model, hardware_version: '', software_version: '',
               user_name: '', user_description: '' },
  snip_status: 'Complete',
  connection_status: 'Unknown',
  last_verified: nowIso, last_seen: nowIso,
  cdi: null, pip_flags: null, pip_status: 'NotSupported',
}
```
Keyed in `nodeInfoStore` by the `placeholder:<uuidv4>` NodeKey (not dotted-hex).

## Remaining tasks (in order)

### T8 — Save-flush composes `AddPlaceholderBoard` delta

**Files**: `app/src/lib/orchestration/saveLayoutOrchestrator.ts`, `app/src/lib/types/bowtie.ts`.

1. Add the delta variant to the frontend union in `bowtie.ts` (around line 78, alongside `addNode`):
   ```ts
   | { type: 'addPlaceholderBoard'; nodeKey: string; profileStem: string;
       configValues: Record<string, unknown> }
   ```
   Wire serde casing matches the Rust struct (camelCase fields).
2. In `saveLayoutOrchestrator.ts` around lines 119–127 where `addNode` deltas are composed from `discoveredOnlyNodeIds`: also iterate `inMemoryPlaceholdersStore.list()` and, for each `(nodeKey, profileStem)` **not** already in the saved set, push:
   ```ts
   { type: 'addPlaceholderBoard', nodeKey, profileStem, configValues: {} }
   ```
   `configValues` is empty here — the standard `configChangesStore` path will produce separate write deltas for any field edits, so the backend just needs to know "create this snapshot at save time". (Verify by checking how field edits currently turn into bus/offline writes for real nodes — placeholders should reuse that flow.)
3. After the save succeeds, call `inMemoryPlaceholdersStore.unregister(nodeKey)` for each newly persisted placeholder (the saved set now owns it). Consider: should the placeholder remain in `nodeInfoStore`? Yes — it just becomes a "saved" node from now on, with the same NodeKey.
4. TDD: add a test that mocks `saveLayoutDirectory`, seeds two placeholders via the orchestrator, calls save, asserts the deltas include both `addPlaceholderBoard` entries with correct stems.

**Open question to verify**: how does the backend re-hydrate the in-memory roster on layout open? After save+close+open, the placeholder's `NodeSnapshot` exists on disk, so backend `open_layout_directory` loads it like any other node. The frontend should see it in `nodeInfoStore` via the same path that real saved nodes use. Confirm — and **clear `inMemoryPlaceholdersStore` on layout close/open** (probably in `layoutOpenOrchestrator` / `closeLayout` handlers — search for where other ephemeral stores are reset).

### T9 — Delete `placeholderBoardsStore` + collapse sidebar

**Files**: delete `app/src/lib/stores/placeholderBoards.svelte.ts` and its test; edit `app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte`, `app/src/lib/types/bowtie.ts`, anywhere else that imports the store.

1. Remove `placeholderBoards?: Record<string, PlaceholderBoard>` from `LayoutFile` interface in `bowtie.ts` (line ~47). Also delete the `PlaceholderBoard` type. Backend already dropped the field; the frontend type just hasn't caught up.
2. In `ConfigSidebar.svelte`:
   - Drop the `placeholderBoardsStore` import (line ~11).
   - Drop the second `{#each placeholderEntries ...}` loop (lines 180–187).
   - Drop the `placeholderEntries` derivation (line ~125) and the `placeholderEntries.length === 0` from the empty-state guard.
   - Placeholders now flow through the `nodeEntries` loop because they're in `nodeInfoStore`. Verify `buildSidebarNodeEntries` accepts placeholder NodeKeys.
3. Search for all other imports of `placeholderBoardsStore` and clean up (likely a route or two).
4. **Watch out for `canonicalizeNodeId` in `app/src/lib/utils/nodeRoster.ts`** — it strips dots + uppercases, which mangles `placeholder:<uuid>` into `PLACEHOLDER:<UUID>`. Add a NodeKey-aware short-circuit using `isPlaceholderKey` (already exists in `nodeKey.ts`) or create a parallel `canonicalizeNodeKey`. This affects `+page.svelte`'s `unsavedInMemoryNodeIds` computation around lines 290–320.
5. Verify the sidebar label falls back to `"{manufacturer} — {model}"` when `snip.user_name === ''`, and updates live as the CDI User Name leaf is edited (the existing `effectiveValue` waterfall + `resolveNodeDisplayName` should handle this — confirm with a focused test).

### T10 — Placeholder eventid badge in TreeLeafRow

**File**: `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte` around line 504 (predicate) and 647–658 (current eventid editor).

When `isPlaceholderKey(nodeId) && leaf.elementType === 'eventId'`, render the same EventId field as a real board — showing the event ID value and the producer/consumer role badge — but disabled, with no add-connection control (FR-014 — placeholder EventId fields should look as much like a real board as possible). The existing EventId editor is reused with `disabled` state; the add-connection button is hidden.
TDD with a focused `TreeLeafRow.test.ts` case.

### T11 — Delete-placeholder UX

**Files**: `app/src-tauri/src/menu.rs`, `app/src/routes/+page.svelte`, possibly a header component in the config pane.

1. Backend: add a sibling `Delete Placeholder Board…` menu item next to `Add Placeholder Board…` (around lines 25–51). Add `MenuHandles.delete_placeholder_board`. Gate enabled state on "selected node is a placeholder" (frontend-driven via existing menu-state IPC).
2. Frontend: in `+page.svelte`, add a `canDeletePlaceholderBoard` computed flag (true when `selectedNodeKey` is a placeholder); pass to `syncMenuState`. Wire the menu event to a confirmation modal → `deletePlaceholderBoard({nodeKey, confirm})` orchestrator call.
3. Also add a delete button in the config-pane header visible only when the selected node is a placeholder. Reuse the existing confirmation modal pattern (search for `DiscardConfirmDialog` for the shape).
4. TDD: tests for the gating logic + that the orchestrator is called.

### T12 — End-to-end test

Write a Vitest/`+page.svelte` integration test covering both quickstart scenarios:

**Scenario A — add → edit → close without save → reopen → absent**
1. Open empty layout.
2. `addPlaceholderBoard({profileStem})`.
3. Set a config value via `configChangesStore.set`.
4. Trigger close without save (use the discard path).
5. Reopen layout.
6. Assert the placeholder NodeKey is not in `nodeInfoStore` and not in `inMemoryPlaceholdersStore`.

**Scenario B — add → edit User Name leaf → save → reopen → present with edits**
1. Open empty layout.
2. `addPlaceholderBoard({profileStem})`.
3. Edit the CDI User Name leaf (use the standard `configChangesStore.set` path with the leaf's path).
4. Save.
5. Reopen.
6. Assert the placeholder is in `nodeInfoStore`, the tree carries the edited User Name, and the sidebar label shows the edited name.

Mock `invoke` calls at the `@tauri-apps/api/core` layer; reuse the page-test infrastructure from `src/routes/page.route.test.ts`.

## Risk seams to watch

- **`canonicalizeNodeId`** in `nodeRoster.ts` mangles placeholder keys — must short-circuit (T9).
- **Save flush ordering**: `addPlaceholderBoard` delta must be applied *before* any field-write deltas the user composed for that placeholder, since the snapshot must exist before its config values can be set. Verify the backend's `apply_layout_deltas` handles this order correctly (it should — `AddPlaceholderBoard` is currently a no-op for `apply_layout_deltas` and synthesis happens at save-flush-write time before field writes execute against the snapshot). If field writes go through the standard write path (bus or offline-changes), they don't touch the snapshot file directly, so ordering may not matter — confirm.
- **Layout open / close lifecycle**: `inMemoryPlaceholdersStore.reset()` must fire on close and new-layout. Search for where `nodeInfoStore.set(new Map())` is called and add the reset there.
- **Type mismatch on `node_id: []`**: the synthesized empty array may break callers that assume `node_id.length === 6`. Audit any consumer that calls `formatNodeId(node.node_id)` or iterates `node_id` for placeholder keys. Likely candidates: `nodeInfo.ts` `updateNodeInfo()` (rebuilds keys from byte arrays — would key a placeholder under the empty-string formatted form). The orchestrator inserts directly into the Map by key so it dodges that, but if any sync ever rebuilds the Map via `updateNodeInfo`, placeholders would disappear. **Verify** that no code path rebuilds `nodeInfoStore` from a `DiscoveredNode[]` that includes placeholders, or extend `updateNodeInfo` to preserve placeholder entries.

## Architectural decisions baked in

- Placeholders use the same `nodeInfoStore` / `nodeTreeStore` / `configReadNodesStore` as real nodes, keyed by `placeholder:<uuidv4>` NodeKey (ADR-0008).
- Tree-build for unsaved placeholders happens **backend-side** via `build_placeholder_tree` IPC (Option A — keeps single source of truth for tree assembly; rejected: porting the build pipeline to TypeScript).
- Implicit naming: no name prompt anywhere. Sidebar label resolution is `snip.user_name || "{manufacturer} — {model}"` (existing `resolveNodeDisplayName` helper).
- No per-field placeholder IPCs — field edits use `configChangesStore.set` like real nodes.
- `inMemoryPlaceholdersStore` is the seam between "in-memory placeholder created" and "save-flush composes delta".

## Test commands

```powershell
# Backend
cd d:\src\github\LCC\Bowties\app\src-tauri
cargo test --lib

# Frontend (full suite)
cd d:\src\github\LCC\Bowties\app
npx vitest run

# Frontend (one file)
npx vitest run src/lib/orchestration/placeholderBoardOrchestrator.test.ts
```

## Progress tracking

After each task, update the **Progress** section in
`specs/014-config-modes-placeholders/slices.md` and tick the corresponding
checkbox in the S8.5 task list (lines ~310–316). Do **not** create new memory
files — slices.md is the source of truth (per repo memory note
`knowledge-base-initiative.md`).

## Reference files

- Backend IPC helper: `app/src-tauri/src/commands/cdi.rs` → `build_placeholder_tree_from_stem`
- Backend IPC: `app/src-tauri/src/commands/placeholders.rs` → `build_placeholder_tree`
- Backend delta: `app/src-tauri/src/layout/types.rs` → `LayoutEditDelta::AddPlaceholderBoard`
- Backend synthesis: `app/src-tauri/src/commands/layout_capture.rs` → `save_layout_directory` (~line 486)
- Frontend orchestrator: `app/src/lib/orchestration/placeholderBoardOrchestrator.ts`
- Frontend in-memory roster: `app/src/lib/stores/inMemoryPlaceholders.svelte.ts`
- Frontend API: `app/src/lib/api/layout.ts` → `buildPlaceholderTree`
- Sidebar: `app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte`
- TreeLeafRow: `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`
- NodeKey helpers: `app/src/lib/utils/nodeKey.ts` → `isPlaceholderKey`
- Display name: `app/src/lib/utils/nodeDisplayName.ts` → `resolveNodeDisplayName`
- NodeRoster (canonicalize bug): `app/src/lib/utils/nodeRoster.ts`
- Menu: `app/src-tauri/src/menu.rs`
