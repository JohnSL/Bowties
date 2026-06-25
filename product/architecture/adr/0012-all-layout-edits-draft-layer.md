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
