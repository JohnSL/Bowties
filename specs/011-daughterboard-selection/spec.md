# Feature Specification: Connector Daughterboard Selection

**Feature Branch**: `011-daughterboard-selection`  
**Created**: 2026-04-27  
**Status**: Draft  
**Input**: User description: "I would like to extend the board profile feature so that it can support scenarios like with the Tower LCC where you have two 10-pin connectors to which you can attach a BOD4, BOD4-CP, BOD-8-SM, FOB-A, or FOB-C, etc. Each of these boards basically constrains the settings that are valid for a specific line that is connected to the board. Therefore, the idea is I would like to say that on this Tower LCC (or one of the other boards), what daughter boards, if any, I have connected to each of the two 10-pin connectors. Doing this would allow us to have a much simpler configuration interface because you could only select options that make sense for that type of connection."

## Clarifications

### Session 2026-04-29

- Q: How durable should per-node connector daughterboard selections be? → A: Persist with the saved node/layout/project state until changed.
- Q: How should Bowties handle existing settings that become incompatible after a connector daughterboard change? → A: Auto-stage compatible replacements or resets for affected fields and show those staged changes before apply.
- Q: What is the source of daughterboard compatibility and repair knowledge, and how should Bowties avoid a combination explosion across carrier boards and daughterboards? → A: Compatibility and repair rules come only from authored profiles, and carrier-board plus daughterboard rules compose independently per slot instead of enumerating full-board combinations.
- Q: How should daughterboard compatibility data be organized to support reuse across multiple carrier boards? → A: Define reusable daughterboard profile entries and let carrier-board slots reference them.
- Q: Should reusable daughterboard profiles support carrier-specific variations? → A: Reusable daughterboard profiles may define a shared base plus optional carrier-specific overrides.

## Initial Supported Hardware Scope

Initial delivery for this feature is scoped to hardware that is already concretely identified in the current Bowties workspace and source materials.

- **In-scope carrier boards**: RR-CirKits Tower-LCC, Signal LCC-32H, Signal LCC-S, and Signal LCC-P.
- **In-scope daughterboards/cards for initial delivery**: RR-CirKits aux-port I/O modules and companion boards that are described as compatible with Tower LCC or Signal LCC carrier boards, including BOD4, BOD4-CP, BOD-8-SM, FOB-A, FOB-C, BOB-S, OI-IB-8, OI-OB-8, RB-2, RB-4, SMD-8, SCSD-8, Isolator-8, MSS I/OAdapter, Sampled I/O Splitter Pair, and I/O Test-SM.
- **Deferred pending confirmation**: SPROG IO-LCC is a candidate carrier board if its two 10-pin connectors are confirmed to be electrically and behaviorally compatible with the RR-CirKits daughterboard model used by this feature.
- **Out of scope for this feature's initial delivery unless separately added during planning**: additional carrier boards from other manufacturers that are not yet backed by concrete profile/manual evidence for connector-based daughterboard selection behavior.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Choose Installed Daughterboards Per Connector (Priority: P1)

As an operator configuring a modular board such as Tower-LCC, I want to record which daughterboard is attached to each connector so Bowties can tailor the configuration interface to the actual hardware installed on that node.

**Why this priority**: Without connector-specific daughterboard selection, the UI must expose every possible option for every line, which makes supported boards harder to configure correctly and increases invalid combinations.

**Independent Test**: Connect or simulate a supported node type with two connector slots. Set connector A to one daughterboard type and connector B to another. Confirm the selected hardware is shown for each slot, saved for that node, and restored when the saved layout or project is reopened.

**Acceptance Scenarios**:

1. **Given** a supported board profile defines two connector slots and supported daughterboard types for each slot, **When** the user opens configuration for that node, **Then** the interface presents each connector slot with the allowed daughterboard choices plus an explicit "None installed" choice.
2. **Given** a user selects `BOD4-CP` for one connector and `FOB-A` for the other, **When** the selections are applied and the layout or project is later reopened, **Then** Bowties restores each connector choice independently for that node.

---

### User Story 2 - See Only Valid Line Options (Priority: P1)

As an operator, I want each line's available settings to reflect the installed daughterboard on its connector so I only see choices that are valid for that connection.

**Why this priority**: The main user value is a simpler, safer configuration interface that prevents choosing settings that do not make sense for the installed daughterboard.

