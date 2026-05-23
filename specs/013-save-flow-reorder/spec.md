# Feature Specification: Layout-First Model

**Feature Branch**: `013-save-flow-reorder`  
**Created**: 2025-05-16  
**Status**: Draft  
**Input**: Adopt a layout-first model where a layout is always required before any work can begin. Connections are properties of the layout. Online/offline are phases of the layout. This eliminates the 4-state complexity (connected/disconnected × layout/no-layout) that causes blank bowties during save, stale catalog after bus writes, confusing `isOfflineMode` flipping, and fragile state transitions.

## Problem

The current architecture treats connection and layout as independent axes, producing four states:

| | No Layout | Layout Open |
|---|---|---|
| **Disconnected** | Idle — nothing to do | Offline editing |
| **Connected** | Live browsing — no file to save metadata to | Merged — edits flip dynamically |

This causes multiple interrelated defects:

1. **Blank bowties during and after save.** Online save writes config to bus before saving the layout file. Bus writes trigger draft pruning, which flips the bowtie preview to a stale catalog. The catalog never rebuilds until the file save that comes too late.
2. **Cancel leaves blank bowties.** Cancelling the Save dialog after bus writes have fired means the catalog rebuild in the file-save path never executes.
3. **Stale catalog after config-only save.** Even without a layout file, writing config to bus triggers draft pruning and switches to a stale catalog that has no rebuild trigger.
4. **`isOfflineMode` flips dynamically.** Connecting while a layout is open changes where edits go (offline cache → bus sync) without any user action — a source of confusion and edge-case bugs.
5. **Metadata edits without a layout.** Creating bowties, classifying roles, or selecting daughter boards while connected without a layout forces implicit layout creation at save time, surprising the user.
6. **Event roles lost on offline reopen.** Only user-override role classifications are persisted. Roles resolved via protocol exchange are lost when reopening offline.
7. **Saves fail in cloud-synced folders.** The atomic save renames a `.layout.d.staging` directory into place. On Windows under Dropbox, OneDrive, or antivirus, the directory rename fails with a sharing-violation error because the sync agent has opened files inside the staging directory between write-close and rename. The user is left with a base `.layout` file but no companion directory, and the layout cannot be reopened. The root cause is twofold: the persistence layer relies on a directory rename (the single most contention-prone Windows filesystem operation), and the `layout/` module is not deep enough — command modules construct companion-dir paths and write into them directly, so any journal added inside `layout/` would be bypassed.

These are symptoms of a structural problem: the app allows a "connected without layout" state that has no durable home for metadata, and the connection/layout independence creates combinatorial transition complexity.

## Solution

Adopt a **layout-first model**: the app always operates within a layout context.

- **Startup** presents a layout picker showing known layouts (name, path, last-opened date) plus "New Layout" and "Browse…" options. The picker abstracts the underlying storage — the user sees layout names, not files and directories.
- **A layout** is persisted as a base file (`.layout`) plus a companion directory (`.layout.d/`) containing per-node snapshot files. This structure is designed for git-friendly diffs: changing one node's configuration produces a localized diff in that node's snapshot file.
- **The layout** is the durable container for node snapshots, bowtie metadata, role classifications, connector selections, offline changes, AND connection definitions.
- **Connections** are properties of the layout. A layout can define multiple named connections (e.g., "Home Workbench", "Club Layout").
- **Online/offline** are phases of the layout, not independent axes. The layout is always present; connecting activates a layout connection.
- **Save** always means "save the layout." If online, it also writes config changes to bus. The ordering is always: layout first, bus second, reconcile third.

This reduces the state space from four states to two: **layout-offline** and **layout-online**.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create or open a layout on startup (Priority: P1)

When the user launches Bowties, the app presents a layout picker: a list of known layouts (showing name, location, and last-opened date), plus "New Layout" and "Browse…" options. No other functionality is available until a layout is active. Selecting a known layout opens it immediately. "New Layout" prompts for a name and location, creates the base file and companion directory, and opens the empty layout. "Browse…" allows navigating to a layout base file not yet in the known list. The picker abstracts the file+directory storage — the user sees layout names, not the underlying `.layout` file and `.layout.d/` directory.

**Why this priority**: This is the foundational change — everything else depends on always having a layout context.

