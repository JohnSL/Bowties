# Feature Specification: Offline Layout Editing

**Feature Branch**: `010-offline-layout-editing`  
**Created**: 2026-04-04  
**Status**: Draft  
**Input**: User description: "Currently you need to be connected to the LCC bus to be able to see or edit any node configuration. However, I have realized that there is a lot of value in support off-line editing of the configuration and bowties. For example, I might go to someone's layout and then 'capture' the entire layout. That means connecting to the LCC network and than saving the layout. That saved layout should contain not just the CDI and any event names, but also all of the current configuration values, and all of the captured producer identified events as well. This would then allow openign the layout without connecting to the LCC bus. Then then next time I open the layout and connect to the bus, I should have the option to save all the changes that have been saved only to files while I was offline."

## Clarifications

### Session 2026-04-04

- Q: How should bus-to-layout matching be classified? → A: Use weighted node-ID overlap thresholds: likely same >=80%, uncertain 40-79%, likely different <40%.
- Q: What on-disk file format should be canonical for persisted layout data? → A: YAML everywhere (manifest, node snapshots, bowtie metadata, and offline changes).
- Q: How should node snapshot files be named? → A: Canonical Node ID only (for example `nodes/0501010114A2B3C4.yaml`).
- Q: How should sync handle partial write failures? → A: Continue applying independent changes, mark failed rows, and keep failed rows pending for retry.
- Q: How should sync handle a write reply that indicates the target field is read-only? → A: Treat it as not dirty and reset to the previously read node value.

### Session 2026-04-05

- Q: What is the canonical persisted entry point for offline layouts? → A: A single user-selected base file (recommended extension `.layout`) plus a deterministic companion directory beside it (recommended suffix `.layout.d`).
- Q: How is legacy directory format handled during migration? → A: Legacy directory open (schema v2, `manifest.yaml` at root without a base `.layout` file) was temporarily added as import-only compatibility to allow one-time migration, then removed. Only the new base-file format (schema v3) is accepted.
- Q: How are node snapshots represented in the new format? → A: Canonical nested path-centric YAML using CDI display names as hierarchy keys, with per-leaf exact-write metadata (`space` and `offset`). Raw address-keyed flat values are no longer stored or accepted.

### Session 2026-04-12

- Q: How should offline layout opening handle transient loading states to avoid UI flicker (for example, brief Read Node Configuration CTA and changed badges)? → A: Use an explicit layout-open lifecycle state machine and gate user-facing change indicators until lifecycle phase reaches `ready`.

## Layout Open UX Lifecycle (Design Only)

This section defines a non-implementation design contract for opening a saved layout so asynchronous hydration remains deterministic and free of transient UI noise.

### Lifecycle State Machine

The frontend should expose a single canonical state for layout-opening lifecycle:

- `idle`: no open operation in progress.
- `opening_file`: user selected a file and backend open has started.
- `hydrating_snapshots`: snapshot payload is being mapped into node/view-model stores.
- `replaying_offline_changes`: pending offline changes are being loaded/replayed into edit overlays.
- `ready`: layout is fully usable; all user-facing indicators may render normally.
- `error`: open failed; layout-open UI returns to safe fallback and shows an actionable error.

### Transition Rules

1. `idle` -> `opening_file` when user chooses Open Layout or startup restore begins.
2. `opening_file` -> `hydrating_snapshots` only after backend open succeeds.
3. `hydrating_snapshots` -> `replaying_offline_changes` after node trees/config baselines are available.
4. `replaying_offline_changes` -> `ready` only after replay bookkeeping has settled.
5. Any in-progress phase -> `error` on failure.
6. `error` -> `idle` after user dismisses or recovery path completes.

### UI Gating Rules

1. Loading affordance: show a clear inline loading indicator whenever phase is not `idle` and not `ready`.
2. Read Node Configuration CTA: only eligible in online mode and only when phase is `ready`.
3. Changed/dirty badges (node-level, field-level, and tree-level): suppress while phase is not `ready`.
4. Interaction safety: actions that assume stable hydrated data (for example, bulk save/sync actions) remain disabled until `ready`.

