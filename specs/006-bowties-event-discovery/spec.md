# Feature Specification: Bowties Tab  Discover Existing Connections

**Feature Branch**: `006-bowties-event-discovery`  
**Created**: 2026-02-22  
**Status**: Draft  
**Input**: User description: "Read LCC node event slots to discover existing producer-consumer connections and display them as bowties in the Bowties tab"

## Clarifications

### Session 2026-02-22

- Q: What should the Bowties tab show while CDI reads are still in progress? → A: The Bowties tab is disabled/inaccessible until all CDI reads have completed.
- Q: How should event slots whose CDI does not declare a producer/consumer role be handled? → A: Exclude them from bowtie discovery. When a user later tries to add such an element to a bowtie (future create flow), the app will ask them which role the slot serves.
- Q: What triggers the Bowties tab to rebuild after the initial load? → A: Bowties automatically rebuild whenever the Configuration tab completes a full refresh of all node data.
- Q: What is the card header text for a bowtie that has no user-assigned name? → A: Use the event ID (e.g., 05.02.01.02.03.00.00.01) as the card header.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - View Existing Connections (Priority: P1)

A user who has already configured their LCC layout in another tool (e.g., JMRI) opens the Bowties tab and immediately sees all existing producer-consumer connections that are already programmed into their nodes  without needing to recreate them manually.

**Why this priority**: This is the foundational capability of the Bowties tab. Without it, the tab is always empty for real users who have working layouts. All other Bowties features build on having discovered connections to show.

**Independent Test**: Can be fully tested by connecting to a live LCC network with at least two nodes that share an event ID in their slots, waiting for all CDI reads to complete (tab becomes enabled), opening the Bowties tab, and verifying that a bowtie card appears showing the correct producer and consumer elements. Delivers the core "inspect what I have" value on its own.

**Acceptance Scenarios**:

1. **Given** the app is connected to an LCC network and configuration data has been read for all nodes, **When** the user navigates to the Bowties tab, **Then** a bowtie card appears for every event ID that appears in at least one producer slot on one node AND at least one consumer slot on any node.

2. **Given** the Bowties tab is displayed, **When** a bowtie card is rendered, **Then** it shows: producer element cards stacked in a left column, consumer element cards stacked in a right column, and a right-pointing arrow connecting the two columns with the event ID displayed beneath the arrow. Each element card shows the node name and element name (CDI-derived label or CDI path if unnamed).

3. **Given** an event ID exists in producer slots on two different nodes and in a consumer slot on a third node, **When** the Bowties tab loads, **Then** a single bowtie card appears with two producer entries and one consumer entry.

4. **Given** an event ID exists only on producer slots across all discovered nodes (no consumer slot holds the same event ID), **When** the Bowties tab loads, **Then** no bowtie card is shown for that event ID (it is an unmatched slot).

5. **Given** the Bowties tab is showing a set of bowtie cards, **When** the user triggers a full configuration refresh (re-reads all nodes) and it completes, **Then** the Bowties tab automatically rebuilds and reflects any changes to event slot values without requiring any separate user action in the Bowties tab.

### User Story 2 - Empty Tab Guidance (Priority: P2)

A user who has no connections yet (either a new layout or all slots at factory defaults) opens the Bowties tab and sees clear guidance rather than a blank screen.

**Why this priority**: Good empty-state UX prevents confusion, especially for first-time users. Without it, users cannot tell whether the app failed to discover events or whether there simply are none.

**Independent Test**: Can be tested with a fresh set of nodes with no custom event IDs assigned. Open the Bowties tab and verify guidance is shown. Delivers the "I understand what is happening" value independently.

**Acceptance Scenarios**:

1. **Given** no connections are found across all discovered nodes, **When** the Bowties tab loads, **Then** an empty-state illustration and message "No connections yet  click + New Connection to link a producer to a consumer" are shown centered in the tab.

2. **Given** the Bowties tab has connections displayed, **When** all configs are refreshed and no event ID appears in both a producer slot and a consumer slot across all nodes, **Then** the tab transitions to the empty state.

---

### User Story 3 - Configuration Cross-Reference (Priority: P3)

A user viewing an event slot in the Configuration tab can see which connection it belongs to and navigate directly to that bowtie.

**Why this priority**: Closes the gap between the two tabs without requiring the user to manually correlate event IDs. Lower priority because the Bowties tab itself (Stories 1-2) must work first.

