# All layout edits flow through the in-memory draft layer

## Context

Connector selections (Spec 014/S6) and channel creation (Spec 015/S3) were implemented with a **write-through** pattern: each user interaction immediately invoked a backend IPC that persisted the change to disk. This bypassed the in-memory draft layer that config edits, bowtie metadata, and offline changes use. As a result:

- Changing a daughter board showed no unsaved-change indicator (no Save/Discard buttons).
- There was no way to discard a connector selection change.
- The full save workflow didn't know about connector selections — it couldn't collect them as deltas.
- New companion files (`channels.yaml`) had to independently solve the "don't overwrite on full save" problem, and got it wrong (S2 regression).

The root cause is that the write-through pattern creates a parallel persistence path that doesn't participate in the established edit lifecycle. Each new feature that uses write-through reintroduces the same class of bugs.

## Decision

**The only code path that writes layout data to disk is the save workflow triggered by an explicit user action (Save button or menu item).** All user-initiated edits — regardless of what layout artifact they target — are held in a store's in-memory draft layer until save.

### Contract for stores with editable layout data

Every frontend store that holds user-editable layout data must implement:

| Method | Purpose |
|--------|---------|
| `isDirty` | Returns true when drafts differ from the last-saved baseline |
| `collectDeltas()` | Returns `LayoutEditDelta[]` representing all pending edits |
| `discard()` | Reverts to the baseline (last-saved state) |
| `hydrateBaseline(...)` | Sets the baseline from backend response after save/open |

The store's `isDirty` signal must feed into the aggregate dirty computation (`changeTracker` / `saveControlsPresenter`) so the Save/Discard UI reflects all pending work.

### What "layout data" includes

Any data persisted inside the layout directory: `bowties.yaml` (metadata, role classifications, node mode selections), `channels.yaml`, `offline-changes.yaml`, `manifest.yaml` node roster membership, and any future companion files.

### No exceptions for immediacy

There is no "write-through for survivability" exception. If the application crashes before the user saves, unsaved edits are lost — the same behavior as unsaved config changes today. The user's explicit Save is the only durable checkpoint.

### Backend IPC commands for layout mutation

Backend commands that mutate layout files (`save_layout_directory`, future equivalents) are called exclusively by the save orchestrator. They accept deltas, apply them to the on-disk state, and return the persisted result. Standalone mutation commands (like the former `set_node_mode_selection` IPC called on every interaction) are removed or repurposed as internal helpers not exposed as Tauri commands invoked from user-interaction handlers.

## Considered alternatives

**Keep write-through, add undo-on-discard:** The store would write to disk immediately but remember the previous state. "Discard" would write the old state back. Rejected because: (a) "discard" should not write to disk — it's conceptually "forget my edits"; (b) crash between write-through and undo leaves the file in the new state with no user confirmation; (c) every new companion file would need its own undo mechanism rather than inheriting the established pattern.

## Consequences

- `connectorSelectionsStore.saveDocument()` no longer calls `setNodeModeSelection` / `clearNodeModeSelection` IPC. It updates the in-memory draft and signals dirty.
- `channelsStore` gains a draft layer: auto-created channels are held in memory; `createChannels` IPC is removed from the orchestrator's step 4.
- The save orchestrator collects deltas from `connectorSelectionsStore` and `channelsStore` alongside `bowtieMetadataStore`.
- `set_node_mode_selection` and `clear_node_mode_selection` backend commands are removed as standalone Tauri IPC endpoints. The delta variants remain in `LayoutEditDelta` and are applied during `save_layout_directory`.
- Channels persistence moves to the save workflow: `save_layout_directory` reads `previous.channels`, applies channel deltas (create/rename/delete), and writes the result.
- The `channels.yaml` write-through in `create_channels` IPC is removed.
- `effectiveNodeStore.isDirty` (or `changeTracker`) gains inputs for connector selection and channel dirty state.

## Supersedes

This decision supersedes the Spec 014/S6 "immediate write-through for connector selections" pattern. ADR-0002's delta model is upheld and strengthened — this ADR closes the gap where some edits bypassed it.

