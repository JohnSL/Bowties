# Feature Specification: Miller Columns Configuration Navigator

**Feature Branch**: `003-miller-columns`  
**Created**: February 17, 2026  
**Status**: Draft  
**Input**: User description: "Implement five-column Miller Columns navigation for browsing node configuration hierarchy. Column 1 shows discovered nodes, Column 2 shows CDI segments, Column 3 shows groups with replication support, Column 4 shows configuration elements with status indicators, and Column 5 displays a configuration panel (initially read-only). Supports smooth navigation through the hierarchy with visible context via breadcrumbs or column selection highlighting."

## Purpose

The Miller Columns navigator is designed for **discovering and navigating** the CDI structure to identify configuration elements, particularly Event IDs that will be used for producer/consumer linking in the Event Bowties feature. This feature focuses on **structure visualization only** - displaying the hierarchical organization of configuration elements as defined in the CDI XML. Actual value retrieval from node configuration memory and comprehensive configuration editing will be handled by future features.

**Key Design Principle**: The OpenLCB CDI standard supports variable hierarchy depths from 3 to unlimited levels (see [docs/technical/cdi-structure-analysis.md](../../../docs/technical/cdi-structure-analysis.md)). This navigator uses **dynamic columns** (like macOS Finder) that expand and contract based on the actual CDI structure being navigated, rather than a fixed column layout.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Navigate to Event ID Elements (Priority: P1)

As a model railroad operator setting up event links between nodes, I need to quickly navigate through a node's CDI hierarchy to identify Event ID elements so that I can understand which producers and consumers are available for linking.

**Why this priority**: This is the critical workflow for the Event Bowties feature (F5). Users need to discover all Event IDs in a node's configuration structure to understand what can be linked. Efficient structure navigation directly supports the primary use case of identifying event connection points.

**Independent Test**: Can be fully tested by selecting any discovered node with CDI data, navigating through segments → groups to find elements with Event ID data type, and viewing their CDI metadata (name, description, type). Delivers immediate value by making Event ID structure discoverable for the linking workflow.

**Acceptance Scenarios**:

1. **Given** nodes with CDI data have been discovered, **When** the user opens the Miller Columns view, **Then** Column 1 displays a list of all discovered nodes with their user-assigned names or SNIP descriptions
2. **Given** a node is selected in Column 1, **When** the CDI data is loaded, **Then** Column 2 displays all configuration segments defined in the CDI (e.g., "Inputs", "Outputs", "Events", "Node Info")
3. **Given** a segment is selected in Column 2, **When** the user clicks on it, **Then** Column 3 displays all groups within that segment, including replicated groups with instance indicators
4. **Given** a group is selected in Column 3, **When** the user clicks on it, **Then** Column 4 displays all configuration elements within that group with their names and data type indicators
5. **Given** an element is selected in Column 4, **When** the user clicks on it, **Then** the Details Panel displays the element's CDI metadata (description, data type, constraints, default value if specified)
6. **Given** the user is navigating the hierarchy, **When** viewing any column, **Then** breadcrumbs or visual highlighting clearly show the current selection path (Node → Segment → Group → Element)

---

### User Story 2 - Navigate Replicated Configuration Groups (Priority: P1)

As a model railroad operator configuring nodes with multiple identical inputs/outputs, I need to navigate through replicated groups (e.g., "Input 1", "Input 2", ..., "Input 16") so that I can configure each instance individually without confusion.

**Why this priority**: Most LCC nodes use replication to define multiple similar configuration items (like 16 input pins). Without proper replication support, the interface would be unusable for real-world nodes.

**Independent Test**: Can be tested by selecting a node with replicated groups (common in all I/O nodes), then verifying that each instance is listed separately with clear numbering. Delivers value by making multi-instance configuration manageable.

**Acceptance Scenarios**:

1. **Given** a segment contains a replicated group definition (e.g., group "Input" replicated 16 times), **When** the user selects the segment, **Then** Column 3 displays 16 separate entries labeled "Input 1" through "Input 16"
2. **Given** multiple replicated group instances are displayed, **When** the user selects any instance, **Then** Column 4 shows the configuration elements specific to that instance
3. **Given** the user is viewing a replicated group instance, **When** looking at the breadcrumb or context indicator, **Then** the instance number is clearly visible (e.g., "Outputs → Output 5 → Description")

