# Feature Specification: Node Configuration Value Reading with Progress

**Feature Branch**: `004-read-node-config`  
**Created**: February 19, 2026  
**Status**: Draft  
**Input**: User description: "Support reading the configuration values from the node. For now, I would like to read these values after refreshing the node, and before showing the Miller Columns. Because this might take some time, I would like to show some status indication, such as '1/2 Reading Tower LCC config...'. My thinking is that we'll show an indication of how many nodes for which we'll be retrieving settings, and which one we're on to provide some idea of progress. Another option would be to have a progress bar, plus text saying which node it's reading from. Once we're able to read the config, the next step is to show the values when you select an element in the Miller diagram."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - View Current Configuration Values (Priority: P1)

A user configuring LCC nodes needs to see the current values stored in each configuration element to understand the node's current state and make informed decisions about what changes are needed.

**Why this priority**: This is the core value proposition - without being able to view current values, users cannot effectively configure nodes. This enables the fundamental use case of inspecting node configuration.

**Independent Test**: Can be fully tested by selecting a configuration element in the Miller Columns interface and verifying that the current value stored on the node is displayed. Delivers immediate value by showing what's actually configured on the node.

**Acceptance Scenarios**:

1. **Given** a user has refreshed nodes and opened the Miller Columns interface, **When** they select a configuration element (e.g., "Node Name" or "Manufacturer"), **Then** the current value read from the node's memory is displayed in the details panel
2. **Given** a configuration element with an integer value, **When** the user selects it, **Then** the numeric value is displayed in the appropriate format (decimal or hexadecimal based on element type)
3. **Given** a configuration element with a string value, **When** the user selects it, **Then** the full text string is displayed as stored on the node
4. **Given** a configuration element with an event ID value, **When** the user selects it, **Then** the 8-byte event ID is displayed in dotted hexadecimal format

---

### User Story 2 - Monitor Configuration Reading Progress (Priority: P2)

A user refreshing multiple nodes needs to see progress while configuration values are being read so they understand the system is working and have an estimate of time remaining.

**Why this priority**: Reading configuration from multiple nodes can take several seconds. Without progress indication, users may think the application has frozen. This significantly improves user experience and reduces uncertainty.

**Independent Test**: Can be tested by refreshing nodes with at least 2 discovered nodes and observing the progress indicator displays accurate count and current node name (e.g., "1/3 Reading Tower LCC config..."). Works independently of the value display functionality.

**Acceptance Scenarios**:

1. **Given** a user has 3 discovered nodes, **When** they click "Refresh Nodes", **Then** progress text appears showing "Reading [First Node Name] config... 0%", progressing through each node with increasing percentages (minimum 1 update per second), until reaching "Reading [Last Node Name] config... 100%"
2. **Given** configuration reading is in progress, **When** a user views the interface, **Then** they can see which specific node is currently being read and the overall completion percentage
3. **Given** all nodes have been read, **When** the reading completes, **Then** the progress indicator disappears and the Miller Columns interface becomes available
4. **Given** configuration reading fails for one node, **When** the system continues to the next node, **Then** the progress percentage continues to increment and shows the next node name
5. **Given** configuration reading is in progress, **When** a user clicks "Cancel", **Then** the current read operation stops, partial data is retained, and the Miller Columns interface becomes available with successfully read values

---

### User Story 3 - Refresh Configuration Values on Demand (Priority: P3)

A user who has made changes to a node's configuration needs to refresh the displayed values to see the updated state without restarting the entire node discovery process.

**Why this priority**: While less critical than initial reading, this enables an iterative workflow where users can verify changes. It's lower priority because the initial read (P1) already provides the core value.

**Independent Test**: Can be tested by selecting an element with a known value, changing that value externally, then clicking a "Refresh Value" button and verifying the display updates. Works as a standalone feature once basic value reading is implemented.

**Acceptance Scenarios**:

1. **Given** a user is viewing a configuration element's current value, **When** they click "Refresh Value", **Then** the system re-reads the value from the node and updates the display
2. **Given** a value has been updated on the node by another tool, **When** the user refreshes the value, **Then** the new value is displayed
3. **Given** a node is unreachable when refreshing, **When** the read fails, **Then** an error message is displayed and the previous value is retained with a "stale" indicator

---

### Edge Cases