## 2026-06-25 extension: No-op edit suppression

### Rule

Stores MUST suppress draft entries where the proposed value equals the
current effective value (baseline, or baseline + already-pending draft).
`isDirty = true` means a real semantic difference exists between the
in-memory state and the last-saved state — not merely that an edit gesture
occurred.

### Where the guard lives

The guard MUST live in the **store's mutation method** (the single entry
point), not only in the calling component. Components SHOULD also avoid
emitting no-change callbacks as a belt-and-braces measure, but the store is
the authoritative gatekeeper because multiple callers (components, tests,
orchestrators) may invoke the same mutation.

### Pattern

```ts
// Store mutation — suppress no-op
renameChannel(id: string, newName: string): boolean {
  const trimmed = newName.trim();
  if (trimmed.length === 0) return false;
  const current = this._resolveEffectiveName(id);
  if (current === trimmed) return false; // no-op — no draft recorded
  this._pendingRenames = new Map(this._pendingRenames).set(id, trimmed);
  return true;
}
```

### Applies to

Every store that implements the draft-layer contract (channelsStore,
connectorSelectionsStore, configChangesStore, offlineChangesStore,
bowtieMetadataStore, and any future layout-scoped editable store).

### Trigger

Spec 015 S5: pressing Escape after entering channel rename mode (without
changing the value) recorded a spurious rename draft, causing `isDirty` to
report unsaved changes when none existed.

## 2026-06-27 extension: facilities.yaml joins the draft-layer family

### Context

Spec 018 (Block Indicator Facility) adds **Facilities** as a new
first-class persisted entity in the layout folder, with their own file
`facilities.yaml` sibling to `bowties.yaml`, `channels.yaml`,
`offline-changes.yaml`, and `manifest.yaml`. Facility CRUD and slot
bind/unbind are user-initiated edits that must participate in the same
edit lifecycle as channels and bowtie metadata, or every interaction would
recreate the write-through bugs ADR-0012 was written to prevent.

### Decision

`facilitiesStore` (frontend, `app/src/lib/stores/facilities.svelte.ts`)
implements the same four-method draft-layer contract: `isDirty`,
`collectDeltas()`, `discard()`, `hydrateBaseline(facilities)`. The save
orchestrator collects facility deltas alongside channels / connectors /
bowtie-metadata and passes them to `save_layout_directory`, which applies
them to `facilities.yaml` through the journaled writer (ADR-0006). No
backend `bind_slot` / `add_facility` IPC writes to disk from a user
interaction handler; only `collectDeltas()` on save does.

The companion read-only `behaviorTemplatesStore` is **not** an editable
store — behavior templates are hardcoded in `bowties-core` for this slice
— so it does not implement the draft contract. It is hydrated once on app
start from a `list_behavior_templates` IPC and treated as a registry
mirror.

### Pattern

Mirrors the channelsStore / connectorSelectionsStore convention:

- `addFacility(templateId, name): facilityId` — records an `AddFacility`
  delta entry; returns the new UUID v4 facility ID.
- `bindSlot(facilityId, slotLabel, channelId): boolean` — records a
  `BindSlot` delta; returns false on no-op (already bound to that
  channel).
- `unbindSlot(facilityId, slotLabel): boolean` — records an `UnbindSlot`
  delta; returns false on no-op (already empty).
- `renameFacility`, `deleteFacility` — analogous.

The no-op-suppression rule from the 2026-06-25 extension applies: an
attempt to bind a slot to the channel it is already bound to records no
draft and leaves `isDirty` unchanged.

### Lifecycle reset enumeration

Both `facilitiesStore` and `behaviorTemplatesStore` are layout-scoped and
MUST be enumerated in `layoutLifecycleOrchestrator.resetForNewLayout()`
per ADR-0011, with assertions in the orchestrator's test that both clear.
The enumeration is added in the same slice that introduces each store,
not deferred — that is the failure mode ADR-0011 was written to prevent.

### Consequences

- Facility CRUD shows the unsaved-change indicator and gains a working
  Discard path automatically by reusing the existing save/discard UI.
