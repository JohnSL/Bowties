# Session Handoff — LCC Bowties App — 2026-02-22

## What Was Built (Features 001–006)

A Tauri + SvelteKit desktop app for visualising LCC (Layout Command Control) event connections
as "bowties" — shared event IDs with ≥1 producer slot and ≥1 consumer slot across discovered nodes.

### Stack

- **Frontend**: SvelteKit + Svelte 5 runes, TypeScript, Vitest + Testing Library
- **Backend**: Tauri 2, Rust, Tokio async
- **LCC library**: `lcc-rs` (workspace crate at `lcc-rs/`)
- **App root**: `app/` (contains `src/` and `src-tauri/`)

---

## Three Active Bugs to Fix

### Bug 1 — Bowties renders as a separate page (theme change + data loss)

**Symptom**: Clicking "Bowties" navigates to `/bowties` — a separate SvelteKit route — which
applies its own dark theme, and navigating back to `/` destroys all previously loaded node and
config data (because it is held in component-local `$state` on `+page.svelte`).

**Root cause**: `app/src/routes/bowties/+page.svelte` is a full SvelteKit route. Clicking the
Bowties button calls `goto('/bowties')`. SvelteKit unmounts `+page.svelte` and mounts the
bowties page, losing all component-local state.

**Fix**: Delete `app/src/routes/bowties/+page.svelte`. Convert the Bowties view into an
in-page tab panel rendered conditionally inside `app/src/routes/+page.svelte`, similar to how
the existing config sidebar and segment view panels work. A `let activeTab = $state<'config' |
'bowties'>('config')` variable and `{#if activeTab === 'bowties'}` block is sufficient.

The Bowties tab button already exists in `+page.svelte` (line ~469); change its `onclick` from
`goto('/bowties')` to `activeTab = 'bowties'`. Move the catalog display markup (currently in
`routes/bowties/+page.svelte`) into a new component, e.g.
`app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte`, and render it in that `{#if}` block.

### Bug 2 — Bowtie cards show node names, not element names

**Symptom**: The producer and consumer columns in each BowtieCard show only the node name
(e.g. "Tower Node") but the `element_label` field is blank or wrong — it looks like the same
node is repeated rather than showing specific CDI event slot names.

**Root cause in `build_bowtie_catalog`**
(`app/src-tauri/src/commands/bowties.rs`):

The Identify Events exchange tells us: "Node X produces/consumes event Y". But to find *which
specific CDI slot* on Node X holds event Y, the code calls `best_slot()` — a heuristic that
picks the first slot whose *CDI keyword-based role classification* matches. It has no
knowledge of the **actual configured value** written at each slot address.

The config values were read (`read_all_config_values`) and streamed to the frontend as a
`HashMap<String, ConfigValueWithMetadata>`. However, the Rust backend never stores those
values in `AppState` — they exist only in `millerColumnsStore.configValues` on the frontend.

**Fix** (two steps):

1. **Cache config values in `AppState`** after `read_all_config_values` completes. Add a field:
   ```rust
   // in state.rs AppState
   pub config_value_cache: Arc<RwLock<HashMap<String, HashMap<String, [u8; 8]>>>>
   // outer key = node_id_hex; inner key = element_path joined by "/"; value = 8-byte event ID
   ```
   At the end of `read_all_config_values` (in `cdi.rs`), for every `ConfigValue::EventId`
   found, write it into this cache keyed by `(node_id, element_path.join("/"))`.

2. **Use the cache in `build_bowtie_catalog`** (in `bowties.rs`): instead of calling
   `best_slot()` with a heuristic, look up the config value cache to find which slot on node X
   actually contains the 8-byte event ID being catalogued. That slot is the correct
   `EventSlotEntry`. The heuristic remains as a fallback if a slot has no cached value yet.

### Bug 3 — Navigation data loss (nodes list cleared on page re-mount)

`let nodes = $state<DiscoveredNode[]>([])` on line 30 of `app/src/routes/+page.svelte` is
component-local. If the component is ever unmounted (which should not happen once Bug 1 is
fixed — no more `goto`), nodes are lost. The backend `AppState.nodes` always retains
discovered nodes; the frontend should call `invoke('discover_nodes')` / `invoke('get_discovered_nodes')`
on mount to repopulate from backend rather than keeping a separate authoritative copy.

