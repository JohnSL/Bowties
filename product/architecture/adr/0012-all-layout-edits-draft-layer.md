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