### Data Semantics During Open

1. Hydration writes are baseline reconstruction, not user edits.
2. Replay of persisted offline changes is restoration, not new user interaction.
3. Visible changed indicators should represent post-open actionable state and should appear only after `ready`.

### Architecture Constraints

1. Use one lifecycle source of truth instead of multiple independent booleans for the same concern.
2. Derive UX visibility from lifecycle phase, not from incidental timing of store mutation order.
3. Keep lifecycle state transport-agnostic so it can support future async steps (schema migration, validation, remote fetch) without rewriting UI rules.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Capture a Full Layout for Offline Use (Priority: P1)

A user visits a friend's layout and connects the Bowties app to the LCC bus. After all nodes are discovered and their configurations have been read, they click **Save Layout**. The layout is written as a directory containing readable text files (including one node file per node) with current configuration values, SNIP data, and all producer-identified event IDs announced during the session. In addition, the existing cross-node data already stored in the layout file is preserved: bowtie connection names and tags (keyed by event ID) and user-assigned role classifications (Producer/Consumer overrides for ambiguous event slots). CDI XML is referenced from the local cache and not duplicated in every captured layout by default. The user can now take this layout directory home and open it without any bus connection.

**Why this priority**: This is the prerequisite for all offline work. Without a complete captured layout, no other offline scenario is possible. It is also immediately valuable even without offline editing — the captured file becomes a point-in-time backup of the entire layout's configuration state.

**Independent Test**: Can be tested by connecting to a bus with at least two nodes, reading all configuration, clicking Save Layout, then opening the saved directory in a text editor to verify it contains human-readable files, one node file per node, configuration values, SNIP data, and producer-identified event IDs. Also verify the layout-level file contains any existing bowtie names/tags and role classifications. No offline mode or sync logic is required.

**Acceptance Scenarios**:

1. **Given** a user has connected to the bus and node discovery + config reading is complete, **When** they click Save Layout, **Then** the layout is saved as a directory structure and includes, for each discovered node, all configuration values (keyed by memory address and address space), SNIP data (user name, user description, manufacturer name, model name), and all producer-identified event IDs received during the session.
2. **Given** bowtie connections and role classifications exist in the current layout, **When** the user saves, **Then** all bowtie names, tags, and role classifications are preserved in the layout directory exactly as they were, alongside the node snapshot data.
3. **Given** one or more nodes failed to have their configuration fully read, **When** the user saves the layout, **Then** the layout is still written successfully, with a capture status of `partial` for those nodes and a note explaining which data is missing.
4. **Given** no producer-identified events were received for a node (e.g., the node does not send them unprompted), **When** the layout is saved, **Then** the node snapshot records an empty event list rather than omitting the field.
5. **Given** a layout directory was previously saved and the user is replacing it, **When** they save, **Then** the existing directory is updated atomically (no data loss if the app crashes mid-write).
6. **Given** CDI XML needed for a captured node is already available in local cache, **When** the layout is saved, **Then** the capture stores a stable CDI reference (cache key/version/fingerprint) rather than duplicating the CDI body in each node file.
7. **Given** a user wants to start a brand-new capture, **When** they choose Close Layout and then New Layout Capture, **Then** the app starts from an empty layout context and does not carry over node snapshots, offline changes, or bowtie metadata from the previously open layout.
8. **Given** the layout is committed to source control, **When** only one node's values change, **Then** the saved output uses stable, readable text formatting so diffs are localized to the affected node file and avoid unrelated formatting churn.

---

### User Story 2 - Open a Captured Layout Without Connecting to the Bus (Priority: P1)

A user opens the Bowties app at home (no LCC bus available). They open the captured layout directory from the club layout. The app enters **Offline Mode** and loads all nodes, their configuration values, and bowties from disk — no bus connection required. The user can browse every node's configuration in the Configuration view and view all bowtie connections, just as if they were connected to the live bus. A persistent status banner clearly indicates "Offline — captured [date/time]."

