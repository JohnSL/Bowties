# CDI Structure Analysis for UI Design

**Date**: February 17, 2026  
**Purpose**: Document OpenLCB CDI structural capabilities to inform navigation UI design  
**Standards Analyzed**:
- S-9.7.4.1-ConfigurationDescriptionInformation-2024-07-22.md (Normative Standard)
- TN-9.7.4.1-ConfigurationDescriptionInformation-2024-07-22.md (Technical Notes)

## Executive Summary

The OpenLCB Configuration Description Information (CDI) standard supports **highly flexible hierarchical structures with unlimited nesting depth**. The hierarchy is **not fixed at 4 levels** as commonly assumed. A dynamic column-based UI (like macOS Finder) is required to handle the full range of valid CDI structures.

### Key Findings

| Capability | Supported? | UI Impact |
|------------|------------|-----------|
| Variable hierarchy depth | ✅ YES | Dynamic columns required |
| Segments with direct elements (no groups) | ✅ YES | Sometimes skip "Groups" column |
| Unlimited group nesting | ✅ YES | Must expand columns dynamically |
| Empty segments/groups | ✅ YES | Filter from UI (per standard) |
| Mixed replicated/non-replicated siblings | ✅ YES | Show in same column |
| Variable depth within same CDI | ✅ YES | Different branches have different depths |

## Hierarchy Structure Rules

### Core Principles

**From S-9.7.4.1 Section 5.1.3:**
> A \<segment\> element shall contain an optional user-readable name and optional description tags, and a **sequence of zero or more data elements**.

**From S-9.7.4.1 Section 5.1.4.1:**
> A \<group\> element shall contain an optional user-readable name, optional description, optional link information, optional display hints, optional replication-name information, and a **sequence of zero or more data elements**.

**Data elements include**: `<group>`, `<int>`, `<string>`, `<eventid>`, `<float>`, `<action>`, `<blob>`

### Hierarchy Depth Possibilities

| Depth | Structure | Example | Validity |
|-------|-----------|---------|----------|
| 2 | Node → Segment (empty) | Segment with no elements | ✅ Valid |
| 3 | Node → Segment → Element | Simple flat config | ✅ Valid |
| 4 | Node → Segment → Group → Element | Typical structure | ✅ Valid |
| 5 | Node → Segment → Group → Group → Element | Tower-LCC "Conditionals" | ✅ Valid |
| 6+ | Node → Segment → Group × N → Element | No limit in standard | ✅ Valid |

**Critical:** The standard **imposes NO DEPTH LIMIT** on group nesting. The XML schema defines groups recursively, allowing infinite nesting.

### Can Segments Contain Elements Directly?

**YES** - Segments can skip groups entirely and contain elements directly.

**Schema Evidence:**
```xml
<xs:element name="segment" minOccurs="0" maxOccurs="unbounded">
  <xs:choice minOccurs="0" maxOccurs="unbounded">
    <xs:element name="group" type="groupType"/>
    <xs:element name="string" type="stringType"/>
    <xs:element name="int" type="intType"/>
    <xs:element name="eventid" type="eventidType"/>
    <xs:element name="float" type="floatType"/>
    <!-- etc. -->
  </xs:choice>
</xs:element>
```

**Example:**
```xml
<segment space='253'>
  <name>Node Power Monitor</name>
  <int size='1'><name>Message Options</name></int>
  <eventid><name>Power OK</name></eventid>
  <eventid><name>Power Not OK</name></eventid>
</segment>
```

**Navigation:** Node → Node Power Monitor → Power OK (3 levels, no groups)

### Can Groups Nest Recursively?

**YES** - Groups contain other groups with no depth limit.

**Schema Evidence:**
```xml
<xs:complexType name="groupType">
  <xs:choice minOccurs="0" maxOccurs="unbounded">
    <xs:element name="group" type="groupType"/>  <!-- RECURSIVE -->
    <!-- other data elements -->
  </xs:choice>
</xs:complexType>
```

**Real-World Example (DS54 Accessory Decoder):**
```
Segment "Channels"
  └─ Group "Channel" [×4]
       ├─ Group "Turnout output"
       │    └─ eventid "Turnout closed"
       └─ Group "Inputs" [×2]
            └─ Group "Trigger"
                 └─ eventid "Trigger event"
```

**Depth:** Segment → Group → Group → Group → Element = 5 levels

## Data Elements

### Element Types

| Element | Size | Use for Event Linking? | Notes |
|---------|------|------------------------|-------|
| `<eventid>` | 8 bytes | **YES** ✓ | Primary type for producer/consumer events |
| `<int>` | 1, 2, 4, 8 bytes | No | Configuration parameters |
| `<string>` | Variable + null | No | Names, descriptions |
| `<float>` | 2, 4, 8 bytes | No | Numeric with decimals |
| `<action>` | 1, 2, 4, 8 bytes | No | Write-only button triggers |
| `<blob>` | 10 bytes (descriptor) | No | Bulk memory access |

