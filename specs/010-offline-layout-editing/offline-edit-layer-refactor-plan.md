# Edit Layer Refactor — Architectural Plan

**Date**: 2026-05-06  
**Status**: Approved — ready for implementation  
**Scope**: Replace the overloaded `leaf.modifiedValue` design with two deep modules — a **changes module** and a **ConfigEditor** — that unify online and offline config-edit tracking behind a layered, data-driven architecture.

---

## Why This Refactor, Not Another Quick Fix

Every attempt to fix offline editing bugs has introduced new regressions. The pattern is clear:

- Fix annotation "from" value → combo box stops accepting edits after tree rebuild.
- Switch `persistedRows` to `effectiveRows` so drafts survive rebuilds → pill selectors and segment controls disappear due to double tree-store mutation within a single event cycle.
- Guard `applyOfflinePendingValues` to skip user-draft leaves → dirty indicators diverge between online and offline modes.

These are not independent bugs. They share one root cause: **`leaf.modifiedValue` serves two unrelated roles**, and every fix that touches one role destabilizes the other. Quick fixes cannot resolve this because the field itself is the design defect. The only path forward is to separate the two roles structurally so they can never interfere.

**This plan is exclusively focused on refactoring.** It does not include quick fixes, interim patches, or partial mitigations. Every step replaces a problematic structure with a better one rather than adding another conditional around the existing structure.

---

## Architectural Problem

### What `modifiedValue` does today

```
leaf.value            ← committed bus/snapshot value (Layer 1)
leaf.modifiedValue    ← OVERLOADED:
                         Role A: "user just typed something"       (in-memory edit)
                         Role B: "persisted offline planned value"  (layout stamp)
leaf.isOfflinePending ← disambiguator bolt-on: true = Role B, false = Role A
```

Every consumer of `modifiedValue` must handle four states:

| `modifiedValue` | `isOfflinePending` | Meaning |
|---|---|---|
| `null` | `false` | No change — show `leaf.value` |
| `{value}` | `false` | User edit — show as dirty, count in save |
| `{value}` | `true` | Persisted offline stamp — show as pending, exclude from save count |
| `{value}` | `false` (but a persisted row also exists) | User edit on top of a persisted stamp — show as dirty, but "from" value is the stamp, not the bus |

This fourth state is where every regression lives.

### What makes the current design unstable

1. **`applyOfflinePendingValues` mutates the tree store.** It deep-clones every affected tree and reassigns `this._trees`. This triggers Svelte 5 reactivity for every component that derives from `trees`. When this runs inside the `node-tree-updated` callback — which already set `this._trees` once via `loadTree` — the tree store is mutated twice in one event cycle. The double mutation cascades through `SegmentView`, `TreeGroupAccordion`, and `PillSelector`, causing layout components to briefly lose their segment and re-derive it as `null`.

2. **Online and offline edit paths diverge.** Online: value → Rust IPC → `node-tree-updated` → tree refresh → `modifiedValue` arrives from backend. Offline: value → `offlineChangesStore.upsertConfigChange` → `setLeafModifiedValue` (frontend-only) → tree cloned locally. Same user action, two completely different flows that must produce identical UX.

3. **The clear-and-restamp pattern is a fragile coupling.** `clearAllModifiedValues()` + `applyOfflinePendingValues()` must be called in exact sequence. Anyone who calls one without the other gets inconsistent state. This pattern runs after every save, every discard, every sync apply, and every tree rebuild.

---

## Target Architecture — Two Modules

The refactor replaces the overloaded `modifiedValue` field and its scattering of display/edit/dirty logic with two cooperating modules:

### Module 1: Changes Module (state)

**File**: `app/src/lib/stores/configChanges.svelte.ts`

Owns layered change state for every config field. Has no knowledge of CDI structure, constraints, profiles, or cascading. Its interface is small and data-driven:

**Read interface — what callers consume:**

