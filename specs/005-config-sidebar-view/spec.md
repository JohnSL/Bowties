# Feature Specification: Configuration Tab — Sidebar and Element Card Deck

**Feature Branch**: `005-config-sidebar-view`  
**Created**: February 22, 2026  
**Status**: Draft  
**Input**: User description: "Converting the Miller Columns into the sidebar plus element card deck view defined in the Configuration tab design"

## Clarifications

### Session 2026-02-22

- Q: How does the user enter a new value before clicking [W]? → A: No field editing in this iteration; [W] action is deferred to a follow-on feature; fields are read-only
- Q: What CDI structure level maps to one card? → A: One card per top-level CDI group within the selected segment (e.g., each "Line N" group); leaf fields appear inside the card body
- Q: What qualifies as "Advanced Settings" / when are sub-groups collapsed? → A: No collapse in this iteration; all CDI sub-groups within a card render inline and fully expanded
- Q: What happens to sidebar state when the user triggers a node refresh? → A: Clear all sidebar state (expanded nodes, selected segment, card deck) and reset to initial empty/loading state
- Q: How should replicated group instances be labeled in card headers? → A: GroupName N format without # (e.g., "Line 3"); if user assigned a name, show it first (e.g., "Yard Button (Line 3)"); if unnamed, append "(unnamed)"

## Overview

The Configuration tab currently uses a Miller Columns layout — a row of horizontally-scrolling columns where clicking nodes, segments, groups, and elements drills deeper to the right, with a details panel at the far right. This feature replaces that layout with the two-panel design described in the MVP design document:

- **Left sidebar**: Collapsible node list with segment names as navigation items
- **Main content area**: All elements for the selected segment displayed as an accordion card deck, each card showing configuration fields inline

The new layout is designed for real configuration work — reading and editing fields — rather than exploration. It surfaces all elements at once and lets users work down a segment without drilling back and forth.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Browse Nodes and Segments via the Sidebar (Priority: P1)

A user opens the Configuration tab after nodes have been discovered. They see a sidebar listing all nodes, can expand any node to see its segments, and click a segment to load its content into the main area.

**Why this priority**: This is the foundation of the new layout. Without a working sidebar, no other part of the feature is reachable. It is also the simplest slice to test independently.

**Independent Test**: Can be fully tested by verifying that discovered nodes appear in the sidebar, that clicking a node expands it to show segments, and that clicking a segment updates the main area header to reflect the selection — even before any element cards are rendered.

**Acceptance Scenarios**:

1. **Given** nodes have been discovered and their CDI has been loaded, **When** the user opens the Configuration tab, **Then** the sidebar lists all discovered node names, each collapsed by default
2. **Given** a collapsed node in the sidebar, **When** the user clicks it, **Then** the node expands to show its CDI segment names (e.g., "Identification", "Port I/O", "Settings") as clickable items
3. **Given** an expanded node with segments listed, **When** the user clicks a segment name, **Then** that segment is highlighted as selected in the sidebar
4. **Given** a selected segment, **When** the user clicks a different node to expand it, **Then** the previously expanded node can remain expanded and the user can select a segment from either node without losing context
5. **Given** no nodes have been discovered, **When** the user opens the Configuration tab, **Then** the sidebar shows an empty state ("No nodes discovered — use Discover Nodes to scan the network") and the main area shows a corresponding prompt

---

### User Story 2 - Inspect Element Configuration Values in the Card Deck (Priority: P2)

A user selects a segment in the sidebar and sees all of its elements displayed as accordion cards in the main area. Each card they expand shows the element's current configuration values, with the option to refresh or write values.

**Why this priority**: This is the core configuration workflow. Once the sidebar is working, this delivers the primary value: seeing what is configured on a node. It builds directly on Story 1.

**Independent Test**: Can be tested by selecting a segment that has at least one named element, expanding an element card, and verifying that the displayed field values match what was read from the node during the preceding refresh.

**Acceptance Scenarios**:

1. **Given** a segment is selected in the sidebar, **When** the main area loads, **Then** one accordion card per element in that segment is shown; named elements display their user-given name in the card header (e.g., "Yard Button (Line 3)"), unnamed elements show their CDI path with an "(unnamed)" suffix (e.g., "Line 4 (unnamed)")
2. **Given** an element card is collapsed, **When** the user clicks the card header, **Then** the card expands to show the element's configuration fields (e.g., name, function type) and their current values
3. **Given** an expanded element card showing configuration fields, **When** the user clicks the **[R]** (Refresh) action next to a field, **Then** the current value is re-read from the node and the display updates
4. **Given** an element card is expanded, **When** the card body renders, **Then** all CDI sub-groups within the card (e.g., "Advanced Settings", "Delay") are displayed inline and fully expanded; there are no collapsible disclosure toggles within a card in this iteration
5. **Given** an element card showing an event slot, **When** the slot has a value set by the user, **Then** the raw event ID is shown as a secondary detail alongside the slot
6. **Given** CDI description text exists for a field (long explanatory text from the CDI), **When** the card is expanded, **Then** that text is hidden by default and accessible via a small "?" expander next to the field label

---

### Edge Cases

