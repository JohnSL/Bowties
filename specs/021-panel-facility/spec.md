# Feature Specification: Panel Facility

**Feature Branch**: `021-panel-facility`  
**Created**: 2026-11-25  
**Status**: Draft  
**Input**: User description: "Panel facility: replace the one-facility-per-LED Block Indicator with a Panel facility, one per physical panel or fascia. The Panel editor has two steps, Blocks then Wiring, with Done on Wiring. The Blocks step starts empty; the modeler adds detected blocks one at a time or all blocks of a signals facility or detector board, and orders them. The Wiring step is block-first: pick a block, then pick its lamp's board and pin on a pin grid labelled as the board labels them (custom panel PCBs wire pins in arbitrary order). No panel track diagram. "Lit when" is a panel setting (occupied/clear) with a per-block override. Done saves even with unwired blocks (panel Incomplete). Railroad tab card lists blocks in panel order with live lamp state. Existing Block Indicator facilities migrate into panels (one per lamp board)."

**Mockup**: [mockups.html](mockups.html). Sections 1–4 follow the modeler's path; sections 5–9 record the options considered and the decisions made while iterating.

## Context

Block occupancy lamps almost always sit on a panel or fascia. Today each lamp is its own Block Indicator facility (Spec 018): one occupancy channel, one lamp, one card. A layout with 24 lamps has 24 facilities, and nothing records that the lamps share a panel. Automatic Block Signals (Spec 020) showed that one facility can configure many things at once. This feature applies that to indicators: one **Panel** facility per physical panel, holding every block lamp on it.

Panel wiring follows the panel, not the block order. A modeler who builds a custom panel PCB routes pins to suit the board, so the pin for each block is chosen deliberately, the same way the signals editor's Wiring step chooses each signal's pins.

## Clarifications

### Session 2026-11-25

- Q: How should existing Block Indicator facilities become panels? → A: Converted automatically when the layout opens, one panel per lamp board, without asking first. The layout is marked as changed and the modeler is told what was converted. Block Indicator does not remain as a legacy facility type.
- Q: What should a panel do with a block whose detector goes away? → A: Keep the entry and its lamp pin, mark it "no longer detected", and count the panel as Incomplete. The modeler removes the entry or points it at another block, which keeps its lamp.
- Q: Does this feature include a per-lamp "Flash lamp" test? → A: No. It belongs to a general output-test feature for lamps and signal heads, and the button is removed from the mockup. Modelers check wiring by changing a block's occupancy.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Build a Panel and Wire Its Lamps (Priority: P1)

A modeler adds a Panel facility named for a physical panel ("Eagle Creek fascia"). The editor opens on the Blocks step, which is empty. The modeler adds the blocks whose lamps are on that panel, puts them in the order they want, and moves to Wiring. For each block they choose the board that drives the lamp and the pin it is wired to, picking from the board's pins as the board labels them. They press Done and save. The lamps then follow their blocks' occupancy.

**Why this priority**: This is the whole replacement for Block Indicator. Without it the feature delivers nothing.

**Independent Test**: On a layout with detected blocks and a Signal LCC with free LED drivers, create a panel with three blocks wired to non-sequential pins, save while connected, change each block's occupancy, and confirm each physical lamp follows its own block.

**Acceptance Scenarios**:

1. **Given** a named Panel facility was just added, **When** the editor opens, **Then** it shows the Blocks step with no blocks and explains how to add them.
2. **Given** the Blocks step, **When** the modeler adds blocks one at a time, **Then** each appears at the end of the panel's list, and a block already on the panel cannot be added again.
3. **Given** the Blocks step, **When** the modeler adds all blocks of an Automatic Block Signals facility or of a detector board, **Then** every such block not already on the panel is added, in that facility's travel order or that board's input order.
4. **Given** blocks on the panel, **When** the modeler moves a block up or down or removes it, **Then** the panel's order or contents change accordingly, and a removed block's lamp choice is discarded.
5. **Given** the Wiring step, **When** the modeler selects a block and then a free pin on a chosen board, **Then** that pin is the block's lamp, and the block's status changes from needing a lamp to wired.
6. **Given** a block wired to pin #9 and the next block wired to pin #3, **When** the panel is saved and written, **Then** each lamp lights for its own block, regardless of pin order.
7. **Given** "Lit when" is Occupied for the panel, **When** a wired block becomes occupied, **Then** its lamp lights, and it goes out when the block is clear.
8. **Given** one block overrides "Lit when" to Clear, **When** that block is clear, **Then** its lamp lights, while the other lamps keep the panel setting.

---

