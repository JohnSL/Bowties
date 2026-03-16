# Feature Specification: Editable Bowties

**Feature Branch**: `009-editable-bowties`  
**Created**: 2026-03-15  
**Status**: Draft  
**Input**: User description: "Make bowties view editable with bidirectional config sync, YAML persistence for connection names, and multiple creation modes (intent-first and config-first)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create a Connection from the Bowties Tab (Priority: P1)

A user opens the Bowties tab and clicks **+ New Connection**. A dialog appears with two panels — one for selecting a producer element and one for selecting a consumer element. The user browses the node tree on each side, picks an element with at least one unconnected event slot, optionally names the connection, and clicks **Create**. The app determines the appropriate event ID (see Event ID Selection Rules below) and writes it to the selected slots. A new bowtie card appears on the canvas. The connection name is saved to a local YAML file.

**Why this priority**: This is the core value proposition — letting users create connections visually without ever needing to understand or manually enter event IDs. Without this, the Bowties tab remains read-only.

**Independent Test**: Can be tested by opening the Bowties tab, creating a new connection between two discovered nodes, verifying the bowtie card appears, verifying the event ID was written to both nodes' slots, and verifying the connection name persists in the YAML file after app restart.

**Acceptance Scenarios**:

1. **Given** one or more nodes are discovered and their config has been read, **When** the user clicks + New Connection and selects a producer element and a consumer element (which may be on the same node or different nodes), **Then** a bowtie card appears showing the producer on the left and the consumer on the right with the shared event ID written to the appropriate slots.
2. **Given** the user is in the New Connection dialog, **When** they type a connection name and click Create, **Then** the bowtie card displays that name and it is persisted to the local YAML file.
3. **Given** all event slots on a candidate element are connected (their event IDs are shared with other elements), **When** the user browses elements in the picker, **Then** that element appears grayed out with a "No free slots" indicator and cannot be selected.
4. **Given** a connection was just created, **When** the user switches to the Configuration tab and navigates to the producer or consumer element, **Then** the event slot shows "Used in: [connection name]" with a link back to the bowtie.

---

### User Story 2 - Bidirectional Sync and Unsaved Change Tracking (Priority: P1)

A user edits an event ID value directly in the Configuration tab (e.g., by pasting a new event ID into a slot). The Bowties tab automatically reflects the change — if the edit creates a new connection or breaks an existing one, the bowtie catalog updates accordingly. Conversely, when a user adds or removes an element from a bowtie, the corresponding event slot values in the Configuration tab update immediately.

All bowtie changes — creating connections, naming, renaming, adding/removing elements, tagging — are tracked as unsaved until the user explicitly saves. Unsaved bowtie changes show visual indicators (similar to the dirty-field indicators in the Configuration tab) so the user always knows what has changed. A single Save action writes both the node event ID values (to physical nodes) and the bowtie metadata (names, tags) to the layout file together.

**Why this priority**: Without bidirectional sync, users lose trust in the tool — stale bowtie data or config data that doesn't match reality is worse than having no bowties view at all. Unsaved-change tracking prevents silent data loss and gives users confidence about what will happen when they save.

**Independent Test**: Can be tested by editing an event ID in the Configuration tab and verifying the Bowties tab updates, then modifying a bowtie and verifying the Configuration tab reflects the change. Can also be tested by making several bowtie changes, verifying unsaved indicators appear, saving, and verifying indicators clear.

**Acceptance Scenarios**:

1. **Given** a bowtie exists linking Producer A to Consumer B, **When** the user changes Consumer B's event slot to a different event ID in the Configuration tab, **Then** the bowtie card updates to remove Consumer B from that connection (and the connection is deleted if Consumer B was the only consumer).
2. **Given** the user pastes a known event ID into a free slot on a new element in the Configuration tab, **When** that event ID already belongs to an existing bowtie, **Then** the new element appears in that bowtie's producer or consumer list automatically.
3. **Given** the user adds a consumer to a bowtie via the Bowties tab, **When** they switch to the Configuration tab, **Then** the consumer element's event slot shows the updated event ID value.
4. **Given** the user creates a new bowtie and names it, **When** they have not yet saved, **Then** the bowtie card shows an unsaved indicator (e.g., a dot or badge) and the app's title or toolbar reflects unsaved changes.
5. **Given** the user has unsaved bowtie changes, **When** they click Save, **Then** event ID values are written to participating nodes and bowtie metadata is written to the layout file in a single coordinated operation, and all unsaved indicators clear.
6. **Given** the user has unsaved bowtie changes, **When** they click Discard, **Then** all pending bowtie changes are reverted — node event slots return to their pre-edit values and metadata changes are undone.