**Independent Test**: Can be tested by navigating to a node event slot in the Configuration tab and verifying the "Used in: [connection name]" cross-reference link is present and functional.

**Acceptance Scenarios**:

1. **Given** an event slot on a node contains an event ID that is part of a discovered bowtie, **When** the user views that slot in the Configuration tab, **Then** the slot shows "Used in: [user-assigned name, or the event ID if no name has been assigned]" as a navigable link.

2. **Given** the user clicks a "Used in" link in the Configuration tab, **When** the navigation completes, **Then** the Bowties tab is shown, scrolled to and visually highlighting the corresponding bowtie card.

3. **Given** an event slot contains an event ID that is not part of any discovered bowtie (sole producer with no matching consumer), **When** the user views that slot, **Then** no "Used in" link is shown.

---

### Edge Cases

- What happens when a CDI element's role (producer vs. consumer) cannot be determined? Role is determined by the Identify Events protocol exchange (not from the CDI XML, which has no producer/consumer declaration). For cross-node cases (different nodes reply as producer vs consumer) the role is definitive. For same-node cases (one node replies both Producer Identified and Consumer Identified for the same event), a CDI text heuristic is applied as a fallback. If the heuristic is also inconclusive, the slot is marked **Ambiguous** and shown in a dedicated "Unknown role — needs clarification" section within the bowtie card — it is not silently excluded. When the user later tries to clarify such a slot in a future create/edit flow, the app will let them assign the role and persist the decision.
- What happens when a node is discovered but its CDI has not yet been read? The Bowties tab remains disabled until all CDI reads for all discovered nodes are complete; partial data is never shown.
- What happens when the same event ID appears in multiple producer slots on the same node (two lines on the same node produce the same event)? Both slots appear as separate producer entries in the same bowtie card.
- What happens when a node goes offline between network discovery and event slot reading? Out of scope for MVP — the app has no mechanism to detect node-offline events in real time. Bowtie discovery operates on a snapshot of whatever configuration data was successfully read at load time.
- What is the definition of an "unassigned" event slot? A producer slot is unassigned when no consumer slot on any node holds the same event ID. A consumer slot is unassigned when no producer slot on any node holds the same event ID. This is a purely structural rule — the event ID value itself (factory-assigned or otherwise) is irrelevant. A factory-assigned event ID that appears on both a producer and a consumer forms a valid bowtie and is shown.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST scan all discovered nodes' event slots after configuration data is loaded, classify each slot as producer or consumer by performing an `IdentifyEventsAddressed` protocol exchange per node (plus a CDI text heuristic fallback for same-node ambiguous cases), and build an in-memory map of which event IDs are shared across which node elements. Note: the CDI XML schema has no producer/consumer declaration — roles are determined entirely at runtime via the protocol exchange.

- **FR-013**: The Bowties tab MUST be disabled and non-navigable while CDI data for any discovered node is still being read. The tab MUST become enabled only after all CDI reads have completed. The tab control MUST be visually distinguishable in its disabled state (e.g., greyed out label).

- **FR-002**: The app MUST include an event ID in a bowtie card if and only if that event ID has at least one confirmed producer slot AND at least one confirmed consumer slot across all discovered nodes. Same-node ambiguous slots (role unresolvable by heuristic) MAY appear within the card in a separate "Unknown role" section alongside confirmed entries, but event IDs with zero confirmed entries on either side are silently excluded; no error or warning is shown.

- **FR-003**: The Bowties tab MUST display a bowtie card for every event ID that has at least one producer element and at least one consumer element across the discovered nodes.

- **FR-004**: Each bowtie card MUST use a three-column layout: a left column of producer element cards, a centre connector, and a right column of consumer element cards. Each element card MUST show the node name and the element name (CDI `<name>` if present, otherwise first sentence of CDI `<description>`, otherwise the slash-joined CDI path). The layout MUST follow this structure:

  ```
  ┌──────────────────────────────────────────┐
  │  PRODUCERS          →          CONSUMERS │
  │  ┌─────────────┐  ───→  ┌─────────────┐ │
  │  │ Element A   │        │ Element C   │ │
  │  │  Node 1     │  event │  Node 3     │ │
  │  └─────────────┘   ID   └─────────────┘ │
  │  ┌─────────────┐        ┌─────────────┐ │
  │  │ Element B   │        │ Element D   │ │
  │  │  Node 2     │        │  Node 4     │ │
  │  └─────────────┘        └─────────────┘ │
  └──────────────────────────────────────────┘
  ```