- What happens when a node becomes unreachable during configuration reading? System should skip that node, show an error indicator, continue with remaining nodes, and report which nodes failed
- How does the system handle configuration elements with zero-length or invalid data? Display a "(empty)" or "(invalid)" indicator rather than showing corrupt data
- What happens when a user cancels while configuration is being read? Stop the current node read operation, mark partial data as incomplete, and allow Miller Columns to display with whatever was successfully read
- How does the system handle extremely large configuration values (e.g., 64-byte strings)? Display them completely with appropriate scrolling or truncation with "show more" option
- What happens when memory read fails for a specific element during batch reading? Log the error for that element, continue reading remaining elements, and display an error state for failed elements while showing successfully read values for others
- How does the interface behave when reading from nodes with many configuration elements (100+ elements)? Show progress indication during the upfront read, block Miller Columns display until reading completes or user cancels, optimize by reading elements in parallel where possible

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST read configuration values from node memory using the LCC Memory Configuration Protocol
- **FR-002**: System MUST read values from the Configuration address space (0xFD) at the memory addresses specified in the CDI
- **FR-003**: System MUST automatically read ALL configuration element values from ALL discovered nodes after node refresh completes and before Miller Columns interface is displayed
- **FR-004**: System MUST display accurate progress indication showing completion percentage and current node being read (format: "Reading [Node Name] config... X%")
- **FR-005**: System MUST identify nodes using a priority cascade from SNIP data: 1) User Name, 2) User Description, 3) Model Name, 4) Node ID (as fallback). This same identification logic MUST be used consistently in both the progress indicator and the Miller Columns node display
- **FR-006**: System MUST handle and display different configuration value types: integers (1, 2, 4, 8 bytes), strings (variable length), event IDs (8 bytes), and floating point numbers (4 bytes)
- **FR-007**: System MUST format values appropriately based on element type (e.g., hexadecimal for event IDs, decimal for integers, UTF-8 text for strings)
- **FR-008**: System MUST display configuration values in the Miller Columns details panel when a user selects a configuration element
- **FR-009**: System MUST show the value alongside existing element information (name, description, memory address)
- **FR-010**: System MUST continue reading remaining nodes if one node fails to respond, logging the error and marking that node's values as unavailable
- **FR-011**: System MUST store all read configuration values and display them from this stored data when navigating between elements, without re-reading from the node unless explicitly refreshed
- **FR-012**: Users MUST be able to manually refresh a single element's value via a "Refresh Value" action
- **FR-013**: System MUST indicate when a value read fails with a clear error message (e.g., "Failed to read value: node not responding")
- **FR-014**: System MUST complete configuration reading for typical nodes (10-50 config elements) within 5 seconds per node
- **FR-015**: System MUST allow users to cancel an in-progress configuration read operation
- **FR-016**: System MUST read all instances of replicated groups during upfront loading (e.g., if a group has 16 replications, read configuration values for all 16 instances)

### Key Entities *(include if feature involves data)*

- **Configuration Value**: Represents data stored in a node's configuration memory, including the raw bytes read, the interpreted value based on element type, the memory address it was read from, and timestamp of when it was read
- **Read Progress State**: Tracks the current state of configuration reading operation, including total nodes to read, current node index, current node name, success/error status per node, and overall completion percentage
- **Value Cache**: Stores recently read configuration values mapped by node ID and element path to avoid redundant network reads during navigation

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can view current configuration values for any configuration element within 2 seconds of selecting it in the Miller Columns interface
- **SC-002**: Progress indicator updates smoothly during configuration reading (minimum 1 update per element read, maintaining UI refresh rate >30fps), showing accurate completion percentage and current node name
- **SC-003**: System successfully reads and displays configuration values from nodes with up to 100 configuration elements
- **SC-004**: Configuration reading completes for typical 3-node networks within 15 seconds total
- **SC-005**: Users can identify which node is currently being read by viewing the progress text showing the node's name
- **SC-006**: Error rate for configuration reading is less than 5% under normal network conditions
- **SC-007**: UI remains responsive during configuration reading (UI thread blocks <100ms per operation, maintains animation frame rate) - users can cancel or navigate away at any time
- **SC-008**: 90% of configuration values are displayed in a format that users can immediately understand without conversion (e.g., strings shown as text, not hex bytes)

## Assumptions

- Nodes respond to Memory Configuration Protocol read requests within 2 seconds under normal conditions
- Configuration memory addresses are correctly specified in the node's CDI XML
- Most configuration elements are small (under 64 bytes), allowing single-datagram reads
- Network latency between application and nodes is typically under 100ms
- Users have already discovered nodes and downloaded their CDI before attempting to read configuration values
- The existing Miller Columns interface will be extended with a value display section rather than requiring a new UI component
- Progress indication will use simple text format initially, with option to enhance to progress bar in future iterations

## Clarifications

### Session 2026-02-19

- Q: Configuration Reading Strategy - Should the system read all values upfront with progress blocking Miller Columns (Eager), or only read values when elements are selected (Lazy)? → A: Eager/Upfront - Read ALL configuration values from all nodes immediately after refresh, show progress indicator, block Miller Columns until complete. Rationale: Next feature (bowties support) will need configuration values for all nodes, making upfront loading more efficient.
- Q: Scope of Configuration Elements to Read - Should the system read only leaf values (int, string, eventid, float) or everything with a memory address including structural elements (segments, groups)? → A: Everything with a memory address - Read all CDI elements that have memory addresses (segments, groups, and all element types). Rationale: Structural elements like segments and groups can have configuration values that provide user-configurable names for those elements.
- Q: Progress Indicator Node Identification - What should be displayed to identify the node in progress text when SNIP data is available? → A: Priority cascade - Use first available from SNIP data: 1) User Name, 2) User Description, 3) Model Name, 4) Node ID (as final fallback). This same priority logic applies to node display in Miller Columns interface for consistency.
- Q: Replicated Group Handling - When reading configuration values for replicated groups during upfront load, should the system read all instances or defer some? → A: Read all instances upfront - If a group has 16 replications, read configuration values for all 16 instances during the initial load.
- Q: Progress Granularity - Should progress track nodes ("2/3 Reading...") or elements ("127/348 Reading...") or use percentage? → A: Percentage - Display format "Reading [Node Name] config... X%" showing overall completion percentage based on total elements read across all nodes.