---

### User Story 3 - Create a Connection Starting from a Config Element (Priority: P1)

A user is browsing a node's configuration and sees an event slot on a producer element. They right-click or use a context action to "Create Connection from Here." A dialog opens with the producer side pre-filled and the consumer side ready for selection. The user picks a consumer, names the connection, and creates the bowtie — all without leaving the config-first mental model.

**Why this priority**: Many users think config-first — they're looking at a button input and want to connect it to something. This mode meets users where they already are and provides an alternative entry point to the same bowtie creation flow.

**Independent Test**: Can be tested by right-clicking a producer event slot in the Configuration tab, selecting "Create Connection from Here," picking a consumer, and verifying the bowtie appears in both tabs.

**Acceptance Scenarios**:

1. **Given** a user is viewing a producer element with at least one event slot in the Configuration tab, **When** they invoke "Create Connection from Here," **Then** the New Connection dialog opens with the producer side pre-populated and the user only needs to select a consumer.
2. **Given** a user is viewing a consumer element, **When** they invoke "Create Connection from Here," **Then** the dialog opens with the consumer side pre-populated and the user only selects a producer.
3. **Given** the connection is created from the config-first flow, **When** the user navigates to the Bowties tab, **Then** the new bowtie card is visible.

---

### User Story 4 - Intent-First Bowtie Creation (Priority: P2)

A user wants to plan their layout logic before wiring physical connections. They click **+ New Connection** and immediately name the bowtie (e.g., "Yard Entry Signal") without selecting any elements yet. The bowtie card appears on the canvas in an "empty" or "planning" state. The user later adds producers and consumers to it. The bowtie adopts the event ID of the first element attached to it. When a second element is added, the bowtie's event ID is written to that element's slot.

**Why this priority**: This supports a design-first workflow where users sketch out their intended layout logic before committing to physical wiring. It's valuable but not essential for the core connection-making flow.

**Independent Test**: Can be tested by creating an empty named bowtie, verifying it persists, then adding a producer and consumer to it and verifying the event ID is assigned and written.

**Acceptance Scenarios**:

1. **Given** the user is on the Bowties tab, **When** they create a new bowtie with only a name and no element selections, **Then** a bowtie card appears in a "planning" state showing the name but no producers or consumers.
2. **Given** an empty bowtie exists, **When** the user adds a producer element, **Then** the bowtie adopts that element's current event ID as its identity (no write needed to the element's node).
3. **Given** a bowtie has one producer and no consumer, **When** the user adds a consumer element, **Then** the bowtie's event ID (adopted from the producer) is written to the consumer's slot.
4. **Given** an empty bowtie exists, **When** the app is closed and reopened, **Then** the named bowtie is still visible from the YAML file (though it has no live node references until nodes are rediscovered).

---

### User Story 5 - Add and Remove Elements from Existing Bowties (Priority: P1)

A user clicks **+ Add producer** or **+ Add consumer** on an existing bowtie card. An element picker opens showing only elements with at least one unconnected slot, filtered to the appropriate role. The user selects an element and the bowtie's existing event ID is written to the new element's slot. Conversely, a user can remove an element from a bowtie — the slot on the physical node is restored to its previous value. If the last element on one side is removed from a multi-element bowtie, the bowtie enters an "incomplete" state.

**Why this priority**: Adding and removing elements from existing bowties is essential for iterative connection building and is a core editing operation.

**Independent Test**: Can be tested by adding a second consumer to an existing bowtie and verifying both consumers share the same event ID, then removing one consumer and verifying its slot is cleared on the node.

**Acceptance Scenarios**:

1. **Given** a bowtie has one producer and one consumer, **When** the user clicks + Add consumer and selects a new element, **Then** the bowtie's event ID is written to the new consumer's first unconnected slot and the card shows two consumers.
2. **Given** a bowtie has two producers and one consumer, **When** the user removes one producer, **Then** the removed producer's event slot is restored to its previous value on the physical node and the bowtie card shows one producer.
3. **Given** a bowtie has one producer and one consumer, **When** the user removes the only consumer, **Then** the system prompts the user to either keep the bowtie (it reverts to a single-element state) or delete it entirely.
4. **Given** an element has no unconnected event slots, **When** the element picker is shown, **Then** that element appears grayed out and cannot be selected.

---

### User Story 6 - Name and Rename Bowties (Priority: P2)