**Why this priority**: This is the core value of the entire feature. Reading a captured layout offline is the most anticipated workflow (plan ahead at home, then apply changes at the club). It can and should be deliverable independently of offline editing or sync.

**Independent Test**: Can be tested by opening a valid captured layout directory while the LCC adapter is disconnected. Verify the interface displays all nodes in the sidebar, all configuration values in the Configuration view, all bowtie connections in the Bowties tab, and a clearly visible offline-mode indicator showing the capture date. No changes need to be made or synced.

**Acceptance Scenarios**:

1. **Given** a saved layout directory with full node snapshots and the LCC bus is not connected, **When** the user opens the layout, **Then** the app loads fully into Offline Mode showing all nodes, configuration values, and bowties without any network activity.
2. **Given** the app is in Offline Mode, **When** the user views the status area, **Then** a persistent banner or indicator reads "Offline — Captured [date and time of capture]" and the Connect button is visible but the bus is not auto-connected.
3. **Given** the app is in Offline Mode, **When** the user browses to any node's configuration in the Configuration view, **Then** all captured configuration values are displayed exactly as they were when the layout was captured.
4. **Given** the app is in Offline Mode, **When** the user opens the Bowties tab, **Then** all bowtie connections, names, and tags are visible and the producer/consumer elements show the correct captured event IDs.
5. **Given** a layout has partial capture status for a node (some data missing), **When** the layout is opened offline, **Then** affected configuration elements display a "(Not captured)" indicator, but the rest of the node's data still loads.

---

### User Story 3 - Edit Configuration and Bowties While Offline (Priority: P2)

A user browsing a captured layout at home decides they want to plan some configuration changes before visiting the club layout. They edit field values, rename bowties, and add new bowtie connections — all while offline. Each change is visually marked as an **offline change** (distinct from the unsaved-to-bus dirty indicator from Feature 007). The user can save the layout directory at any time to persist their offline changes to disk. None of these changes touch any physical LCC node.

**Why this priority**: Offline viewing alone is valuable, but offline editing is what makes the captured-layout workflow practical. Planning and preparing changes at home before a layout visit avoids errors under pressure. This is, however, a separate and additive capability beyond offline viewing.

**Independent Test**: Can be tested entirely without a bus by opening a captured layout, editing a configuration field value, verifying the field is marked with an offline-change indicator, saving the layout directory, closing the app, reopening the layout, and confirming the offline change is still present. Can also test bowtie creation and renaming.

**Acceptance Scenarios**:

1. **Given** the app is in Offline Mode with a captured layout open, **When** the user edits a configuration field value, **Then** the field is visually marked with an "offline change" indicator (visually distinct from the standard unsaved-to-bus indicator) showing both the captured value and the pending offline value.
2. **Given** the user changes a field back to its captured value, **When** viewing the field, **Then** the offline change indicator is removed and the field returns to its captured state.
3. **Given** the app is in Offline Mode, **When** the user creates or edits a bowtie (adds elements, renames, tags), **Then** those changes are tracked as offline changes and the bowtie canvas shows an indicator for each modified bowtie.
4. **Given** the user has made one or more offline changes, **When** they click Save, **Then** the layout directory is written to disk with both the captured baseline values and the pending offline changes stored separately, so the history is preserved.
5. **Given** the user saves while offline, **When** they reopen the layout later, **Then** all offline changes are restored exactly as left, with their offline-change indicators still visible.
6. **Given** the user wants to discard a specific offline change, **When** they revert a field to its captured value, **Then** that individual change is removed without affecting other offline changes.

---

### User Story 4 - Sync Offline Changes to the Bus (Priority: P2)

After working offline, the user visits the club layout and opens Bowties. By default, the app reopens the most recently used layout, but the active layout is always shown clearly and can be switched in one step before connecting. The user connects to the LCC bus with the intended layout active.