- Atomic slot operations (e.g., *Add channel* on the lamp output slot,
  which creates a user-owned channel + claims a Direct Lamp Control row +
  binds the slot) collect as multiple deltas in a single save and apply
  in order; partial failure during apply rolls back the file via the
  journal.
- The Wired ↔ Incomplete transition (creating or freeing underlying
  bowtie(s) via the existing bowtie creation mechanism + slot-detach
  pipeline) is orchestrator-driven on the *effective* state, not on
  persisted state — bowtie creation is a save-time action because it
  depends on the saved facility shape, not on intermediate drafts.

## Invariants

Structured testable rules for the `/design` audit. Each invariant resolves to OK / Drift / Unknown with file:line evidence.

- The only code path that writes layout data to disk is the save workflow triggered by an explicit user action (Save button / menu). No store implements write-through to a backend IPC from a user-interaction handler. Audit: grep for backend mutation commands invoked outside the save orchestrator.
- Every store with editable layout data implements the draft-layer contract: `isDirty`, `collectDeltas(): LayoutEditDelta[]`, `discard()`, `hydrateBaseline(...)`. Audit: list editable-layout stores and confirm all four methods exist.
- Every editable-layout store's `isDirty` signal feeds the aggregate via `effectiveNodeStore.dirtyBreakdown` (this joins ADR-0011's first invariant). A store with a draft layer that does not appear in `dirtyBreakdown` is drift.
- Backend layout-mutation commands (`save_layout_directory`, future equivalents) are invoked exclusively from the save orchestrator. Standalone Tauri commands that mutate layout files from user-interaction handlers (`set_node_mode_selection`, `clear_node_mode_selection`, `create_channels`, etc.) are not exposed as IPC endpoints called from interaction code.
- Stores suppress no-op edits at the mutation entry point (2026-06-25 extension). `isDirty = true` means a real semantic difference between in-memory state and the last-saved baseline — not merely that an edit gesture occurred. Audit: for each draft-layer store, confirm its mutation methods compare against the current effective value before recording a draft.

When extending this ADR, add or amend invariants in this section rather than scattering them across the file.

## 2026-07-01 extension: cascade side effects appear in the draft layer

Automated cascades that flow from an in-app trigger (such as a hardware-owned channel disappearing from `channelsStore.channels` because the user cleared its BOD daughter-board) MUST stage their side effects in the appropriate draft stores (`facilitiesStore`, `bowtieMetadataStore`, `configChangesStore`) so the user sees them **NEXT TO** their trigger — visible immediately in the toolbar's unsaved-change count, atomic on Save, revertable on Discard.

**Rationale.** The alternative — an on-save "fixup" that only appears at commit time — creates a surprising user experience: Discard restores the trigger (the daughter-board comes back) but the cascade's side effects (facility un-Wiring, bowtie teardown) already collapsed before the user got a chance to see them, so Discard looks like it did nothing to the cascade. The draft-layer approach makes the cascade a first-class part of the transaction: the same Discard restores both.

**Concrete rules:**

