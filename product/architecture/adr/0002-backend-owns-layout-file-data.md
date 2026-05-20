# Backend owns layout file data

## Context

The layout file (`bowties.yaml`) stores bowtie metadata, role classifications, and connector selections. Currently both the frontend and backend independently maintain copies of this data, and the save flow passes the frontend's copy to the backend, which wholesale-replaces the on-disk data with whatever the frontend sent.

This creates a class of bugs where the frontend's copy is stale or incomplete — most critically, empty `roleClassifications` overwriting the correct values the backend previously saved. Five debugging sessions have failed to stabilize the save-then-reopen cycle because each patch fixes one state owner but introduces a regression in another.

## Decision

**The backend is the sole owner of layout file data.** The frontend does not send a `LayoutFile` object to the backend on save. Instead:

1. **Save commands accept edit deltas, not full layout objects.** The frontend sends structured edit operations (create bowtie, classify role, rename, add tag, delete) and the backend applies them to its own authoritative copy, which starts from the on-disk file.

2. **Save commands return the saved layout.** After writing to disk, the backend returns the persisted `LayoutFile` to the frontend. The frontend hydrates its store from this response — never from a locally-constructed object.

3. **`merge_saved_layout_metadata` is replaced with an append-only delta application.** The backend reads the current file from disk, applies the provided deltas, overlays catalog-derived roles (when a live catalog exists), and writes the result. No wholesale field replacement.

4. **The frontend `layoutStore._layout` becomes a read cache.** It is populated only from backend responses (open, save) and never directly mutated by metadata stores. `_applyToLayout()` is removed; metadata edits live exclusively in `bowtieMetadataStore._edits` until save.

5. **The "effective layout" for display is computed, not stored.** When the frontend needs merged data for UI rendering (preview cards, dirty indicators), it derives it from `_layout` + `_edits` on read, rather than eagerly writing edits into the layout store.

## Consequences

- The `layout` parameter on `save_layout_directory` and `save_layout_with_bus_writes` changes from `Option<LayoutFile>` to a delta structure (list of bowtie metadata edits + role classification edits + connector selection updates).
- `bowtieMetadataStore._applyToLayout()` and `_mirrorOfflineMetadataDelta()` are removed. The dual-write pattern that kept `layoutStore._layout` and `offlineChangesStore` in sync is no longer needed because the layout store is read-only between open/save.
- `editableBowtiePreviewStore` already merges catalog + metadata + config drafts reactively — this becomes the canonical pattern for all "effective layout" consumers.
- `getInstanceDisplayName()` and `buildElementLabel()` must be updated to resolve display names through the effective-value path (draft → offline pending → baseline) rather than reading only `child.value.value`, so user-configured descriptions show in the picker and bowtie cards while offline.
- The offline changes store continues to track pending config and metadata deltas for persistence to `offline-changes.yaml`, but it no longer needs to mirror layout store mutations — the backend reads `offline-changes.yaml` directly during save.

## Considered options

- **Keep frontend-as-authority, fix the merge** — make `merge_saved_layout_metadata` do field-level union instead of wholesale replace. Rejected: this treats the symptom (bad merge) but not the cause (two independent copies drifting). Every new field on `LayoutFile` would need merge logic, and the frontend copy would still be stale after any backend-computed operation (catalog roles, profile annotations).
- **Send `None` for offline saves, layout for online saves** — the offline path would preserve disk data by not sending a frontend layout, while the online path would continue sending one. Rejected: creates two save semantics that must stay aligned, and doesn't fix the underlying drift between frontend and backend state.
- **Backend owns layout, frontend sends full layout but backend ignores roleClassifications** — selective trust. Rejected: ad-hoc field-by-field rules are fragile and hard to reason about as the schema evolves.