A user can name a bowtie at creation time or rename it later by clicking a pencil icon next to the name on the bowtie card header. Clicking the icon makes the name editable inline. Names are free-form text stored in the local YAML file. The name appears in the bowtie card header, in "Used in:" references in the Configuration tab, and in search/filter results.

**Why this priority**: Naming makes bowties meaningful and navigable. Without names, users have to rely on element descriptions and event ID hex strings to identify connections.

**Independent Test**: Can be tested by creating a bowtie with a name, renaming it, and verifying the name updates in the bowtie card, the YAML file, and the "Used in:" references in the Configuration tab.

**Acceptance Scenarios**:

1. **Given** an unnamed bowtie, **When** the user clicks the pencil icon next to the name on the card header and types a name, **Then** the bowtie card header updates and the name is saved to the YAML file.
2. **Given** a named bowtie, **When** the user views the corresponding event slot in the Configuration tab, **Then** the "Used in:" reference shows the bowtie's name.
3. **Given** the user has named bowties, **When** they type in the filter bar on the Bowties tab, **Then** results match against bowtie names.

---

### User Story 7 - YAML Layout File Persistence (Priority: P1)

All bowtie display metadata — names, tags, and user-assigned labels — is stored in a user-managed YAML layout file. The user chooses where to save the file and what to name it using native OS file dialogs (Save As / Open). Each file represents a single layout. The YAML file is human-readable and editable outside the app. It uses the event ID as the stable key to associate metadata with live node data. When the user opens a layout file, it is loaded and merged with discovered node state to reconstruct the bowtie catalog.

**Why this priority**: Without persistence, all user-assigned names, tags, and organizational metadata is lost on restart. A user-managed file gives users control over where their layout data lives, enables sharing layout files between machines, and supports multiple layouts. YAML was chosen for human readability and consistency with existing profile files in the project.

**Independent Test**: Can be tested by creating named bowties, saving the layout file to a chosen location, closing the app, reopening, opening the saved file, and verifying all names and tags are restored.

**Acceptance Scenarios**:

1. **Given** the user has created bowties with names and tags, **When** they save the layout file using a native Save As dialog, **Then** a YAML file is written to the user's chosen location with their chosen filename.
2. **Given** a saved YAML layout file exists, **When** the user opens it via a native Open dialog, **Then** the metadata is loaded and matched to discovered nodes by event ID.
3. **Given** a bowtie referenced in the YAML file no longer has matching event IDs on any discovered node, **Then** the bowtie metadata is retained in the file but the bowtie card shows an "offline" or "unresolved" indicator.
4. **Given** the user edits the YAML file externally, **When** they reopen it in the app, **Then** the updated metadata is reflected in the UI.
5. **Given** the user has unsaved bowtie changes, **When** they attempt to close the app or open a different layout file, **Then** they are prompted to save the current layout first.
6. **Given** no layout file is currently open, **When** the user creates their first bowtie, **Then** they are prompted to save the layout file before (or immediately after) the first connection is created.

---

### User Story 8 - Classify Ambiguous Event Roles (Priority: P1)

A node without a profile has event slots whose role (producer vs. consumer) cannot be automatically determined. When the user encounters one of these ambiguous elements — either in the element picker, in the New Connection dialog, or on an existing bowtie card — the system prompts them to classify it as a producer or consumer. The user's classification is saved in the layout file so they are not asked again for that element on future sessions.

**Why this priority**: Many real-world nodes lack profiles. If the system cannot classify event roles, the element picker and bowtie cards cannot correctly separate producers from consumers, making connection creation unreliable. This is a prerequisite for Story 1 to work with all nodes.

**Independent Test**: Can be tested by discovering a node that has no profile, opening the New Connection dialog, verifying ambiguous elements prompt for role classification, classifying them, and verifying the classification persists after app restart.

**Acceptance Scenarios**:

1. **Given** a node has no profile and its event slots are classified as "ambiguous," **When** the user selects one of these elements in the New Connection dialog, **Then** the system prompts the user to classify it as a producer or consumer before allowing it to be placed on either side.
2. **Given** the user has classified an ambiguous element as a producer, **When** they view the element in the picker or on a bowtie card, **Then** it appears on the producer side and the classification is indicated visually.
3. **Given** the user has classified ambiguous elements, **When** they save the layout file and reopen it later, **Then** the classifications are restored from the layout file without re-prompting.
4. **Given** the user previously classified an element as a producer, **When** they realize it should be a consumer, **Then** they can re-classify it, which moves it to the consumer side of any bowtie it belongs to.
5. **Given** a node has a mix of known-role and ambiguous event slots, **When** the user browses it in the element picker, **Then** known-role elements appear normally on the correct side while ambiguous elements show a role indicator prompting classification.