- `visibleValue(key)` → always returns the value a control should display. Always resolved — callers never compute this themselves.
- `changeLayers(key)` → returns an ordered list of change entries, each with a `ChangeLayerType` (`'draft'` | `'offlinePending'` | `'baseline'`). Baseline is always present when config is loaded (from bus or snapshot). Components map layer type to color and label via a trivial presentation adapter; they do not need to understand what each layer means operationally.
- Per-node aggregate queries: `countDraftsForNode(nodeId)`, `hasDraftsForNode(nodeId)`, `hasDraftsUnderPath(nodeId, pathPrefix)`.

**Write interface — restricted to ConfigEditor and orchestration workflows:**

- `set(key, value)` → creates or updates the top (draft) layer.
- `revert(key)` → removes the top layer. The next layer's value becomes the new `visibleValue`.
- `clearAllDrafts()` → bulk revert for discard.
- `clearDraftsForNode(nodeId)` → per-node clear for save.

**Layer resolution:**

```
visibleValue(key) = draft(key) ?? offlinePending(key) ?? baseline(key)

changeLayers(key) = [
  { type: 'draft',          value: ... },   // present if set() was called
  { type: 'offlinePending', value: ... },   // present if a saved offline change exists
  { type: 'baseline',       value: ... },   // always present when config is loaded
]
```

**Baseline source:** The changes module reads the tree store on demand to resolve the baseline value for a key. A lightweight address-indexed lookup avoids O(n) tree traversal on every call. The dependency is one-way and read-only: changes module reads from tree store, never writes to it.

**Offline pending source:** The changes module reads from `offlineChangesStore` on demand for Layer 2 values. `offlineChangesStore` remains the persistence adapter for saved offline change rows but is no longer directly queried by components or presenters.

**Design rules:**

- Uses `SvelteMap` for native Svelte 5 reactivity.
- Contains zero knowledge of online/offline mode, layout files, CDI structure, or Rust backend.
- Never touches the tree store — reads only.
- The tree is never mutated for display purposes. `leaf.value` is read-only from the frontend's perspective.

### Module 2: ConfigEditor (policy)

**File**: `app/src/lib/stores/configEditor.svelte.ts`

Owns dependency-aware edit application. This is the single public entry point for all user-initiated config changes. When a user edits a field, ConfigEditor:

1. Calls `changes.set(key, value)` for the user's edit.
2. Finds dependent fields (via CDI tree structure, leaf constraints, and profile-sourced cascade rules).
3. Checks whether each dependent field's current `visibleValue` is still valid under the new constraints.
4. Calls `changes.set(dependentKey, correctedValue)` for each invalidated dependent field.

All of this runs synchronously within the same call stack — no window where a component sees an invalid transient value.

**Public interface:**

```ts
configEditor.applyEdit(key: string, value: TreeConfigValue): void
```

Components call `configEditor.applyEdit()`, never `changes.set()` directly. Direct `changes.set()` is restricted to:
- ConfigEditor (for the edit itself and cascade corrections)
- Save/discard/replay orchestration workflows

**Cascade rules:**

ConfigEditor starts as a thin pass-through (calls `changes.set()` with no cascade logic). Dependency resolution is added when profile cascade rules are authored — the same extraction pipeline that already produces relevance rules. No cascade profiles need to exist for the initial implementation.

**Design rules:**

- Reads CDI tree structure and leaf constraints from the tree store.
- Reads profile cascade rules when available.
- Purely synchronous — no IPC, no async.
- Lives in `stores/` because its primary job is to coordinate writes into the changes store.

### Ownership Split

| Concern | Owner |
|---|---|
| Layered change state, `visibleValue`, layer list, counts | Changes module (state) |
| Dependency-aware edit application, cascade corrections | ConfigEditor (policy) |
| Async save/discard/reload/replay sequencing | Orchestration |
| Online IPC flush (sending drafts to Rust backend) | Orchestration (reactive watcher) |
| Rendering, dirty indicators, annotations | Components (read from changes module) |
| Value→label mapping, color/label for layer types | Presentation adapter (trivial) |

