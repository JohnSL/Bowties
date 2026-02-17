# Feature Specification: Enhanced Node Discovery with SNIP Data

**Feature Branch**: `001-node-snip-data`  
**Created**: February 16, 2026  
**Status**: Draft  
**Input**: User description: "Enhance node discovery to retrieve and display SNIP data (manufacturer, model, version, user name) with friendly names and status indicators"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - View Discovered Nodes with Friendly Names (Priority: P1)

A hobbyist opens Bowties and wants to see which LCC devices are on their network with recognizable names instead of cryptic Node IDs.

**Why this priority**: This is the foundation for all other features - users must be able to identify their devices before they can configure them. Without SNIP data, users see only Node IDs (like "05.02.01.02.00.00") which are meaningless to non-technical hobbyists.

**Independent Test**: Can be fully tested by launching the application with an active LCC network and verifying that the node list displays manufacturer, model, software version, and user-assigned names instead of just Node IDs and aliases.

**Acceptance Scenarios**:

1. **Given** the application has connected to an LCC network with 3 nodes, **When** the node discovery completes, **Then** each node displays its manufacturer name (e.g., "RR-CirKits"), model name (e.g., "Tower-LCC"), user name (e.g., "East Panel Controller"), and user description (e.g., "Controls east hallway section") in the main list view
2. **Given** a node list is displayed, **When** the user hovers over a node, **Then** additional details appear in a tooltip showing the full Node ID, alias, and software version for reference
3. **Given** a node has not been assigned a user name by its owner, **When** the node list is displayed, **Then** the node shows its manufacturer and model (e.g., "RR-CirKits Tower-LCC") as the display name
4. **Given** a node has not been assigned a user description, **When** the node is shown in the list, **Then** the description field is simply omitted or shows as empty
5. **Given** a node has a user description that is too long for the list display, **When** the node is shown, **Then** the description is truncated with ellipsis and the full text is available in the tooltip

---

### User Story 2 - On-Demand Node Status Verification (Priority: P2)

A user wants to verify which nodes are currently online and responding before attempting configuration, using a manual refresh action.

**Why this priority**: Prevents wasted time trying to configure unresponsive nodes while avoiding continuous network polling overhead. User controls when to check status freshness.

**Independent Test**: Can be tested by connecting to a network with all nodes online, physically disconnecting one node, clicking the "Refresh" button, and verifying the node's status updates to show it as not responding.

**Acceptance Scenarios**:

1. **Given** the user is viewing the node list, **When** they click the "Refresh" or "Rescan Network" button, **Then** the system sends verification requests to all nodes and updates their status indicators
2. **Given** a user clicks "Refresh" and a node fails to respond within 5 seconds, **When** the status updates, **Then** the node shows a "Not Responding" indicator with a red icon
3. **Given** a user attempts to configure a node that hasn't been verified recently, **When** they click on the node, **Then** the system automatically verifies the node's status before opening the configuration view
4. **Given** a node is marked "Not Responding", **When** the user tries to configure it, **Then** the application displays a warning dialog with an option to retry verification or cancel
5. **Given** nodes have been verified at different times, **When** the user views the node list, **Then** each node shows when it was last verified (e.g., "Verified 30 seconds ago", "Verified 5 minutes ago")

---

### User Story 3 - Automatic Discovery of New Nodes (Priority: P2)

A user adds a new LCC device to their network while Bowties is running and wants it to appear automatically without manual intervention.

**Why this priority**: Improves workflow efficiency during initial setup and testing. New nodes broadcast messages when they join the network, allowing automatic discovery without user action.

**Independent Test**: Can be tested by launching the application, noting the initial node count, then physically connecting a new LCC device to the network and verifying it appears in the node list within 10 seconds without clicking refresh.

**Acceptance Scenarios**:

1. **Given** the application is running with 3 discovered nodes, **When** a new node joins the network and begins broadcasting, **Then** the new node appears in the list within 10 seconds showing its SNIP data
2. **Given** the application is listening for node announcements, **When** a new node sends its initial Verified Node ID message, **Then** the application automatically initiates SNIP retrieval for that node
3. **Given** a node was previously known but disconnected, **When** the node reconnects and broadcasts its presence, **Then** it automatically re-appears in the list with refreshed SNIP data

---

### Edge Cases

- What happens when a node supports the Verify Node ID protocol but does not support SNIP (older/minimal devices)?
  - Display Node ID and alias with a note "SNIP not supported" instead of failing
  
- How does the system handle nodes that respond slowly to SNIP requests (slow/busy controllers)?
  - Show a "Loading..." indicator and wait up to 5 seconds before timing out
  - If timeout occurs, show partial data with a "Partial data" warning
  