---

### Edge Cases

- What happens when the user creates a bowtie and a participating node goes offline before the write completes? The write is blocked entirely — no partial writes are made, and the user sees an error indicating which node is offline.
- What happens when two bowties end up with the same event ID (e.g., due to manual editing in the config tab)? The system detects the conflict and surfaces it as a warning on the affected bowtie cards.
- What happens when the YAML file is corrupted or has syntax errors? The app loads with a warning that bowtie metadata could not be read, and operates in a degraded mode using only live node data (no names or tags).
- What happens when a user removes the last element from both sides of a bowtie? The system prompts the user to keep the bowtie (reverting it to a "planning" state) or delete it. If kept, it remains in the YAML file with its name and tags. If deleted, its entry is removed from the YAML file on save.
- What happens when the user creates an intent-first bowtie with a name but never adds elements, then closes and reopens the app? The empty named bowtie is preserved in the YAML file and displayed in a "planning" state.
- What happens when the user has unsaved bowtie changes and also unsaved config changes? Both are tracked together and saved or discarded as one operation.
- What happens when an event ID is shared across more nodes than expected (e.g., factory default IDs)? Since bowties are formed by shared event IDs, any shared ID forms a bowtie. If a spurious connection appears, the user can break it by removing the unwanted element from the bowtie.
- What happens when a node's local event number space is exhausted? Not applicable — the system reuses existing event IDs from unconnected slots rather than generating new ones.
- What happens when a node has no profile and all its event slots are ambiguous? Every event slot on that node appears as "ambiguous" in pickers and the existing bowtie catalog. The user is prompted to classify each slot's role when using it in a bowtie. The user's classification is persisted in the layout file so they are not asked again.
- What happens when the user classifies a role incorrectly and wants to change it? The user can re-classify an element's role from the bowtie card or from the Configuration tab, which moves the element from the producer side to the consumer side (or vice versa) of any bowtie it belongs to.

## Clarifications

### Session 2026-03-15

- Q: What is the undo granularity for bowtie operations — per-operation undo/redo, all-or-nothing discard, or checkpoint-based? → A: All-or-nothing discard only; users manually reverse individual mistakes (e.g., remove a wrong element and re-add the correct one). Per-operation undo is out of scope for v1.
- Q: What tag model should bowties use — free-form labels, predefined categories, or deferred? → A: Free-form text labels. The app auto-suggests from previously used tags in the current layout file.
- Q: What happens during multi-node writes if the second write fails after the first succeeds? → A: Sequential writes with rollback attempt. If the second node write fails, the app attempts to roll back the first write to its original value. If rollback also fails, the app shows an error with details of the inconsistent state for manual resolution.
- Q: What format should event IDs use as YAML keys? → A: Dotted hex format (e.g., `05.01.01.01.FF.00.00.01`), the canonical LCC/OpenLCB display format.
- Q: During Save, what is the order of operations and failure handling between node writes and YAML file save? → A: Write to nodes first, then save YAML. If the YAML save fails, node writes stand (already committed to hardware); the app retains unsaved metadata in memory and prompts the user to retry or Save As to a different location.

## Requirements *(mandatory)*

### Functional Requirements

#### Bowtie Creation

- **FR-001**: Users MUST be able to create a new bowtie connection by selecting one producer element and one consumer element from discovered nodes.
- **FR-002**: The system MUST determine the event ID for a new connection using the following rules, in order:
  1. If one side's selected element is already connected (its event ID is shared with other elements), use that event ID and write it to the other side's slot.
  2. If both sides' selected elements are already connected to different bowties, the user MUST choose which bowtie's event ID to use (or cancel). The unchosen side's slot is overwritten, which may affect its previous bowtie.
  3. If both sides' selected elements are unconnected (their event IDs are not shared with any other element), use the producer's current event ID and write it to the consumer's slot.
- **FR-003**: Users MUST be able to create a bowtie from the Bowties tab via a **+ New Connection** button.
- **FR-004**: Users MUST be able to create a bowtie starting from a config element via a context action ("Create Connection from Here") in the Configuration tab.
- **FR-005**: Users MUST be able to create an empty named bowtie (intent-first mode) that has no element attachments yet.
- **FR-006**: When adding the first element to an intent-first bowtie, the system MUST adopt that element's current event ID as the bowtie's identity (no node write needed). When a second element is added on the opposite side, the bowtie's adopted event ID MUST be written to the new element's slot.