### Online IPC Strategy

ConfigEditor is synchronous. It writes only to the changes module, never to the Rust backend.

A small reactive orchestrator watches for new draft entries in the changes module and decides:
- **Online**: immediately calls `setModifiedValue` IPC to notify the Rust backend.
- **Offline**: no-op — draft stays local until save.

Write-state tracking (`dirty` → `writing` → `error`) is a status on the draft layer entry, updated by the orchestrator when the IPC call starts/succeeds/fails.

This eliminates the current divergence where online and offline edits take completely different paths through the same component.

### The Layer List — UX-Driven Design

The layer list is the read interface that components consume for annotations. The UX maps each layer type to a color and label:

```ts
type ChangeLayerType = 'draft' | 'offlinePending' | 'baseline';

const layerColor: Record<ChangeLayerType, string> = {
  draft: 'amber',
  offlinePending: 'teal',
  baseline: 'neutral',
};
const layerLabel: Record<ChangeLayerType, string> = {
  draft: 'Unsaved edit',
  offlinePending: 'Offline pending',
  baseline: 'Bus value',
};
```

Components don't need to understand what each layer means operationally — they render a typed list. Annotations are data-driven:

- "From → To" annotation: `layers[1].value → layers[0].value`
- If a field has three layers (draft over offline pending over baseline), the "from" value in the annotation is the next-lower layer — automatically correct for the daughter board scenario where a draft edit sits on top of a saved offline change.

If a new layer type is added in the future (e.g., `'templateDefault'` or `'profileOverride'`), the changes module adds an entry to the list. The UX just needs two new entries in the color/label maps. No new query methods, no new component branching.

### Canonical Key Format

**File**: `app/src/lib/utils/editKey.ts`

```ts
export function editKeyForLeaf(nodeId: string, space: number, address: number): string;
```

Returns `"${normalizeNodeId(nodeId)}:${space}:${address}"` where `nodeId` is normalized (uppercase, no dots) and `address` is the raw decimal number. This is the single source of truth for field identity.

Conversion to/from the hex offset format used by offline YAML rows (`"0x00000064"`) happens at the `offlineChangesStore` adapter boundary, not in the key builder.

Called by: ConfigEditor, changes module queries, components, presenters, and orchestration workflows.

### Shared Value Formatter

**File**: `app/src/lib/utils/formatters.ts` (extend existing file)

```ts
export function formatConfigValue(
  value: TreeConfigValue | null,
  mapEntries?: TreeMapEntry[] | null,
): string;
```

Replaces per-component `formatValue`, `formatIntValue`, and inline annotation interpolation. Called at display time only.

---

## What Gets Removed

### Symbols deleted from production code

| Symbol | File | Why it goes away |
|---|---|---|
| `leaf.modifiedValue` (frontend usage) | `nodeTree.ts` type | Replaced by changes module. Field may remain in Rust struct for backend tracking, but the frontend never reads or writes it for display or dirty logic. |
| `leaf.isOfflinePending` | `nodeTree.ts` type | No longer needed — layer types distinguish structurally. |
| `setLeafModifiedValue()` | `nodeTree.svelte.ts` | Replaced by `changes.set()` via ConfigEditor. |
| `clearAllModifiedValues()` | `nodeTree.svelte.ts` | Replaced by `changes.clearAllDrafts()`. |
| `applyOfflinePendingValues()` | `nodeTree.svelte.ts` | Eliminated entirely. Layer 2 values show through by derivation. |
| `applyPendingInChildren()` | `nodeTree.svelte.ts` | Internal helper for the eliminated stamping function. |
| `clearModifiedInChildren()` | `nodeTree.svelte.ts` | Internal helper for the eliminated clear function. |
| `parseOfflineValueString()` | `nodeTree.svelte.ts` | Replaced by shared parse/format utilities. |
| `reapplyPersistedOfflinePendingValues()` | `SaveControls.svelte` | The clear-and-restamp pattern disappears. |
| `applyOfflineChange()` | `TreeLeafRow.svelte` | Offline-specific edit handler replaced by unified `configEditor.applyEdit()`. |
| `valueToOfflineString()` | `TreeLeafRow.svelte` | Serialization moves to shared utility; called only at save time. |
| `parseOfflinePlannedValue()` | `TreeLeafRow.svelte` | Display derivation reads from changes module, not raw strings. |
| `effectiveValue()` | `nodeTree.ts` | Replaced by `changes.visibleValue(key)`. |
| `countModifiedLeaves()` | `nodeTree.ts` | Replaced by `changes.countDraftsForNode(nodeId)`. |
| `hasModifiedLeaves()` | `nodeTree.ts` | Replaced by `changes.hasDraftsForNode(nodeId)`. |
| `hasModifiedDescendant()` | `nodeTree.ts` | Replaced by `changes.hasDraftsUnderPath(nodeId, path)`. |
| `applyLeafValueChange()` inline branching | `TreeLeafRow.svelte` | Unified: `configEditor.applyEdit(key, value)`. No offline-specific branch. |