This is largely mitigated once Bug 1 is fixed (no navigation away from `+page.svelte`), but
the correct long-term fix is to make the frontend stateless for discovery data — query the
backend on mount and on demand.

---

## Architecture Diagnosis

### What the backend persists correctly (in `AppState`, `app/src-tauri/src/state.rs`)

| Field | What it holds |
|---|---|
| `nodes: Arc<RwLock<Vec<DiscoveredNode>>>` | All discovered nodes with SNIP + CDI XML |
| `bowties_catalog: Arc<RwLock<Option<BowtieCatalog>>>` | Built catalog |
| `CDI_PARSE_CACHE: Lazy<RwLock<HashMap<...>>>` | Parsed CDI ASTs (in `cdi.rs`) |

### What the backend does NOT persist (but should)

| Missing | Where it currently lives | Fix |
|---|---|---|
| Config values (event ID bytes per slot) | `millerColumnsStore.configValues` in frontend JS | Add `config_value_cache` to `AppState` |

### What the frontend holds in component-local `$state` (lost on unmount)

| Variable | Location | Should be |
|---|---|---|
| `nodes` | `+page.svelte:30` | Fetched from backend on mount |
| `readProgress` | `+page.svelte:38` | Transient UI state — fine |
| Connection state (`connected`, `host`, `port`) | `+page.svelte` | Fine — fetched from backend on mount already |

### What the frontend holds in module-level stores (survive tab switching)

- `millerColumnsStore` — CDI column navigation + `configValues` cache
- `configSidebarStore` — expanded node IDs, selected segment
- `bowtieCatalogStore` — bowtie catalog (populated via `cdi-read-complete` Tauri event)
- `nodeInfoStore` — node info updates

Module-level stores survive within the same browser tab session (JS VM stays alive), so they
are fine as long as we don't do full-page navigation.

---

## Key File Locations

```
app/
  src/
    routes/
      +page.svelte              — main app page (all tabs should live here)
      bowties/+page.svelte      — SHOULD BE DELETED / merged into +page.svelte
    lib/
      api/
        tauri.ts                — Tauri command wrappers + TypeScript types
      stores/
        bowties.svelte.ts       — BowtieCatalogStore (Svelte 5 $state class)
        millerColumns.ts        — CDI navigation + config value cache
        configSidebar.ts        — config sidebar navigation state
      components/
        Bowtie/
          BowtieCard.svelte     — renders one bowtie card (producers | arrow | consumers)
          ElementEntry.svelte   — renders one EventSlotEntry (node_name + element_label)
          ConnectorArrow.svelte — renders → symbol with event ID
          EmptyState.svelte     — shown when no bowties found
        ElementCardDeck/
          ElementCard.svelte    — CDI card with lazy-loaded fields
          EventSlotRow.svelte   — renders one event ID field row (+ "Used in" cross-ref)

app/src-tauri/src/
  state.rs                      — AppState struct (add config_value_cache here)
  commands/
    bowties.rs                  — build_bowtie_catalog, query_event_roles, get_bowties
    cdi.rs                      — read_all_config_values, CDI parse cache, config value types
    discovery.rs                — discover_nodes, query_snip
    mod.rs                      — registers all command modules

lcc-rs/src/
  cdi/
    role.rs                     — EventRole enum + classify_event_slot heuristic
    hierarchy.rs                — walk_event_slots, navigate_to_path
    mod.rs                      — re-exports EventRole, classify_event_slot, walk_event_slots
  lib.rs                        — crate root re-exports
  types.rs                      — DiscoveredNode, NodeID, SNIPData, etc.
```

---

## Tauri Events

| Event name | Direction | Payload | Purpose |
|---|---|---|---|
| `cdi-read-complete` | backend → frontend | `CdiReadCompletePayload { catalog, node_count }` | Triggers bowtieCatalogStore update |
| `config-read-progress` | backend → frontend | `ReadProgressUpdate` | Progress bar during bulk read |

---

## Key Rust Types