**Independent Test**: For a supported node type, select a daughterboard that supports only a subset of line modes. Open a line controlled by that connector and verify that unsupported settings are not selectable while supported settings remain available.

**Acceptance Scenarios**:

1. **Given** a connector slot is assigned a daughterboard that limits valid line modes, **When** the user edits a line attached to that connector, **Then** Bowties shows only the settings, sections, and choices allowed for that daughterboard.
2. **Given** two connectors on the same node use different daughterboard types, **When** the user switches between lines served by those connectors, **Then** each line reflects the rules for its own connector rather than a node-wide average of all possibilities.
3. **Given** a connector slot is set to "None installed" and the profile does not author an empty-slot rule, **When** the user opens a line associated with that connector, **Then** Bowties applies no daughterboard-specific constraints and leaves the base carrier-board options available.
4. **Given** a connector slot is set to "None installed" and the profile explicitly authors an empty-slot rule, **When** the user opens a line associated with that connector, **Then** Bowties applies that authored hide, disable, or allow-subset behavior for the affected lines.

---

### User Story 3 - Resolve Incompatible Existing Settings (Priority: P2)

As an operator changing connector hardware assumptions, I want Bowties to stage compatible updates for settings that no longer fit the selected daughterboard so I do not have to manually repair the configuration before apply.

**Why this priority**: Existing configurations may predate the connector selection feature or may have been authored under different hardware assumptions. The system must reconcile those settings safely instead of silently allowing a misleading configuration or forcing the user to discover every required follow-up edit.

**Independent Test**: Start with a line configured using an option supported by one daughterboard type. Change the connector selection to a different daughterboard that does not support that option. Confirm Bowties stages compatible replacements or resets for every affected setting, shows those staged changes to the user, and prevents applying any newly incompatible values.

**Acceptance Scenarios**:

1. **Given** a line currently uses a setting that becomes invalid after a connector daughterboard change, **When** the new daughterboard selection is staged, **Then** Bowties automatically stages a compatible replacement or reset for each affected setting and shows those staged changes before apply.
2. **Given** Bowties has staged compatible replacements or resets after a connector daughterboard change, **When** the user reviews the line, **Then** the line presents only values that remain compatible with the selected hardware and no newly invalid choices can be staged.

---

### User Story 4 - Preserve Current Behavior For Non-Modular Boards (Priority: P3)

As an operator working with a board that does not use daughterboards, I want Bowties to behave exactly as it does today so this feature adds no extra complexity where it is not needed.

**Why this priority**: The feature should expand the profile system for modular boards without forcing extra setup or empty UI on nodes that have fixed hardware.

**Independent Test**: Open configuration for a node type whose profile does not define connector slots. Confirm no connector-selection UI is shown and the existing board-profile behavior remains unchanged.

**Acceptance Scenarios**:

1. **Given** a node type has no connector-slot metadata in its board profile, **When** the user opens that node's configuration, **Then** Bowties shows no daughterboard-selection controls and preserves the current configuration experience.

### Edge Cases