### Event ID Elements

**The primary target for navigation** - used to discover producer/consumer event relationships.

**Properties:**
- Fixed 8-byte size
- Big-endian byte order
- Typically displayed in dotted hex format: `01.02.03.04.05.06.07.08`
- Can be read/written via Memory Configuration Protocol (space 0xFD)

## Replication

### How Replication Works

**From S-9.7.4.1:**
> If the \`replication' attribute is present with the value of N, then the group shall be considered as if the entire sequence of data elements were repeated N times.

**Characteristics:**
- Any group at any level can be replicated
- Creates N instances numbered 1 to N
- `<repname>` provides instance labels (e.g., "Line", "Channel")
- Each instance occupies contiguous memory space

**Example:**
```xml
<group replication='16'>
  <name>Line</name>
  <repname>Line</repname>
  <eventid><name>Producer Event</name></eventid>
</group>
```

**UI Display:**
```
Line 1
Line 2
Line 3
...
Line 16
```

### Nested Replication

**Replication can nest** - replicated groups can contain other replicated groups.

**Example:**
```xml
<group replication='4'>
  <name>Channel</name>
  <group replication='2'>
    <name>Input</name>
    <eventid><name>Event</name></eventid>
  </group>
</group>
```

**Total instances:** 4 × 2 = 8 Event IDs

**UI Navigation:**
```
Column: Groups (Channels)
  Channel 1
  Channel 2
  Channel 3
  Channel 4

(User selects Channel 1)

Column: Groups (Inputs)
  Input 1
  Input 2
```

### Mixed Replication

**Replicated and non-replicated groups can be siblings** at the same level.

**Example:**
```xml
<segment>
  <group>
    <name>Global Settings</name>
    <int size="1"><name>Version</name></int>
  </group>
  <group replication="8">
    <name>I/O Ports</name>
    <eventid><name>Event</name></eventid>
  </group>
</segment>
```

**UI should show both in the same column**, distinguishing replicated groups with instance counts or badges.

## Edge Cases

### Empty Groups

**Rule:** Do NOT render empty groups in the UI.

**From S-9.7.4.1 Footnote 4:**
> Configuration Tools shall not render a \<group\> element with no child elements on their UI.

**Definition of empty:** No name, no description, no link, AND no data elements.

**Implementation:** Filter empty groups during CDI parsing or before rendering.

### Groups Containing Only Other Groups

**Valid** - Groups can contain only nested groups with no direct elements.

**Example:**
```xml
<group>
  <name>Advanced Configuration</name>
  <group>
    <name>Network Settings</name>
    <int size="1"><name>Timeout</name></int>
  </group>
  <group>
    <name>Display Settings</name>
    <int size="1"><name>Brightness</name></int>
  </group>
</group>
```

### Segments with Mixed Content

**Valid** - Segments can contain a mix of groups and direct elements.

**Example:**
```xml
<segment space="253">
  <int size="2"><name>Node Address</name></int>
  <group replication="4">
    <name>Outputs</name>
    <eventid><name>Event</name></eventid>
  </group>
  <string size="32"><name>Description</name></string>
