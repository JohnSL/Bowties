# Data Model: Miller Columns Configuration Navigator

**Feature**: 003-miller-columns  
**Date**: 2026-02-17  
**Status**: Phase 1 Design

## Overview

This document defines the data structures for the Miller Columns navigation feature. It covers both CDI domain entities (parsed from XML) and UI state entities (navigation and display).

---

## Domain Entities (CDI Structure)

### Cdi

Root structure representing a complete Configuration Description Information document.

```rust
pub struct Cdi {
    /// Optional identification information (manufacturer, model, versions)
    pub identification: Option<Identification>,
    
    /// Optional ACDI (standardized node info)
    pub acdi: Option<Acdi>,
    
    /// Configuration segments (0 or more)
    pub segments: Vec<Segment>,
}
```

**Source:** S-9.7.4.1 CDI specification  
**Purpose:** Provides complete node configuration structure  
**Validation:** Must have valid XML structure per spec

---

### Segment

Top-level organizational unit within a CDI, representing a memory space.

```rust
pub struct Segment {
    /// Segment name (optional, user-visible)
    pub name: Option<String>,
    
    /// Segment description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory space number (e.g., 253 for configuration memory)
    pub space: u8,
    
    /// Starting address in memory space
    pub origin: i32,
    
    /// Child elements (groups and primitive elements)
    pub elements: Vec<DataElement>,
}
```

**Example:** "Inputs", "Outputs", "Node Settings"  
**Relationships:** Contains 0+ DataElements  
**Rendering:** Displayed in Segments column (2nd column)

---

### DataElement (Enum)

Discriminated union of all possible CDI element types.

```rust
pub enum DataElement {
    Group(Group),
    Int(IntElement),
    String(StringElement),
    EventId(EventIdElement),
    Float(FloatElement),
    Action(ActionElement),
    Blob(BlobElement),
}
```

**Purpose:** Type-safe representation of heterogeneous CDI content  
**Pattern:** Rust enum for exhaustive matching in navigation logic

---

### Group

Collection of related configuration elements, supporting replication for repeated structures.

```rust
pub struct Group {
    /// Group name (optional, user-visible)
    pub name: Option<String>,
    
    /// Group description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory offset from parent address
    pub offset: i32,
    
    /// Number of times this group is replicated (default: 1)
    pub replication: u32,
    
    /// Template for instance naming (e.g., ["Line"] → "Line 1", "Line 2")
    pub repname: Vec<String>,
    
    /// Child elements (can include nested groups - RECURSIVE)
    pub elements: Vec<DataElement>,
    
    /// Optional rendering hints (future use)
    pub hints: Option<GroupHints>,
}
```

**Key Feature:** Recursion - groups can contain groups (unlimited nesting depth)  
**Replication Example:** `replication=16` with `name="Input"` expands to "Input 1" through "Input 16"  
**Rendering Rule:** Per S-9.7.4.1 Footnote 4, filter groups with no name AND no description AND no elements  
**Relationships:** Contains 0+ DataElements (including other Groups)

```rust
impl Group {
    /// Check if group should be rendered (Footnote 4 compliance)
    pub fn should_render(&self) -> bool {
        self.name.is_some() 
            || self.description.is_some() 
            || !self.elements.is_empty()
    }
    
    /// Expand replicated group into individual instances
    pub fn expand_replications(&self, base_address: i32) -> Vec<ExpandedGroup> {
        (0..self.replication).map(|i| {
            ExpandedGroup {
                index: i,
                name: self.compute_repname(i),
                address: base_address + (i as i32 * self.size_per_replication()),
                elements: self.elements.clone(),
            }
        }).collect()
    }
}
```

---

### IntElement

Integer configuration element with optional constraints.