- A supported board profile defines multiple connector slots, and both slots use the same daughterboard type.
- A connector slot is left unpopulated, and the profile omits any empty-slot rule so governed lines retain the carrier board's base options.
- A connector slot is left unpopulated, and the profile explicitly authors an empty-slot rule for that slot.
- A previously saved configuration references line settings that are incompatible with the newly selected daughterboard.
- Changing one connector selection makes multiple fields across one or more affected lines incompatible, and Bowties must stage all required compatible follow-up changes together.
- A board profile no longer recognizes a previously selected daughterboard type because the profile was updated or replaced.
- A node type supports connector slots, but only some lines are affected by each slot while other lines remain governed by the base board profile.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST allow a board profile to declare zero or more named connector slots for a supported node type.
- **FR-002**: Each connector slot declaration MUST define the supported daughterboard types that a user may choose for that slot, including an explicit "None installed" state when the connector may be left empty.
- **FR-003**: Users MUST be able to select the installed daughterboard type independently for each declared connector slot on a node.
- **FR-003a**: The system MUST persist each connector slot's selected daughterboard type per node instance in saved layout or project state and restore those selections when that saved context is reopened.
- **FR-004**: The system MUST evaluate connector-slot daughterboard selections before presenting dependent configuration choices for affected lines or sections.
- **FR-005**: A board profile MUST be able to declare which lines, groups, or configuration sections are governed by each connector slot.
- **FR-006**: A board profile MUST be able to declare which settings, modes, or options are valid for each supported daughterboard type on a connector slot.
- **FR-007**: When a connector slot has an installed daughterboard selection, the system MUST show only the settings, modes, and options that are valid for that daughterboard for the affected lines or sections.
- **FR-008**: When a connector slot is set to "None installed" and the profile does not author `baseBehaviorWhenEmpty`, the system MUST apply no additional daughterboard-specific constraints for that slot.
- **FR-008a**: When a connector slot is set to "None installed" and the profile authors `baseBehaviorWhenEmpty`, the system MUST apply only that authored empty-slot behavior for the affected lines or sections.
- **FR-009**: When different connector slots on the same node have different daughterboard selections, the system MUST apply each slot's constraints independently so affected lines reflect only the rules for their own connector slot.
- **FR-010**: When a connector daughterboard selection makes an existing line setting invalid, the system MUST automatically stage a compatible replacement or reset for that setting before apply.
- **FR-011**: The system MUST show users the staged compatible replacements or resets caused by a connector daughterboard change before those changes are applied to the node.
- **FR-012**: After a connector daughterboard selection has been staged, the system MUST prevent users from staging any newly incompatible settings for affected lines.
- **FR-012a**: If one connector daughterboard change invalidates multiple settings across affected lines or sections, the system MUST stage all required compatible replacements or resets as part of the same pending change set.
- **FR-013**: If the system encounters a previously stored daughterboard selection that is not recognized by the active board profile, it MUST preserve the record as unknown, flag it for user attention, and avoid silently remapping it to another daughterboard type.
- **FR-014**: Node types whose board profiles do not declare connector slots MUST retain the current board-profile behavior with no daughterboard-selection controls or connector-specific constraints shown.
- **FR-015**: The feature MUST support the initial RR-CirKits carrier-board set for connector daughterboard selection: Tower-LCC, Signal LCC-32H, Signal LCC-S, and Signal LCC-P.
- **FR-015a**: The initial in-scope daughterboards/cards for those RR-CirKits carrier boards MUST include the aux-port modules and companion boards listed in this specification's initial supported hardware scope.
- **FR-016**: The feature MUST allow the same daughterboard type to be selected on more than one connector slot when the board profile permits that combination.
- **FR-017**: Connector-slot metadata and daughterboard constraint rules MUST be authored as part of the existing board profile capability rather than as a separate user-maintained lookup outside the profile system.
- **FR-018**: Daughterboard compatibility, invalid-option filtering, and automatic repair behavior MUST be derived only from authored profile data rather than hardcoded Bowties logic or any external runtime source.
- **FR-019**: The profile model MUST allow carrier-board connector-slot rules and daughterboard rules to compose independently per slot so supported hardware does not require enumerating every multi-slot carrier-board and daughterboard combination.
- **FR-020**: The profile model MUST support reusable daughterboard profile definitions that carrier-board connector slots reference instead of duplicating the same daughterboard compatibility data in each carrier-board profile.
- **FR-021**: Reusable daughterboard profiles MUST support an optional carrier-specific override layer so a shared daughterboard definition can adjust compatibility or repair behavior for specific carrier boards without duplicating the full daughterboard profile.
- **FR-022**: The profile model MUST allow one reusable daughterboard profile to be referenced by multiple RR-CirKits carrier-board families when those boards expose compatible aux-port connector behavior.

### Key Entities *(include if feature involves data)*

- **Connector Slot**: A named hardware attachment point defined by a board profile, such as one of Tower-LCC's two 10-pin connectors. It has its own supported daughterboard list and governs a defined subset of the node's configuration.
- **Daughterboard Type**: A selectable hardware option for a connector slot, such as BOD4, BOD4-CP, BOD-8-SM, FOB-A, FOB-C, BOB-S, OI-IB-8, OI-OB-8, RB-2, RB-4, SMD-8, SCSD-8, Isolator-8, MSS I/OAdapter, Sampled I/O Splitter Pair, I/O Test-SM, or none installed. Each type carries a set of valid configuration capabilities for the lines served by that connector.
- **Daughterboard Profile**: A reusable authored profile definition for one daughterboard type that describes its compatibility constraints and automatic repair behavior for connector slots that reference it.
- **Carrier Override Rule**: An optional profile-authored variation that refines a reusable daughterboard profile for one specific carrier board or connector-slot context.
- **Connector Constraint Rule**: A profile-defined rule that maps a connector slot and daughterboard type to the settings, sections, and choices that are valid or invalid for the affected lines.
- **Affected Line Group**: The collection of lines, groups, or sections whose available settings are controlled by one connector slot rather than only by the base node type.
- **Compatibility Warning**: A user-visible indication that an existing setting no longer matches the currently selected daughterboard for that connector slot.
- **Staged Compatibility Change**: A pending configuration edit automatically added by Bowties to keep affected lines compatible after a connector daughterboard selection changes.
- **Composed Slot Rule**: The effective compatibility rule for one connector slot produced by combining the carrier-board profile's slot mapping with the selected daughterboard profile's constraints for that slot.