#### Bowtie Editing

- **FR-007**: Users MUST be able to add additional producers or consumers to an existing bowtie.
- **FR-008**: When adding an element to an existing bowtie, the system MUST write the bowtie's existing event ID to the new element's first free slot.
- **FR-009**: Users MUST be able to remove a producer or consumer from a bowtie, which restores the event slot on the physical node to its previous value.
- **FR-010**: If removing an element leaves zero elements on one side of a multi-element bowtie, the bowtie MUST enter an "incomplete" state with a visual indicator.
- **FR-011**: If removing an element would leave a bowtie with zero producers and zero consumers, the system MUST prompt the user to either keep the bowtie (returning it to a "planning" state) or delete it. This preserves intent-first bowties when the user is changing their mind about which elements to use.
- **FR-012**: The element picker MUST only allow selection of elements that have at least one unconnected event slot; elements with no unconnected slots MUST appear grayed out.

#### Naming and Metadata

- **FR-013**: Users MUST be able to assign a free-form text name to any bowtie at creation time or later via a pencil icon on the bowtie card header that enables inline editing.
- **FR-014**: Bowtie names MUST appear in the card header, in "Used in:" references in the Configuration tab, and be searchable via the filter bar.
- **FR-015**: Users MUST be able to assign free-form text tags to bowties for grouping purposes. Tags are arbitrary strings; the app MUST auto-suggest from previously used tags in the current layout file.

#### Ambiguous Event Role Classification

- **FR-015a**: When an event slot's role cannot be automatically determined (no profile, protocol response is ambiguous), the system MUST prompt the user to classify it as producer or consumer before it can be used in a bowtie.
- **FR-015b**: The element picker and New Connection dialog MUST visually distinguish ambiguous elements from those with known roles (e.g., with a "?" badge or different styling).
- **FR-015c**: User-provided role classifications MUST be persisted in the layout file, keyed by node ID and element path, so users are not re-prompted on future sessions.
- **FR-015d**: Users MUST be able to re-classify an element's role after initial classification, which updates the element's placement (producer side vs. consumer side) on any bowtie it belongs to.
- **FR-015e**: When an ambiguous element appears on an existing bowtie card (discovered via matching event IDs), the card MUST show it in an "ambiguous" section until the user classifies its role.

#### Bidirectional Synchronization

- **FR-016**: When an event ID is changed in the Configuration tab, the bowtie catalog MUST update to reflect the change — adding, removing, or modifying bowtie memberships as appropriate.
- **FR-017**: When a bowtie is modified (element added or removed), the Configuration tab MUST reflect the updated event slot values immediately.
- **FR-018**: An event slot is considered "connected" when its event ID value is shared with at least one other event slot across all discovered nodes. An event slot whose event ID is unique (appears on only that one slot) is considered "unconnected."

#### Unsaved Change Tracking

- **FR-018a**: All bowtie changes (creation, naming, renaming, adding/removing elements, tagging, deletion) MUST be tracked as pending unsaved changes until explicitly saved.
- **FR-018b**: Unsaved bowtie changes MUST show visual indicators on the affected bowtie cards (e.g., a dot, badge, or highlight), consistent with the dirty-field indicators used in the Configuration tab.
- **FR-018c**: The app MUST provide a global unsaved-changes indicator (e.g., in the title bar or toolbar) when any bowtie or config changes are pending.
- **FR-018d**: A single Save action MUST write node event ID values to physical nodes first, then write bowtie metadata (names, tags) to the layout file. If node writes succeed but the YAML file save fails (e.g., disk full, file locked), the node writes stand and the app MUST retain unsaved metadata in memory and prompt the user to retry the file save or use Save As to write to a different location.
- **FR-018e**: Users MUST be able to discard all pending bowtie changes, reverting node event slots to their pre-edit values and undoing metadata changes. There is no per-operation undo/redo; users who need to reverse a single action (e.g., removing a wrong element) do so manually while other pending changes remain intact.
- **FR-018f**: Pending bowtie changes and pending config changes MUST share the same save/discard lifecycle — the user saves or discards everything together, not separately.

#### Persistence