</segment>
```

**UI Handling:** Show all content in sequence. Use icons/badges to distinguish element types from groups.

### Negative Offsets

**Valid** - Offset attributes can be negative, allowing overlapping memory locations.

**From S-9.7.4.1:**
> Each time an offset attribute is encountered, the value of the address is incremented by the offset **(which may be negative)**.

**Use Case:** Multiple action buttons writing different values to the same memory location.

**UI Impact:** **None** - Offsets are memory layout concerns, not navigation hierarchy. Ignore for UI structure.

## Navigation UI Implications

### Why Fixed Columns Fail

A fixed-column approach (e.g., always showing "Nodes | Segments | Groups | Elements | Detail") **violates the CDI standard**:

❌ Cannot handle depth 3 (Segment → Element)  
❌ Cannot handle depth 5+ (nested groups)  
❌ Wastes space on shallow configurations  
❌ Creates confusing empty columns

### Why Dynamic Columns Work

A dynamic, telescoping column approach (like macOS Finder) **handles all valid CDI structures**:

✅ Adapts to depth 3 through 8+ seamlessly  
✅ Shows only relevant columns for current path  
✅ Handles variable depth within same CDI  
✅ Efficient use of screen space

### Recommended Column Architecture

**Principle:** Add/remove columns dynamically as the user navigates deeper or shallower.

#### Shallow Tree Example (Depth 3)
```
┌─────────┬──────────┬──────────┬──────────────┐
│ Nodes   │ Segments │ Elements │ Detail Panel │
└─────────┴──────────┴──────────┴──────────────┘
```

**Path:** Tower-LCC → Node Power Monitor → Power OK

#### Typical Tree Example (Depth 4)
```
┌─────────┬──────────┬─────────┬──────────┬──────────────┐
│ Nodes   │ Segments │ Groups  │ Elements │ Detail Panel │
└─────────┴──────────┴─────────┴──────────┴──────────────┘
```

**Path:** Tower-LCC → Track Receiver → Circuit #1 → Link Address

#### Deep Tree Example (Depth 6)
```
┌─────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────────┐
│ Nodes   │ Segments │ Groups 1 │ Groups 2 │ Groups 3 │ Elements │ Detail Panel │
└─────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────────┘
```

**Path:** Tower-LCC → Conditionals → Logic #12 → Variable #1 → (nested group) → Trigger Event

### Column Detection Algorithm

**When segment is selected:**
1. Parse segment's child elements
2. **If contains ONLY primitive elements** → Show "Elements" column next (depth 3)
3. **If contains ANY groups** → Show "Groups" column next (depth 4+)
4. **If contains MIX** → Show "Groups" column, render primitives as items alongside groups

**When group is selected:**
1. Parse group's child elements
2. **If contains ANY nested groups** → Add another "Groups" column
3. **Else (only primitives)** → Show "Elements" column

**Result:** Columns dynamically adjust to match actual CDI structure.

### Breadcrumb Navigation

**Always display full path** to prevent user disorientation:

```
Home › Tower-LCC › Conditionals › Logic #12 › Variable #1 › Source
```

**Requirements:**
- Each segment is clickable to jump back
- Shows instance numbers for replicated groups
- Collapses gracefully on narrow screens
- Updates immediately on navigation

**Benefits:**
- User never loses context
- Quick navigation back to any level
- Clear indication of current position
- Works even when columns are compressed/hidden

## Real-World Examples

### Tower-LCC CDI Structure

**Segments:**
1. **NODE ID** (space 251) - Depth 3
   - Elements: Node Name (string), Node Description (string)
   
2. **Node Power Monitor** (space 253) - Depth 3
   - Elements: Message Options (int), Power OK (eventid), Power Not OK (eventid)
   
3. **Port I/O** (space 253) - Depth 5
   - Group: Line [×16]
     - Elements: Line Description (string), Output Function (int), Input Function (int)
     - Group: Delay [×2]
       - Elements: Delay Time (int), Units (int), Retrigger (int)
     - Group: Event [×6]
       - Elements: Command (eventid), Action (int)
     - Group: Event [×6]
       - Elements: Upon this action (int), Indicator (eventid)
   
4. **Conditionals** (space 253) - Depth 5
   - Group: Logic [×32]
     - Elements: Description (string), Function (int)
     - Group: Variable #1
       - Elements: Trigger (int), Source (int), Track Speed (int), set true (eventid), set false (eventid)
     - Group: Variable #2
       - (similar structure)
     - Group: Action
       - Elements: when true (int), when false (int)
     - Group: Time Delay
       - Elements: Delay Time (int), Units (int), Retriggerable (int)
     - Group: Action [×4]
       - Elements: Condition (int), Destination (int), Track Speed (int), Action Event (eventid)

**Depth Analysis:**
- Minimum: 3 levels (NODE ID, Node Power Monitor)
- Maximum: 5 levels (Port I/O, Conditionals with nested groups)
- **Varied depth within single CDI** ✓

## Design Recommendations

### For Miller Columns UI

1. ✅ **Use dynamic column expansion** - Add/remove columns based on hierarchy depth
2. ✅ **Provide breadcrumb navigation** - Show full path at all times
3. ✅ **Filter empty groups** - Per standard requirement (Footnote 4)
4. ✅ **Support both shallow (3-level) and deep (6+ level) trees**
5. ✅ **Distinguish element types with icons** - eventid, int, string, etc.
6. ✅ **Show replication counts** - Badge or label showing "×16" for replicated groups
7. ✅ **Handle horizontal overflow** - Scroll or compress left columns when too deep
8. ✅ **Instance numbering** - Clear "Line #7", "Logic #12" labels

### For Data Model

1. Parse CDI into tree structure preserving full hierarchy
2. Track current path as array: `[node, segment, group1, group2, ..., element]`
3. Expand replicated groups into instances (at parse or render time)
4. Ignore `offset` attribute for hierarchy (memory layout only)
5. Detect and filter empty groups early in rendering pipeline
6. Cache depth metrics per segment/group for optimization

### For Event Discovery Workflow

Since the primary use case is **finding Event IDs for producer/consumer linking**:

1. **Highlight eventid elements** - Visual badge or icon
2. **Filter by type** - Option to show only Event IDs
3. **Quick copy** - One-click copy Event ID to clipboard from detail panel
4. **Event ID format** - Always display as dotted hex: `01.02.03.04.05.06.07.08`
5. **Search/filter** - Find Event IDs by name across entire CDI

## Testing Scenarios

### Shallow CDI (Depth 3)
```xml
<segment space='253'>
  <name>Simple Config</name>
  <int size='1'><name>Setting</name></int>
  <eventid><name>Event</name></eventid>
