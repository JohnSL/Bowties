# Data Model: Configuration Tab — Sidebar and Element Card Deck

**Feature**: 005-config-sidebar-view  
**Phase**: 1 — Design  
**Date**: 2026-02-22  

---

## Entities

### NodeEntry (Frontend — UI State)

Represents a discovered LCC node in the sidebar. Sourced from `nodeInfoStore` (`DiscoveredNode`).

| Field | Type | Source / Notes |
|-------|------|---------------|
| `nodeId` | `string` | Dotted-hex (e.g., `"02.01.57.00.00.01"`); primary key |
| `nodeName` | `string` | `snip_data.user_name` if non-empty, else `"Node {nodeId}"` |
| `manufacturer` | `string \| null` | `snip_data.manufacturer`; shown as secondary detail to distinguish duplicate names |
| `model` | `string \| null` | `snip_data.model`; shown as secondary detail |
| `isExpanded` | `boolean` | Managed by `configSidebarStore.expandedNodeIds`; FR-015: preserved across segment selections |
| `connectionStatus` | `ConnectionStatus` | From `DiscoveredNode.connection_status`; `'NotResponding'` triggers offline indicator |
| `hasCdi` | `boolean` | `DiscoveredNode.cdi !== null`; false → segments cannot be listed; clicking shows download prompt |

**State transitions**:
- `collapsed` → `expanded`: user click; triggers `getCdiStructure(nodeId)` if segments not yet loaded
- `expanded` → `collapsed`: user click on already-expanded node
- `expanded + hasCdi = false`: shows "No CDI available — Download CDI from Node" inline prompt

---

### SegmentEntry (Frontend — UI State)

A CDI memory segment within an expanded node. Clicking it populates the card deck.

| Field | Type | Source / Notes |
|-------|------|---------------|
| `segmentId` | `string` | Backend-assigned segment path ID (e.g., segment index or name slug) |
| `segmentName` | `string` | CDI `<segment name="…">` text per FR-004; displayed verbatim |
| `description` | `string \| null` | CDI segment description; shown as tooltip |
| `space` | `number` | LCC memory space number (e.g., `253`) |
| `nodeId` | `string` | Parent node ID (for selection tracking) |
| `isSelected` | `boolean` | True when this segment is the active selection; one selected at a time globally |

**Validation**: Segment must have `segmentName` or a CDI path to be selectable; a segment with neither shows a fallback label `"Segment {space}"`.

---

### ElementCard (Frontend — UI State)

One accordion card in the main content area. Represents one top-level CDI `<group>` within the selected segment. FR-006: one card per top-level group.

| Field | Type | Source / Notes |
|-------|------|---------------|
| `cardId` | `string` | `groupPath.join('/')` — unique within a card deck |
| `groupPath` | `string[]` | Full CDI navigation path from segment root to this group |
| `cdGroupName` | `string` | Raw CDI group name (e.g., `"Line"`, `"Port I/O"`); shown in parentheses per FR-007 |
| `instanceIndex` | `number \| null` | 1-based instance number for replicated groups (e.g., `3` for "Line 3"); `null` for non-replicated |
| `isReplicated` | `boolean` | True when `group.replication > 1` |
| `userGivenName` | `string \| null` | Resolved by `resolveCardTitle()` from config cache; null if cache miss or unset |
| `cardTitle` | `string` | Computed per FR-007; see naming algorithm below |
| `isExpanded` | `boolean` | Accordion open/close; `false` by default on segment load (FR-008) |
| `elements` | `CardElementTree \| null` | Loaded lazily when card first expanded; `null` until `get_card_elements` resolves |
| `isLoading` | `boolean` | True while `get_card_elements` in flight |
| `loadError` | `string \| null` | Error message if element tree load failed |

#### Card Title Naming Algorithm (FR-007)

```
resolveCardTitle(group, nodeId, configValues):
  userGivenName = lookup first StringElement child named "User Name" / "Name" / "Description"
                  from configValues; null if missing, empty, or only whitespace/null-bytes

  if group.isReplicated:
    cdiLabel = "{cdGroupName} {instanceIndex}"    // e.g., "Line 3"
    return userGivenName
      ? "{userGivenName} ({cdiLabel})"            // "Yard Button (Line 3)"
      : "{cdiLabel} (unnamed)"                    // "Line 3 (unnamed)"
  else:
    return userGivenName
      ? "{userGivenName} ({cdGroupName})"         // "Yard Button (Port I/O)"
      : cdGroupName                               // "Port I/O"
```

---

### FieldRow (Frontend — View Element)

A single read-only configuration field within an element card body. Corresponds to a CDI leaf element (`IntElement`, `StringElement`, `FloatElement`).

| Field | Type | Source / Notes |
|-------|------|---------------|
| `elementPath` | `string[]` | Full CDI path from root; used as cache key and for [R] action |
| `label` | `string` | CDI element `name`; display label |
| `dataType` | `'int' \| 'string' \| 'float'` | Determines value rendering |
| `currentValue` | `ConfigValue \| null` | From `millerColumnsStore.configValues`; null if not yet read |
| `description` | `string \| null` | CDI element description text; hidden by default (FR-012) |
| `isDescriptionExpanded` | `boolean` | Controlled by [?] toggle; false by default (FR-012) |
| `memoryAddress` | `number` | Absolute address (from `CardField.memoryAddress`); passed to `readConfigValue` for [R] |

**Validation**:
- No editing allowed (FR-010); field is display-only
- [R] calls `readConfigValue(nodeId, elementPath)` → result stored in `millerColumnsStore.configValues`

---

### EventSlotRow (Frontend — View Element)