**Independent Test**: Launch the app fresh → verify layout picker appears → create a new layout → verify empty layout opens → close and relaunch → verify layout appears in the known list.

**Acceptance Scenarios**:

1. **Given** the app is launched with no known layouts, **When** the layout picker appears, **Then** the user sees "New Layout" and "Browse…" options with an empty known-layouts list.
2. **Given** the app is launched with known layouts, **When** the layout picker appears, **Then** the user sees a list of known layouts with names, locations, and last-opened dates, and can open one with a single click.
3. **Given** the user clicks "New Layout," **When** they provide a name and location, **Then** a base file and companion directory are created and the empty layout opens.
4. **Given** a layout is open, **When** the user views the app, **Then** the layout name is clearly visible in the title bar or header area.
5. **Given** a layout is open, **When** the user wants to switch layouts, **Then** they can close the current layout and return to the layout picker.
6. **Given** the user clicks "Browse…," **When** they navigate to a layout base file not in the known list, **Then** the layout opens and is added to the known list for future access.

---

### User Story 2 - Connect to a bus from within a layout (Priority: P1)

With a layout open, the user connects to an LCC bus. The connection settings are saved in the layout. All edits (configuration changes, bowtie metadata, role classifications, connector selections) are tracked within the layout. Disconnecting returns to offline layout editing — the layout remains open with all data intact.

**Why this priority**: Connection must be a layout-scoped action, not an independent axis. This eliminates the "connected without layout" state.

**Independent Test**: Open a layout → add a connection → connect → edit some config → disconnect → verify layout stays open and all data is intact → reconnect → verify same connection settings are available.

**Acceptance Scenarios**:

1. **Given** a layout is open and no connection exists, **When** the user adds a connection, **Then** the connection settings (host/port or serial port) are stored in the layout.
2. **Given** a layout has a saved connection, **When** the user connects, **Then** the app uses the stored connection settings without requiring re-entry.
3. **Given** a layout is connected (online), **When** the user disconnects, **Then** the layout remains open, all node data is preserved from snapshots, and the user continues editing offline.
4. **Given** a layout is connected, **When** the user edits configuration fields, **Then** the changes are tracked as pending edits within the layout — the same mechanism regardless of online/offline state.
5. **Given** a layout has a connection defined, **When** the user reopens the layout later, **Then** the connection definition is available but the app does not auto-connect without user action.

---

### User Story 3 - Multiple connections per layout (Priority: P1)

A layout can define more than one named connection. This supports the workflow of configuring new nodes at home on a workbench bus, then installing them at the club and connecting to the club's bus — all within the same layout.

**Why this priority**: Multi-connection is essential for the home/club workflow that spec 010 describes. It also supports users with multiple bus segments or test setups.

**Independent Test**: Open a layout → add "Home Workbench" connection → add "Club Layout" connection → connect to Home → configure a node → disconnect → connect to Club → verify both connections are available and node config is preserved.

**Acceptance Scenarios**:

1. **Given** a layout is open, **When** the user adds multiple named connections, **Then** each connection has a distinct name and settings, and both are persisted in the layout.
2. **Given** a layout has multiple connections, **When** the user chooses to connect, **Then** they can select which connection to activate.
3. **Given** a layout is connected via "Home Workbench," **When** the user disconnects and connects via "Club Layout," **Then** all layout data (node snapshots, offline changes, bowties) is preserved across the connection switch. Only one connection may be active at a time.
4. **Given** a layout has exactly one connection, **When** the user connects, **Then** the app uses that connection directly without requiring a selection step.

---

### User Story 4 - Save always persists the layout first (Priority: P1)

When the user clicks Save, the layout (base file and companion directory) is always written first. If online, configuration changes are then written to the bus. If some bus writes fail, the failed changes remain in the layout as offline changes for retry. The bowtie preview never goes blank or shows stale data during any part of the save process.

**Why this priority**: This fixes the blank-bowtie bugs and stale-catalog defects caused by the current save ordering. With a layout always present, the layout-first ordering is natural and consistent.

**Independent Test**: Make configuration changes while connected → Save → verify bowties stay correct throughout → verify layout is saved before bus writes → verify bowties are correct after save completes.

**Acceptance Scenarios**:

1. **Given** a user has pending configuration changes while online, **When** they click Save, **Then** the layout is saved before any data is written to bus nodes.
2. **Given** a save is in progress, **When** the layout is written and the bowtie catalog is rebuilt, **Then** the bowtie preview displays correct producers and consumers — never blank or stale.
3. **Given** a user clicks Save and then cancels the Save dialog (for Save As or first-time save), **When** the dialog is dismissed, **Then** no data is written to bus nodes and all pending changes are preserved.
4. **Given** configuration writes to the bus partially fail, **When** the save process completes, **Then** succeeded changes are cleared, failed changes remain as offline changes in the layout, and the user can retry.
5. **Given** a user has only metadata edits (bowtie names, roles, connector selections) and no config changes, **When** they click Save, **Then** only the layout is saved — no bus writes occur.
6. **Given** a save completes with all bus writes successful, **When** the layout is saved again (reconciliation), **Then** the layout contains zero offline changes for the values that were written.
7. **Given** a layout lives in a folder actively synced by Dropbox or OneDrive, or scanned by antivirus, **When** the user saves, **Then** the save succeeds without sharing-violation errors and the layout reopens cleanly with all data intact.
8. **Given** a previous save was interrupted (process kill, power loss, or unrecoverable sharing violation) leaving the layout in a partially-written state, **When** the user next opens the layout, **Then** the previous coherent state is restored automatically and the user sees a one-line notice that recovery occurred.

---

### User Story 5 - Resolved event roles persist in the layout (Priority: P2)

When the user saves, all resolved event role classifications (Producer/Consumer) from the current bowtie catalog are persisted in the layout — not just user overrides for ambiguous slots. When the layout is reopened offline, bowties display the correct roles from the saved data.

**Why this priority**: Losing role classifications on offline reopen degrades the offline experience. With a layout-first model, the layout is the natural home for all resolved state.

**Independent Test**: Configure same-board connections → save → disconnect → close layout → reopen layout → verify roles display correctly without "Unknown role."

**Acceptance Scenarios**:

1. **Given** a layout with connections whose event roles were resolved while online, **When** the user saves, **Then** the layout contains all resolved (non-ambiguous) event role classifications.
2. **Given** a saved layout containing resolved event roles, **When** the user reopens the layout offline, **Then** bowties display the correct Producer/Consumer roles from the saved data.
3. **Given** a layout with some ambiguous event roles that were not resolved, **When** the layout is saved, **Then** ambiguous entries are not written to role classifications — they remain ambiguous on reopen.

---

### User Story 6 - Progress feedback during save (Priority: P3)

During the save process, the user sees progress feedback indicating the current phase: saving the layout, writing configuration to nodes (with per-field progress), and reconciling the layout after writes complete.

**Why this priority**: Progress feedback improves user confidence during multi-step operations but is not functionally critical.

**Independent Test**: Initiate a save with several pending changes → observe progress labels updating through the phases.

**Acceptance Scenarios**:

1. **Given** a save is in progress, **When** the layout is being written, **Then** the user sees "Saving layout…" or equivalent progress indication.
2. **Given** a save is in progress, **When** configuration is being written to nodes, **Then** the user sees per-field progress (e.g., "Writing configuration… 3 of 7").
3. **Given** a save is in progress, **When** the layout is being reconciled, **Then** the user sees "Updating layout…" or equivalent progress indication.

---

### Edge Cases

- What happens if the app crashes between saving the layout and writing to the bus? The layout contains offline changes that can be replayed on next launch — matching existing crash recovery behavior.
- What happens if the user closes the app during the bus-write phase? Partially written changes are on the bus (irreversible), but unwritten changes remain as offline changes in the layout.
- What happens if the file system fails during reconciliation? The layout still contains the offline changes from the initial save. On next save or reopen, those changes are available for retry.
- What happens if the user opens a layout that was saved with connection settings for a bus that's not available? The layout opens in offline mode. The connection definition is preserved but inactive. The user can edit offline or connect to a different defined connection.
- What happens to existing layout files? They are migrated to include connection definitions on open, with a one-time prompt. The connection settings section starts empty (user adds connections manually).
- What about Save As? The same layout-first, bus-second flow applies. Save As prompts for a new location, then proceeds identically.
- What happens if a layout's companion directory is missing or corrupted? The app reports the issue and offers to create a fresh companion directory, preserving whatever data exists in the base file.
- What happens if the user removes a layout from the known list? The layout files are not deleted — only the app's reference is removed. The user can re-add it via "Browse…"