### User Story 2 - See the Panel on the Railroad Tab (Priority: P2)

After Done, the Railroad tab shows one card for the panel instead of one card per lamp. The card lists the panel's blocks in panel order, each with its lamp's pin and live lamp state, and says whether the panel is Wired or Incomplete.

**Why this priority**: It is how the modeler checks a panel at a glance and the visible payoff of grouping, but the panel works without it.

**Independent Test**: With a saved panel of five blocks, one without a lamp, open the Railroad tab, confirm the card shows five entries in panel order, an Incomplete status that counts one block needing a lamp, and lamp states that change as occupancy changes.

**Acceptance Scenarios**:

1. **Given** a saved panel with every block wired, **When** the Railroad tab is shown, **Then** the card shows Wired and lists every block with its pin and lamp state.
2. **Given** a saved panel where two blocks have no lamp, **When** the Railroad tab is shown, **Then** the card shows Incomplete with "2 need a lamp", and those entries say they need a lamp.
3. **Given** a lamp whose state has not been observed yet, **When** the card is shown, **Then** that lamp is shown as unknown rather than lit or unlit.
4. **Given** a saved panel, **When** the modeler chooses Edit on its card, **Then** the editor opens with both steps filled from the saved panel.

---

### User Story 3 - Edit and Remove a Panel (Priority: P2)

A modeler reopens a panel to add a newly detected block, rewire a lamp to a different pin, or remove a block. Later they remove the whole panel. Only the lamp configuration the panel set up is reset; blocks, detectors and other facilities are untouched.

**Why this priority**: Panels change as the layout grows. Without safe edit and removal, a mistake can only be fixed by hand in the Configuration tab.

**Independent Test**: Edit a saved panel to move one block's lamp from pin #6 to pin #12 and remove another block, save, and confirm pin #6 and the removed block's pin are reset and pin #12 follows the moved block. Then remove the panel and confirm all its lamp rows are reset while an Automatic Block Signals facility sharing its blocks is unaffected.

**Acceptance Scenarios**:

1. **Given** a saved panel, **When** the modeler moves a block's lamp to another pin and saves, **Then** the new pin follows the block and the old pin's lamp configuration is reset.
2. **Given** a saved panel, **When** the modeler removes a block and saves, **Then** that block's lamp configuration is reset and the block itself, its detector and any other facility using it are unchanged.
3. **Given** a saved panel, **When** the modeler chooses Remove, **Then** Bowties asks for confirmation, naming the lamp pins it will reset on each board, and after confirmation resets only those pins and removes the panel and its lamps.
4. **Given** the editor is open on an existing panel, **When** the modeler cancels or closes it, **Then** the saved panel is unchanged.

---

### User Story 4 - Keep Existing Block Indicators Working (Priority: P3)

A modeler opens a layout that has Block Indicator facilities. Bowties converts them into panels, one per lamp board, keeping every block, lamp and name. The lamps keep working, and nothing is written to the boards.

**Why this priority**: Only layouts that already use Block Indicator need it, but for them it is required. Block Indicator leaves Add facility in this feature.

**Independent Test**: Open a layout saved with four Block Indicators across two lamp boards plus one Block Indicator with no lamp, and confirm three panels with the expected blocks, pins and names, no pending changes to boards, and that the lamps still follow occupancy.

**Acceptance Scenarios**:

1. **Given** a layout with Block Indicator facilities whose lamps are on one board, **When** it is opened, **Then** they appear as one panel named after that board, with one block per former facility, in the order the facilities were listed.
2. **Given** Block Indicators with lamps on two boards, **When** the layout is opened, **Then** there are two panels, one per board.
3. **Given** a Block Indicator with a block but no lamp, **When** the layout is opened, **Then** its block is on a panel for indicators without a lamp, and that panel is Incomplete.
4. **Given** a converted layout, **When** it is opened, **Then** no board writes are pending because of the conversion, and each former facility name is kept as that lamp's name.
5. **Given** the Add facility dialog, **When** it is opened, **Then** it offers Panel and no longer offers Block Indicator.

---

### Edge Cases

