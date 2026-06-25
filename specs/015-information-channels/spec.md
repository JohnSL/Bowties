# Feature Specification: Information Channels — Auto-Create & Inventory

**Feature Branch**: `015-information-channels`  
**Created**: 2026-06-24  
**Status**: Complete (all 6 slices delivered)  
**Input**: Auto-create typed information channels from hardware selection (BOD-family daughter boards) and display them in a new Railroad tab as a channel inventory.

**Deferred**: US1-Acceptance-3 (hardware reference navigation to Config tab) deferred to backlog item "Channel hardware references as navigable hyperlinks (ADR-0003 display-reference rule)".

## Context

This feature introduces the **information channel** abstraction — a typed, named representation of a single piece of layout-meaningful information (e.g., "Block 7 Occupancy") independent of protocol details. Channels are the foundational data layer for the broader UX vision described in `specs/proposals/app-ux-vision.md`.

This first feature is deliberately narrow: channels are auto-created from Tier 1 hardware selection (BOD-family daughter boards — BOD4, BOD4-CP, BOD-8, BOD-8-SM — where all pins have a single fixed type), displayed in a new Railroad tab, and renamable. No behavior, no wiring between channels, no facilities, no templates. The scope is additive — nothing in the existing Config or Bowties tabs changes.

## Clarifications

### Session 2026-06-24

- Q: The spec contains duplicate Key Entities and Success Criteria sections — one matching the narrow scope (BOD-8 auto-create, Railroad tab, rename) and one referencing the broader vision (behavior templates, facilities, logic resources). Which scope does this feature target? → A: Remove broader-vision sections; keep only the narrow-scope Key Entities and Success Criteria (SC-001–SC-004).
- Q: Where should channel data live within the layout folder (which is folder-based, not a single file)? → A: New `channels.yaml` file in the layout folder root, following the one-file-per-concern pattern alongside `bowties.yaml`, `manifest.yaml`, etc.
- Q: When are channels created relative to hardware sync? → A: Channels are layout-level abstractions never written to nodes. They are created in-memory on daughter board selection and persisted on layout save, following the same layered storage pattern as all other layout mutations.
- Q: What format should channel IDs use? → A: UUID v4 — globally unique without coordination, consistent with existing ID patterns (e.g., connectionId) in the layout.
- Q: Where should the Railroad tab appear in the tab bar? → A: Added as the last (rightmost) tab, after Config and Bowties.
- Q: Should the Railroad tab display live channel state (occupied/clear) when connected? → A: No — inventory only (name, type, hardware reference). Live state display is a future feature.
- Q: Should the default channel name include the node name to disambiguate across multiple nodes? → A: Yes — include the node’s user-assigned name (e.g., “West Yard — Connector A — Input 1”).
- Q: What happens to channels when their backing node is removed from the layout? → A: Out of scope — there is no node removal capability today. Defer to future feature if/when node removal is added.
- Q: What format should the hardware reference use for connector and input identification? → A: Follow existing codebase convention: connector as lowercase-letter slug (`connector-a`, `connector-b`), input as 1-based ordinal. This is the single consistent pattern across profiles, backend, and frontend.
- Q: Should this feature support auto-creating channels from all BOD variants or only BOD-8? → A: All BOD variants (BOD4, BOD4-CP, BOD-8, BOD-8-SM) — same auto-creation logic, different pin counts. All are block-occupancy detectors with fixed pin types.
- Q: What happens when the same BOD daughter board is re-selected after removal? → A: Fresh channels — new UUIDs and default names. Re-selection is a new intent; no attempt to restore previous channels.
- Q: Should default channel names auto-update when the backing node is renamed? → A: No — channel names are set at creation and independently user-editable. The hardware reference (which uses node ID, not name) always resolves to the current node; the displayed node name in the reference updates, but the channel’s own name does not.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Auto-Create Channels from BOD Daughter Board Selection (Priority: P1)