## Clarifications

### Session 2026-05-17

- Q: Can only one connection be active at a time, or can the user connect to multiple buses simultaneously? → A: Only one connection active at a time; switching requires disconnecting first.
- Q: What happens if the user triggers Save while a previous save's bus writes are still in progress? → A: Block — disable Save while a save is in progress. The progress dialog (User Story 6) serves as the modal blocking indicator.
- Q: How does the layout identify a node across different connections? → A: By Node ID (48-bit unique identifier), which is globally unique per the LCC protocol spec.
- Q: When connected to a bus, should the node list show layout-saved nodes not present on the current bus? → A: Yes — show all layout nodes while online; mark nodes not discovered on the current bus with a visual indicator.
- Q: When a node is discovered on the live bus that has no saved snapshot in the layout, is it automatically added to the layout or must the user explicitly add it? → A: Auto-add — discovered nodes are automatically included in the layout.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST require an active layout before any node browsing, configuration editing, bowtie creation, or connection activity can occur.
- **FR-002**: On startup, the app MUST present a layout picker showing known layouts plus "New Layout" and "Browse…" options.
- **FR-003**: The layout picker MUST display layout names, locations, and last-opened dates — abstracting the underlying file+directory storage from the user.
- **FR-004**: A layout MUST store connection definitions (name, type, host/port or serial settings) as part of the layout's base file.
- **FR-005**: A layout MUST support multiple named connection definitions.
- **FR-006**: Connecting to a bus MUST be an action within an open layout, using a connection defined in that layout. Only one connection may be active at a time; the user MUST disconnect before switching to a different connection.
- **FR-007**: Disconnecting MUST NOT close the layout. The layout remains open in offline mode with all data preserved.
- **FR-008**: Save MUST always write the layout (base file and companion directory) before writing any configuration data to bus nodes.
- **FR-009**: The system MUST NOT write any data to bus nodes if the user cancels the Save dialog.
- **FR-010**: If the user cancels the Save dialog after intent has been staged, the system MUST restore pending changes to their pre-save state.
- **FR-011**: After writing config to the bus, the system MUST save the layout again to clear successfully written offline changes.
- **FR-012**: After writing config to the bus, the system MUST rebuild the bowtie catalog from the updated node state.
- **FR-013**: If some node writes fail, the system MUST retain failed changes as offline changes in the layout for retry.
- **FR-014**: If some node writes fail, the system MUST report the failure count to the user.
- **FR-015**: During save, the system MUST persist all resolved (non-ambiguous) event role classifications from the current bowtie catalog into the layout.
- **FR-016**: The system MUST display progress feedback indicating the current save phase.
- **FR-016a**: The save progress feedback MUST be modal, preventing the user from initiating another save while one is in progress.
- **FR-017**: Existing layout files (without connection definitions) MUST be openable and migrated seamlessly.
- **FR-018**: The layout picker MUST allow removing a layout from the known list without deleting its files.
- **FR-019**: When connected to a bus, the node list MUST show all nodes in the layout, including nodes with saved snapshots that are not discovered on the current bus. Nodes not present on the current bus MUST be visually distinguished from live nodes.
- **FR-020**: When a node is discovered on the live bus that has no existing snapshot in the layout, the system MUST automatically include it in the layout. The node becomes part of the layout and its snapshot is persisted on the next save.

### Key Entities