```rust
pub struct IntElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Size in bytes (1, 2, 4, or 8)
    pub size: u8,
    
    /// Memory offset from parent address
    pub offset: i32,
    
    /// Minimum allowed value (optional constraint)
    pub min: Option<i64>,
    
    /// Maximum allowed value (optional constraint)
    pub max: Option<i64>,
    
    /// Default value (optional)
    pub default: Option<i64>,
    
    /// Value mapping (e.g., 0="Inactive", 1="Active")
    pub map: Option<Map>,
}
```

**Validation:** `size` must be 1, 2, 4, or 8 bytes  
**Display:** Show constraints in Details Panel (e.g., "Range: 0-255")

---

### EventIdElement

Event ID configuration element (always 8 bytes).

```rust
pub struct EventIdElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory offset from parent address
    pub offset: i32,
}
```

**Fixed Size:** Event IDs are always 8 bytes per LCC standard  
**Display Importance:** Primary target for Event Bowties feature (F5)  
**UI Indicator:** Show distinctive icon/badge in Elements column

---

### StringElement

String configuration element with length constraint.

```rust
pub struct StringElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Maximum string length in bytes
    pub size: usize,
    
    /// Memory offset from parent address
    pub offset: i32,
}
```

**Validation:** String length must not exceed `size` bytes

---

### Map

Value-to-label mapping for constrained selections.

```rust
pub struct Map {
    /// Map entries (value → label)
    pub entries: Vec<MapEntry>,
}

pub struct MapEntry {
    /// Numeric value
    pub value: i64,
    
    /// User-visible label
    pub label: String,
}
```

**Example:** `[(0, "Inactive"), (1, "Active Hi"), (2, "Active Lo")]`  
**Display:** Show as dropdown options in future configuration editing

---

## UI State Entities (Navigation)

### NavigationStep

Single step in the navigation path (breadcrumb).

```typescript
interface NavigationStep {
    /// Hierarchy depth (0 = nodes, 1 = segments, 2+ = groups/elements)
    depth: number;
    
    /// Unique identifier for selected item at this depth
    itemId: string;
    
    /// Type of item (determines next column type)
    itemType: 'node' | 'segment' | 'group' | 'element';
    
    /// User-visible label for breadcrumb
    label: string;
}
```

**Example Path:**  
```typescript
[
  { depth: 0, itemId: 'node-01.02.03.04.05.06', itemType: 'node', label: 'Tower-LCC' },
  { depth: 1, itemId: 'seg-conditionals', itemType: 'segment', label: 'Conditionals' },
  { depth: 2, itemId: 'grp-logic-12', itemType: 'group', label: 'Logic 12' },
  { depth: 3, itemId: 'grp-var-1', itemType: 'group', label: 'Variable #1' },
  { depth: 4, itemId: 'elem-trigger', itemType: 'element', label: 'Trigger' }
]
```

**Purpose:** Breadcrumb rendering and back-navigation

---

### ColumnData

Content for a single column in the Miller Columns view.

```typescript
interface ColumnData {
    /// Column position (0 = nodes, 1 = segments, 2+ = groups/elements)
    depth: number;
    
    /// Column type (determines rendering behavior)
    type: 'nodes' | 'segments' | 'groups' | 'elements';
    
    /// Items to display in this column
    items: ColumnItem[];
    
    /// Path to parent (for context and caching)
    parentPath: string[];
}
```

**Lifecycle:** Created when navigating into a level, removed when navigating back

---

### ColumnItem

Individual selectable item within a column.

```typescript
interface ColumnItem {
    /// Unique identifier (used for selection and caching)
    id: string;
    
    /// Display name (may be truncated for long names)
    name: string;
    
    /// Data type for elements (e.g., "eventid", "int", "string")
    type?: string;
    
    /// Whether this item has children (determines if clicking adds column)
    hasChildren: boolean;
    
    /// Additional metadata (instance number, constraints, etc.)
    metadata?: Record<string, unknown>;
}
```

**Example - Replicated Group Instance:**
```typescript
{
    id: 'grp-input-7',
    name: 'Input 7',
    type: 'group',
    hasChildren: true,
    metadata: { instanceIndex: 6, replicationCount: 16 }
}
```