- What happens when a node becomes unreachable after the sidebar has already loaded it? The node's name remains in the sidebar but is shown with an offline indicator; its segments remain listed; attempting to expand an element card for an offline node's elements shows an error state for any field that requires a live read, while previously cached values are still shown
- What happens when a segment contains a very large number of elements (50+)? The card deck renders all elements; cards are collapsed by default so the segment loads quickly; the user scrolls to find the element they want
- What happens when an element has no fields (e.g., a pure group container)? The card is still shown in the deck but expands to display a "(no configurable fields)" message
- What happens when CDI has not yet been loaded for a node? Clicking the node in the sidebar shows a loading indicator; if CDI cannot be loaded an error message replaces the segment list for that node
- What happens when the user triggers a node refresh while a segment is selected and cards are displayed? The sidebar clears entirely and the main area returns to the empty/loading state; no attempt is made to preserve or restore the previous selection
- What happens when two nodes share the same user-given name? Both are shown in the sidebar; the secondary detail (node ID or manufacturer/model) is displayed beneath the name to distinguish them

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Configuration tab layout MUST consist of a fixed-width left sidebar and a scrollable main content area
- **FR-002**: The sidebar MUST list all discovered nodes; each node entry MUST be collapsible/expandable
- **FR-003**: When expanded, a node entry MUST show all CDI segment names for that node as individually selectable items; no deeper hierarchy is shown in the sidebar
- **FR-004**: Segment names in the sidebar MUST use the text from the CDI (e.g., "Port I/O", "Identification"); user renaming of segments is not supported
- **FR-005**: Selecting a segment MUST load the element card deck in the main area, replacing any previously shown content
- **FR-006**: The main content area MUST display one accordion card per top-level CDI group within the selected segment (e.g., each "Line N" group under "Port I/O" is one card); leaf-level fields within that group appear in the card body
- **FR-007**: A card header MUST follow this naming convention:
  - For non-replicated groups: user-given name (if any) followed by the CDI group name in parentheses (e.g., "Yard Button (Port I/O)")
  - For replicated group instances: user-given name (if any) followed by the group name and instance number without a `#` separator (e.g., "Yard Button (Line 3)"; if unnamed, "Line 3 (unnamed)")
  - When no user-given name exists: CDI positional label with "(unnamed)" suffix (e.g., "Line 4 (unnamed)")
- **FR-008**: All element cards MUST be collapsed by default when a segment is first selected; expanding a card reveals its configuration fields and current values
- **FR-009**: Each configuration field MUST show a **[R]** (Refresh) action that re-reads the single field value from the node on demand
- **FR-010**: Configuration fields are read-only in this iteration; no **[W]** (Write) action is exposed; field editing is deferred to a follow-on feature
- **FR-011**: All CDI sub-groups within a card (e.g., "Advanced Settings", "Delay") MUST be rendered inline and fully expanded; collapsible disclosure toggles within cards are not required in this iteration
- **FR-012**: CDI description text for any field MUST be accessible via a "?" expander next to the field label; it MUST NOT be shown by default
- **FR-013**: The event slots section of an element card MUST display each slot's raw event ID as a secondary detail
- **FR-014**: Event slots that hold a default/unset value MUST be labeled "(free)" rather than showing a raw default ID
- **FR-015**: The sidebar MUST preserve expansion state across segment selections within the same session (expanding a node does not collapse when the user selects a different segment)
- **FR-016**: The existing node context-menu actions (View CDI XML, Download CDI from node) MUST remain accessible from the sidebar
- **FR-017**: All previously implemented functionality for reading and caching configuration values (from feature 004) MUST continue to work unchanged under the new layout
- **FR-018**: When a node refresh operation is initiated (Discover Nodes or Refresh Nodes), the sidebar MUST clear all state — collapsed nodes, selected segment, and loaded card deck — and return to its initial empty/loading state

### Key Entities

- **Node Entry**: Represents a discovered LCC node in the sidebar; has a display name, an expanded/collapsed state, and a list of segment entries; may show an offline indicator if the node is unreachable
- **Segment Entry**: A navigation item within an expanded node; clicking it populates the main area with that segment's element cards; holds the selection state for the sidebar
- **Element Card**: A single accordion item in the main area representing one top-level CDI group within the selected segment; has a collapsed/expanded state, a header with the group name, and a body containing field rows and the event slots section
- **Field Row**: A read-only configuration field within an element card; displays the field label, current value, optional CDI description, and a [R] (Refresh) action; no write capability in this iteration
- **Event Slot Row**: A specialised field row for event ID slots; displays the raw event ID (or "(free)")

## Assumptions

- Configuration values have already been read from nodes (by feature 004) before the element cards are displayed; the card deck reads from the existing cache, not the node, when first rendered
- A "segment" in the CDI maps to the top-level memory spaces / groups declared in the CDI XML; the existing CDI parsing already exposes these and no new parsing work is required
- The sidebar width is fixed and non-resizable in this iteration
- Cross-tab navigation from event slots to bowtie connections is out of scope; it will be specified once the Bowties tab data model exists

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can navigate from opening the Configuration tab to viewing the configuration fields of any element on any discovered node in 3 clicks or fewer (click node → click segment → click element card to expand)
- **SC-002**: All elements in a selected segment are visible in the card deck within 500 milliseconds of the segment being selected, using cached configuration values
- **SC-003**: Users can distinguish at a glance between named and unnamed elements in a segment's card deck without expanding any cards
- **SC-004**: Advanced Settings fields, CDI description text, and event slot details add no visual clutter when the user has not explicitly requested them — they are invisible until the user chooses to reveal them