A layout owner selects a BOD-family daughter board (BOD4, BOD4-CP, BOD-8, or BOD-8-SM) for a TowerLCC connector in the existing Config tab. The system automatically creates block-occupancy information channels (count matching the board’s pin count) with default sequential names. These channels appear immediately in the new Railroad tab.

**Why this priority**: Channels are the foundational abstraction. Without auto-creation from hardware selection, there is nothing to display, rename, or persist. This is the minimum slice that proves the concept end-to-end.

**Independent Test**: Can be tested by selecting a BOD-family daughter board in the Config tab and switching to the Railroad tab to verify the correct number of occupancy channels appear with default names and correct hardware references.

**Acceptance Scenarios**:

1. **Given** a TowerLCC node with an unconfigured connector, **When** the user selects a BOD-family daughter board for that connector, **Then** the system creates block-occupancy channels (count matching the board’s pin count) with default names that include the node name, connector, and input number (e.g., “West Yard — Connector A — Input 1” through “West Yard — Connector A — Input 8” for BOD-8).
2. **Given** channels have been auto-created, **When** the user switches to the Railroad tab, **Then** all 8 channels are visible in a channel inventory grouped by type, each showing its backing hardware reference (node name + connector + input number).
3. **Given** the user is on the Railroad tab viewing channels, **When** they click a channel's hardware reference, **Then** the view navigates to that node/connector in the Config tab.

---

### User Story 2 — Rename Channels (Priority: P1)

A layout owner renames auto-created channels to match their physical layout (e.g., "Mainline Block 7", "Eagle Creek East Approach"). The names persist across sessions.

**Why this priority**: Default sequential names have no layout meaning. Naming is what transforms a generic input into a comprehensible piece of layout infrastructure. Without persistence, naming has no value.

**Independent Test**: Can be tested by renaming a channel in the Railroad tab, closing and reopening the layout, and verifying the new name is retained.

**Acceptance Scenarios**:

1. **Given** an auto-created channel with a default name, **When** the user edits the name inline in the Railroad tab to "Mainline Block 7 — Occupancy", **Then** the channel displays the new name immediately.
2. **Given** a renamed channel, **When** the user closes the layout and reopens it, **Then** the channel retains its user-assigned name.
3. **Given** a renamed channel, **When** the user views it in the Railroad tab, **Then** both the user-assigned name and the backing hardware reference are visible.

---

### User Story 3 — Railroad Tab as Channel Inventory (Priority: P1)

A layout owner navigates to the new Railroad tab to see all information channels across their layout in one place. Channels are grouped by type. The inventory shows channel name, type, and backing hardware.

**Why this priority**: The Railroad tab is the visible home for channels — without it, the channel data model has no user-facing surface. This is co-required with Story 1 to produce a testable slice.

**Independent Test**: Can be tested by creating channels on multiple nodes (if multiple TowerLCC nodes with BOD-family boards are present) and verifying they all appear in the Railroad tab grouped under “Block Occupancy.”

**Acceptance Scenarios**:

1. **Given** no channels exist in the layout, **When** the user opens the Railroad tab, **Then** it shows an empty state with guidance (e.g., "No channels yet. Select a daughter board in the Config tab to create channels.").
2. **Given** channels exist across multiple nodes, **When** the user opens the Railroad tab, **Then** all channels are visible, grouped by type, with a count per group.
3. **Given** the Railroad tab is open, **When** new channels are auto-created (user selects a daughter board in Config), **Then** the Railroad tab updates to show the new channels without requiring a manual refresh.

---

### User Story 4 — Remove Channels When Daughter Board Changes (Priority: P2)

A layout owner changes a connector’s daughter board away from a BOD-family board (to a different daughter board type or back to unconfigured). The system warns about affected channels and removes them upon confirmation.

**Why this priority**: Without cleanup on hardware change, orphaned channels would accumulate and confuse the inventory. This is the natural complement to auto-creation but can be deferred behind the core create/display/rename flow.

**Independent Test**: Can be tested by selecting a BOD-family board, verifying channels appear, then changing the daughter board and verifying the warning and channel removal.

