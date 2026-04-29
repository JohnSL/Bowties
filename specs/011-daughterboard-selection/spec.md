# Feature Specification: Connector Daughterboard Selection

**Feature Branch**: `011-daughterboard-selection`  
**Created**: 2026-04-27  
**Status**: Draft  
**Input**: User description: "I would like to extend the board profile feature so that it can support scenarios like with the Tower LCC where you have two 10-pin connectors to which you can attach a BOD4, BOD4-CP, BOD-8-SM, FOB-A, or FOB-C, etc. Each of these boards basically constrains the settings that are valid for a specific line that is connected to the board. Therefore, the idea is I would like to say that on this Tower LCC (or one of the other boards), what daughter boards, if any, I have connected to each of the two 10-pin connectors. Doing this would allow us to have a much simpler configuration interface because you could only select options that make sense for that type of connection."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Choose Installed Daughterboards Per Connector (Priority: P1)

As an operator configuring a modular board such as Tower-LCC, I want to record which daughterboard is attached to each connector so Bowties can tailor the configuration interface to the actual hardware installed on that node.

**Why this priority**: Without connector-specific daughterboard selection, the UI must expose every possible option for every line, which makes supported boards harder to configure correctly and increases invalid combinations.

**Independent Test**: Connect or simulate a supported node type with two connector slots. Set connector A to one daughterboard type and connector B to another. Confirm the selected hardware is shown for each slot and persists for that node's configuration session.

**Acceptance Scenarios**:

1. **Given** a supported board profile defines two connector slots and supported daughterboard types for each slot, **When** the user opens configuration for that node, **Then** the interface presents each connector slot with the allowed daughterboard choices plus an explicit "None installed" choice.
2. **Given** a user selects `BOD4` for one connector and `FOB-C` for the other, **When** the selections are applied, **Then** Bowties records each connector choice independently and uses both choices in the same configuration session.

---

### User Story 2 - See Only Valid Line Options (Priority: P1)

As an operator, I want each line's available settings to reflect the installed daughterboard on its connector so I only see choices that are valid for that connection.

**Why this priority**: The main user value is a simpler, safer configuration interface that prevents choosing settings that do not make sense for the installed daughterboard.

**Independent Test**: For a supported node type, select a daughterboard that supports only a subset of line modes. Open a line controlled by that connector and verify that unsupported settings are not selectable while supported settings remain available.

**Acceptance Scenarios**:

1. **Given** a connector slot is assigned a daughterboard that limits valid line modes, **When** the user edits a line attached to that connector, **Then** Bowties shows only the settings, sections, and choices allowed for that daughterboard.
2. **Given** two connectors on the same node use different daughterboard types, **When** the user switches between lines served by those connectors, **Then** each line reflects the rules for its own connector rather than a node-wide average of all possibilities.
3. **Given** a connector slot is set to "None installed", **When** the user opens a line associated with that connector, **Then** Bowties hides or disables daughterboard-dependent options and shows only behavior valid for an unpopulated connector.

---

### User Story 3 - Resolve Incompatible Existing Settings (Priority: P2)

As an operator changing connector hardware assumptions, I want Bowties to identify settings that no longer fit the selected daughterboard so I can correct them before saving an invalid configuration.

**Why this priority**: Existing configurations may predate the connector selection feature or may have been authored under different hardware assumptions. The system must surface incompatibilities instead of silently allowing a misleading configuration.

**Independent Test**: Start with a line configured using an option supported by one daughterboard type. Change the connector selection to a different daughterboard that does not support that option. Confirm Bowties marks the affected setting as incompatible and prevents selecting additional invalid values until the line is brought back into a valid state.

**Acceptance Scenarios**:

1. **Given** a line currently uses a setting that becomes invalid after a connector daughterboard change, **When** the new daughterboard selection is applied, **Then** Bowties clearly identifies the affected setting as incompatible with the selected hardware.
2. **Given** incompatible settings exist after a connector daughterboard change, **When** the user updates those settings to a compatible state, **Then** the incompatibility indication is removed and the line again presents only valid choices.

---

### User Story 4 - Preserve Current Behavior For Non-Modular Boards (Priority: P3)

As an operator working with a board that does not use daughterboards, I want Bowties to behave exactly as it does today so this feature adds no extra complexity where it is not needed.

**Why this priority**: The feature should expand the profile system for modular boards without forcing extra setup or empty UI on nodes that have fixed hardware.

**Independent Test**: Open configuration for a node type whose profile does not define connector slots. Confirm no connector-selection UI is shown and the existing board-profile behavior remains unchanged.

**Acceptance Scenarios**:

1. **Given** a node type has no connector-slot metadata in its board profile, **When** the user opens that node's configuration, **Then** Bowties shows no daughterboard-selection controls and preserves the current configuration experience.

### Edge Cases