---

## What Gets Created

| File | Kind | Responsibility |
|---|---|---|
| `app/src/lib/stores/configChanges.svelte.ts` | Store | Changes module — layered change state, `visibleValue`, layer list, counts |
| `app/src/lib/stores/configEditor.svelte.ts` | Store | ConfigEditor — dependency-aware edit application, cascade corrections |
| `app/src/lib/utils/editKey.ts` | Util | Canonical key builder: `editKeyForLeaf(nodeId, space, address)` |

---

## Affected Files — Complete Inventory

### Files that change structurally (edit handler / display / dirty logic rewrite)

| File | Kind | What changes |
|---|---|---|
| `app/src/lib/types/nodeTree.ts` | Type | Remove frontend usage of `modifiedValue`, `isOfflinePending`. Delete `effectiveValue`, `countModifiedLeaves`, `hasModifiedLeaves`, `hasModifiedDescendant`. |
| `app/src/lib/stores/nodeTree.svelte.ts` | Store | Remove `setLeafModifiedValue`, `clearAllModifiedValues`, `applyOfflinePendingValues`, `clearModifiedInChildren`, `applyPendingInChildren`, `parseOfflineValueString`. |
| `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte` | Comp | Rewrite: display reads `changes.visibleValue()`, edits call `configEditor.applyEdit()`, annotations read `changes.changeLayers()`. Remove all mode-branching, offline-specific handlers, and direct store calls. |
| `app/src/lib/components/ElementCardDeck/SaveControls.svelte` | Comp | Remove `reapplyPersistedOfflinePendingValues`. Save/discard uses changes module interface. |
| `app/src/lib/components/ElementCardDeck/saveControlsPresenter.ts` | Presenter | Replace `countModifiedLeaves(tree)` with `changes.countDraftsForNode(nodeId)`. |
| `app/src/lib/components/ConfigSidebar/configSidebarPresenter.ts` | Presenter | Replace `hasModifiedLeaves` / `hasModifiedDescendant` with changes module queries. |
| `app/src/lib/components/ElementCardDeck/TreeGroupAccordion.svelte` | Comp | Replace `hasModifiedDescendant` with `changes.hasDraftsUnderPath`. |
| `app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte` | Comp | Replace `setLeafModifiedValue` with `configEditor.applyEdit`. |

### Files that change at call sites only