- **FR-005**: The event ID MUST be displayed beneath the right-pointing arrow in the centre connector — not inside the element cards. It is a secondary label on the connection itself, not a property of any individual element.

- **FR-006**: The Bowties tab MUST display an empty state (illustration, explanatory message, and prompt to create a connection) when no bowties are found.

- **FR-007**: Bowtie discovery MUST reuse the node list and CDI data already loaded during the Configuration tab's read operation — it MUST NOT require a second full node-topology discovery scan. A targeted `IdentifyEventsAddressed` protocol exchange (one addressed message per already-known node, no new node discovery) is performed automatically after CDI reads complete and is required for role classification — this is not considered a second full scan. The bowtie view MUST automatically rebuild whenever a full configuration refresh completes, reflecting any changes to event slot values without requiring a separate user action in the Bowties tab.

- **FR-008**: Each event slot displayed in the Configuration tab MUST show a "Used in: [name]" cross-reference if that slot's event ID is part of a discovered bowtie, where [name] is the user-assigned connection name or the event ID string if no name has been assigned.

- **FR-014**: Each bowtie card MUST display a header. When the user has assigned a name, the header MUST show that name. When no name has been assigned, the header MUST display the event ID in dotted-hex notation (e.g., 05.02.01.02.03.00.00.01).

- **FR-009**: Clicking a "Used in" cross-reference link in the Configuration tab MUST navigate to the Bowties tab and scroll to highlight the corresponding bowtie card.

- **FR-010**: Bowtie cards MUST be sorted and presented in a scrollable vertical list.

- **FR-011**: The app MUST correctly handle an event ID that appears in slots on more than two nodes, grouping all producers and all consumers into a single bowtie card.

- **FR-012**: In this phase the Bowties tab is read-only; no create, edit, or delete operations are in scope.

### Key Entities

- **Event ID**: A unique 8-byte identifier representing a state change signal. All producers and consumers sharing an event ID form a single bowtie.
- **Producer slot**: A CDI-declared event configuration field from which the node emits this event ID onto the network when its physical condition occurs.
- **Consumer slot**: A CDI-declared event configuration field to which the node reacts when it hears this event ID on the network.
- **Bowtie**: The unit of connection — one event ID, one or more producers, one or more consumers. Named by the user or shown as untitled.
- **Unmatched slot**: An event slot whose event ID does not appear on the opposite side across all discovered nodes; excluded from bowtie construction. This includes factory-assigned IDs with no counterpart and user-assigned IDs whose counterpart node has not yet been configured.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All existing producer-consumer connections on a live LCC network are discovered and displayed as bowtie cards within 5 seconds of configuration data finishing its initial read.

- **SC-002**: No duplicate bowtie cards are shown for the same event ID; every unique shared event ID produces exactly one card.

- **SC-003**: A user who has a known set of connections programmed into their nodes can verify that every connection appears in the Bowties tab without any manual data entry.

- **SC-004**: The empty-state message is shown within 1 second of the Bowties tab becoming active when no connections exist.

- **SC-005**: Cross-reference links in the Configuration tab are present for 100% of event slots that participate in a discovered bowtie.

## Assumptions

1. CDI data and event slot values are already loaded into the app's in-memory state before bowtie discovery begins. Bowtie discovery adds one targeted `IdentifyEventsAddressed` exchange (one message per already-discovered node) to determine producer/consumer roles — this is not a new node-topology scan. The bowtie tab remains disabled until both CDI reads and the Identify Events reply collection are complete.
2. No factory-default detection is required. The sole criterion for inclusion is whether an event ID appears on both sides (at least one producer AND at least one consumer) across all loaded nodes. This means factory-assigned IDs are valid bowtie participants if they are intentionally shared.
3. Same-node event slots whose role cannot be resolved by the CDI text heuristic (Ambiguous) are shown in a dedicated section within the bowtie card rather than excluded. A user-facing role-clarification action (letting the user assign Producer or Consumer) is deferred to a future phase and is out of scope for this feature.
4. Connection names are not yet stored locally (a future feature); bowties derived from event slot discovery in this phase display the event ID in dotted-hex notation as their card header until the user assigns a name.
5. The bowtie canvas layout described in the design document (vertical scrollable list) is the target; freeform canvas layout is explicitly out of scope for this phase.