---

### User Story 3 - Preview Element Details (Priority: P2)

As a model railroad operator navigating the configuration hierarchy, I need to see CDI metadata about a selected element (name, description, data type, constraints) so that I can confirm I've found the right element and understand its purpose without leaving the navigation view.

**Why this priority**: Quick metadata preview improves navigation efficiency. Users can verify they're at the correct element type before taking action (like noting Event IDs for producer/consumer linking). This is helpful but not essential for basic structure browsing.

**Independent Test**: Can be tested by selecting any element in the Elements column and verifying that the Details Panel shows CDI metadata. Delivers value by providing quick confirmation without disrupting navigation flow.

**Acceptance Scenarios**:

1. **Given** an element is selected in the Elements column, **When** the Details Panel updates, **Then** the element's name and description from CDI are displayed
2. **Given** the element is an Event ID, **When** viewing the Details Panel, **Then** the data type "Event ID (8 bytes)" is clearly labeled
3. **Given** the element has constraints defined in CDI (min/max, map values), **When** viewing the Details Panel, **Then** these constraints are displayed in a readable format
4. **Given** the element has a default value specified in CDI, **When** viewing the Details Panel, **Then** the default value is shown with a clear "Default:" label

---

### User Story 4 - Navigate Back Through Hierarchy (Priority: P3)

As a model railroad operator browsing deep configuration structures, I need to click on previous column selections to navigate back up the hierarchy so that I can explore different branches without restarting from the beginning.

**Why this priority**: This improves navigation efficiency for power users but is not essential for initial feature value. Users can always restart from Column 1.

**Independent Test**: Can be tested by navigating deep into the hierarchy (Node → Segment → Group → Element), then clicking on a segment in Column 2 to jump back. Delivers value by reducing repetitive clicking.

**Acceptance Scenarios**:

1. **Given** the user has navigated to an element in the Elements column, **When** the user clicks on a different segment in the Segments column, **Then** subsequent columns clear and update to show the newly selected segment's structure
2. **Given** the user has navigated to an element, **When** the user clicks on a different group in a Groups column, **Then** columns to the right clear and update to show the newly selected group's contents
3. **Given** the user has selected different items at any level, **When** navigating back, **Then** the column selection states are visually highlighted to show the current path

---

### Edge Cases

**Resolution Strategy**: Use graceful degradation with inline indicators - show inline warnings/errors without blocking navigation, provide helpful error messages in Details Panel, allow continued exploration of available data.

- **No CDI data available**: Display node in Nodes column with disabled/grayed appearance and "⚠️ No CDI" indicator. Details panel shows helpful message explaining how to retrieve CDI.
- **Extremely large CDI structures (100+ replicated groups)**: Use standard scrollable lists - real-world maximum observed is ~32 items per column. Warn user in Details panel if structure exceeds tested limits (e.g., "⚠️ Large configuration - 250 groups") but render all items directly without virtual scrolling.
- **Segment with no groups (depth 3)**: Display Elements column directly after Segments column. This is expected behavior per CDI standard.
- **Group with only nested groups (no elements)**: Add additional Groups column. Continue until reaching elements or hitting maximum depth.
- **Malformed CDI XML**: Parse what is valid, display available structure. Show "⚠️ Parsing issue" indicators on affected items. Details panel explains specific problems (e.g., "Missing name attribute").
- **Very long element names**: Truncate with ellipsis in column list view. Show full name on hover tooltip and in Details panel.
- **Duplicate group names**: Display as-is with names from CDI. Rely on position/context to distinguish. Consider adding index numbers if ambiguous (e.g., "Event (1)", "Event (2)").
- **6+ levels of nesting**: Add columns dynamically as needed. If viewport width exceeded, allow horizontal scrolling with scroll indicators.
- **Rapid navigation clicks**: Debounce selection changes (50ms). Show loading indicators in columns being populated. Prevent race conditions with request cancellation.
- **Horizontal space overflow (>5 columns)**: Enable horizontal scrolling. Consider compact column width mode or telescoping animation.
- **Deep to shallow navigation**: Remove excess columns immediately (<150ms) with smooth transition. Update breadcrumb simultaneously.
- **Long breadcrumb path**: Truncate middle segments with "..." ellipsis. Keep first (node) and last 2-3 segments visible. Show full path on hover/tooltip.