</segment>
```

**Expected UI:** Nodes │ Segments │ Elements │ Detail

### Typical CDI (Depth 4)
```xml
<segment space='253'>
  <group replication='8'>
    <name>Output</name>
    <eventid><name>Producer</name></eventid>
  </group>
</segment>
```

**Expected UI:** Nodes │ Segments │ Groups │ Elements │ Detail

### Deep CDI (Depth 6)
```xml
<segment space='253'>
  <group replication='4'>
    <name>Channel</name>
    <group>
      <name>Settings</name>
      <group replication='2'>
        <name>Trigger</name>
        <eventid><name>Event</name></eventid>
      </group>
    </group>
  </group>
</segment>
```

**Expected UI:** Nodes │ Segments │ Groups (Channel) │ Groups (Settings) │ Groups (Trigger) │ Elements │ Detail

### Mixed Depth CDI
```xml
<cdi>
  <segment space='251'>
    <name>Basic</name>
    <string size='63'><name>Name</name></string>
  </segment>
  <segment space='253'>
    <name>Advanced</name>
    <group replication='16'>
      <name>I/O</name>
      <group replication='6'>
        <name>Event</name>
        <eventid><name>ID</name></eventid>
      </group>
    </group>
  </segment>
</cdi>
```

**Expected UI:** Column count changes from 4 (Basic) to 5 (Advanced) as user navigates between segments.

## Implementation Notes

### pathId-Based Navigation System

**Implementation Status:** ✅ Implemented in Feature 003 (Miller Columns)

The Miller Columns feature uses an **index-based pathId system** for CDI hierarchy navigation. This approach provides stable, unambiguous references to CDI elements.

**pathId Format:**
- Segments: `seg:N` where N is 0-based segment index
- Non-replicated elements: `elem:N` where N is 0-based element index
- Replicated group instances: `elem:N#I` where N is element index, I is 1-based instance number

**Why Index-Based:**
1. **Eliminates name ambiguity:** CDI element names can contain special characters like '#' (e.g., "Variable #1", "Group#2")
2. **Stable references:** Independent of name changes in CDI updates
3. **Efficient resolution:** O(1) array indexing instead of string matching
4. **Separation of concerns:** UI identifiers (UUIDs) vs. navigation paths (pathIds)

**Example Navigation Path:**

```
User navigates: Tower-LCC → Conditionals → Logic #12 → Variable #1 → Trigger

Backend path:   ["seg:0", "elem:0#12", "elem:2", "elem:0"]
                 └─────┘  └─────────┘  └─────┘  └─────┘
                 segment  Logic grp    Variable element
                 index 0  elem 0, #12  elem 2   elem 0

UI Display IDs: [UUID-1, UUID-2, UUID-3, UUID-4]  (React/Svelte keys)
```

**Path Resolution Algorithm:**
1. Parse pathId (e.g., "elem:2#5")
2. Extract element index (2) and optional instance number (5)
3. Navigate to `elements[2]` in current context
4. If replicated (`#5`), use as template for instance #5
5. Return element reference

**Benefits Over Name-Based Navigation:**
- No parsing ambiguity with names like "Variable #1", "Output#3", "Item #7"
- Consistent with array-based CDI data structures in Rust
- Fast lookup via direct indexing
- Frontend can generate any display format (names, numbers, UUIDs) without affecting backend

**UI vs Navigation Separation:**
- **Display ID (UUID):** Generated per render for React/Svelte list keys (prevents UI collisions)
- **Navigation pathId:** Index-based identifier for backend traversal (stable across renders)
- Both are sent to frontend; UUID for `key=`, pathId for onclick handlers

**Test Coverage:**
- Unit tests for path parsing and resolution (lcc-rs/src/cdi/hierarchy.rs)
- Integration tests with CDI elements containing '#' in names
- Property-based tests for path round-trip validation

## References

- **Standard**: S-9.7.4.1 Configuration Description Information (2024-07-22)
- **Technical Note**: TN-9.7.4.1 Configuration Description Information (2024-07-22)
- **Schema**: https://openlcb.org/schema/cdi/1/4/cdi.xsd
- **Related**: OpenLCB Memory Configuration Protocol Standard

## Revision History

| Date | Author | Changes |
|------|--------|---------|
| 2026-02-17 | GitHub Copilot | Initial analysis based on S-9.7.4.1 and TN-9.7.4.1 standards |
| 2026-02-18 | GitHub Copilot | Added pathId-based navigation system documentation |
