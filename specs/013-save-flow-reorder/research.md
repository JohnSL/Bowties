# Research: Layout-First Model

## R1: Save Flow Reorder — Three-Phase Approach

**Decision**: Save layout first → write to bus → reconcile layout.

**Rationale**: ADR 0001 established this ordering. The current flow writes to bus first, triggering `node-tree-updated` events that cause `pruneResolvedDraftsForNode()` to clear drafts and flip the bowtie preview to a stale catalog. The three-phase approach eliminates this category of bugs entirely:

1. **Phase 1 — Save layout**: Stage pending drafts as offline changes. Write layout file (base + companion dir) atomically. Rebuild bowtie catalog from saved state. Bowties always display correctly.
2. **Phase 2 — Write to bus**: Call `write_modified_values` for each pending change. The existing command handles per-leaf write, error tracking, and `node-tree-updated` events. Draft pruning is harmless because the catalog was already rebuilt from the saved layout.
3. **Phase 3 — Reconcile**: Save the layout again to clear successfully written offline changes. Report any failures.

**Alternatives considered**:
- Save-in-progress flag (rejected: symptom patch, doesn't fix cancel)
- Rebuild catalog after writes (rejected: timing race during async rebuild)
- Suppress draft pruning during save (rejected: fragile around error paths)

## R2: Known-Layout Registry

**Decision**: Store known layouts in `$APPDATA/bowties/known-layouts.json` as a JSON array.

**Rationale**: The existing `recent-layout.json` stores exactly one layout path. The known-layout registry extends this to track multiple layouts with metadata. Stored in app preferences (not in the layout itself) per the spec.

**Structure**:
```json
[
  {
    "name": "Club Layout",
    "path": "/path/to/club.layout",
    "lastOpened": "2026-05-17T12:00:00Z"
  }
]
```

**Alternatives considered**:
- SQLite database (rejected: YAGNI, JSON file is sufficient for <100 entries)
- Store in each layout file (rejected: chicken-and-egg — need to find layouts before opening them)
- Reuse recent-layout.json with a list (rejected: cleaner to have a separate file for the registry)

**Migration**: `recent-layout.json` continues to work for "reopen last used" behavior. The known-layouts registry is additive. On first launch with a recent layout, that layout is auto-added to the known list.

## R3: Connection Definitions in Layout

**Decision**: Add a `connections` section to the layout manifest (`.layout` base file).

**Rationale**: Connections are properties of the layout, not global app settings. The existing `ConnectionConfig` struct already defines the right fields (id, name, adapter_type, host, port, serial_port, baud_rate). Moving connection definitions into the layout base file makes them travel with the layout. Global `connections.json` remains for backward compatibility and as a source for importing connections into a layout.

**Schema change**: Layout manifest schema version bumps from 3 to 4. A migration function reads v3 manifests and adds an empty `connections: []` section.

**Alternatives considered**:
- Store connections in companion directory (rejected: connections are layout-level metadata, not per-node state)
- Store only connection names, look up details from global prefs (rejected: breaks portability — layout should be self-contained)
- Use a separate `.connections.yaml` file (rejected: unnecessary indirection)

## R4: Layout Manifest Schema Migration

**Decision**: Implement a version-tolerant load path with forward migration transforms.

**Rationale**: Currently both `LayoutManifest::validate()` and `LayoutFile::validate()` reject any schema version that doesn't match the expected constant exactly. This means adding connection definitions (v3→v4) would break opening existing layouts. A migration path is essential.

**Approach**:
- Load raw YAML without validation
- Check `schema_version` and apply transforms sequentially (v3→v4, future v4→v5, etc.)
- Validate after migration
- Save with current schema version on next write
- Maintain a minimum supported version (v3) — layouts older than v3 are rejected

**Alternatives considered**:
- In-place file upgrade on open (rejected: destructive before user consents)
- Separate migration tool (rejected: YAGNI, inline migration is simpler)

## R5: Layout Picker UI Pattern

**Decision**: Layout picker is a full-screen overlay shown before the main app UI, not a dialog.

**Rationale**: The spec requires "no other functionality is available until a layout is active." A full-screen overlay (or replacement page) enforces this naturally. The picker shows known layouts (name, location, last-opened date) plus "New Layout" and "Browse…" actions. 

**Implementation approach**:
- Frontend: `LayoutPicker.svelte` component rendered conditionally when no layout is active
- Route `+page.svelte` checks `layoutStore.activeContext` — if null, render picker instead of tabs
- No new route needed — the picker is a screen-level gate within the existing page
- The picker abstracts the `.layout` + `.layout.d/` storage from the user (shows names, not file paths)

**Alternatives considered**:
- Separate `/picker` route (rejected: adds routing complexity, the picker is a gate not a destination)
- System file dialog only (rejected: doesn't show known layouts or metadata)
- Startup wizard with multiple steps (rejected: over-engineered for picking a layout)

## R6: Online Save Orchestration

**Decision**: The save orchestrator manages the three-phase flow. The UI blocks interaction via a modal progress dialog.

**Rationale**: The save flow spans frontend (stage drafts, show progress) and backend (persist layout, write to bus). The orchestrator owns the sequencing. The modal progress dialog (FR-016a) prevents concurrent saves.

**Flow**:
1. User clicks Save → `saveLayoutOrchestrator` starts
2. Show modal progress dialog ("Saving layout…")
3. If offline mode has pending drafts, flush them as offline changes
4. Save layout (base + companion dir) — backend `save_layout_directory`
5. Rebuild bowtie catalog — backend `build_bowtie_catalog_command`
6. Update catalog store
7. If online with pending config changes:
   a. Update progress ("Writing configuration… 1 of N")
   b. Call `write_modified_values` — backend handles per-leaf writes
   c. Update progress with results
   d. If any writes succeeded: re-save layout to clear applied offline changes
   e. If any writes failed: report failures, keep failed changes as offline changes
8. Update progress ("Complete" or "N changes failed")
9. Mark layout clean, close progress dialog

**Cancel handling**: If the user cancels the Save dialog (for Save As or first-time save), nothing is staged, nothing is written. The orchestrator exits cleanly.

## R7: Offline Change Replay for Online Writes

**Decision**: Reuse existing offline changes infrastructure to stage pending changes before bus writes.

**Rationale**: The spec calls for staging drafts as offline changes before writing to the bus. The existing `offlineChanges.svelte.ts` store and `set_offline_change` backend command already implement this pattern. The three-phase save just uses it in a new order:

1. `stageDraftsForOfflineSave()` (existing) — converts active drafts to offline changes
2. `save_layout_directory` (existing) — persists offline changes to disk
3. `write_modified_values` (existing) — writes to bus, but now the layout has already been saved
4. Clear successfully written offline changes from the saved layout

**No new persistence mechanisms needed.** The offline changes file in the companion directory (`offline-changes.yaml`) already handles serialization, status tracking, and error recording.

## R8: Event Role Persistence

**Decision**: Persist all resolved (non-ambiguous) event role classifications during save.

**Rationale**: Currently `role_classifications` in the layout file stores only user overrides for ambiguous roles. The spec extends this to store all resolved roles — roles determined via protocol exchange (producer-identified events, consumer-identified events) during online operation. This data is already available in the bowtie catalog and in the backend's `OfflineBowtieData`.

**Implementation**: During `save_layout_directory`, merge protocol-resolved roles from the live catalog into `role_classifications`. The existing `merge_layout_metadata` function already applies saved role classifications on layout reopen — no changes needed on the load path.

## R9: Node Visibility When Connected

**Decision**: Show all layout nodes while online. Mark offline-only nodes with a visual indicator.

**Rationale**: FR-019 requires showing layout nodes that aren't discovered on the current bus. The backend already tracks `active_layout.layout_node_ids` (set of Node IDs in the layout). During discovery, compare discovered nodes against layout nodes. Nodes in the layout but not on the bus get a "not on bus" badge. Nodes on the bus but not in the layout are auto-added (FR-020).

**No protocol changes needed.** This is a UI presentation concern backed by set comparison in the frontend.

## R10: Multiple Connections per Layout

**Decision**: The layout stores a `Vec<ConnectionConfig>` in the manifest. The UI shows a connection selector when multiple connections exist.

**Rationale**: The existing `ConnectionConfig` struct from `connection.rs` is reused directly. The layout manifest gets a `connections` field. When connecting from within a layout, the user picks from the layout's connections. If only one exists, it's used directly (spec: US3-AS4).

**Active connection tracking**: The backend `active_connection` field already tracks the currently active config. The layout just stores the definitions; the backend activates one at a time.
