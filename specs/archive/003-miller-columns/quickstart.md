# Miller Columns Configuration Navigator Quickstart

**Feature**: Miller Columns Configuration Navigator  
**Version**: 1.0.0  
**Last Updated**: 2026-02-17

## Overview

The Miller Columns navigator provides an interactive, visual way to explore the configuration structure (CDI) of OpenLCB nodes on your network. It uses a familiar macOS Finder-style interface with multiple columns that expand and contract as you navigate through the hierarchy.

**Primary Use Case:** Discovering Event ID elements in your nodes' configurations to understand what producers and consumers are available for event linking.

---

## Getting Started

### Prerequisites

- Bowties application installed and running
- LCC/OpenLCB network connection established (TCP)
- At least one node discovered on the network
- CDI data retrieved for the node(s) you want to explore

### Opening Miller Columns View

1. Launch Bowties application
2. Ensure nodes are discovered (check Nodes panel)
3. Navigate to **Configuration** → **Browse Structure** (or similar menu)
4. The Miller Columns view opens with discovered nodes in the leftmost column

---

## Interface Overview

```
┌─────────────┬─────────────┬─────────────┬─────────────┬───────────────────┐
│   Nodes     │  Segments   │   Groups    │  Elements   │   Details Panel   │
│             │             │             │             │                   │
│ Tower-LCC   │ Node Info   │ Line 1      │ Command     │ Name: Command     │
│ >Bridge     │>Port I/O    │ Line 2      │ Debounce    │ Type: Event ID    │
│ Switch-8    │ Events      │ Line 3      │ Active High │ (8 bytes)         │
│             │ Settings    │ ...         │             │                   │
│             │             │ Line 16     │             │ Description:      │
│             │             │             │             │ Event to trigger  │
│             │             │             │             │ input action      │
└─────────────┴─────────────┴─────────────┴─────────────┴───────────────────┘
       ↑ Breadcrumb: Tower-LCC › Port I/O › Line 7 › Command
```

### Column Types

1. **Nodes Column** (leftmost, always visible)
   - Lists all discovered nodes with CDI data
   - Shows node name or manufacturer/model
   - Nodes without CDI are grayed out with ⚠️ icon

2. **Segments Column** (appears after selecting a node)
   - Shows top-level configuration areas
   - Examples: "Inputs", "Outputs", "Events", "Node Info"

3. **Groups Columns** (dynamic, variable count)
   - Displays groups of related configuration items
   - Replicated groups show numbered instances (e.g., "Line 1" through "Line 16")
   - Can nest multiple levels deep for complex configurations

4. **Elements Column** (appears when reaching configuration items)
   - Lists individual configurable settings
   - Event ID elements marked with 🎯 icon
   - Integer, string, and other types shown with appropriate indicators

5. **Details Panel** (rightmost, always visible)
   - Shows detailed information about selected element
   - Displays: name, description, data type, constraints, default value
   - Future: Will include "Configure..." button for editing

---

## Basic Navigation

### Navigating Forward (Deeper into Hierarchy)

1. **Click any item** in a column to select it
2. If the item has children:
   - A new column appears to the right showing child items
   - Previous columns remain visible for context
3. Continue clicking items in successive columns to navigate deeper
4. The **breadcrumb** at the bottom updates to show your current path

**Example Flow:**
```
Click "Tower-LCC" (Nodes)
  → Segments column appears
Click "Port I/O" (Segments)
  → Groups column appears with "Line 1" through "Line 16"
Click "Line 7" (Groups)
  → Elements column appears with "Command", "Debounce", etc.
Click "Command" (Elements)
  → Details Panel updates with Event ID information
```

### Navigating Backward (Up the Hierarchy)

**Method 1: Click Previous Column Selection**
- Click on any item in a previous column (to the left)
- All columns to the right are removed
- New columns appear based on the newly selected item

**Method 2: Click Breadcrumb**
- Click any segment in the breadcrumb trail
- Navigation jumps to that level
- Subsequent columns are removed

**Example:**
```
Current: Tower-LCC › Port I/O › Line 7 › Command
Click "Port I/O" in breadcrumb
  → Columns reset to: Nodes | Segments | (waiting for selection)
```

---

## Finding Event IDs

Event IDs are the primary focus for the Event Bowties feature. Here's how to find them:

### Visual Indicators

- Event ID elements show **🎯 icon** or lightning bolt in Elements column
- Details Panel shows **"Event ID (8 bytes)"** as the data type
- Event IDs only appear in Elements columns (leaf nodes)

### Common Locations

