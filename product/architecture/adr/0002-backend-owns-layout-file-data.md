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

## 2026-07-03 extension — every layout edit travels as a LayoutEditDelta

The original ADR required deltas for bowtie metadata + role classifications + connector selections. Spec 015 (channels), Spec 018 (facilities), and Spec 018 / S5 (user-owned channels) added new persistent state that landed as parallel post-save IPCs (`create_channels` / `rename_channel` / `delete_channels`, `list_facilities` re-hydrate). The Wired Block Indicator save-then-reopen bug (facility slot binding persisted, channel did not) exposed the atomicity gap: any post-save mutation IPC can partial-fail with no rollback, and a route-level omission (the copy-paste that added `facilities` deltas but not `channels`) silently drops edits the store already dirty-marked.

**Extension:** every layout edit — bowtie, connector, facility, channel — travels as a `LayoutEditDelta` through `save_layout_directory`. Post-save IPC calls to mutate persistent layout state are prohibited. The route calls exactly one save entry point per save.

**Enforcement seams:**

- **`app/src/lib/layout/collectSaveDeltas.ts`** — the sole aggregator over every edit-bearing store. Adding a new edit-bearing store means adding a call here (symmetric to `effectiveNodeStore.dirtyBreakdown`). Missing enrolment produces a UI-dirty / save-no-op signal at test time via the aggregator's fixtures instead of a silent runtime drop.
- **`LayoutEditDelta` variants** — `CreateChannel { channel }`, `RenameChannel { channel_id, new_name }`, `DeleteChannel { channel_id }` join the existing bowtie / facility / connector families. `apply_channel_deltas` sits beside `apply_facility_deltas` inside `save_layout_directory`, applied against the just-read baseline before the file is written.
- **Removed IPCs** — `create_channels`, `rename_channel`, `delete_channels` (backend commands + TS wrappers). `list_channels` remains as the read-side hydration path.
- **Rehydration** — after save, the route re-reads authoritative state via the existing `loadChannels` / `loadFacilities` IPCs. The pending edit buckets clear as a side effect of hydration, not as separate flushes.

**Cross-references:** closes the "Channel/facility persistence atomicity" backlog entry. Reinforces ADR-0011 (dirtyBreakdown facade) — the dirty aggregation and the delta aggregation are now dual invariants over the same set of edit-bearing stores. Consistent with ADR-0012 (all layout edits flow through the draft layer until save) and ADR-0015 (`LayoutState` owns the effective view including draft channels).

## 2026-07-03 extension — read returns a referentially-consistent schema

The atomic-save fold above prevents *future* dangling references between `facilities.yaml` and `channels.yaml`. Layouts saved before the fold can still carry them (a Wired Block Indicator whose user-owned lamp channel was silently dropped on save leaves a slot binding pointing at a nonexistent channel id). Three separate Consumers of that data — the facility bowtie composer, the attach-cardinality guard, and the "Used by" render path — each treated the dangling id differently: hard error, false "at max, refused", silent filter. Same seam, three semantics. That is the seam-symmetry failure mode called out in the `architecture-first-fix` skill.

**Extension:** the layout module's read API is responsible for returning a referentially-consistent view. Repairs happen at read time, in memory; disk is untouched until the next save; the repair set surfaces to the caller.

**Enforcement seams:**

- **`bowties_core::layout::facilities::normalize_facility_channel_refs(facilities, channels)`** — pure function. Removes every slot-binding channel id absent from `channels.channels` and returns one human-readable warning per removal. Called from `read_layout_capture` immediately after both documents are parsed.
- **`LayoutDirectoryReadData.load_warnings: Vec<String>`** — new field on the read result carrying the warnings. Propagates through `OpenLayoutResult.load_warnings` (Tauri) → `OpenLayoutResult.loadWarnings` (TS) → the route's `surfaceLoadWarnings` helper, which logs every line and pushes a single summary toast. Silent auto-heal is prohibited.
- **Write-back on save.** The cleaned facility documents reach disk on the next save through the existing delta path — no separate write-back at read time. This preserves the "read is side-effect free with respect to the layout directory" invariant, and lets the user see the repair result (empty slots) before choosing whether to save.
- **Downstream simplification.** With referential integrity enforced at the Owner, downstream Consumers (`compose_bowtie_ops`, cardinality guards, `effectiveLayoutStore.channelUsageMap`) may keep their existing tolerance code as belt-and-braces but MUST NOT depend on it being present — the invariant now holds at the read boundary.

**Cross-references:** ADR-0015 §"2026-07-03 extension" — `LayoutState.saved` is now guaranteed referentially clean, so `effective_facilities()` / `effective_channels()` and every backend read through them (compose_facility_bowties, catalog rebuild, sync) see a valid schema. Related to the `architecture-first-fix` skill's seam-symmetry rule.