- A supported board profile defines multiple connector slots, and both slots use the same daughterboard type.
- A connector slot is left unpopulated, and its dependent lines must not expose options that require installed daughterboard hardware.
- A previously saved configuration references line settings that are incompatible with the newly selected daughterboard.
- A board profile no longer recognizes a previously selected daughterboard type because the profile was updated or replaced.
- A node type supports connector slots, but only some lines are affected by each slot while other lines remain governed by the base board profile.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST allow a board profile to declare zero or more named connector slots for a supported node type.
- **FR-002**: Each connector slot declaration MUST define the supported daughterboard types that a user may choose for that slot, including an explicit "None installed" state when the connector may be left empty.
- **FR-003**: Users MUST be able to select the installed daughterboard type independently for each declared connector slot on a node.
- **FR-004**: The system MUST evaluate connector-slot daughterboard selections before presenting dependent configuration choices for affected lines or sections.
- **FR-005**: A board profile MUST be able to declare which lines, groups, or configuration sections are governed by each connector slot.
- **FR-006**: A board profile MUST be able to declare which settings, modes, or options are valid for each supported daughterboard type on a connector slot.
- **FR-007**: When a connector slot has an installed daughterboard selection, the system MUST show only the settings, modes, and options that are valid for that daughterboard for the affected lines or sections.
- **FR-008**: When a connector slot is set to "None installed", the system MUST suppress or disable daughterboard-dependent settings for the affected lines and preserve only behavior that is valid with no daughterboard present.
- **FR-009**: When different connector slots on the same node have different daughterboard selections, the system MUST apply each slot's constraints independently so affected lines reflect only the rules for their own connector slot.
- **FR-010**: When a connector daughterboard selection makes an existing line setting invalid, the system MUST identify the incompatible setting to the user before that line is treated as valid again.
- **FR-011**: Once a line has been brought back into a state allowed by the selected daughterboard, the system MUST remove the incompatibility indication and continue presenting only compatible choices.
- **FR-012**: The system MUST prevent users from choosing newly incompatible settings for a line after the connector daughterboard selection has been applied.
- **FR-013**: If the system encounters a previously stored daughterboard selection that is not recognized by the active board profile, it MUST preserve the record as unknown, flag it for user attention, and avoid silently remapping it to another daughterboard type.
- **FR-014**: Node types whose board profiles do not declare connector slots MUST retain the current board-profile behavior with no daughterboard-selection controls or connector-specific constraints shown.
- **FR-015**: The feature MUST support Tower-LCC style boards with two independent 10-pin connector slots whose allowed daughterboard choices can include boards such as BOD4, BOD4-CP, BOD-8-SM, FOB-A, and FOB-C.
- **FR-016**: The feature MUST allow the same daughterboard type to be selected on more than one connector slot when the board profile permits that combination.
- **FR-017**: Connector-slot metadata and daughterboard constraint rules MUST be authored as part of the existing board profile capability rather than as a separate user-maintained lookup outside the profile system.

### Key Entities *(include if feature involves data)*

- **Connector Slot**: A named hardware attachment point defined by a board profile, such as one of Tower-LCC's two 10-pin connectors. It has its own supported daughterboard list and governs a defined subset of the node's configuration.
- **Daughterboard Type**: A selectable hardware option for a connector slot, such as BOD4, BOD4-CP, BOD-8-SM, FOB-A, FOB-C, or none installed. Each type carries a set of valid configuration capabilities for the lines served by that connector.
- **Connector Constraint Rule**: A profile-defined rule that maps a connector slot and daughterboard type to the settings, sections, and choices that are valid or invalid for the affected lines.
- **Affected Line Group**: The collection of lines, groups, or sections whose available settings are controlled by one connector slot rather than only by the base node type.
- **Compatibility Warning**: A user-visible indication that an existing setting no longer matches the currently selected daughterboard for that connector slot.

## Dependencies

- Existing board profiles continue to identify the base node type and remain the source of truth for profile-driven configuration behavior.
- Supported modular boards provide enough profile metadata to name connector slots, enumerate allowed daughterboard types, and map affected lines or sections to each connector slot.
- The current profile-driven configuration flow remains available so connector-specific constraints can narrow choices without replacing existing node-profile features.

## Assumptions

- Daughterboard selections are made per node instance, not as a global preference for all nodes of the same model.
- Connector slots are named and ordered by the board profile so the UI can clearly distinguish which physical connector is being configured.
- Users choose from a profile-defined list of daughterboard types rather than typing arbitrary board names.
- This feature extends the existing board profile system and should compose with existing profile-driven relevance and naming behavior rather than replacing it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a supported modular board, users can identify and set the installed daughterboard for every connector slot in under 1 minute without consulting external documentation for available choices.
- **SC-002**: For supported daughterboard-equipped board profiles, 100% of settings presented for connector-governed lines are valid for the selected daughterboard type.
- **SC-003**: When a connector daughterboard selection invalidates existing settings, Bowties identifies every affected setting before the user leaves that line in a valid state again.
- **SC-004**: Boards without connector-slot metadata show no additional daughterboard-selection UI and no loss of existing configuration capability.
- **SC-005**: A supported two-connector board can be configured with different daughterboard types on each connector in the same session without cross-applying invalid constraints between connectors.