- **Pin in use by Bowties.** A pin that a signal mast, another panel or another facility uses cannot be chosen, and the grid names its owner.
- **Pin in use outside Bowties.** A pin with Direct Lamp Control configuration that Bowties did not create is shown as used outside Bowties with its Lamp Description, and can be chosen only after the modeler releases it, as in the signals Wiring step.
- **Pin taken from another block on the same panel.** A pin another block on this panel already uses shows that block and cannot be chosen; the modeler clears it on that block first (the same rule as signals in an Automatic Block Signals edit; decided 2026-11-26).
- **Block lost.** If a block's detector disappears (its detector board is cleared or changed), the panel keeps the block's entry and lamp pin, marks it as no longer detected, and counts the panel as Incomplete. Nothing is reset on the board. The modeler removes the entry, or points it at another block, which keeps the lamp pin. If the same block is detected again, the entry recovers without changes.
- **Same block on two panels.** Allowed. Each panel owns its own lamp for that block.
- **Panel with no blocks.** Next and Done are available. A saved panel with no blocks is shown as Empty on the Railroad tab.
- **Offline.** Done and Save work offline; the board changes wait as Pending Changes and the card shows them as waiting per board, as for Automatic Block Signals.
- **Board without free pins.** The Wiring step lets the modeler choose another board or leave the block without a lamp.
- **Lamp board missing.** If a lamp's board is no longer in the layout, the block shows that its lamp's board is missing and the panel is Incomplete.

## Requirements *(mandatory)*

### Functional Requirements

**Creating and editing**

- **FR-001**: Add facility MUST offer **Panel** and MUST NOT offer Block Indicator. Adding a Panel requires a name and opens the Panel editor on the Blocks step.
- **FR-002**: The Panel editor MUST have two steps, **Blocks** and **Wiring**, with **Done** on Wiring. Each step MUST show its state in the step bar (number of blocks; number needing a lamp or all wired).
- **FR-003**: Nothing MUST be created or changed until Done. Cancel or close MUST discard the editor's work, for new and existing panels.
- **FR-004**: Done MUST be available even when some blocks have no lamp; the panel is then saved as Incomplete.

**Blocks step**

- **FR-005**: The Blocks step MUST start empty for a new panel and MUST explain how to add blocks.
- **FR-006**: The modeler MUST be able to add any detected block in the layout, one at a time, from a list grouped by detector board and searchable by name. Each entry MUST name any Automatic Block Signals facility the block belongs to.
- **FR-007**: The modeler MUST be able to add all blocks of a chosen Automatic Block Signals facility or a chosen detector board at once. Blocks already on the panel MUST be skipped. Blocks MUST be added in that facility's travel order or that board's input order.
- **FR-008**: Adding blocks from a signals facility or board MUST be a one-time selection. The panel MUST NOT follow later changes to that facility or board.
- **FR-009**: A block MUST appear at most once on a panel. The modeler MUST be able to reorder and remove blocks. Removing a block discards its lamp choice. The modeler MUST also be able to point an entry at a different block, which keeps the entry's lamp pin and position.
- **FR-010**: The panel's block order MUST be kept and MUST be the order used on the Wiring step and the Railroad tab card.
- **FR-011**: The editor MUST NOT draw a track diagram for the panel.

**Wiring step**

- **FR-012**: The Wiring step MUST list the panel's blocks in panel order, each with its lamp (board and pin) or a "needs a lamp" status.
- **FR-013**: Selecting a block MUST show a choice of lamp board among the layout's boards that can drive a panel lamp, and that board's pins grouped and labelled as the board labels them. The board choice MUST default to the board last chosen in this editor.
- **FR-014**: Each pin MUST show whether it is free, this block's, another block's on this panel, used by Bowties elsewhere (naming the owner), or used outside Bowties (showing its Lamp Description).
- **FR-015**: Choosing a free pin MUST make it the selected block's lamp. A pin another block on this panel uses MUST NOT be choosable until it is cleared on that block (decided 2026-11-26). Pins used by Bowties elsewhere MUST NOT be choosable. Pins used outside Bowties MUST be choosable only after the modeler releases them.
- **FR-016**: Choosing the selected block's own pin again MUST clear it, leaving the block needing a lamp, the same as choosing an already-selected pin in the signals Wiring step. There MUST NOT be a separate clear action.
- **FR-017**: The panel MUST have a **Lit when** setting, Occupied (default) or Clear, and each block MUST be able to override it. The Wiring step MUST show any override on the block's entry.
- **FR-018**: Each block's entry MUST name its detector board and any Automatic Block Signals facility it belongs to, as context.

**Effect on hardware**

- **FR-019**: For each wired block, Done MUST stage the lamp configuration so the lamp lights in the block's "lit when" state and goes out in the other. Both the lighting and the extinguishing event MUST always be set.
- **FR-020**: Each staged lamp MUST be named after its block by default, and that name MUST be the lamp's description on the board.
- **FR-021**: All board changes MUST be ordinary Pending Changes tagged with the panel and written by Save or the Sync Panel through the existing single writer. There is no separate apply action.
- **FR-022**: Editing a panel MUST reset the lamp configuration of any pin the panel stops using (block removed, lamp moved or cleared) and MUST leave every other pin, block, detector and facility unchanged.
- **FR-023**: Removing a panel MUST ask for confirmation, naming the pins it resets on each board, and then MUST reset only the pins the panel configured and remove the panel and its lamps. Blocks and detectors MUST remain.
- **FR-024**: Releasing a pin used outside Bowties MUST be recorded as part of the panel's changes, so it is written and saved together with them.