**Example - Event ID Element:**
```typescript
{
    id: 'elem-event-1',
    name: 'Command',
    type: 'eventid',
    hasChildren: false,
    metadata: { size: 8, offset: 2048 }
}
```

---

### ElementDetails

Detailed information for the Details Panel.

```typescript
interface ElementDetails {
    /// Element name (full, not truncated)
    name: string;
    
    /// Element description (from CDI)
    description: string | null;
    
    /// Data type (e.g., "Event ID (8 bytes)", "Integer (2 bytes)")
    dataType: string;
    
    /// Full navigation path for context
    fullPath: string;
    
    /// Constraints (min/max, map values, string length)
    constraints: Constraint[];
    
    /// Default value (if specified in CDI)
    defaultValue: string | null;
    
    /// Memory address (for reference, not user-editable)
    memoryAddress: number;
}

interface Constraint {
    /// Constraint type
    type: 'range' | 'map' | 'length';
    
    /// Human-readable description
    description: string;
    
    /// Structured data (e.g., { min: 0, max: 255 })
    value: Record<string, unknown>;
}
```

**Example - Event ID Element:**
```typescript
{
    name: 'Command',
    description: 'Event to trigger input action',
    dataType: 'Event ID (8 bytes)',
    fullPath: 'Tower-LCC › Port I/O › Line 7 › Command',
    constraints: [],
    defaultValue: null,
    memoryAddress: 2048
}
```

**Example - Integer with Constraints:**
```typescript
{
    name: 'Debounce Time',
    description: 'Time in milliseconds to debounce input',
    dataType: 'Integer (2 bytes)',
    fullPath: 'Tower-LCC › Port I/O › Line 7 › Debounce Time',
    constraints: [
        { 
            type: 'range', 
            description: 'Range: 0-1000 ms', 
            value: { min: 0, max: 1000 } 
        }
    ],
    defaultValue: '50',
    memoryAddress: 2056
}
```

---

## State Management (Svelte Store)

### MillerColumnsStore

Single source of truth for navigation state.

```typescript
class MillerColumnsStore {
    // Navigation path: breadcrumb trail
    private _path: NavigationStep[];
    
    // Visible columns (indexed by depth)
    private _columns: ColumnData[];
    
    // Selected item ID at each depth level
    private _selections: Map<number, string>;
    
    // Loading state per column
    private _loading: Map<number, boolean>;
    
    // Details panel content
    private _detailsContent: ElementDetails | null;
    
    // Data caching (avoid re-fetching)
    private columnCache: Map<string, ColumnData>;
    
    // Request cancellation
    private abortControllers: Map<number, AbortController>;
}
```

**Purpose:** Centralized reactive state for all components  
**Pattern:** Svelte 5 runes (`$state`) with class-based organization

---

## Relationships Diagram

```
Cdi
 └─ segments: Vec<Segment>
     └─ elements: Vec<DataElement>
         ├─ Group (RECURSIVE)
         │   └─ elements: Vec<DataElement>  ← Can contain more Groups!
         ├─ IntElement
         ├─ StringElement
         ├─ EventIdElement  ← PRIMARY TARGET for Event Bowties
         ├─ FloatElement
         ├─ ActionElement
         └─ BlobElement

NavigationStep[] (breadcrumb path)
 ↓
ColumnData[] (visible columns)
 └─ items: ColumnItem[] (selectable items)
     ↓ (when selected)
ElementDetails (details panel)
```

---

## Data Flow