Event IDs are typically found in these segment types:
- **"Inputs"** or **"Port I/O"**: Producer events triggered by physical inputs
- **"Outputs"**: Consumer events that control physical outputs
- **"Events"**: General-purpose event producers/consumers
- **"Conditionals"**: Logic-based event triggers

### Systematic Search Strategy

1. **Select a node** in Nodes column
2. **Check each segment** in Segments column:
   - Inputs/Outputs: Usually have replicated groups (16 lines, 8 ports, etc.)
   - Events: Often contains event pairs (producer/consumer)
3. **Expand replicated groups**: Click instance (e.g., "Line 7")
4. **Look for Event ID elements** in Elements column
5. **Note the full path** from breadcrumb for later reference

### Example: Finding Input Event Producers

```
1. Select "Tower-LCC" → Segments column shows "Port I/O"
2. Select "Port I/O" → Groups column shows "Line 1" through "Line 16"
3. Select "Line 7" → Elements column shows:
   - Command (Event ID) 🎯
   - Debounce (Integer)
   - Active High (Integer)
4. Select "Command" → Details Panel confirms:
   - Type: Event ID (8 bytes)
   - Description: "Event to trigger input action"
5. Record path: Tower-LCC › Port I/O › Line 7 › Command
```

---

## Working with Replicated Groups

Many nodes use **replication** to define multiple similar items (like 16 input lines or 8 outputs). The Miller Columns interface automatically expands these into individual numbered instances.

### Understanding Replication

**In CDI XML:**
```xml
<group replication="16">
  <name>Line</name>
  <eventid><name>Command</name></eventid>
</group>
```

**In Miller Columns:**
```
Groups Column displays:
  Line 1
  Line 2
  Line 3
  ...
  Line 16
```

Each instance has identical configuration elements but separate memory addresses and values.

### Navigating Replicated Groups

1. **Identify replicated groups**: Look for numbered sequences (Line 1-16, Output 1-8)
2. **Select the instance** you want to configure
3. **Elements are identical** across instances but have independent values
4. Use breadcrumb to verify instance number: "Line 7" not just "Line"

### Tip: Large Replication Counts

For nodes with 100+ replicated instances:
- Scroll through the Groups column to find the instance you need
- Instance numbers follow the pattern: 1, 2, 3, ..., N (1-based indexing)
- Consider using search/filter (future feature) for very large groups

---

## Understanding Deep Hierarchies

Some nodes have very deep configuration structures (8+ levels). The Miller Columns interface adapts by adding as many columns as needed.

### Example: Tower-LCC Conditionals (8 Levels Deep)

```
Navigation Path:
Tower-LCC (Node)
  → Conditionals (Segment)
    → Logic 12 (Replicated Group, instance 12 of 32)
      → Variable #1 (Nested Group)
        → Trigger (Nested Group)
          → On Variable Change (Element)

Breadcrumb:
Tower-LCC › Conditionals › Logic 12 › Variable #1 › Trigger › On Variable Change
```

### Tips for Deep Hierarchies

1. **Use breadcrumb** to track your location
2. **Scroll horizontally** if columns exceed screen width
3. **Navigate backward** by clicking earlier columns to explore different branches
4. **Deep nesting is normal** for complex features like logic programming

---

## Details Panel

The **Details Panel** (right side) shows comprehensive information about the selected element.

### Information Displayed

**For All Elements:**
- **Name**: Element name from CDI
- **Description**: Detailed explanation of element's purpose
- **Data Type**: Type and size (e.g., "Event ID (8 bytes)", "Integer (2 bytes)")
- **Full Path**: Breadcrumb trail showing element's location

**For Elements with Constraints:**
- **Range**: Min/max values (e.g., "Range: 0-1000 ms")
- **Map Values**: Predefined options (e.g., "0: Inactive, 1: Active Hi, 2: Active Lo")
- **Length**: Maximum string length

**For Elements with Defaults:**
- **Default Value**: Factory default specified in CDI

**Future:**
- Current value from node memory
- Edit controls for modifying configuration
- Save/apply buttons

### Example Details - Event ID

```
╔═══════════════════════════════════╗
║         DETAILS PANEL             ║
╠═══════════════════════════════════╣
║ Name: Command                     ║
║                                   ║
║ Description:                      ║
║ Event to trigger input action     ║
║                                   ║
║ Type: Event ID (8 bytes)          ║
║                                   ║
║ Path:                             ║
║ Tower-LCC › Port I/O › Line 7     ║
║   › Command                       ║
║                                   ║
║ Memory Address: 0x0800            ║
║ (Reference only)                  ║
╚═══════════════════════════════════╝
```

### Example Details - Integer with Constraints