**Railroad tab**

- **FR-025**: Each panel MUST appear as one card on the Railroad tab showing its name, status (Empty, Incomplete with the number of blocks needing a lamp, or Wired), the boards its lamps are on, and Edit and Remove actions.
- **FR-026**: The card MUST list the panel's blocks in panel order, each with its pin (or "needs a lamp") and its lamp's live state: lit, unlit, or unknown when no state has been observed.
- **FR-027**: The card MUST show board changes that have not yet been written as waiting, per board, as for Automatic Block Signals.
- **FR-028**: The Channels panel MUST show every facility that uses a block's occupancy channel, for example both a panel and an Automatic Block Signals facility.

**Status**

- **FR-029**: A panel's status MUST be derived, never stored: Empty with no blocks, Incomplete if any block has no lamp, is no longer detected or has a missing lamp board, Wired otherwise.

**Migration**

- **FR-030**: When a layout with Block Indicator facilities is opened, Bowties MUST convert them into panels automatically, without asking first, and Block Indicator MUST NOT remain as a facility type: one panel per lamp board, named after that board, with blocks in the order the facilities were listed. Block Indicators with a block but no lamp MUST go into one separate panel. Block Indicators with no block MUST be dropped, because they configure nothing.
- **FR-031**: Conversion MUST keep each block's lamp, its "lit when" behavior and the former facility name as the lamp's name, and MUST NOT create board writes.
- **FR-032**: After conversion, the layout MUST be marked as changed so the converted form is written on the next save, and the modeler MUST be told how many Block Indicators became how many panels.

### Key Entities

- **Panel**: A named facility for one physical panel or fascia. Holds an ordered list of panel blocks and a panel-wide "lit when" setting. It owns its lamps and only refers to blocks. Its status is derived (FR-029).
- **Panel block**: One entry on a panel. Refers to one detected block (shared, not owned), has an optional lamp, and an optional "lit when" override.
- **Lamp**: A lamp-indicator channel the panel owns, bound to one Direct Lamp Control pin on one board, named after its block by default. Removed when its panel block loses it or the panel is removed.
- **Block**: An existing detected block, shared with Automatic Block Signals and other panels. A panel never creates or deletes blocks.
- **Claimed pin**: A Direct Lamp Control row on a board that a panel configured. Edit and Remove reset only claimed pins.

### Assumptions

- Lamp boards are Signal LCC boards using Direct Lamp Control, the only lamp style Bowties has today. Other boards are added later as further lamp styles; the Wiring step's board list and pin grid come from those styles.
- A panel's lamps may be on more than one board; a board choice per block covers both one-board and multi-board panels.
- A panel refers to blocks the way Automatic Block Signals does, so one block can be on several panels and in a signals facility at once. The rule that a channel is bound to at most one facility slot applied to Block Indicator's fixed slots and does not apply to panels.
- A per-lamp "Flash lamp" test is not part of this feature. It belongs to a general output-test feature, for lamps and signal heads, that restores configuration afterwards.
- The pin-first wiring view and the "fill the rest in order" helper are out of scope (mockup decisions D3 and D5).
- Turnout-position lamps, signal repeaters and panel push buttons are later row types and are out of scope. The panel's saved form should be able to hold other row types later without a migration.
- Read-back verification of written lamp configuration is out of scope, as it is for Automatic Block Signals.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A modeler can set up a panel of 12 block lamps on one board, wired to pins in any order, in under 5 minutes, with one facility instead of 12.
- **SC-002**: The Railroad tab shows one card per panel. A layout with 24 block lamps on two panels shows 2 panel cards, not 24.
- **SC-003**: After Save while connected, 100% of wired lamps follow their own block's occupancy with the panel's or block's "lit when" setting, including lamps wired to non-sequential pins.
- **SC-004**: Editing or removing a panel changes no pin, block, detector or facility outside that panel's claimed pins, verified by comparing board configuration before and after.
- **SC-005**: Every existing layout with Block Indicator facilities opens with all lamps still working, all lamp names kept, and no board writes caused by the conversion.
- **SC-006**: A modeler can tell from the Railroad tab alone whether every lamp on a panel is wired, and which blocks still need a lamp.