```
1. User selects node in Nodes column
   → MillerColumnsStore.navigateTo(0, nodeId)
   → Fetch CDI XML from cache (Tauri command: get_cdi_xml)
   → Parse CDI XML to Cdi struct
   → Extract segments → Create ColumnData with ColumnItem[] for segments
   → Display Segments column

2. User selects segment
   → MillerColumnsStore.navigateTo(1, segmentId)
   → Analyze segment.elements:
      - If contains Groups → Create ColumnData with type='groups'
      - If contains only primitives → Create ColumnData with type='elements'
   → Display Groups/Elements column
   → Clear any subsequent columns

3. User selects group (replication=16)
   → MillerColumnsStore.navigateTo(2, groupId)
   → Expand replications: Group → 16 ExpandedGroup instances
   → Create ColumnData with 16 ColumnItem[] ("Line 1"..."Line 16")
   → Display expanded Groups column

4. User selects group instance
   → MillerColumnsStore.navigateTo(3, instanceId)
   → Analyze group.elements:
      - If contains nested Groups → Add another Groups column
      - If contains primitives → Create Elements column
   → Display next column

5. User selects element (Event ID)
   → MillerColumnsStore.navigateTo(4, elementId)
   → Create ElementDetails from EventIdElement
   → Display in Details Panel
   → No new column (elements are leaf nodes)
```

---

## Validation Rules

### CDI Structure Validation

1. **Empty Group Filtering** (S-9.7.4.1 Footnote 4):
   ```rust
   Group::should_render() → false if:
     - name is None AND
     - description is None AND
     - elements is empty
   ```

2. **Element Size Validation**:
   - `IntElement.size` ∈ {1, 2, 4, 8}
   - `EventIdElement` always 8 bytes (implicit)
   - `StringElement.size` > 0

3. **Replication Validation**:
   - `Group.replication` >= 1 (default: 1)
   - Maximum tested: 32 instances (Tower-LCC Conditionals)

### Navigation State Validation

1. **Depth Consistency**:
   - `NavigationStep[i].depth == i` for all steps
   - `ColumnData[i].depth == i` for all columns

2. **Column Type Transitions**:
   - Node → Segments (always)
   - Segment → Groups OR Elements (depends on content)
   - Group → Groups (nested) OR Elements (depends on content)
   - Element → Nothing (leaf node)

3. **Selection Integrity**:
   - `_selections.get(depth)` must exist in `_columns[depth].items`

---

## Performance Considerations

### Data Size Estimates

- **Typical CDI**: 5-20 segments, 50-200 total elements → <50 KB XML
- **Large CDI (Tower-LCC)**: 5 segments, 32 replicated groups, 8 levels deep → ~100 KB XML
- **Extreme Edge Case**: 100 replicated groups, 1000 total elements → ~500 KB XML

### Optimization Strategies

1. **Lazy Expansion**: Expand replications only when navigating into group
2. **Column Caching**: Cache `ColumnData` by `${depth}:${parentId}` key
3. **Request Cancellation**: AbortController per depth level
4. **Progressive Rendering**: Render first 50 items immediately, defer rest for >100 items

### Memory Trade-offs

- **Expand Replications Early** (chosen): Higher memory, simpler UI code
- **Expand On-Demand**: Lower memory, more complex navigation logic

**Decision:** Expand early for initial implementation (simplicity). Optimize later if memory issues observed.

---

## TypeScript/Rust Type Mappings

| Rust Type | TypeScript Equivalent | Notes |
|-----------|----------------------|-------|
| `Vec<T>` | `T[]` | Arrays |
| `Option<T>` | `T \| null` | Nullable values |
| `String` | `string` | UTF-8 strings |
| `u8`, `u32`, `i32`, `i64` | `number` | JavaScript numbers |
| `enum DataElement` | TypeScript union type | Discriminated union |
| `HashMap<K, V>` | `Map<K, V>` or `Record<K, V>` | Tauri serializes to objects |

**Serialization:** Tauri uses `serde_json` for Rust ↔ TypeScript conversion

---

## Summary

**Domain Model:** CDI structures (Cdi → Segment → Group → Element) with recursive groups  
**UI Model:** Navigation state (path, columns, selections) in single reactive store  
**Key Features:** Replication expansion, empty group filtering, recursive nesting  
**Performance:** Caching, request cancellation, progressive rendering for large lists  
**Type Safety:** Rust enums + TypeScript interfaces ensure correctness

**Data model complete. Ready for contracts generation.**