- **Layout**: The top-level container for all user work, persisted as a base file (`.layout`) plus a companion directory (`.layout.d/`) containing per-node snapshot files and offline changes. This structure is designed for git-friendly diffs. Contains node snapshots, bowtie metadata, role classifications, connector selections, offline changes, and connection definitions. Always required. Nodes are identified by their globally unique 48-bit Node ID, which serves as the stable key for correlating a node across different connections.
- **Known Layout**: An entry in the app's layout registry (name, file path, last-opened date). The registry is stored in app preferences, not in the layout itself. The layout picker displays known layouts.
- **Connection Definition**: A named set of connection parameters (type, host/port or serial) stored in the layout's base file. A layout can have multiple connection definitions.
- **Draft Entry**: A pending user configuration change that has not yet been written to a node or saved to the layout.
- **Offline Change**: A configuration change staged into the layout's persistence layer. Offline changes survive app restarts and can be replayed when connectivity is restored.
- **Bowtie Catalog**: The computed mapping of event IDs to producer/consumer roles, used to render the bowtie preview. Rebuilt from CDI tree state and role classifications.
- **Role Classification**: A resolved determination that a particular event slot is a Producer or Consumer. Persisted in the layout.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The app never allows node browsing, configuration editing, or bowtie creation without an active layout.
- **SC-002**: Connecting and disconnecting does not change the layout's open/closed state — the layout always remains open.
- **SC-003**: Bowtie previews never display blank or stale data at any point during or after saving.
- **SC-004**: Cancelling the Save dialog results in zero bytes written to bus nodes and all pending changes fully preserved.
- **SC-005**: After a successful online save, the layout contains zero offline changes for values that were written to the bus.
- **SC-006**: Event roles resolved while online are preserved through a save → close → offline reopen cycle with 100% accuracy for non-ambiguous roles.
- **SC-007**: The full automated test suite passes with no regressions after the change (baseline: 790+ tests, expected to grow).
- **SC-008**: Existing layout files can be opened and migrated without data loss.
- **SC-009**: The layout picker correctly shows known layouts and a newly created layout appears in the list on next launch.

## Scope Boundaries

### In Scope

- Layout-first startup flow: layout picker with known layouts, "New Layout," and "Browse…"
- Known-layout registry: app-level list of layout names, paths, and last-opened dates
- Layout storage model: base file (`.layout`) + companion directory (`.layout.d/`) — git-friendly per-node snapshot files
- Connection definitions stored in the layout's base file
- Multiple named connections per layout
- Elimination of the "connected without layout" state
- Save flow: always layout-first, then bus writes if online, then reconciliation
- Cancel recovery: undo staging if the user cancels the Save dialog
- Partial failure handling: retain failed offline changes for retry
- Progress feedback across save phases
- Persist all resolved event role classifications during save
- Migration path for existing layout files (add empty connections section)
- Update tests for the new startup flow, connection model, and save flow

### Out of Scope

- Auto-connect on layout open (user must explicitly connect — can be added later)
- Layout templates or layout-creation wizards beyond name + location
- Sharing layouts between multiple users or devices (future work)
- Changes to the `writeModifiedValues` backend command itself
- Pending-writes marker in layout for crash recovery (deferred — YAGNI for now)

## Assumptions

- The layout is persisted as a base file (`.layout`) plus a companion directory (`.layout.d/`). This is the existing storage format from spec 010. Per-node snapshot files in the companion directory produce localized git diffs when individual node configurations change.
- The layout picker uses an app-level registry of known layouts (stored in app preferences). This registry maps layout names to file paths. The layout files themselves do not need to know about the registry.
- The existing layout YAML format is extended with a `connections` section in the base file — it is not a completely new format.
- The existing offline changes infrastructure (staging, clearing, reload) works within the layout-first model without fundamental changes to its mechanics.
- The number of configuration fields per save is typically small, so per-field IPC overhead for replaying offline changes is acceptable.
- File writes are fast and atomic (temp → flush → rename), so two file writes per online save (before and after bus writes) are acceptable.
- Migration of existing layout files adds an empty `connections` section and otherwise preserves all data.

## Dependencies

- Existing layout file format and persistence infrastructure (extended, not replaced)
- Existing `stageDraftsForOfflineSave()` function for staging drafts
- Existing `writeModifiedValues` and `setModifiedValue` backend commands for bus writes
- Existing `buildBowtieCatalog` and `setCatalog` for catalog rebuilds
- Existing `role_classifications` map in layout types for role persistence
- Existing `merge_layout_metadata` logic for applying saved role classifications on reopen

## Relationship to Other Specs

- **Spec 010 (Offline Layout Editing)**: The layout-first model strengthens offline editing by making the layout the always-present container. The multi-connection feature directly enables the "configure at home, sync at club" workflow from spec 010. The sync panel and offline change replay work within the layout context.
- **Spec 009 (T019 — Unified Save Flow)**: This spec supersedes the original "write to nodes first, then save layout" ordering decision from T019. The new ordering is: save layout first, write to bus second, reconcile third.