Specialised field row for LCC event ID slots. Corresponds to a CDI `EventIdElement`.

| Field | Type | Source / Notes |
|-------|------|---------------|
| `elementPath` | `string[]` | Full CDI path; used for cache lookup and [R] action |
| `label` | `string` | CDI element name |
| `rawEventId` | `string \| null` | Dotted-hex event ID (e.g., `"01.02.03.04.05.06.07.08"`); null if not read |
| `isFree` | `boolean` | True if value matches the all-zeros or LCC "unset" default pattern (FR-014) |
| `displayValue` | `string` | `"(free)"` when `isFree`; `rawEventId` otherwise; `"—"` if not read |
| `isDescriptionExpanded` | `boolean` | Controlled by [?] toggle; false by default |
| `description` | `string \| null` | CDI element description text |

**Validation**:
- Event IDs formatted as dotted-hex (Constitution §VII)
- FR-014: default/zero event ID displays as `"(free)"` not raw bytes

---

### CardElementTree (Frontend — Data Transfer / Backend Response)

Recursive tree returned by `get_card_elements`. Represents the complete element hierarchy within a CDI group, flattened only enough for ordered inline rendering per FR-011.

```typescript
interface CardElementTree {
  groupName: string | null;
  groupDescription: string | null;
  fields: CardField[];        // Leaf elements directly in this group (in CDI order)
  subGroups: CardSubGroup[];  // Sub-groups (rendered inline, fully expanded — FR-011)
}

interface CardField {
  elementPath: string[];      // Full CDI path from root
  name: string;               // CDI element name
  description: string | null;
  dataType: 'int' | 'string' | 'eventid' | 'float' | 'action' | 'blob';
  memoryAddress: number;      // Absolute address (for readConfigValue)
  sizeBytes: number;
  defaultValue: string | null;
}

interface CardSubGroup {
  name: string;
  description: string | null;
  groupPath: string[];
  fields: CardField[];
  subGroups: CardSubGroup[];  // Recursive — arbitrary depth supported
}
```

---

### ConfigSidebarState (Frontend — Svelte Store)

Managed by `configSidebarStore` in `app/src/lib/stores/configSidebar.ts`.

```typescript
interface ConfigSidebarState {
  // FR-015: Preserved across segment selections within a session
  expandedNodeIds: string[];

  // Currently selected segment (one at a time globally)
  selectedSegment: { nodeId: string; segmentId: string } | null;

  // Card deck for the selected segment
  cardDeck: ConfigSidebarCardDeck | null;

  // Loading state per node while fetching segments (e.g., after first expansion)
  nodeLoadingStates: Record<string, 'idle' | 'loading' | 'error'>;

  // Error message per node (null = no error)
  nodeErrors: Record<string, string | null>;
}

interface ConfigSidebarCardDeck {
  nodeId: string;
  segmentId: string;
  segmentName: string;
  cards: CardData[];
  expandedCardIds: string[];  // Cards currently open (default: none — FR-008)
  isLoading: boolean;         // True while top-level groups are being fetched
  error: string | null;
}

interface CardData {
  cardId: string;
  groupPath: string[];
  cdGroupName: string;
  isReplicated: boolean;
  instanceIndex: number | null;
  cardTitle: string;           // Pre-computed per FR-007
  elements: CardElementTree | null;  // Null until card expanded
  isLoading: boolean;
  loadError: string | null;
}
```

---

## State Machine: Sidebar Lifecycle

```
[INITIAL]
  → nodeInfoStore is empty
  → Sidebar: "No nodes discovered…" (FR-005 empty state)
  → Main area: "Select a segment to view configuration"

[NODES_LOADED]
  → nodeInfoStore populated (after Discover Nodes)
  → Sidebar: all nodes listed, all collapsed
  → configSidebarStore.expandedNodeIds = []

[NODE_EXPANDED]
  → User clicks a collapsed node
  → Fetch getCdiStructure(nodeId) → segments[]
  → configSidebarStore.expandedNodeIds adds nodeId
  → Node renders segment list; nodeLoadingStates[nodeId] = 'idle'

[SEGMENT_SELECTED]
  → User clicks a segment
  → configSidebarStore.selectedSegment = { nodeId, segmentId }
  → Fetch getColumnItems(nodeId, [segmentId], 2) → top-level groups
  → Build CardData[] (cards collapsed by default — FR-008)
  → cardDeck loaded; isLoading = false

[CARD_EXPANDED]
  → User clicks card header
  → If elements === null: fetch get_card_elements(nodeId, groupPath)
  → cardDeck.expandedCardIds adds cardId
  → Card body renders FieldRow / EventSlotRow from configValues cache

[FIELD_REFRESHED]
  → User clicks [R] on a field
  → Call readConfigValue(nodeId, elementPath)
  → millerColumnsStore.configValues updated reactively
  → FieldRow re-renders with fresh value

[NODE_REFRESH]
  → Discover/Refresh Nodes triggered
  → configSidebarStore.reset() called from config +page.svelte
  → Returns to [INITIAL]
```

---

## Relationships

```
nodeInfoStore (DiscoveredNode)
  └── NodeEntry (1:1, UI projection)
        └── SegmentEntry (1:N, loaded on expansion)
              └── ElementCard (1:N, loaded on segment selection)
                    ├── FieldRow (1:N, from CardElementTree.fields)
                    ├── EventSlotRow (1:N, from CardElementTree.fields where dataType='eventid')
                    └── CardSubGroup → FieldRow / EventSlotRow (recursive, FR-011)

millerColumnsStore.configValues (ConfigValueMap) ─── read by ──→ FieldRow.currentValue
                                                 ─── written by ──→ [R] action (readConfigValue)
```
