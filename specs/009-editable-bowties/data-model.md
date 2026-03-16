# Data Model: Editable Bowties

**Feature Branch**: `009-editable-bowties`  
**Date**: 2026-03-15

## Entities

### 1. LayoutFile (YAML Persistence)

The user-managed YAML file storing bowtie metadata and role classifications.

```yaml
# Example: my-layout.bowties.yaml
schemaVersion: "1.0"
bowties:
  "05.01.01.01.FF.00.00.01":
    name: "Yard Entry Signal"
    tags: ["yard", "signals"]
  "05.01.01.01.FF.00.00.02":
    name: "Main Turnout Control"
    tags: ["mainline", "turnouts"]
  "05.01.01.01.FF.00.00.03":
    name: "Block Occupancy Detector"
    tags: []
roleClassifications:
  "05.02.01.02.03.00:Port I/O/Line #1/Event Produced":
    role: "Producer"
  "05.02.01.02.03.00:Port I/O/Line #1/Event Consumed":
    role: "Consumer"
```

#### Rust Types (Backend)

```rust
/// Root structure for the YAML layout file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFile {
    pub schema_version: String,  // "1.0"
    #[serde(default)]
    pub bowties: BTreeMap<String, BowtieMetadata>,  // key: event_id dotted hex
    #[serde(default)]
    pub role_classifications: BTreeMap<String, RoleClassification>,  // key: "{nodeId}:{path}"
}

/// Metadata for a single bowtie, stored in layout YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BowtieMetadata {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// User-provided role classification for an ambiguous event slot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleClassification {
    pub role: String,  // "Producer" | "Consumer"
}
```

#### TypeScript Types (Frontend)

```typescript
/** Root structure matching YAML layout file */
interface LayoutFile {
  schemaVersion: string;
  bowties: Record<string, BowtieMetadata>;  // key: event_id dotted hex
  roleClassifications: Record<string, RoleClassification>;  // key: "{nodeId}:{path}"
}

/** Metadata for a single bowtie */
interface BowtieMetadata {
  name?: string;
  tags: string[];
}

/** User-provided role for an ambiguous event slot */
interface RoleClassification {
  role: 'Producer' | 'Consumer';
}
```

#### Validation Rules
- `schemaVersion` must be `"1.0"`
- Bowtie keys must match event ID dotted hex format: `^[0-9A-Fa-f]{2}(\.[0-9A-Fa-f]{2}){7}$`
- Role classification keys must match `{nodeId}:{path}` format
- `role` must be `"Producer"` or `"Consumer"`
- `tags` elements are free-form strings, no duplicates within a bowtie
- `name` is optional free-form text

---

### 2. BowtieCard (Extended Runtime Entity)

Extends the existing `BowtieCard` from `state.rs` with state tracking.

#### Rust Type Changes

```rust
/// Bowtie state reflecting current element membership
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BowtieState {
    Active,      // has ≥1 producer AND ≥1 consumer
    Incomplete,  // has elements but one side is empty
    Planning,    // no elements attached (name-only, intent-first)
}

/// Extended BowtieCard (existing fields + new fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BowtieCard {
    // --- Existing fields ---
    pub event_id_hex: String,
    pub event_id_bytes: [u8; 8],
    pub producers: Vec<EventSlotEntry>,
    pub consumers: Vec<EventSlotEntry>,
    pub ambiguous_entries: Vec<EventSlotEntry>,

    // --- New fields ---
    pub name: Option<String>,         // from layout YAML metadata
    pub tags: Vec<String>,            // from layout YAML metadata
    pub state: BowtieState,           // derived from element counts
}
```

#### State Transitions

```
planning  ──[add first element to one side]──►  incomplete
planning  ──[add elements to both sides]──────►  active
incomplete ──[add element to empty side]──────►  active
active     ──[remove last from one side]──────►  incomplete
incomplete ──[remove all elements]────────────►  planning (user confirms) OR deleted
active     ──[remove all elements]────────────►  planning (user confirms) OR deleted
```

#### TypeScript Type

```typescript
type BowtieState = 'active' | 'incomplete' | 'planning';

interface BowtieCard {
  event_id_hex: string;
  event_id_bytes: number[];
  producers: EventSlotEntry[];
  consumers: EventSlotEntry[];
  ambiguous_entries: EventSlotEntry[];
  name?: string;
  tags: string[];
  state: BowtieState;
}
```

---

### 3. BowtieEdit (Frontend Pending Edit for Metadata)

Tracks unsaved changes to bowtie metadata (names, tags, role classifications). Parallels `PendingEdit` from the config store but for YAML-only changes.

```typescript
type BowtieEditKind =
  | { type: 'create'; eventIdHex: string; name?: string }
  | { type: 'delete'; eventIdHex: string }
  | { type: 'rename'; eventIdHex: string; oldName?: string; newName: string }
  | { type: 'addTag'; eventIdHex: string; tag: string }
  | { type: 'removeTag'; eventIdHex: string; tag: string }
  | { type: 'classifyRole'; key: string; role: 'Producer' | 'Consumer' };

interface BowtieMetadataEdit {
  id: string;              // unique per edit
  kind: BowtieEditKind;
  timestamp: number;
}
```

---

### 4. BowtieMetadataStore (Frontend Store)

Manages unsaved bowtie metadata changes. Works alongside `pendingEditsStore`.