On first connection, the app initially knows only discovered node identities from SNIP. It uses those node IDs for a preliminary layout-match assessment while additional reads are still in progress. Once enough live values are available to compare pending offline changes, the app presents a **Sync Panel** before entering normal online mode. The panel is organized by urgency:

- **Conflicts** (bus value changed since capture) are shown prominently and must be resolved individually before they can be applied.
- **Clean changes** (bus value still matches the captured baseline) are collapsed into a pre-approved summary section. The user can expand it to deselect any they no longer want, then click **Apply** to write them all in one action.
- **Already-applied changes** (bus value already matches the planned offline value) are silently cleared and shown only as a count.

The user resolves any conflicts, optionally reviews clean changes, and clicks **Apply**. Only conflict-resolution results and non-deselected clean changes are written to the bus.

**Why this priority**: Sync closes the offline loop and delivers the full value of offline editing. Without it, offline changes are stranded in the file. This depends on P1 (capture) and P2 (offline editing) being complete first.

**Independent Test**: Can be tested with three prepared offline changes — one clean (bus untouched), one conflict (bus changed after capture), and one already-applied (bus already has the planned value). Start the app and confirm the last layout auto-loads and can be switched quickly. Open the target layout, connect, and verify: (a) a preliminary "matching in progress" state appears during SNIP-only discovery; (b) the Sync Panel appears only after sufficient live values are available for comparison; (c) the conflict appears prominently and blocks Apply until resolved; (d) the clean change is pre-selected in the summary section and can be deselected; (e) the already-applied change is silently cleared and shown only as a count. Then verify no sync panel appears if there are no offline changes.

**Acceptance Scenarios**:

1. **Given** Bowties starts and a recent layout exists, **When** the app opens, **Then** that recent layout is auto-loaded and the active layout identity is clearly visible with an immediate Switch Layout action.
2. **Given** a layout is open, **When** the user selects Close Layout, **Then** the app enters a no-layout state and presents actions to open an existing layout or start New Layout Capture.
3. **Given** the app is in no-layout state, **When** the user starts New Layout Capture, **Then** a new empty layout context is created and marked as the active layout for subsequent capture.
4. **Given** a layout with offline changes is active, **When** the user connects to the LCC bus and only SNIP discovery has completed, **Then** the app shows a preliminary "layout matching in progress" state and does not yet present conflict rows that require value reads.
5. **Given** the app has enough live values to compare pending offline changes, **When** matching and comparison complete, **Then** the Sync Panel appears with conflicts listed prominently in the primary section and clean changes collapsed in a summary section; any offline changes whose bus value already equals the planned value are silently cleared and shown only as a count.
6. **Given** the Sync Panel is shown and a field's current bus value differs from the captured baseline, **When** the user reviews that conflict row, **Then** the row shows the captured baseline value, the offline planned value, and the current bus value side-by-side, and the user must explicitly choose to apply the offline value or skip it before Apply is enabled for that row.
7. **Given** the Sync Panel is shown and a field's current bus value still equals the captured baseline, **When** the user views the clean changes summary, **Then** the section is pre-selected for bulk apply and can be expanded to reveal individual rows; the user can deselect any row they no longer want to apply.
8. **Given** the preliminary node-ID overlap suggests this is likely not the original layout bus, **When** connection matching finishes, **Then** the app prompts the user to choose either "Treat this as target layout bus" or "Treat this as bench/other bus" before any bulk sync action is performed.
9. **Given** the user chooses bench/other bus mode, **When** they proceed, **Then** the app suppresses automatic bulk sync prompting and allows normal browsing/configuration while keeping offline changes pending for the target layout bus.
10. **Given** the user has resolved all conflicts (applying or skipping each), **When** they click Apply, **Then** all conflict resolutions and all non-deselected clean changes are written to the bus in one operation.
11. **Given** all changes are applied successfully, **When** the operation completes, **Then** applied changes are removed from the offline-change list, their fields show the newly written value, and the Sync Panel closes, entering normal online mode.
12. **Given** the user deselects a clean change before applying, **When** they click Apply, **Then** that change is left as a pending offline change in the layout and is not written to the bus.
13. **Given** a node from the captured layout is not discovered on the current bus, **When** the Sync Panel is shown, **Then** that node's offline changes are displayed with a "Node not found on bus" indicator and do not block applying changes for other nodes.
14. **Given** the user opens a layout with no offline changes while connected to the bus, **When** loading completes, **Then** no Sync Panel appears and the app enters normal online mode directly.
15. **Given** Apply is in progress and one row write fails for a reason other than read-only, **When** the operation continues, **Then** independent remaining rows are still attempted, successful rows are cleared, and failed rows remain pending with explicit failure reasons for retry.
16. **Given** a write reply explicitly indicates the target field is read-only, **When** that reply is processed, **Then** that row is treated as not dirty, removed from pending offline changes, and the displayed value is reset to the previously read node value.

