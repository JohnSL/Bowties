# Data Model: Bowties Tab — Discover Existing Connections

**Feature**: `006-bowties-event-discovery`  
**Date**: 2026-02-22

---

## Entities

### 1. `EventRole` (lcc-rs, `cdi/role.rs`)

Rust enum that classifies a single event ID slot's role based on its CDI context.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventRole {
    Producer,
    Consumer,
    Ambiguous,  // excluded from bowtie discovery
}
```

**Validation rules**:
- Derived from parent group names + element description text (two-tier heuristic — see research.md RQ-2).
- `Ambiguous` is the default when neither tier fires.
- No I/O; pure function over string slices.

---

### 2. `EventSlotEntry` (Tauri backend, `commands/bowties.rs`)

A single classified event ID configuration field from one node.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSlotEntry {
    /// Node identifier (dotted-hex, e.g. "02.01.57.00.00.01")
    pub node_id: String,

    /// Human-readable node name (SNIP user_name, or node model, or node_id fallback)
    pub node_name: String,

    /// CDI element path (e.g. ["Segment 1", "Producers", "Track 1", "Output Active"])
    pub element_path: Vec<String>,

    /// Display label derived from CDI, in priority order:
    ///   1. CDI element `<name>` (if non-empty)
    ///   2. First sentence of CDI element `<description>` (if non-empty)
    ///   3. Slash-joined `element_path` as fallback
    /// This mirrors the "Also Known As" intent in JMRI's EventTable, but
    /// sourced directly from CDI rather than from JMRI-managed objects.
    pub element_label: String,

    /// The 8-byte event ID value currently stored in this slot
    pub event_id: [u8; 8],

    /// Classified role.
    /// Producer or Consumer when determined by Identify Events protocol (cross-node case)
    /// or confirmed by CDI heuristic (same-node case).
    /// Ambiguous when same-node AND CDI heuristic inconclusive — these entries go
    /// into BowtieCard.ambiguous_entries, not producers/consumers.
    pub role: EventRole,
}
```

**Validation rules**:
- `event_id` must be exactly 8 bytes.
- `role` MUST be `Producer` or `Consumer` — `Ambiguous` slots are never stored in this struct (filtered before creation).
- `element_label` priority: CDI `<name>` non-empty → CDI `<description>` first sentence → slash-joined `element_path`.
- `node_name` priority (matches JMRI `EventTablePane.recordProducer` convention): SNIP `user_name` → `"{mfg_name} — {model_name}"` → dotted-hex `node_id`.

---

### 3. `NodeRoles` (Tauri backend, `AppState`)

Protocol-level ground truth from the Identify Events exchange — which nodes replied as producer vs consumer for a given event ID.

```rust
#[derive(Debug, Clone, Default)]
pub struct NodeRoles {
    /// Node IDs that replied Producer Identified for this event
    pub producers: HashSet<String>,   // dotted-hex node_id
    /// Node IDs that replied Consumer Identified for this event
    pub consumers: HashSet<String>,
}
```

Stored in `AppState`:
```rust
/// Map from event_id_hex → protocol-confirmed node roles.
/// Keyed on the event ID that appeared in Producer/Consumer Identified replies.
/// Populated by sending IdentifyEventsAddressed to each known node (125 ms apart),
/// then collecting all replies within a configurable window (default 500 ms).
pub event_roles: Arc<RwLock<HashMap<String, NodeRoles>>>,
```

---

### 4. `BowtieCard` (Tauri backend, `commands/bowties.rs`)