```typescript
class BowtieMetadataStore {
  // --- State ---
  private _edits: Map<string, BowtieMetadataEdit>;   // key varies by kind
  private _layoutFile: LayoutFile | null;             // loaded file
  private _layoutPath: string | null;                 // file path
  private _isDirty: boolean;                          // any unsaved changes

  // --- Mutations ---
  createBowtie(eventIdHex: string, name?: string): void;
  deleteBowtie(eventIdHex: string): void;
  renameBowtie(eventIdHex: string, newName: string): void;
  addTag(eventIdHex: string, tag: string): void;
  removeTag(eventIdHex: string, tag: string): void;
  classifyRole(key: string, role: 'Producer' | 'Consumer'): void;
  reclassifyRole(key: string, newRole: 'Producer' | 'Consumer'): void;
  clearAll(): void;

  // --- Queries ---
  get isDirty(): boolean;
  get hasPendingEdits(): boolean;
  getMetadata(eventIdHex: string): BowtieMetadata | undefined;
  getRoleClassification(key: string): RoleClassification | undefined;
  getAllTags(): string[];            // for auto-suggest

  // --- Layout File Operations ---
  loadLayout(path: string): Promise<void>;
  saveLayout(path?: string): Promise<void>;   // path for Save As
  get layoutPath(): string | null;
  get isLayoutLoaded(): boolean;
}
```

---

### 5. EditableBowtiePreview (Derived View)

A computed/derived view that merges the live `BowtieCatalog` with pending config edits and pending metadata edits to produce the current user-visible bowtie state.

```typescript
interface EditableBowtiePreview {
  bowties: PreviewBowtieCard[];
  hasUnsavedChanges: boolean;    // any dirty in either store
}

interface PreviewBowtieCard extends BowtieCard {
  isDirty: boolean;             // this card has unsaved changes
  dirtyFields: Set<string>;     // which fields are dirty (name, tags, elements)
}
```

This is a derived computation, not a store. It is recomputed reactively from:
- `bowtieCatalogStore.catalog` (live discovered bowties)
- `bowtieMetadataStore` (pending metadata changes)
- `pendingEditsStore` (pending event ID value changes)

---

### 6. NewConnectionRequest (IPC Payload)

Payload sent from frontend to backend when creating a new bowtie connection.

```typescript
/** Request to create a new bowtie connection */
interface CreateConnectionRequest {
  producer?: ElementSelection;   // null for intent-first
  consumer?: ElementSelection;   // null for intent-first
  name?: string;
}

/** A selected element for one side of a connection */
interface ElementSelection {
  nodeId: string;               // dotted hex
  elementPath: string[];        // CDI path segments
  address: number;              // memory address of the event slot
  space: number;                // memory space (0xFD)
  currentEventId: string;       // dotted hex of current slot value
}

/** Result of event ID selection rule evaluation */
interface EventIdResolution {
  eventIdHex: string;           // the event ID to use
  writeTo: 'producer' | 'consumer' | 'both' | 'none';
  conflictPrompt?: {            // present if both sides already connected
    producerBowtie: string;     // name or event ID of existing bowtie
    consumerBowtie: string;
  };
}
```

---

### 7. WriteOperation (Multi-Node Write Tracking)

Tracks the state of a multi-node write operation with rollback support.

```typescript
interface WriteOperation {
  id: string;
  steps: WriteStep[];
  status: 'pending' | 'writing' | 'completed' | 'partial-failure' | 'rolled-back' | 'rollback-failed';
}

interface WriteStep {
  nodeId: string;
  address: number;
  space: number;
  originalValue: number[];      // for rollback
  newValue: number[];
  status: 'pending' | 'writing' | 'success' | 'failed' | 'rolled-back' | 'rollback-failed';
  error?: string;
}
```

---

### 8. RecentLayout (App Preferences)

```rust
/// Stored in app_data_dir/recent-layout.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentLayout {
    pub path: String,
    pub last_opened: String,  // ISO 8601
}
```

```typescript
interface RecentLayout {
  path: string;
  lastOpened: string;
}
```

---

## Entity Relationships

```
LayoutFile (YAML on disk)
  ├── bowties: {eventIdHex → BowtieMetadata}
  └── roleClassifications: {nodeId:path → RoleClassification}
        │
        ▼ (loaded into)
BowtieMetadataStore (frontend, tracks unsaved changes)
        │
        ▼ (merged with)
BowtieCatalog (built from live node data)
  └── BowtieCard[] (discovered from shared event IDs)
        │
        ▼ (produces)
EditableBowtiePreview (derived view for UI)
  └── PreviewBowtieCard[] (live + metadata + pending edits)

PendingEditsStore (event slot value changes)
  └── PendingEdit[] (config tab OR bowtie tab origin)
        │
        ▼ (feeds into)
SaveControls (unified Save/Discard across both stores)
```

## Key Invariants

1. **Event ID as stable key**: A bowtie's identity is its shared event ID. The layout file uses this as the key. If event IDs change, the mapping may break — the bowtie shows "unresolved."
2. **No orphan writes**: Every event slot write must be associated with either a pending edit (config) or a bowtie operation. No writes happen without user-visible tracking.
3. **Unified dirty state**: `hasUnsavedChanges = pendingEditsStore.hasPendingEdits || bowtieMetadataStore.isDirty`
4. **Atomic YAML save**: Layout file writes use temp-file-then-rename pattern (matching existing `save_connection_prefs`).
5. **Role classification precedence**: Profile > user classification > heuristic > ambiguous