## Requirements *(mandatory)*

### Functional Requirements

#### Column Structure & Layout

- **FR-001**: System MUST use a dynamic column layout that expands and contracts based on the CDI hierarchy depth being navigated
- **FR-002**: System MUST always display a fixed Nodes column (leftmost) and Details Panel (rightmost), with a variable number of navigation columns between them
- **FR-003**: System MUST add columns dynamically when navigating into groups that contain other groups (nested groups)
- **FR-004**: System MUST remove columns when navigating back to shallower hierarchy levels
- **FR-005**: System MUST maintain visual consistency across all navigation columns, with each column having clear boundaries and scrollable content areas
- **FR-006**: System MUST display columns in left-to-right order representing the hierarchy depth (Nodes → Segments → [Groups...] → Elements)
- **FR-007**: Each navigation column MUST support independent scrolling when content exceeds the visible area
- **FR-008**: System MUST visually highlight the currently selected item in each column to show the active navigation path
- **FR-009**: System MUST handle minimum hierarchy depth of 3 levels (Node → Segment → Element) for segments with no groups
- **FR-010**: System MUST handle maximum practical depth of at least 8 levels to accommodate deeply nested CDI structures

#### Nodes Column (Fixed - Leftmost)

- **FR-011**: Nodes column MUST display all nodes discovered on the network that have successfully retrieved CDI data
- **FR-012**: Each node entry MUST display a user-recognizable identifier (user-assigned name if available, otherwise SNIP manufacturer + model name, or Node ID as fallback)
- **FR-013**: Nodes column MUST indicate when a node has no CDI data available with a visual indicator or disabled state
- **FR-014**: Nodes column MUST support selection of a single node at a time, with selection triggering population of the next column (Segments)

#### Segments Column (Fixed - Second Position)

- **FR-015**: Segments column MUST display all top-level segments defined in the selected node's CDI XML
- **FR-016**: Each segment entry MUST display the segment name as defined in the CDI
- **FR-017**: Segments column MUST populate immediately after a node is selected, or display a loading indicator if CDI parsing is in progress
- **FR-018**: System MUST handle CDI structures with zero segments gracefully (display empty state or explanatory message)
- **FR-019**: When a segment is selected, system MUST analyze its contents to determine next column type:
  - If segment contains only primitive elements (int, string, eventid, etc.) → Show Elements column (depth 3)
  - If segment contains groups → Show Groups column (depth 4+)
  - If segment contains mixed content → Show Groups column with primitives displayed alongside groups

#### Groups Columns (Dynamic - Variable Count)

- **FR-020**: System MUST display Groups columns dynamically based on CDI structure depth
- **FR-021**: Each Groups column MUST display all groups at the current hierarchy level, including both single and replicated groups
- **FR-022**: For replicated groups, system MUST display each instance as a separate entry with clear instance numbering (e.g., "Line 1", "Line 2", ..., "Line 16")
- **FR-023**: System MUST parse the CDI replication count attribute correctly and generate the appropriate number of group instances
- **FR-024**: Each group entry MUST display the group name as defined in the CDI, combined with the instance number for replicated groups
- **FR-025**: When a group is selected, system MUST analyze its contents to determine next action:
  - If group contains nested groups → Add another Groups column
  - If group contains only primitive elements → Show Elements column
  - If group is empty (per CDI Footnote 4) → Filter it from display entirely
- **FR-026**: System MUST filter empty groups from display (groups with no name, description, link, AND no child elements) per S-9.7.4.1 Footnote 4
- **FR-027**: System MUST support at least 5 nested Groups columns to accommodate deeply nested CDI structures like Tower-LCC Conditionals