The primary unit of display — one shared event ID with ≥1 confirmed producer and ≥1 confirmed consumer.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BowtieCard {
    /// The shared event ID in dotted-hex notation (e.g. "05.02.01.02.03.00.00.01")
    /// Used as the unique key and default card header.
    pub event_id_hex: String,

    /// Raw 8-byte event ID (for sorting/comparisons)
    pub event_id_bytes: [u8; 8],

    /// All confirmed producer slots (≥1 guaranteed)
    pub producers: Vec<EventSlotEntry>,

    /// All confirmed consumer slots (≥1 guaranteed)
    pub consumers: Vec<EventSlotEntry>,

    /// Slots whose role could not be determined (same-node, heuristic inconclusive).
    /// Shown in the card UI as "Unknown role — needs clarification".
    /// Future phase: user can clarify and persist the decision.
    pub ambiguous_entries: Vec<EventSlotEntry>,

    /// User-assigned name (None in this phase — name storage is out of scope)
    pub name: Option<String>,
}
```

**`EventSlotEntry.role` values within a card**:
- `producers` list: always `Producer`
- `consumers` list: always `Consumer`
- `ambiguous_entries` list: always `Ambiguous` — these are same-node slots where the CDI heuristic could not determine the role

**Derived field**:
- `display_name()`: returns `name` if `Some(_)`, otherwise `event_id_hex`. Used as the card header (FR-014).

**Validation rules**:
- A `BowtieCard` MUST have `producers.len() >= 1` AND `consumers.len() >= 1`. Cards where the protocol produced no confirmed sides are never created.
- `event_id_hex` is the 8 bytes formatted as `XX.XX.XX.XX.XX.XX.XX.XX` (uppercase hex, dot-separated).
- No duplicate `EventSlotEntry` within the same card (same node_id + element_path).
- `ambiguous_entries` may be empty (most cards will have none).

**State transitions**: In this phase, cards are created by builder and are read-only. Future phases: user can assign a `name` and clarify `ambiguous_entries` roles.

---

### 5. `BowtieCatalog` (Tauri backend / `AppState`)

The complete in-memory collection of discovered bowties for the current session.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BowtieCatalog {
    /// Bowties sorted by event_id_bytes (lexicographic)
    pub bowties: Vec<BowtieCard>,

    /// Timestamp of last rebuild (ISO 8601)
    pub built_at: String,

    /// Number of nodes whose data was included in this catalog
    pub source_node_count: usize,

    /// Total event slots scanned (including excluded ambiguous/unmatched)
    pub total_slots_scanned: usize,
}
```

**State transitions**:
- `None` → `Some(BowtieCatalog)` when the first full CDI read completes.
- Replaced atomically on every subsequent full refresh.
- Never partially updated (atomic swap only).

---

### 6. Frontend Svelte Store Types (`app/src/lib/stores/bowties.ts`)

```typescript
/** Mirror of Rust BowtieCard for the frontend */
export interface BowtieCard {
  event_id_hex: string;              // "05.02.01.02.03.00.00.01"
  event_id_bytes: number[];          // 8-byte array
  producers: EventSlotEntry[];
  consumers: EventSlotEntry[];
  ambiguous_entries: EventSlotEntry[]; // same-node, role unknown
  name: string | null;
}

export interface EventSlotEntry {
  node_id: string;
  node_name: string;
  element_path: string[];
  element_label: string;
  event_id: number[];          // 8 bytes
  role: 'Producer' | 'Consumer';
}

export interface BowtieCatalog {
  bowties: BowtieCard[];
  built_at: string;            // ISO 8601
  source_node_count: number;
  total_slots_scanned: number;
}
```

**Stores**:
```typescript
// Populated via get_bowties command after cdi-read-complete event
export const bowtieCatalogStore = writable<BowtieCatalog | null>(null);

// True when all CDI reads for all discovered nodes have completed
export const cdiReadCompleteStore = writable<boolean>(false);

// Derived: Map<eventIdHex, BowtieCard> for O(1) lookup in EventSlotRow
export const usedInMap: Readable<Map<string, BowtieCard>>;
```

---

## Relationships

```
AppState.nodes (Vec<DiscoveredNode>)               AppState.event_roles (HashMap<event_id_hex, NodeRoles>)
  └─ each DiscoveredNode.cdi_values                    └─ populated by Identify Events exchange
       └─ per-field: if eventid                              (Producer/Consumer Identified replies)
            │
            └─ combined in build_bowtie_catalog():
                 node_is_producer = event_roles[V].producers.contains(node_id)
                 node_is_consumer = event_roles[V].consumers.contains(node_id)
                 cross-node: role = Producer | Consumer (definitive)
                 same-node:  role = CDI heuristic → Producer | Consumer | Ambiguous
                      │
                      └─ grouped by event_id_bytes
                           └─ if ≥1 confirmed producer AND ≥1 confirmed consumer
                                → BowtieCard { producers, consumers, ambiguous_entries }
                                     └─ collected into BowtieCatalog
```

---

## CDI Role Classification Context

The `classify_event_slot()` function in `lcc-rs/src/cdi/role.rs` is called **only in the same-node case** (node replied both `Producer Identified` and `Consumer Identified`). It receives:
1. The `EventIdElement` struct (for `name` and `description`).
2. A slice of ancestor group names from the CDI tree root to the element's parent (outermost-first).

Cross-node classification uses protocol replies directly — no CDI heuristic needed.