- **FR-019**: Bowtie display metadata (names, tags) MUST be stored in a user-managed YAML layout file.
- **FR-020**: The YAML file MUST use the event ID in dotted hex format (e.g., `05.01.01.01.FF.00.00.01`) as the stable key to associate metadata with live node data.
- **FR-021**: Users MUST be able to save the layout file to a location and filename of their choosing via a native OS Save As dialog.
- **FR-022**: Users MUST be able to open a previously saved layout file via a native OS Open dialog.
- **FR-023**: When bowtie metadata changes (name, tag, creation, deletion), the system MUST track the layout as having unsaved changes.
- **FR-024**: Users MUST be prompted to save unsaved changes before closing the app, opening a different layout file, or creating a new layout.
- **FR-025**: The YAML file MUST be human-readable and editable outside the application.
- **FR-026**: If a layout file is corrupted or has syntax errors, the system MUST display a warning and allow the user to continue without metadata (degraded mode using only live node data).
- **FR-027**: The app MUST remember the most recently opened layout file path and offer to reopen it on startup.

#### Write Operations

- **FR-028**: All write operations (creating connections, adding elements, removing elements) MUST require all participating nodes to be online.
- **FR-029**: If any participating node is offline, the write operation MUST be blocked entirely with an error — no partial writes are permitted.
- **FR-029a**: For multi-node write operations, writes MUST be performed sequentially. If a subsequent write fails after an earlier write succeeded, the system MUST attempt to roll back the already-written node to its original value. If rollback also fails, the system MUST display an error detailing the inconsistent state (which nodes succeeded, which failed) so the user can manually resolve it.
- **FR-030**: Write operations MUST provide visual feedback: a spinner during write, green confirmation on success, and red error state on failure with a retry option.

#### New Connection Dialog

- **FR-031**: The New Connection dialog MUST present two panels — producer (left) and consumer (right) — each showing a browsable tree of nodes, segments, and elements.
- **FR-032**: Each panel MUST include search functionality to filter elements by name or path.
- **FR-033**: The dialog MUST show a selection preview card with the element's CDI path, key config values, and available slot count.
- **FR-034**: The **Create Connection** button MUST remain disabled until at least one element is selected on each side (unless creating an intent-first bowtie with name only).
- **FR-035**: When an ambiguous element is selected in the dialog, the system MUST prompt the user to classify it as producer or consumer before placing it on the corresponding side.

### Key Entities

- **Bowtie**: A named connection linking one or more producer elements to one or more consumer elements via a shared event ID. Has a name (optional, user-assigned), zero or more tags, and a state (active, incomplete, planning).
- **Event Slot**: A configured location on a physical LCC node that holds an event ID value. Each slot has an address, a role (producer/consumer/ambiguous), and a value (the event ID bytes). A slot is "connected" when its event ID is shared with at least one other slot; otherwise it is "unconnected."
- **Layout File**: A user-managed YAML file storing bowtie display metadata (names, tags) and user-provided event role classifications, keyed by event ID and node/element path respectively. The user chooses its name and location. Provides persistence across sessions independent of node state. Represents a single layout.
- **Element**: A configurable unit within a node's CDI (e.g., a line/port with event slots). Elements have a path within the CDI tree, a label, and one or more event slots.

## Assumptions

- The existing node discovery and CDI read functionality (Features 004, 006) is operational and provides the event slot data needed for bowtie construction.
- The pending edits infrastructure (Feature 007) handles the low-level write protocol for updating event slot values on nodes.
- Event IDs are 8-byte values. An event slot is considered "unconnected" when its event ID appears on no other slot across all discovered nodes, regardless of whether the value is a factory default or user-assigned.
- The YAML layout file is stored wherever the user chooses via native OS file dialogs; the app remembers the most recently opened file path.
- Users interact with one layout file at a time; opening a different file replaces the current layout.
- Multiple layout files can exist on disk, but only one is active in the app at a time.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can create a new producer-to-consumer connection in under 60 seconds using the New Connection dialog, without needing to know or type any event ID values.
- **SC-002**: When an event ID is modified in the Configuration tab, the Bowties tab reflects the change within 1 second, and vice versa.
- **SC-003**: 100% of bowtie names and tags survive an app restart when the user saves and reopens their layout file — no user-assigned metadata is lost.
- **SC-004**: Users can find a specific bowtie among 50+ connections in under 10 seconds using the name filter.
- **SC-005**: All write operations either complete fully on all participating nodes or fail with no partial writes — zero data inconsistency from interrupted operations.
- **SC-006**: The YAML layout file can be opened and understood by a user in a text editor without documentation.
- **SC-007**: Users can save and reopen a layout file using familiar OS-native file dialogs with no learning curve.