**Acceptance Scenarios**:

1. **Given** a connector with a BOD-family board selected and auto-created channels, **When** the user changes the daughter board to a different type, **Then** the system warns that the channels will be removed.
2. **Given** the warning is displayed, **When** the user confirms, **Then** the channels are removed from the inventory and from persistence.
3. **Given** the warning is displayed, **When** the user cancels, **Then** the daughter board selection is reverted and channels remain.

---

### Edge Cases

- What happens when a layout is opened that was saved before this feature existed (no channel data)? The Railroad tab shows empty — channels are not retroactively inferred from existing daughter board selections. (This avoids guessing at names the user never assigned.)
- What happens when a node with auto-created channels goes offline? Channels remain visible in the inventory with an "offline" indicator — they represent layout intent, not live state.
- What happens if the user renames a channel to an empty string? The system rejects empty names and retains the previous name.
- What happens if two channels are given the same name? The system allows it — names are user-facing labels, not unique keys. The hardware reference distinguishes them.
- Does the Railroad tab show live occupancy state (occupied/clear)? No — this feature scope covers the channel inventory only (name, type, hardware reference). Live state display is a future feature.
- What happens if a node with channels is removed from the layout? Out of scope — there is no node removal capability today. Channel cleanup for node removal will be addressed if/when that feature is added.
- What happens if a BOD daughter board is removed and then re-selected on the same connector? Fresh channels are created with new UUIDs and default names. The system does not attempt to restore previously removed channels.
- What happens if the backing node is renamed after channels were created? The channel’s own name does not change — it is independently editable. The hardware reference uses the node ID (not the name), so the displayed node name in the reference resolves to the current name.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST auto-create typed information channels when a BOD-family daughter board (BOD4, BOD4-CP, BOD-8, BOD-8-SM) is selected for a TowerLCC connector. The number of channels matches the board’s pin count.
- **FR-002**: System MUST assign each auto-created channel a default name that includes the node’s user-assigned name, the connector identifier, and the input number (e.g., “West Yard — Connector A — Input 1”).
- **FR-003**: System MUST assign each auto-created channel the "block-occupancy" type.
- **FR-004**: System MUST persist channels (name, type, hardware reference) in a `channels.yaml` file in the layout folder root, following the existing one-file-per-concern pattern.
- **FR-005**: System MUST provide a Railroad tab as the last (rightmost) tab after Config and Bowties that displays the channel inventory.
- **FR-006**: The channel inventory MUST group channels by type and show a count per group.
- **FR-007**: Each channel in the inventory MUST display its name, type, and backing hardware reference (node + connector + input).
- **FR-008**: Users MUST be able to rename channels inline in the Railroad tab.
- **FR-009**: System MUST remove auto-created channels when their backing daughter board selection is changed, after displaying a confirmation warning.
- **FR-010**: The Railroad tab MUST show an empty state with guidance when no channels exist.
- **FR-011**: Channel data MUST be additive to the existing layout folder — existing layout files are not modified or removed by channel operations.
- **FR-012**: The existing Config and Bowties tabs MUST remain unchanged in behavior.

### Key Entities

- **Information Channel**: A typed, named representation of a single piece of layout-meaningful information. Key attributes: unique ID (UUID v4), user-assigned name, channel type, hardware reference (node ID + connector slug e.g. `connector-a` + 1-based input ordinal).
- **Channel Type**: A well-known classification (initially only "block-occupancy") that defines what kind of information the channel carries and its possible states (occupied / clear).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can go from "unconfigured connector" to "8 named occupancy channels visible in Railroad tab" in under 2 minutes (select daughter board + rename).
- **SC-002**: Channel names persist with 100% fidelity across layout close/reopen cycles.
- **SC-003**: The Railroad tab accurately reflects the current channel state — no stale or missing channels after daughter board changes.
- **SC-004**: Zero regressions in existing Config or Bowties tab behavior (existing test suites pass without modification).