| File | Kind | What changes |
|---|---|---|
| `app/src/routes/+page.svelte` | Route | Remove `applyPersistedOfflinePendingToTrees`, `applyOfflinePendingValues` from tree-updated callback. Replace `hasModifiedLeaves` with changes module queries. |
| `app/src/routes/config/+page.svelte` | Route | Replace `hasModifiedLeaves` with changes module query. |
| `app/src/lib/orchestration/syncApplyOrchestrator.ts` | Orch | Remove `applyOfflinePendingValues` call after sync apply. |
| `app/src/lib/orchestration/unsavedChangesGuard.ts` | Orch | Replace `hasModifiedLeaves` with changes module query. |
| `app/src/lib/orchestration/offlineLayoutOrchestrator.ts` | Orch | Remove `modifiedValue: null` from synthetic leaf stub. |
| `app/src/lib/stores/bowties.svelte.ts` | Store | Replace `effectiveValue` with `changes.visibleValue`. |
| `app/src/lib/utils/eventIds.ts` | Util | Replace `effectiveValue` with `changes.visibleValue`. |

### Files that are NOT modified

| File | Why |
|---|---|
| `app/src/lib/api/config.ts` | Online IPC wrappers remain. The IPC flush orchestrator calls them. |
| `app/src/lib/stores/offlineChanges.svelte.ts` | Hidden behind the changes module. Internal row structure unchanged. |
| `app/src-tauri/src/node_tree.rs` | Rust backend tree structure. Frontend ignores `modified_value`. |
| `app/src-tauri/src/commands/cdi.rs` | Backend IPC commands unchanged. |

---

## Migration Strategy — Full Cutover in Scoped PRs

Previous incremental migration attempts created a ball of mud where old and new state models coexisted and had to stay consistent — which is the same coupling problem this refactor is trying to solve. This plan uses a **full cutover** strategy: no coexistence period where some callers use old APIs and some use new.

The cutover is split into three PRs for review manageability, but no intermediate state is merged where both systems are live:

### PR 1: Foundation (additive only)

Create the three new files — changes module, ConfigEditor, and editKey utility — with full test suites. No existing callers are modified. Both old and new systems exist side by side, but nothing connects to the new one. Safe to merge — it adds, doesn't modify.

**Tests written at this stage (TDD):**

- Changes module: draft only, offline pending only, draft-over-offline-pending, baseline always present, node-level counts, revert removes top layer, `clearAllDrafts` bulk revert.
- ConfigEditor: `applyEdit` calls `changes.set()`, single-field edit pass-through. (Cascade tests come later when cascade rules are authored.)
- editKey: normalized key format, consistency with offline-changes hex offset conversion.

### PR 2: Full Switchover

In one PR, switch every caller from the old path to the new path **and** delete the old symbols. This is the big PR, but it's a direct replacement — no coexistence. The old tests become the new tests, re-anchored on the new interface.

Specifically:
- All components, presenters, and routes switch reads to `changes.visibleValue(key)` and `changes.changeLayers(key)`.
- All edit entry points switch to `configEditor.applyEdit(key, value)`.
- All dirty-indicator queries switch to `changes.countDraftsForNode()` / `changes.hasDraftsForNode()` / `changes.hasDraftsUnderPath()`.
- Save/discard orchestration consumes the changes module interface.
- Delete all removed symbols from `nodeTree.ts`, `nodeTree.svelte.ts`, `TreeLeafRow.svelte`, `SaveControls.svelte`.
- Delete `applyPersistedOfflinePendingToTrees` from `+page.svelte`.
- All test files are updated and pass.

### PR 2b: Consumer Granularity Fixes

PR 2 switched all callers to the new `configChangesStore` API but introduced a systematic granularity mismatch: the old tree-mutation model gave consumers **free spatial granularity** (walk any subtree → find dirty leaves within that subtree), while the new key-based model requires consumers to reconstruct spatial awareness themselves. PR 2 used node-level queries (`hasDraftsForNode`) where segment-level or instance-level granularity was actually needed.

This PR fixes all identified mismatches and rendering bugs discovered during testing.

**Fix 1 — Daughter board controls never visible** (SegmentView wrapper group misclassification)