---

### User Story 5 - Prepare New Uninstalled Nodes at Home (Priority: P2)

A technically skilled helper receives a set of new nodes that are not yet installed on the layout's bus. At home, they connect those nodes to their own temporary bench setup. On initial connect, only node identity/SNIP-level data is available; they then read configuration values (per-node or read-all) to make fields visible and editable. They add those nodes into the same captured layout as staged nodes and configure them to match the target layout plan. At the next work session, they connect to the real layout bus and use the same sync flow to apply any remaining pending changes and validate that each staged node now matches the planned configuration.

**Why this priority**: This is a high-value practical workflow for clubs where the layout owner cannot do all setup work personally. It reduces on-site work to install/test/fix rather than initial configuration, and enables distributed preparation.

**Independent Test**: Can be tested by creating a layout with existing captured nodes, connecting new nodes on a bench bus, verifying they first appear with identity-only data, reading configuration values (per-node or read-all), then adding a new node not present in the original capture as staged. Make configuration and bowtie-related event assignments, save, reopen, and verify the staged node appears with pending changes. Then connect to a bus where that node is now present and verify sync can apply/validate staged changes.

**Acceptance Scenarios**:

1. **Given** a captured layout is opened offline, **When** the user adds a node that was not part of the original bus capture, **Then** the node is stored as a staged node with its own configuration snapshot and pending changes.
2. **Given** the user connects to a bench bus with staged nodes, **When** only discovery/SNIP has completed, **Then** the app shows the nodes as discovered but does not show editable configuration values until read operations are run.
3. **Given** staged nodes are connected on a bench bus, **When** the user runs per-node read or read-all, **Then** configuration fields become visible/editable for nodes whose reads succeeded.
4. **Given** a staged node has planned configuration values, **When** the layout is saved, **Then** those values are preserved in readable node files and clearly marked as staged (not yet validated on target bus).
5. **Given** a staged node later appears on the target layout bus, **When** the user opens Sync Panel, **Then** staged-node changes are listed and can be applied/validated the same way as other offline changes.
6. **Given** a staged node still does not appear on the target bus, **When** sync is run, **Then** its changes remain pending and non-blocking.

---

### Edge Cases