- Frontend cascade orchestrators (e.g., `facilityCascadeOrchestrator.svelte.ts`, introduced by Spec 018 / S6) subscribe to the trigger source (a store's reactive state), diff transitions in a private `.root`, and dispatch draft-layer mutations through the same store methods and orchestrator entry points user actions use. They do NOT reach into the backend save flow, do NOT persist a parallel "pending cascade" bucket, and do NOT introduce a new dirty-breakdown category — the cascade's effects surface through the existing `dirtyBreakdown` buckets of the stores it mutates (which joins ADR-0011).
- On-save "fixup" cascades that only appear at commit time are explicitly ruled out. If a save-flow reconciliation step is genuinely required (e.g., a backend integrity guard), it MUST NOT change user-observable state at commit time; it MUST reject the save with a clear error so the user can adjust their drafts.
- Cascade orchestrators are lifecycle-scoped: mount them in the layout-open path (adjacent to store hydration) and tear them down via `layoutLifecycleOrchestrator.resetForNewLayout()`. A cascade subscription that outlives its layout is drift.
- When adding a cascade for a new trigger source (spec 019+ hardware seams), extend `facilityCascadeOrchestrator` (or introduce a sibling orchestrator that follows the same diff-based subscription pattern). Never create ad-hoc Svelte effects inside components or route files that mutate the same draft stores — that path breaks the single-orchestrator contract.

## 2026-07-01 extension: cascade side effects appear in the draft layer

Automated cascades that flow from an in-app trigger (such as a hardware-owned channel disappearing from `channelsStore.channels` because the user cleared its BOD daughter-board) MUST stage their side effects in the appropriate draft stores (`facilitiesStore`, `bowtieMetadataStore`, `configChangesStore`) so the user sees them **NEXT TO** their trigger — visible immediately in the toolbar's unsaved-change count, atomic on Save, revertable on Discard.

**Rationale.** The alternative — an on-save "fixup" that only appears at commit time — creates a surprising user experience: Discard restores the trigger (the daughter-board comes back) but the cascade's side effects (facility un-Wiring, bowtie teardown) already collapsed before the user got a chance to see them, so Discard looks like it did nothing to the cascade. The draft-layer approach makes the cascade a first-class part of the transaction: the same Discard restores both.

**Concrete rules:**

- Frontend cascade orchestrators (e.g., `facilityCascadeOrchestrator.svelte.ts`, introduced by Spec 018 / S6) subscribe to the trigger source (a store's reactive state), diff transitions in a private `.root`, and dispatch draft-layer mutations through the same store methods and orchestrator entry points user actions use. They do NOT reach into the backend save flow, do NOT persist a parallel "pending cascade" bucket, and do NOT introduce a new dirty-breakdown category — the cascade's effects surface through the existing `dirtyBreakdown` buckets of the stores it mutates (which joins ADR-0011).
- On-save "fixup" cascades that only appear at commit time are explicitly ruled out. If a save-flow reconciliation step is genuinely required (e.g., a backend integrity guard), it MUST NOT change user-observable state at commit time; it MUST reject the save with a clear error so the user can adjust their drafts.
- Cascade orchestrators are lifecycle-scoped: mount them in the layout-open path (adjacent to store hydration) and tear them down via `layoutLifecycleOrchestrator.resetForNewLayout()`. A cascade subscription that outlives its layout is drift.
- When adding a cascade for a new trigger source (spec 019+ hardware seams), extend `facilityCascadeOrchestrator` (or introduce a sibling orchestrator that follows the same diff-based subscription pattern). Never create ad-hoc Svelte effects inside components or route files that mutate the same draft stores — that path breaks the single-orchestrator contract.


## 2026-07-03 extension: load-time schema repairs stage as drafts

When layout open detects a schema inconsistency that a backend read path already normalises in memory (e.g., `read_layout_capture` running `normalize_facility_channel_refs` against `channels.yaml` and finding a facility slot binding whose channel id is absent from the channel inventory), the frontend MUST stage the equivalent fix-up as a normal draft edit — the same seam any user-triggered edit would use — rather than treating the toast as sufficient.

**Rationale.** The pre-018 `list_facilities` IPC hydrates the frontend baseline from disk without normalising, so the frontend view can carry orphan bindings even after the backend has repaired its own effective view. Surfacing a toast without a matching draft leaves the user with three broken affordances: the ghost id still counts against the slot cap (Add fails), the effective facility still looks Wired against a phantom consumer (Delete fails when it routes through the backend composer), and no dirty flag is set so the toast copy "Save to persist the cleanup" is a lie. Staging the equivalent `detachChannelFromSlot` draft (via `facilityCascadeOrchestrator.reconcileDanglingChannelRefsOnLoad()`) restores every downstream affordance because the effective view drops the ghost, the dirty flag flips through the normal `_pendingSlotBindings` bucket, and Save emits a real delta that persists the cleanup.

**Concrete rules:**

- Any load-time schema-normalization surface (backend `load_warnings`, frontend `surfaceLoadWarnings`, or an equivalent silent auto-repair) MUST have a companion frontend pass that stages the matching draft edit in the affected draft store(s). A toast that says "cleaned up N references" without a corresponding draft is drift.
- Load-time repair passes MUST run through the same orchestrator that owns the runtime cascade for the same seam (e.g., dangling channel refs → `facilityCascadeOrchestrator`) so the detach + teardown side-effect logic is shared. Duplicating that logic in a separate load-time module is drift.
- The load-time repair pass runs AFTER both source stores have hydrated their baselines. In `+page.svelte`'s layout-open path, `Promise.all([channelsStore.loadChannels(), facilitiesStore.loadFacilities()])` sequences the two loads before the repair call.
- Backend reads that populate the frontend baseline are ALLOWED to skip normalisation (the load-time repair covers them), but backend reads that feed other backend consumers (composer, catalog rebuild, sync) MUST go through the normalised view. `LayoutState.saved` / `effective_facilities()` remain referentially clean per ADR-0002's 2026-07-03 extension.
- If the eventual fix routes `list_facilities` / `list_channels` through `LayoutState.effective_*()`, the load-time repair pass stays as defence-in-depth — but its scope shrinks to the corner where an on-disk file survives with orphan refs and a session opens it before the effective view is populated.


## 2026-07-03 extension: teardown reversal is a shared primitive with a fallback strategy

The inverse of composition MUST be reversible from every state a facility can reach, not just the still-Wired state that the initial T13 design assumed. Composition writes to two places (`configEditor.applyEdit` on consumer `EventID` leaves + `bowtieMetadataStore.createBowtie` with a `createdByFacility` back-reference). Teardown MUST reverse both writes regardless of the facility's current `facilityStatus`, otherwise the CDI-scan bowtie catalog re-produces the composed bowtie on the next layout open and the user sees the Bowties view change between saving and reopening the same layout.

**Rationale.** Historically `tearDownFacilityBowties` had two branches: a Wired branch that re-invoked the backend composer to know which leaves to reset, and a non-Wired branch that only deleted metadata rows. The Wired branch worked for `removeFromSlot` (user-initiated, teardown-before-detach). The non-Wired branch fired for the runtime hardware-channel cascade in `_cascadeDetach` (detach-before-teardown ordering) and for the 2026-07-03 load-time repair of ghost bindings (`reconcileDanglingChannelRefsOnLoad`), where the facility is Incomplete when teardown runs — so orphan consumer leaves survived every save, and the auto-catalog re-attached them as unowned bowtie cards on the next open. Callers had no way to know teardown was incomplete because the API was uniform.

**Concrete rules:**

- Every callsite that un-Wires a facility MUST invoke `tearDownFacilityBowties(facilityId)`. Direct calls into `bowtieMetadataStore.deleteBowtie` or `configEditor.applyEdit` from cascade or repair code are drift — teardown owns the pairing.
- `tearDownFacilityBowties` MUST delegate leaf reversal to a shared `resetComposedLeavesForFacility(facilityId)` primitive that owns the two lookup strategies:
  1. **Composer-forward** when the facility is still Wired. Fast, precise, matches the previous Wired-branch behaviour. Preferred when the structure is intact.
  2. **Metadata-driven fallback** when the facility is Incomplete. Reads `bowtieMetadataStore.bowtiesForFacility(facilityId)` for the composed event id hexes, iterates `nodeTreeStore.trees` via `collectEventIdLeaves`, and stages a `configEditor.applyEdit` with a fresh event id on every leaf whose effective value matches. Slower but works when the ghost binding or missing channel has made the composer unavailable.
- The fallback MUST NOT be replaced by "skip the leaf reset because the composer can't run" — that's the pre-consolidation drift the extension exists to prevent. The metadata rows are the durable back-reference the fallback consults; deleting them BEFORE resetting leaves would erase the lookup key.
- Metadata rows are deleted AFTER `resetComposedLeavesForFacility` returns, in the same synchronous transaction, so a partial teardown never persists.
- New cascade sources (spec 019+ hardware seams) MUST route through `tearDownFacilityBowties`. New behaviour templates that need composition MUST return their state-mapping through `BehaviorTemplate.mapping` so both the composer-forward and metadata-fallback paths cover them.


## 2026-07-03 extension: connected-mode draft-to-backend mirror

Every user- or workflow-initiated config-value edit lands in
`configChangesStore` through the same single entry point (`ConfigEditor.applyEdit`).
`ConfigEditor.applyEdit` stays synchronous and IPC-free; a reactive
orchestrator (`configDraftMirrorOrchestrator`) observes the draft snapshot
and forwards each new/changed draft to the backend
`NodeProxy.modified_value` map via `setModifiedValue`. No draft producer
is responsible for remembering to flush.

**Rationale.** Before the mirror existed, `flushDraftToBackend` was called
at only two callsites (leaf-row edit commit and one bowtie-catalog panel
path). Every other draft producer — facility composition
(`composeBowtiesIfWired`), teardown resets (`resetComposedLeavesForFacility`),
load-time repair (`reconcileDanglingChannelRefsOnLoad`), cascade side
effects — staged config drafts that the connected save flow
(`save_layout_with_bus_writes` Phase 2 `write_modified_values`) never
observed. A user Save appeared to succeed, but the bus saw nothing, the
Phase 4 catalog rebuild found empty consumer leaves, and the composed
bowties silently disappeared from the panel. Post-save
`configChangesStore.clearAllDrafts()` erased the only remaining local
trace of the lost edits. The pattern was fragile: the aspirational
"handled by a separate reactive orchestrator" note in
`configEditor.svelte.ts` documented an ownership that had never actually
been implemented, and every new draft producer inherited the bug by
following the same "just call `applyEdit`" convention.

**Concrete rules:**

- `ConfigEditor.applyEdit` MUST remain synchronous and MUST NOT invoke IPC.
  Its contract is "write to `configChangesStore` and return"; any deviation
  breaks the single-owner property that lets the mirror observe every draft
  producer through one reactive dependency.
- `configDraftMirrorOrchestrator` is the SOLE owner of the config-draft →
  backend IPC path. Callers do not invoke `setModifiedValue` directly, and
  `flushDraftToBackend` is retired as a public API (Commit 2 of the
  2026-07-03 activation deletes the export and the two remaining callsites).
- The mirror is offline-mode-quiet: when `layoutStore.isConnected === false`,
  it advances its last-seen snapshot but emits no IPC. Offline persistence
  is owned by `stageDraftsForOfflineSave` in `configDraftOrchestrator`, which
  runs at offline-save time only.
- Placeholder NodeKeys (`placeholder:<uuid>`) are skipped — they have no
  bus identity. Their edits persist through `stageDraftsForOfflineSave` at
  save time. This matches the pre-mirror `flushDraftToBackend` guard.
- Connection state is read inside the mirror body, NOT as a reactive
  dependency of the outer effect. The mirror MUST NOT re-emit every pending
  draft when the user connects or disconnects. A future "flush pending
  drafts on connect" feature is a separate, deliberate follow-up — not an
  emergent side effect of the mirror's dependency graph.
- The mirror MUST be mounted per layout-open and torn down on layout close
  so its last-seen map does not bleed across layouts. Mount lives in
  `+page.svelte` alongside `facilityCascadeOrchestrator.startCascade()`;
  teardown lives in `layoutLifecycleOrchestrator.resetForNewLayout()`.
- Drafts pruned by `pruneResolvedDraftsForNode` (e.g. after a successful
  bus write and tree refresh) appear as "removed" in the mirror's diff. The
  mirror emits NO IPC for removals — the backend has already accepted the
  write; sending a redundant `setModifiedValue` would be drift.

**Ownership of the "save didn't write" regression.** Any new feature that
produces a config-value draft MUST go through `configEditor.applyEdit` and
do nothing else on the IPC side. If a slice adds a second IPC boundary for
config drafts (e.g. a batched `setModifiedValues` in a future extension),
it MUST replace the mirror's emission, not run in parallel — the
single-owner property is what makes the "someone forgot to flush" bug
class impossible.