The Rust backend wraps replicated group instances inside a **wrapper group** (`instance: 0`, `replicationCount > 1`). SegmentView has a condition `{#if item.node.replicationCount > 1}` that routes the wrapper to a bare `TreeGroupAccordion` without passing `siblings`, preventing pill-selector mode. The wrapper should instead be treated as a non-replicated container group — its children are the instance groups that `groupReplicatedChildren` can then collect into a `replicatedSet`.

| File | Change |
|---|---|
| `app/src/lib/components/ElementCardDeck/SegmentView.svelte` | Replace the `replicationCount > 1` condition with `instance !== 0 && replicationCount > 1`. When `instance === 0` (wrapper), render as a non-replicated group section whose children are processed through `groupReplicatedChildren`, producing the `replicatedSet` that drives pill-selector mode. |

**Fix 2 — Multi-layer annotations hidden** (TreeLeafRow only shows top change)

When a draft sits on top of an offline pending value on top of a baseline, `hasPendingApplyVisible` evaluates to `false` because it includes `!isDirty`. The user only sees "Unsaved offline edit: X → Y" and loses visibility into the underlying pending-apply layer.

| File | Change |
|---|---|
| `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte` | When `isDirty && layers.length >= 2`, show the draft annotation ("Unsaved: from → to" with revert). Additionally, when a field has an `offlinePending` layer visible in `changeLayers()` regardless of whether a draft exists on top, show the pending-apply annotation ("Bus: X \| Pending: Y") below the draft annotation. Remove `!isDirty` from `hasPendingApplyVisible`. Guard against double-rendering by rendering the pending annotation only when the offlinePending layer is distinct from the draft layer. |

**Fix 3 — Sidebar segment dirty indicator on ALL segments** (node-level vs segment-level)

`getSegmentPendingState()` calls `configChangesStore.hasDraftsForNode(nodeId)` — a node-level check that returns true for every segment when any draft exists on the node.

| File | Change |
|---|---|
| `app/src/lib/components/ConfigSidebar/configSidebarPresenter.ts` | In `getSegmentPendingState()`, replace `configChangesStore.hasDraftsForNode(nodeId)` with a per-segment children walk: iterate the segment's children recursively, construct `editKeyForLeaf(nodeId, child.space, child.address)` for each leaf, and check `configChangesStore.changeLayers(key)` for a draft layer. Import `editKeyForLeaf` and add a helper `hasDraftInSegmentChildren(nodeId, children)`. |

**Fix 4 — "0 unsaved edits" count in offline mode** (wrong store used for count)

`saveControlsPresenter.ts` computes `pendingEditCount` for offline mode using `offlineDraftCount` (from `offlineChangesStore`), which counts staged offline change rows. In-memory drafts from `configChangesStore` are not counted because they haven't been staged yet. Similarly, `discardFieldCount` and the save progress total use `offlineDraftCount` where they should include `configDraftCount`.

| File | Change |
|---|---|
| `app/src/lib/components/ElementCardDeck/saveControlsPresenter.ts` | In offline mode: set `pendingEditCount = configDraftCount + offlineDraftCount + (layoutIsDirty && configDraftCount === 0 && offlineDraftCount === 0 ? 1 : 0)`. Set `discardFieldCount = configDraftCount + offlineDraftCount`. |
| `app/src/lib/components/ElementCardDeck/SaveControls.svelte` | In the offline save handler, use `configChangesStore.draftEntries().length + offlineChangesStore.draftCount` for the save progress total instead of `offlineChangesStore.draftCount` alone. |

**Fix 5 — Remove `applyPersistedOfflinePendingToTrees` no-op stub**

The function is a no-op stub that was left as a pass-through shim. It is still referenced in three places in `+page.svelte`. Remove the function definition and all call-site references.

| File | Change |
|---|---|
| `app/src/routes/+page.svelte` | Delete the `applyPersistedOfflinePendingToTrees` function definition and all three references: the one passed to `openOfflineLayoutWithReplay`, the one passed to `bootstrapStartupLifecycle`, and the direct call after sync apply. |

**Tests added or updated in this PR:**