```
╔═══════════════════════════════════╗
║         DETAILS PANEL             ║
╠═══════════════════════════════════╣
║ Name: Debounce Time               ║
║                                   ║
║ Description:                      ║
║ Time in milliseconds to debounce  ║
║ input signal                      ║
║                                   ║
║ Type: Integer (2 bytes)           ║
║                                   ║
║ Constraints:                      ║
║ • Range: 0-1000 ms                ║
║                                   ║
║ Default: 50 ms                    ║
║                                   ║
║ Memory Address: 0x0808            ║
╚═══════════════════════════════════╝
```

---

## Edge Cases & Special Situations

### Node with No CDI Data

**Symptom:** Node appears grayed out in Nodes column with ⚠️ icon

**Cause:** CDI not retrieved from node (may not support CDI, network issue, or retrieval failed)

**Action:**
1. Right-click node → "Retrieve CDI" (if available)
2. Check network connection
3. Verify node supports CDI (older nodes may not)

### Shallow Configuration (3-Level Depth)

**Symptom:** Only Nodes → Segments → Elements columns appear (no Groups)

**Cause:** Node has simple configuration without grouping

**Action:** This is normal. Navigate directly from Segment to Elements.

**Example:**
```
Nodes: Basic Node
  → Segments: Simple Config
    → Elements: Address, Event 1, Event 2
```

### Malformed CDI XML

**Symptom:** Missing items, parsing errors, or ⚠️ indicators on items

**Cause:** CDI doesn't conform to OpenLCB standard or has errors

**Action:**
1. Navigation continues with valid portions displayed
2. Check Details Panel for specific error messages
3. Report issue to node manufacturer if widespread

### Very Long Element Names

**Symptom:** Element names truncated with "..." in column view

**Action:**
- **Hover over item**: Tooltip shows full name
- **Select item**: Details Panel shows full name
- Column width optimized for typical names; truncation is normal

### Breadcrumb Path Too Long

**Symptom:** Breadcrumb has "..." in the middle

**Display:** `Tower-LCC › ... › Line 7 › Command`

**Action:**
- Hover over breadcrumb to see full path in tooltip
- First (node) and last 2-3 segments always visible

---

## Keyboard Shortcuts (Future Feature)

*Planned for future release:*

- **Arrow Keys**: Navigate between items in current column
- **Enter**: Select highlighted item (expand next column)
- **Backspace**: Navigate to parent (remove rightmost column)
- **Cmd/Ctrl + F**: Search elements by name
- **Cmd/Ctrl + Arrow**: Jump to first/last column

---

## Performance Notes

### Expected Response Times

- **Column population**: < 500ms for typical CDI structures
- **Column transitions**: < 200ms (smooth animations)
- **Navigation response**: < 100ms (immediate feel)

### Large Configurations

The Miller Columns navigator is tested with:
- **Up to 100 replicated groups** per column (e.g., 100 input lines)
- **Up to 8 levels deep** hierarchy
- **Up to 1000 total elements** in a single node's CDI

Performance remains smooth with direct rendering (no virtual scrolling).

### If Navigation Feels Slow

1. **Check network connection**: CDI data comes from node or cache
2. **Very large replications** (>100): First render may pause briefly
3. **Deep nesting** (>8 levels): Horizontal scrolling may be needed

---

## Next Steps

### After Discovering Event IDs

1. **Record Event ID locations** for reference
2. **Navigate to Event Bowties feature** (when available)
3. **Create producer/consumer links** using discovered Event IDs

### Future Features (Not in v1.0)

- **Configuration value display**: See current values from node memory
- **In-place editing**: Modify values directly in Miller Columns view
- **Search/filter**: Find elements by name or type
- **Comparison view**: See default vs. current values
- **Batch operations**: Copy settings between replicated groups

---

## Troubleshooting

### Problem: No columns appear after selecting node

**Check:**
- Node has CDI data (not grayed out with ⚠️)
- CDI parsing succeeded (check console for errors)
- Refresh node list

### Problem: Missing expected elements

**Check:**
- Navigate to correct segment (check segment names)
- Element may be in nested group (navigate deeper)
- CDI may not define that element (verify with manufacturer docs)

### Problem: Can't navigate backward

**Action:**
- Click any item in a previous (left) column
- Click breadcrumb segment
- Selecting a node resets entire navigation

### Problem: Details Panel shows "Element not found"

**Cause:** Selected element path no longer valid (rare)

**Action:** Re-navigate to element from Nodes column

---

## Support & Feedback

For issues or suggestions:
1. Check Bowties documentation
2. Report bugs via GitHub Issues
3. Community discussion on LCC forums

**Feature Version:** 003-miller-columns v1.0.0  
**Last Updated:** 2026-02-17