## Dependencies

- Existing board profiles continue to identify the base node type and remain the source of truth for profile-driven configuration behavior.
- Supported modular boards provide enough profile metadata to name connector slots, enumerate allowed daughterboard types, and map affected lines or sections to each connector slot.
- Supported daughterboard and carrier-board profiles provide the only machine-readable source for compatibility filtering and automatic repair decisions.
- Supported carrier-board profiles can reference reusable daughterboard profile definitions for slot-specific compatibility behavior.
- Supported profiles can express carrier-specific override rules when a daughterboard behaves differently on different carrier boards.
- The RR-CirKits product descriptions for Tower LCC and Signal LCC families are sufficient to scope those boards as connector-based carrier boards for this feature.
- The current profile-driven configuration flow remains available so connector-specific constraints can narrow choices without replacing existing node-profile features.

## Assumptions

- Daughterboard selections are made per node instance, not as a global preference for all nodes of the same model.
- Daughterboard selections persist with the saved node, layout, or project context until the user changes them.
- Board profiles or existing field defaults provide enough information for Bowties to determine a compatible replacement or reset when a connector change invalidates an existing setting.
- Connector slots are named and ordered by the board profile so the UI can clearly distinguish which physical connector is being configured.
- Carrier-board and daughterboard constraints can be evaluated per connector slot without requiring cross-slot combination-specific authored profiles.
- A reusable daughterboard profile can be referenced by more than one supported carrier board.
- Most daughterboard behavior is expected to come from the shared reusable profile, with carrier-specific overrides used only when needed.
- SPROG IO-LCC remains outside the committed initial scope until its connector compatibility and daughterboard-selection behavior are confirmed from product documentation.
- Users choose from a profile-defined list of daughterboard types rather than typing arbitrary board names.
- This feature extends the existing board profile system and should compose with existing profile-driven relevance and naming behavior rather than replacing it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a supported modular board, users can identify and set the installed daughterboard for every connector slot in under 1 minute without consulting external documentation for available choices.
- **SC-002**: For supported daughterboard-equipped board profiles, 100% of settings presented for connector-governed lines are valid for the selected daughterboard type.
- **SC-003**: When a connector daughterboard selection invalidates existing settings, Bowties stages a compatible replacement or reset for every affected setting and shows those staged changes before apply.
- **SC-004**: Boards without connector-slot metadata show no additional daughterboard-selection UI and no loss of existing configuration capability.
- **SC-005**: A supported two-connector board can be configured with different daughterboard types on each connector in the same session without cross-applying invalid constraints between connectors.
- **SC-006**: Reopening a saved layout or project restores each node's previously selected connector daughterboard types without requiring re-entry.
- **SC-007**: Adding support for a new daughterboard on an existing supported carrier board requires authoring or updating profile data for the affected slot rules, not creating explicit profile variants for every multi-slot hardware combination.
- **SC-008**: Reusing the same daughterboard on an additional supported carrier board requires referencing its reusable daughterboard profile from that carrier board's slot definition rather than copying its compatibility rules into a new carrier-specific duplicate.
- **SC-009**: When a daughterboard needs carrier-specific behavior on one supported board, Bowties can express that difference through a targeted profile override without duplicating the daughterboard's shared base definition for every carrier board.
- **SC-010**: Initial delivery demonstrates the connector-selection workflow on at least one in-scope RR-CirKits Tower carrier board and at least one in-scope RR-CirKits Signal carrier board using listed aux-port daughterboards/cards.
- **SC-011**: Supporting an additional RR-CirKits carrier board from the initial set primarily requires carrier-profile authoring plus any carrier-specific overrides, not a redesign of the daughterboard profile model.