- What happens if the bus has nodes that are not in the captured layout? Those nodes are discovered normally and shown in the live node list; no offline data is associated with them.
- What if a node in the captured layout has had a firmware update that changed its CDI? The node is flagged as "CDI changed since capture." Its captured configuration values are preserved for reference but offline edits for that node are blocked pending a fresh config read.
- What if the app crashes mid-save (while rewriting multiple layout files)? Atomic write behavior ensures either old files remain intact or new files are complete; no partially rewritten node file is treated as valid.
- What if the user makes offline changes to a bowtie that doesn't have a corresponding node configuration change? Bowtie metadata changes (names, tags) are always safe to apply to layout metadata files and never require bus sync.
- What if event IDs changed on the bus since capture (e.g., someone re-programmed a node using another tool)? The Sync Panel flags event-ID fields as conflicts if the current bus value doesn't match the baseline, prompting the user to decide.
- What about read-only configuration fields (e.g., track voltage reported by the hardware) that will always differ from any offline-planned value? Without a profile for that node type, the app cannot know the field is read-only, so it will appear as a conflict. There is no general remedy for this — if no profile is available it is up to the user to recognise and skip those rows. When a profile *is* present and marks a field as read-only, the system should suppress that field from the conflict list entirely.
- What does the app do if the user is offline and tries to connect to the bus? Connecting should be always permitted; the Sync Panel appears only if there are offline changes.
- What if a layout is opened on a machine without the cached CDI XML references used by node snapshots? The app shows a missing-CDI warning for affected nodes and offers an explicit export/import CDI package flow so offline browsing/editing can continue.
- What if the layout is committed to git and two users edited different node files? Git merges should remain meaningful at file and line level, with conflicts localized to changed node files rather than one monolithic blob.
- What if node-ID overlap is inconclusive (for example, only a small subset of nodes is currently powered)? The app should classify bus matching as "uncertain" and require explicit user choice (target layout bus vs bench/other bus) before applying bulk sync.
- What happens if the user closes a layout while there are unsaved or pending offline changes? The app should require an explicit Save, Discard, or Cancel decision before closing.
- What if a node is discovered but its configuration read has not been run (or failed)? The node can be shown in the tree with identity/SNIP data, but configuration fields must remain unavailable until a successful read for that node.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST extend layout persistence (new schema version) from a single file to a layout directory with human-readable text files, including one node file per node plus a top-level manifest.
- **FR-002**: Node snapshot files MUST include configuration values keyed by address space and address offset, SNIP data, producer-identified event IDs, capture timestamp, and capture status.
- **FR-003**: By default, node snapshots MUST store CDI references (cache key/version/fingerprint) rather than duplicating raw CDI XML in every layout capture.
- **FR-004**: System MUST provide an explicit CDI export/import option so users can create a portable package for layouts that will be opened on machines without the same CDI cache.
- **FR-005**: When saving a layout while connected to the bus, the system MUST populate node snapshot data for all nodes whose configuration has been successfully read; nodes with incomplete reads are saved with a `partial` capture status and a list of which elements are missing.
- **FR-006**: System MUST support opening a layout directory without any bus connection, entering Offline Mode and rendering all node configurations, bowtie connections, and event names from captured snapshot data.
- **FR-007**: While in Offline Mode, the system MUST display a persistent status indicator showing that the app is offline and the date and time of the layout capture.
- **FR-008**: While in Offline Mode, configuration elements whose data was not captured (partial captures) MUST be rendered with a "(Not captured)" placeholder instead of an editable field.
- **FR-009**: System MUST allow editing configuration field values while in Offline Mode, tracking each change as an offline change with the captured baseline value and the pending offline value stored separately.
- **FR-010**: Offline changes MUST be visually distinct from standard unsaved-to-bus indicators so users can always tell whether a change is "not yet written to the bus" (online) or "planned while offline and not yet synced."
- **FR-011**: System MUST allow bowtie creation, modification, and deletion while in Offline Mode, tracking those as offline changes.
- **FR-012**: Saving while in Offline Mode MUST write the layout directory with both captured baseline values and pending offline changes preserved, so the sync can distinguish them later.
- **FR-013**: When a layout containing offline changes is opened while connected to a bus, the system MUST present a Sync Panel before entering normal online mode.
- **FR-013a**: On startup, the system SHOULD auto-load the most recently used layout and MUST always show which layout is currently active, with a one-step switch-layout action available before connecting.
- **FR-013a1**: The system MUST provide a Close Layout action that transitions the app to a no-layout state and clears active layout context from the UI.
- **FR-013a2**: In no-layout state, the system MUST provide a New Layout Capture action that creates a new empty layout context for capturing current bus data.
- **FR-013b**: During initial connect, the system MUST use discovered node IDs (from SNIP) to compute a weighted overlap score and classify preliminary bus-to-layout match status with explicit thresholds: `likely same` at >=80%, `uncertain` at 40-79%, and `likely different` at <40%, before full value comparison is complete.
- **FR-013c**: On a fresh bus connection, the system MUST treat discovery/SNIP data and configuration-value reads as separate phases; discovery alone is sufficient for identity/matching, but not for editable configuration values.
- **FR-014**: The Sync Panel MUST separate offline changes into three categories: (a) conflicts — bus value differs from captured baseline, require explicit resolution per row; (b) clean — bus value still matches captured baseline, pre-selected for bulk apply and expandable for deselection; (c) already-applied — bus value already equals the planned offline value, silently cleared and shown only as a count.
- **FR-015**: Conflict rows MUST display the captured baseline value, the offline planned value, and the current bus value side-by-side, and MUST require an explicit per-row apply or skip choice before Apply is enabled for that conflict.
- **FR-016**: The user MUST be able to select a subset of offline changes to apply; unselected changes remain as pending offline changes in layout files.
- **FR-016a**: When a node profile is available and identifies a field as read-only, the system MUST exclude that field from the Sync Panel entirely (neither as a conflict nor as a clean change); without a profile the system has no basis for this exclusion and the field appears as a normal conflict row.
- **FR-016b**: If match status is `uncertain` or `likely different`, the system MUST require explicit user mode selection (`target layout bus` or `bench/other bus`) before any bulk apply action.
- **FR-016c**: In `bench/other bus` mode, the system MUST suppress automatic bulk sync prompting and preserve all pending offline changes for later application on the target layout bus.
- **FR-017**: Applying a change in the Sync Panel MUST write the configuration value to the bus node exactly as the existing save mechanism in Feature 007 does.
- **FR-017a**: During Apply, the system MUST continue processing independent rows after non-fatal per-row failures; successful rows are committed/cleared, failed rows remain pending with per-row error details.
- **FR-017b**: If a write reply indicates a target field is read-only, the system MUST treat that row as non-dirty, clear it from pending offline changes, and restore the displayed value to the most recently read value from the node.
- **FR-018**: If a node in the captured layout is not present on the bus when the Sync Panel is shown, its offline changes MUST be displayed as pending but non-blocking; they do not prevent the Sync Panel from completing for other nodes.
- **FR-019**: If a node's CDI has changed since the layout was captured (CDI XML mismatch), the system MUST flag that node as "CDI changed" and block applying its offline configuration changes until the user acknowledges or re-reads the configuration.
- **FR-020**: Bowtie offline changes (names, tags, connections) are applied to the layout files on Apply and do not require any bus write for metadata-only changes; event ID changes in bowties DO require bus writes and must go through the same sync path as configuration edits.
- **FR-021**: The system MUST NOT automatically push any offline changes to the bus without explicit user action in the Sync Panel.
- **FR-022**: System MUST allow users to add staged nodes that were not present during original capture and persist them as first-class node files in the layout directory.
- **FR-023**: Staged nodes MUST remain marked as pending/not-validated until they are observed on a bus and either synced or explicitly acknowledged by the user.
- **FR-024**: Layout directory structure and file naming MUST produce meaningful git diffs where changes to one node are isolated primarily to that node's file.
- **FR-025**: If Close Layout is requested while unsaved or pending offline changes exist, the system MUST require explicit user choice to Save, Discard, or Cancel before closing.
- **FR-026**: Configuration values for connected nodes MUST only be shown as editable after a successful read for that node (either via per-node read or read-all). Nodes without successful reads MUST remain identity-only.
- **FR-027**: All persisted layout files MUST use YAML as the canonical documented human-readable text format (UTF-8 encoded), including manifest, node snapshots, bowtie metadata, and offline changes, so users can inspect and review changes directly in standard editors.
- **FR-028**: Serialization MUST be deterministic for semantically unchanged data (stable field ordering, stable list ordering where order has no meaning, and normalized formatting) so repeated saves do not create noisy diffs.
- **FR-029**: Volatile/session-only values (for example ephemeral timestamps or runtime-only identifiers) MUST be excluded from per-node content unless required for semantics; when required, they MUST be stored in a clearly scoped metadata location to minimize diff noise.
- **FR-030**: Node snapshot filenames MUST be derived from canonical Node ID only (no user-editable node names in filenames), using a deterministic path pattern such as `nodes/<NODE_ID>.yaml`, so renames do not cause file churn in source control.
- **FR-031**: System MUST model layout-open as an explicit lifecycle (at minimum: `opening_file`, `hydrating_snapshots`, `replaying_offline_changes`, `ready`, `error`) and MUST suppress user-facing changed/dirty indicators and online-read CTAs until lifecycle phase is `ready`.