#### Elements Column (Dynamic - Appears When Needed)

- **FR-028**: System MUST display Elements column when navigation reaches primitive configuration elements (int, string, eventid, float, action, blob)
- **FR-029**: Elements column MUST display all configuration elements at the current hierarchy level
- **FR-030**: Each element entry MUST display the element name as defined in the CDI
- **FR-031**: Each element entry SHOULD include a visual type indicator:
  - Event ID elements: Distinctive icon/badge (e.g., 🎯 or lightning bolt)
  - Integer elements: Numeric icon (e.g., 123)
  - String elements: Text icon (e.g., abc)
  - Other types: Appropriate visual indicator
- **FR-032**: Element names MUST be truncated or wrapped gracefully if they exceed the column width, with full names visible on hover or in the Details Panel
- **FR-033**: System MUST support displaying elements that appear directly in segments (no groups) for shallow CDI structures

#### Details Panel (Right Panel)

- **FR-034**: Details panel MUST be positioned as a right-side panel alongside the navigation columns, following the macOS Finder pattern
- **FR-035**: Details panel MUST display CDI metadata about the selected element from the Elements column
- **FR-036**: Details panel MUST display the element's name and description as defined in the CDI XML
- **FR-037**: Details panel MUST display the element's data type with a clear label ("Event ID (8 bytes)", "String", "Integer", etc.)
- **FR-038**: For elements with map definitions in CDI, details panel SHOULD display the available mapped values (e.g., "0: Inactive, 1: Active Hi, 2: Active Lo")
- **FR-039**: For elements with min/max constraints in CDI, details panel SHOULD display these constraints clearly (e.g., "Range: 0-255")
- **FR-040**: Details panel MUST display the default value if specified in the CDI with a clear "Default:" label
- **FR-041**: Details panel MUST display the full element path as breadcrumb or hierarchical text (e.g., "Port I/O → Line #7 → Event #3 → Command")
- **FR-042**: Details panel MUST remain visible alongside columns, preserving vertical space for column content (long lists of replicated groups)
- **FR-043**: Details panel MAY include a link/button to open comprehensive configuration editing in a future feature, labeled "Configure..." or similar
- **FR-044**: Details panel MUST NOT display actual configuration values retrieved from node memory (structure metadata only)
- **FR-045**: Details panel MUST NOT provide in-place editing controls in this initial implementation (read-only CDI metadata preview only)

#### Navigation & Context

- **FR-046**: System MUST provide breadcrumb navigation showing the full selection path from root to current position
- **FR-047**: Breadcrumb MUST display hierarchy clearly with separators (e.g., "Tower-LCC › Conditionals › Logic #12 › Variable #1 › Trigger")
- **FR-048**: Breadcrumb segments MUST be clickable, allowing navigation back to any level of the hierarchy
- **FR-049**: Breadcrumb MUST show instance numbers for replicated groups (e.g., "Line #7" not just "Line")
- **FR-050**: Users MUST be able to click on any item in any column to navigate back and change the selection at that level
- **FR-042**: When a selection changes in any column, all subsequent columns (to the right) MUST be removed or cleared, and new columns MUST be added based on the selected item's contents
- **FR-043**: System MUST maintain responsive selection highlighting, with selected items visually distinct from unselected items in all columns
- **FR-044**: When columns are added or removed, the transition MUST be smooth (animated or instantaneous) without jarring content shifts
- **FR-045**: System MUST preserve horizontal scroll position when practical during column additions/removals

#### CDI Parsing

- **FR-055**: System MUST parse CDI XML to extract segment, group, and element definitions according to the OpenLCB CDI specification (S-9.7.4.1)
- **FR-056**: System MUST handle unlimited group nesting depth as permitted by the CDI standard
- **FR-057**: System MUST correctly parse the replication attribute on groups and expand into numbered instances
- **FR-058**: System MUST detect and filter empty groups (per S-9.7.4.1 Footnote 4) before rendering
- **FR-059**: System MUST handle segments that contain elements directly without groups
- **FR-060**: System MUST handle groups that contain only other groups (no direct elements)
- **FR-061**: System MUST ignore offset attributes for navigation hierarchy (offsets are memory layout only)
- **FR-062**: System MUST gracefully handle malformed or incomplete CDI XML by displaying what is parseable and indicating errors for missing required elements
- **FR-063**: CDI parsing MUST extract element attributes including: name, description, data type (kind), map values, min/max constraints, and default values