### `EventSlotEntry` (`state.rs`, serialises as snake_case)
```rust
pub struct EventSlotEntry {
    pub node_id: String,       // "02.01.57.00.00.01"
    pub node_name: String,     // human-readable
    pub element_path: Vec<String>,  // ["seg:0", "elem:3", "elem:2"]
    pub element_label: String, // CDI name → description first sentence → path
    pub event_id: [u8; 8],
    pub role: lcc_rs::EventRole,  // Producer | Consumer only (Ambiguous → ambiguous_entries)
}
```

### `BowtieCatalog` (`state.rs`, serialises as snake_case)
```rust
pub struct BowtieCatalog {
    pub bowties: Vec<BowtieCard>,    // sorted by event_id_bytes
    pub built_at: String,            // ISO 8601
    pub source_node_count: usize,
    pub total_slots_scanned: usize,
}
```

**Important**: All three bowtie structs use `#[serde(rename_all = "snake_case")]`. The
TypeScript interfaces in `app/src/lib/api/tauri.ts` use snake_case field names to match.
This was fixed in the last session (was `camelCase`, causing `element_path` to be `undefined`
on the frontend and crashing `.join('/')`).

---

## How `build_bowtie_catalog` Works (Current + Intended)

**Current (broken for element identification)**:
1. `query_event_roles` sends `IdentifyEventsAddressed` to each node, collects
   `ProducerIdentified` / `ConsumerIdentified` replies → `HashMap<[u8;8], NodeRoles>`
   (NodeRoles = which node IDs produce/consume that event ID).
2. `build_bowtie_catalog` iterates event IDs with both producers and consumers.
3. For each node in the producer/consumer set, calls `walk_cdi_slots` → CDI AST walk →
   returns all event slot `SlotInfo` structs with `heuristic_role` (keyword-guessed).
4. `best_slot()` picks the *first* slot whose heuristic role matches — has no connection
   to the actual configured event ID bytes.

**What it should do**:
- After step 3, look up `config_value_cache[node_id][slot.element_path.join("/")]` to find
  which slot actually holds the event ID bytes of interest.
- Use that slot's metadata (`element_path`, `element_label`) for the `EventSlotEntry`.
- Fall back to heuristic `best_slot()` only when the config cache has no value for a slot
  (e.g. config read not yet done or slot was unreadable).

---

## Serde / Type Alignment Pitfall

When adding new Rust structs that serialise to the frontend, decide on casing consistently:

- Most Tauri command response types in `cdi.rs` use `#[serde(rename_all = "camelCase")]`
- The bowtie structs in `state.rs` use `#[serde(rename_all = "snake_case")]`
- The TypeScript types in `tauri.ts` must match exactly

If they diverge, fields silently become `undefined` in TypeScript — the exact bug that caused
`entry.element_path.join('/')` to crash in the last session.

---

## Test Status

```
Frontend (Vitest):
  ✓ BowtieCard.test.ts           9 tests
  ✓ EmptyState.test.ts           4 tests
  ✓ ElementCard.test.ts          7 tests
  ✓ ElementCardDeck.test.ts      5 tests
  ✓ cardTitle.test.ts            7 tests
  ✗ ConfigSidebar.test.ts        1 pre-existing failure (unrelated to bowties)

Backend (cargo test, lcc-rs):
  ✓ All new bowties tests pass
  ✗ 1 pre-existing failure in traffic::tests::test_decode_verified_node
  3 dead_code warnings (pre-existing)
```

---

## Suggested Order of Work for Next Session

1. **Fix Bug 1** (tab vs page): Delete `routes/bowties/+page.svelte`. Extract bowties content
   into `BowtieCatalogPanel.svelte`. Add `activeTab` state to `+page.svelte`. Change Bowties
   button to toggle tab instead of `goto()`.

2. **Fix Bug 2** (element identification): Add `config_value_cache` to `AppState`. Populate
   it in `read_all_config_values` for every `EventId` element. Use it in
   `build_bowtie_catalog` to match slots to event IDs precisely.

3. **Verify Bug 3** is resolved as a side effect of Bug 1 fix (no navigation = no state loss).

4. **Style parity**: Once bowties is an in-page tab, it will inherit the main page's light
   theme automatically. The dark-mode CSS variables in `BowtieCard.svelte` and
   `ElementEntry.svelte` should be revisited to use the app's existing CSS custom properties.