| Test file | What it covers |
|---|---|
| `configSidebarPresenter.test.ts` | `getSegmentPendingState` returns `hasPendingEdits: true` only for the segment containing the drafted leaf, not other segments. |
| `saveControlsPresenter.test.ts` | In offline mode with 1 config draft and 0 offline drafts, `pendingEditCount` is 1, `pendingHintText` is "1 unsaved edit", `discardFieldCount` is 1. |
| `SaveControls.test.ts` | Offline save progress total includes config drafts. |
| `TreeLeafRow.test.ts` or `TreeLeafRow.offline.test.ts` | When 3 layers exist (draft + offlinePending + baseline), both the draft annotation and the pending-apply annotation are rendered. |
| `SegmentView.test.ts` or `TreeGroupAccordion.test.ts` | Wrapper group (`instance: 0`, `replicationCount > 1`) renders its children through `groupReplicatedChildren`, producing pill-selectable instances. |

### PR 3: Cleanup

- Remove any dead code not caught in PR 2.
- Remove dead rendering path: `ElementCardDeck.svelte`, `ElementCard.svelte`, `FieldRow.svelte`, `SubGroupAccordion.svelte`, and their test files. These are not imported by any route — `SegmentView` replaced this rendering path. `EventSlotRow.svelte` should also be evaluated for removal.
- Simplify `offlineChangesStore` internals now that callers no longer depend on its row model directly.
- Update product docs (`product/architecture/code-placement-and-ownership.md`, `product/architecture/lifecycle-and-state-ownership.md`) to reflect the new ownership.

---

## Locked Design Decisions

These decisions were evaluated and approved during the architecture review. They are not open for re-evaluation during implementation.

| # | Decision | Rationale |
|---|---|---|
| D1 | **Key format**: `"${normalizeNodeId(nodeId)}:${space}:${address}"` with address as raw decimal, not hex. | Matches CDI, Rust backend, and `leaf.address`. Hex conversion at offline YAML boundary only. |
| D2 | **Both modules live in `stores/`**. | ConfigEditor's primary job is coordinating writes into a store. Orchestration owns async sequencing, not deterministic policy. |
| D3 | **Baseline is always present** in the layer list when config is loaded. | Components can always read `layers[0]` and `layers[layers.length - 1]` without null checks. |
| D4 | **ConfigEditor is synchronous**. It writes only to the changes module, never to the Rust backend. | Online IPC flush is an orchestration concern (reactive watcher). This eliminates online/offline divergence in the edit path. |
| D5 | **Changes module reads the tree store on demand** for baseline values, with an address-indexed lookup for performance. | Avoids "remember to register" pattern that caused desync bugs with the old restamp approach. |
| D6 | **Full cutover migration**, not incremental. Four scoped PRs: foundation (additive), switchover (replacement + deletion), consumer granularity fixes, cleanup. | Incremental migration previously created mud. No coexistence period in main. |
| D7 | **`visibleValue` is always the top layer's value** — module ignores validity. | Cascade corrections run synchronously within the same `applyEdit()` call stack. No transient invalid state. Validity logic lives in ConfigEditor (one place), not in the module. |
| D8 | **Bowtie metadata edits are out of scope** for this refactor. | Adjacent concern, not the root cause of edit-layer regressions. Interface shaped to not preclude future inclusion. |

---

## Contract Tests — Behavioral Guarantees

These tests encode the behavioral contracts that must pass after the refactor. They are written FIRST (TDD) in PR 1 and updated in PR 2.

### Layer priority

| Test | Assertion |
|---|---|
| No edits, no layout | `visibleValue` returns `leaf.value`; layer list has one entry (baseline) |
| Persisted offline row exists, no draft | `visibleValue` returns persisted planned value; layer list has baseline + offlinePending |
| Draft exists, no persisted row | `visibleValue` returns draft value; layer list has baseline + draft |
| Draft exists AND persisted row exists | `visibleValue` returns draft value (top layer wins); layer list has all three |
| Draft cleared (revert) | `visibleValue` falls back to persisted planned value |
| Persisted row cleared | `visibleValue` falls back to `leaf.value` |