- What if two nodes report the same user name?
  - Display both with a disambiguation (e.g., "East Panel (05.02.01...)" and "East Panel (05.02.02...)")
  - Show a warning icon indicating duplicate names
  
- What happens during network initialization when dozens of nodes respond simultaneously?
  - Queue SNIP requests to avoid overwhelming the network
  - Show progressive loading: nodes appear with basic info first, SNIP data fills in as retrieved
  
- What happens when a user clicks "Refresh" while a previous refresh is still in progress?
  - Cancel the previous operation and start a new refresh
  - Show a "Refreshing..." indicator to prevent multiple simultaneous refresh attempts
  
- How does the system handle malformed SNIP data (invalid characters, missing fields)?
  - Validate and sanitize SNIP data
  - Display what's available and mark invalid fields with defaults (e.g., "Unknown Manufacturer")

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST retrieve SNIP data (manufacturer, model, software version, hardware version, user name, user description) from each discovered node using the LCC SNIP Request protocol (MTI 0x19DE8)
- **FR-002**: System MUST display nodes in the node list using a friendly name formatted as: "[User Name] ([Manufacturer] [Model])" when user name is available, or "[Manufacturer] [Model]" when user name is not set
- **FR-003**: System MUST show Node ID and alias information in a secondary/tooltip manner for advanced users who need technical identifiers
- **FR-004**: System MUST indicate node status with visual indicators: "Connected" (green), "Not Responding" (red), and "Unknown" (gray for nodes not yet verified)
- **FR-005**: System MUST provide a manual "Refresh" or "Rescan Network" action that re-discovers all nodes and updates their status
- **FR-006**: System MUST verify node responsiveness on-demand when the user attempts to configure a node, displaying a warning if the node is not responding
- **FR-007**: System MUST automatically detect and add newly joined nodes to the list by listening for Verified Node ID messages broadcast by nodes when they start up or join the network
- **FR-008**: System MUST handle SNIP retrieval failures gracefully by displaying available information (Node ID, alias) and indicating which data is missing
- **FR-009**: System MUST queue SNIP requests during initial discovery to prevent network flooding (maximum 5 concurrent SNIP requests)
- **FR-010**: System MUST timeout SNIP requests after 5 seconds and mark the node with "Partial data" status if SNIP is not received
- **FR-011**: System MUST validate and sanitize SNIP data fields to prevent display issues from malformed or invalid character encodings
- **FR-012**: System MUST detect duplicate user names across nodes and disambiguate them in the display by appending a portion of the Node ID
- **FR-013**: System MUST persist the order of nodes in the list between application restarts based on Node ID for consistent display
- **FR-014**: System MUST allow users to manually trigger a network rescan via a "Refresh" or "Rescan Network" button to re-discover nodes and update their status and SNIP data
- **FR-015**: System MUST display when each node's status was last verified (e.g., "Verified 2 minutes ago") to help users understand data freshness
- **FR-016**: System MUST display user description in the main node list when available, treating it as primary information alongside manufacturer, model, and user name

### Key Entities

- **Node**: Represents a physical or virtual LCC device on the network
  - Node ID (8 bytes, unique identifier)
  - Alias (2 bytes, dynamic short identifier)
  - Manufacturer (string from SNIP)
  - Model (string from SNIP)
  - Software version (string from SNIP)
  - Hardware version (string from SNIP, optional)
  - User name (string from SNIP, user-configurable on the device)
  - User description (string from SNIP, optional)
  - Connection status (Connected, Not Responding, Unknown)
  - Last verified timestamp (when status was last checked)
  - SNIP retrieval status (Complete, Partial, Failed, Not Supported)

- **SNIP Request/Response**: LCC protocol message for retrieving Simple Node Identification Protocol data
  - Request MTI: 0x19DE8
  - Response MTI: 0x19A08 (datagram)
  - Contains manufacturer, model, versions, user name/description fields

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can identify any node in their network by its friendly name without needing to reference Node IDs or technical documentation
- **SC-002**: SNIP data retrieval completes for 95% of nodes within 3 seconds of discovery on a network with up to 20 nodes
- **SC-003**: Newly joined nodes appear in the node list within 10 seconds of broadcasting their presence without manual intervention
- **SC-004**: Manual network refresh completes and updates all node statuses within 5 seconds for networks with up to 20 nodes
- **SC-005**: On-demand node verification (when clicking to configure) completes within 2 seconds and provides clear feedback if node is unresponsive
- **SC-006**: Application handles networks with up to 50 nodes without performance degradation in the node list UI
- **SC-007**: 100% of nodes that support SNIP display manufacturer and model information; nodes without SNIP show fallback information without errors
- **SC-008**: Users can distinguish between nodes with duplicate user names through automatic disambiguation in the display