### Key Entities

- **Layout Manifest**: Top-level metadata file for a captured layout directory (layout identity, schema version, capture timestamps, file index, optional git metadata).
- **Node Snapshot**: A point-in-time capture of a single LCC node, containing CDI reference(s), all readable configuration values (address space + offset → value), SNIP fields, producer-identified event IDs, capture status (`complete` or `partial`), and capture timestamp.
- **Captured Layout Directory**: The extended layout persistence format that stores manifest, per-node snapshot files, bowtie metadata, event names, and pending offline changes as separate readable files.
- **Offline Change**: A tracked modification made while in Offline Mode, storing the field's identity (node ID, address space, address offset or bowtie ID), the captured baseline value, and the pending offline value. Part of the Captured Layout.
- **Sync Session**: The in-memory comparison built when opening a captured-layout-with-offline-changes while connected to the bus. Pairs each offline change with the current bus value to identify clean cases and conflicts for display in the Sync Panel.
- **Staged Node**: A node intentionally added to the layout during offline/bench work that was not part of the original layout-bus capture. Treated as pending until observed on the target bus.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can save a complete layout capture (config values, SNIP, producer events, CDI references) for a bus with 10 or fewer nodes in under 3 minutes beyond the time already spent reading configuration.
- **SC-002**: Opening a captured layout in Offline Mode and displaying the first node's full configuration takes under 3 seconds from file selection.
- **SC-003**: The Sync Panel correctly identifies and shows 100% of offline changes when reconnecting, with zero changes silently applied or lost.
- **SC-004**: A user with 10 offline configuration changes — where none are conflicts — can apply all of them through the Sync Panel in under 30 seconds; a session with up to 5 conflicts can be fully resolved in under 5 minutes.
- **SC-005**: Conflicts (bus changed after capture) are shown to users with enough information that they can make an informed decision without referring to any external documentation.
- **SC-006**: No previously saved layout data is lost when a capture or save operation is interrupted (crash, power loss).
- **SC-007**: In a git diff between two layout revisions where only one node changed, at least 90% of changed lines are confined to that node's file(s), not unrelated nodes.
- **SC-008**: A user can add and prepare at least 5 staged nodes offline and later sync/validate them on the target bus in one session without blocking on nodes that are still absent.
- **SC-009**: While opening a saved layout, no transient changed/dirty badges or Read Node Configuration CTA are visible before hydration/replay completes; once ready, indicators match final computed state with no intermediate flicker.

## Assumptions

- The LCC bus is a single network segment; multi-bus or gateway topologies are out of scope.
- "Producer-identified events" refers to events announced via LCC Producer Identified messages received on the bus during the session. The capture process actively solicits these by sending an Identify Producers query; passively received announcements are also collected.
- The existing atomic write mechanism from Feature 009 (temp file → flush → rename) is extended to multi-file directory saves using a safe staging-and-swap approach.
- Offline changes to bowtie metadata (names, tags) never conflict with bus state since this metadata is not stored on the nodes.
- The new layout schema version (v3) is backward-incompatible with the legacy capture directory format (v2) and with the Feature 009 single-file layout format; neither is accepted for opening. The v2 capture directory format had a temporary import path that was removed after migration was complete.
- Re-reading configuration from the bus while online always updates the baseline, clearing any offline changes for re-read fields (the fresh bus value becomes the new baseline).
- CDI XML already exists in local cache and is canonical for normal operation; captured layouts only need stable CDI references unless user explicitly requests an export bundle for portability.