### Dirty indicators

| Test | Assertion |
|---|---|
| Draft exists | `isDirty` = true, amber; layer list top entry is `'draft'` |
| Persisted row exists, no draft | `isPending` = true, teal; layer list top entry is `'offlinePending'` |
| Draft cleared | `isDirty` = false |
| Save (online) | Drafts clear, `isDirty` = false |
| Save (offline) | Drafts clear, persisted row exists, `isPending` = true |
| Discard | Drafts clear, dirty indicators gone |

### Tree rebuild isolation

| Test | Assertion |
|---|---|
| Tree rebuilt via `node-tree-updated` | Drafts in changes module survive |
| Tree rebuilt via `node-tree-updated` | Tree store `_trees` assigned exactly once (no double mutation) |
| Tree rebuilt via `node-tree-updated` | Pill selector and segment controls remain visible |

### Annotation formatting

| Test | Assertion |
|---|---|
| Int field with map entries | Annotation shows label ("Steady") not raw int ("1") |
| Draft over persisted | "Unsaved edit: Steady → Pulse" (from = next layer down, not baseline) |
| Persisted only | "Bus: None \| Pending: Steady" with labels |
| String field | Annotation shows raw string value |
| EventId field | Annotation shows dotted hex |

### Online/offline parity

| Test | Assertion |
|---|---|
| Online edit without layout | Dirty indicator appears, same as offline |
| Offline edit with layout | Dirty indicator appears, same as online |
| Online save | Writes to bus, clears drafts |
| Offline save | Writes to layout, clears drafts, persisted annotation appears |
| Online discard | Clears drafts |
| Offline discard | Clears drafts, persisted values reappear |

### Cascade (ConfigEditor)

| Test | Assertion |
|---|---|
| Edit a field with no dependents | Only one `changes.set()` call |
| Edit a field with a dependent whose current value becomes invalid | Two `changes.set()` calls: one for the edit, one for the corrected dependent |
| Revert the parent edit | Parent draft removed; dependent draft remains (explicit secondary revert needed) |
| Edit a field when no cascade rules exist | Pass-through — behaves like single `changes.set()` |

---

## What This Plan Does NOT Do

- **Does not change the Rust backend tree structure.** `modified_value` remains on the Rust `LeafNode` for the `write_modified_values` / `discard_modified_values` IPC commands. The frontend simply stops reading it for display.
- **Does not change the online per-keystroke IPC.** `setModifiedValue` IPC is still called by the IPC flush orchestrator so the Rust backend can track pending writes.
- **Does not change `offlineChangesStore`'s internal structure.** The store's role narrows: it is consumed by the changes module for Layer 2 reads and written to by orchestration at save time only.
- **Does not introduce new IPC commands.** All existing Tauri commands remain.
- **Does not fold bowtie metadata edits** into the changes module.
- **Does not implement cascade profile rules** — ConfigEditor starts as a pass-through and gains cascade behavior when profile rules are authored.

---

## Verification Criteria

The refactor is complete when:

1. All contract tests in the section above pass.
2. `leaf.modifiedValue` and `leaf.isOfflinePending` are never read by any frontend production code.
3. `applyOfflinePendingValues`, `clearAllModifiedValues`, and `setLeafModifiedValue` are deleted from `nodeTree.svelte.ts`.
4. `TreeLeafRow.svelte` does not import `offlineChangesStore` for edit operations.
5. `TreeLeafRow.svelte` does not branch on `layoutStore.isOfflineMode` in its edit handler.
6. The `node-tree-updated` callback in `+page.svelte` does not call `applyOfflinePendingValues`.
7. All edit entry points go through `configEditor.applyEdit()`.
8. Annotations always show map-entry labels for int fields.
9. All existing frontend tests pass (with updates for the new API).
10. Product architecture docs are updated to reflect the new ownership.
