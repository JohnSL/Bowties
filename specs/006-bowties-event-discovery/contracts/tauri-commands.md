# Tauri Command Contracts

**Feature**: `006-bowties-event-discovery`

---

## New Commands

### `get_bowties`

Returns the current `BowtieCatalog` from `AppState`. Returns `null` if no catalog has been built yet (i.e., CDI reads haven't completed).

**Rust signature** (`app/src-tauri/src/commands/bowties.rs`):
```rust
#[tauri::command]
pub async fn get_bowties(
    state: tauri::State<'_, AppState>,
) -> Result<Option<BowtieCatalog>, String>
```

**Frontend call** (`app/src/lib/api/tauri.ts`):
```typescript
export async function getBowties(): Promise<BowtieCatalog | null> {
  return invoke<BowtieCatalog | null>('get_bowties');
}
```

**Returns**: `BowtieCatalog | null`
- `null` — CDI read not yet complete; no catalog built
- `BowtieCatalog` with `bowties: []` — reads complete but no shared event IDs found (empty state)
- `BowtieCatalog` with `bowties.length > 0` — one or more bowties to display

---

## New Events (Tauri → Frontend)

### `cdi-read-complete` (new event)

Emitted by the backend after **both** phases are done: (1) all `read_all_config_values` calls complete, AND (2) the Identify Events exchange has been performed and the `BowtieCatalog` has been built.

**Two-phase flow**:
1. Last `read_all_config_values` returns (`node_index + 1 == total_nodes`).
2. Backend sends `IdentifyEventsAddressed` to each known node (125 ms between sends) — one message per node retrieves all of that node's producer and consumer events.
3. Backend collects `Producer Identified` / `Consumer Identified` replies for all nodes within a collection window (default: 500 ms after last send).
4. Backend runs `build_bowtie_catalog()` over the combined CDI + protocol data.
5. Backend emits `cdi-read-complete` with the finished catalog.

> **Reference**: This mirrors JMRI's `EventTablePane.sendRequestEvents()` which sends `IdentifyEventsAddressedMessage` to each node in `MimicNodeStore` at 125 ms intervals. Using addressed-per-node (vs per-event-ID broadcast) is more efficient: |nodes| sends vs 2×|event IDs| sends.

**Payload**:
```typescript
interface CdiReadCompletePayload {
  catalog: BowtieCatalog;  // freshly-built catalog
  node_count: number;       // how many nodes were included
}
```

**Frontend listener** (in `routes/bowties/+page.svelte` or root `+layout.svelte`):
```typescript
const unlisten = await listen<CdiReadCompletePayload>('cdi-read-complete', (event) => {
  cdiReadCompleteStore.set(true);
  bowtieCatalogStore.set(event.payload.catalog);
});
```

---

## Existing Commands — No Signature Changes

These commands are unchanged but their outputs feed the bowtie builder:

| Command | Used by bowtie feature |
|---|---|
| `read_all_config_values` | Triggers `cdi-read-complete` event after last node; backend reads config values from `AppState.nodes` to build catalog |
| `discover_nodes` | Populates `AppState.nodes` — bowtie builder iterates these |
| `get_card_elements` / `get_segment_elements` | Source of `element_path` and `element_label` for `EventSlotEntry` |

---

## Backend — New AppState Fields

```rust
// state.rs additions
pub bowties_catalog: Arc<RwLock<Option<BowtieCatalog>>>,

/// Node-level producer/consumer roles from Identify Events exchange.
/// Key = event_id_hex (e.g. "05.02.01.02.03.00.00.01")
pub event_roles: Arc<RwLock<HashMap<String, NodeRoles>>>,
```

---

## Backend Builder — Internal API

Not a Tauri command. Called internally when all CDI reads complete.

```rust
/// Step 1: Send IdentifyEventsAddressed to each known node (125 ms between sends);
/// collect all Producer Identified and Consumer Identified replies.
/// Returns map of event_id_hex → NodeRoles (which nodes replied as producer/consumer).
/// Models JMRI EventTablePane.sendRequestEvents() addressed-per-node pattern.
async fn query_event_roles(
    node_ids: &[[u8; 8]],         // all known nodes; addressed one-by-one
    send_delay_ms: u64,           // between each addressed send, default 125 ms
    collect_window_ms: u64,       // reply window after last send, default 500 ms
    state: &AppState,
) -> HashMap<[u8; 8], NodeRoles>

/// Step 2: Build BowtieCatalog from node cache + protocol role map.
/// - Cross-node slots: role from protocol reply (definitive)
/// - Same-node slots: role from CDI heuristic fallback; Ambiguous if inconclusive
pub async fn build_bowtie_catalog(
    nodes: &[DiscoveredNode],
    event_roles: &HashMap<[u8; 8], NodeRoles>,
) -> BowtieCatalog
```

Full flow (see research.md RQ-5, RQ-8):
1. Run `query_event_roles` (IdentifyEventsAddressed per node, 125 ms spacing, collect 500 ms window)
2. Run `build_bowtie_catalog` (CDI slots + protocol role map → BowtieCards)
3. Store result on `AppState.bowties_catalog`
4. Emit `cdi-read-complete`

---

## Error Handling

| Scenario | Behaviour |
|---|---|
| `get_bowties` called before any CDI read | Returns `Ok(None)` — frontend shows Bowties tab as disabled |
| Identify Events query times out (no replies within 500 ms) | Returns empty `NodeRoles`; any event ID with no replies produces no bowtie card (event is treated as unmatched) |
| Node has CDI but all slots are same-node and heuristic is inconclusive | All slots appear in `ambiguous_entries`; card is only emitted if other cross-node sides confirmed |
| `read_all_config_values` fails for a node | That node is skipped in catalog build; remaining nodes still processed |
| Zero bowties built | Returns `Ok(Some(BowtieCatalog { bowties: vec![], ... }))` — frontend shows empty state |