#### Performance & Scalability

- **FR-064**: System MUST render columns and populate data smoothly for nodes with up to 100 replicated group instances without UI freezing
- **FR-065**: Column population MUST occur within 500ms for typical CDI structures (under 1000 elements total)
- **FR-066**: Adding or removing columns during navigation MUST complete within 200ms for responsive feel
- **FR-067**: Configuration value retrieval MUST not block UI interaction (async operation with loading state in details panel)
- **FR-068**: Navigation between columns MUST feel immediate (< 100ms delay) even when triggering background value retrieval
- **FR-069**: System MUST handle CDI structures with depth up to 8 levels without performance degradation
- **FR-070**: System MUST use simple direct rendering for column items (no virtual scrolling) as real-world maximum is ~32 items per column

#### Error Handling & Edge Cases

- **FR-071**: System MUST use graceful degradation for all error conditions - display available data with inline error indicators rather than blocking navigation
- **FR-072**: When a node has no CDI data, system MUST display it in Nodes column with disabled/grayed visual state and "⚠️ No CDI" indicator
- **FR-073**: When CDI parsing encounters malformed XML, system MUST parse valid portions and display "⚠️ Parsing issue" indicators on affected items with explanation in Details panel
- **FR-074**: When configuration value retrieval fails, system MUST display "❌ Read failed" indicator on affected element while allowing continued navigation and retry capability
- **FR-075**: Element names exceeding column width MUST be truncated with ellipsis in list view, with full name shown on hover tooltip and in Details Panel
- **FR-076**: When breadcrumb path exceeds available width, system MUST truncate middle segments with "..." while keeping node name and last 2-3 segments visible, with full path on hover
- **FR-077**: System MUST debounce rapid navigation clicks (50ms threshold) to prevent race conditions and excessive re-rendering
- **FR-078**: When viewport width is exceeded by column count, system MUST enable horizontal scrolling with visible scroll indicators
- **FR-079**: For extremely large CDI structures (100+ groups), system MUST display warning in Details panel (e.g., "⚠️ Large configuration - 250 groups") and render all items directly without virtual scrolling

### Key Entities

- **Node**: An OpenLCB-compliant device on the network with a unique Node ID, discoverable via the network, potentially having an associated CDI and configuration memory
- **CDI (Configuration Description Information)**: An XML document retrieved from a node that describes the structure, naming, and constraints of the node's configuration options
- **Segment**: A top-level organizational unit within a CDI, grouping related configuration areas (e.g., "Inputs", "Outputs", "Node Settings")
- **Group**: A collection of related configuration elements within a segment, which may be replicated multiple times for nodes with repeated structures (e.g., 16 identical input configurations)
- **Replication**: A CDI mechanism where a group definition is instantiated multiple times, creating numbered instances (e.g., "Input 1" through "Input 16" from a single group definition with replication count 16)
- **Element**: An individual configurable item within a group, having a name, description, data type, memory offset, and optionally constraints or default values
- **Configuration Memory**: The node's persistent storage area (memory space 0xFD) where configuration values are stored (not accessed by this feature - structure discovery only)

### Assumptions

- **A-001**: CDI XML data has already been retrieved and cached for nodes via the CDI caching feature (Feature F2 dependency)
- **A-002**: Nodes have been discovered on the network and appear in the nodes list with SNIP data available
- **A-003**: CDI XML documents conform to the OpenLCB Configuration Description Information standard specification (S-9.7.4.1) as documented in [docs/technical/cdi-structure-analysis.md](../../../docs/technical/cdi-structure-analysis.md)
- **A-004**: CDI structures may have variable depth from 3 levels (Node → Segment → Element) to unlimited nesting (Node → Segment → Group... → Element)
- **A-005**: Only one node can be actively navigated at a time within a single Miller Columns view instance
- **A-006**: Users understand dynamic hierarchical navigation (like macOS Finder) where columns appear/disappear based on content
- **A-007**: The primary use case is discovering Event ID element types for producer/consumer linking; actual value display and comprehensive configuration editing is a future feature
- **A-008**: Details panel provides read-only CDI metadata preview; full configuration editing UI will be designed separately

## Scope

### In Scope for This Feature

- Navigation through CDI structure hierarchy (Nodes → Segments → Groups → Elements)
- Discovery of all configuration element types, especially Event IDs
- Visual display of hierarchy with replication support
- CDI metadata preview of element information (name, description, type, constraints, default values from CDI XML)
- Read-only structure visualization
- Responsive navigation with breadcrumb context

### Out of Scope (Future Features)

- **Configuration value retrieval**: Reading actual values from node configuration memory (0xFD space)
- **Configuration element status indicators**: Visual indicators showing which elements have been set, modified, or have warnings (requires value retrieval infrastructure - separate future feature)
- **Value display**: Showing current Event ID values, integer values, string values, etc. from nodes
- **Comprehensive configuration editing**: In-place editing of all configuration types (strings, integers with constraints, etc.) will be designed in a separate feature
- **Validation and constraints**: Full validation UI for min/max ranges, string length limits, map value selection
- **Batch operations**: Multi-element editing, copying configurations between replicated groups
- **Configuration comparison**: Viewing differences between default and current values
- **Undo/redo**: Configuration change history
- **Save/apply workflows**: Writing modified values back to node configuration memory

### Dependencies

- **D-001**: Feature F2 (CDI Caching and Retrieval) must be completed - the Miller Columns feature requires cached CDI XML to function
- **D-002**: Node discovery functionality must be operational to populate the Nodes column with discovered nodes
- **D-003**: SNIP data retrieval must be working to provide node names/descriptions in the Nodes column

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can navigate from a node selection to finding a specific Event ID element type in under 10 seconds for typical CDI structures (fewer than 50 segments/groups)
- **SC-002**: The Miller Columns interface correctly displays the hierarchical structure for 100% of valid CDI XML documents conforming to the OpenLCB CDI specification (S-9.7.4.1)
- **SC-003**: The interface correctly handles CDI structures ranging from depth 3 (shallow) to depth 8+ (deep) with appropriate dynamic column expansion
- **SC-004**: Users can identify the current navigation context (which node, segment, groups, and element are selected) within 2 seconds of viewing any screen state through breadcrumbs and visual highlighting
- **SC-005**: Users can identify all Event ID element types within a node's CDI structure by navigating the Miller Columns, enabling discovery of available producer/consumer types for event linking
- **SC-006**: The interface remains responsive (column additions/removals complete within 200ms, population within 500ms) when navigating through CDI structures with variable depth
- **SC-007**: Users can browse the complete CDI structure hierarchy for any node without encountering UI freezes or performance degradation, even on nodes with 1000+ total configuration elements across 8 hierarchy levels
- **SC-008**: The Details Panel displays comprehensive CDI metadata (name, description, data type, constraints, default values) for 100% of selected elements
- **SC-009**: The system gracefully handles CDI retrieval failures, allowing users to understand which nodes have configuration data available and which do not in under 5 seconds
- **SC-010**: Navigation back through the hierarchy (clicking on breadcrumbs or previous column selections) updates the view within 200ms, providing immediate visual feedback with appropriate column removal

## Clarifications

### Session 2026-02-17

- Q: How should the system handle the 13 edge cases listed (no CDI data, malformed XML, failed value retrieval, etc.) - should navigation be blocked, allow graceful degradation, hide errors, or reject entirely? → A: Graceful degradation with inline indicators
- Q: Should the Details Panel be positioned below the columns or as a right panel alongside them? → A: Right panel (alongside columns)
- Q: Should virtual scrolling be implemented for large column lists, and if so, at what threshold (50/100/200 items)? → A: No virtual scrolling (simple rendering)
- Q: Should the system retrieve and display current configuration values from nodes, or just show CDI structure metadata? → A: Structure-only navigation (no value retrieval)
