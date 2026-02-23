# Quickstart: Bowties Tab — Discover Existing Connections

**For**: Developers implementing `006-bowties-event-discovery`  
**Branch**: `006-bowties-event-discovery`  
**Date**: 2026-02-22

---

## What you're building

A read-only Bowties tab that automatically displays all producer-consumer connections discovered from the LCC node configurations that have already been read. No new network operations — the feature runs over data already loaded by the Configuration tab.

---

## Prerequisites

```powershell
# In the repository root
rustup update stable        # Rust 1.70+
cd app
npm install                 # or pnpm install
```

---

## Implementation order

Work through these layers bottom-up. Each layer has tests that should pass before moving to the next.

### Layer 1: lcc-rs role classifier

**File**: `lcc-rs/src/cdi/role.rs` (new)

Create `EventRole` enum and `classify_event_slot()`:

```rust
pub enum EventRole { Producer, Consumer, Ambiguous }

pub fn classify_event_slot(
    element: &EventIdElement,
    parent_group_names: &[&str],
) -> EventRole { ... }
```

Heuristic (two-tier fallback for same-node cases — see research.md RQ-3):
1. Check `parent_group_names` for producer/consumer keywords (case-insensitive).
2. Fall back to `element.description` phrase patterns.
3. Return `Ambiguous` if neither fires.

**Test immediately**:
```bash
cd lcc-rs
cargo test cdi::role
```

Test cases to write:
- Parent group named "Producers" → `Producer`
- Parent group named "Consumers" → `Consumer`
- Description "Generated when input goes active" → `Producer`
- Description "When this event arrives, turnout moves" → `Consumer`
- No group name, no description → `Ambiguous`
- Mixed signals (producer group + consumer description) → document chosen resolution in test

---

### Layer 2: CDI hierarchy walk with ancestor context

**File**: `lcc-rs/src/cdi/hierarchy.rs` (modify)

Add a walk function that visits every `EventId` element and passes the full ancestor group name stack:

```rust
pub fn walk_event_slots<F>(cdi: &Cdi, mut visitor: F)
where
    F: FnMut(&EventIdElement, &[&str]),  // element, ancestor_group_names
```

This feeds both the role classifier and the address derivation needed for `EventSlotEntry`.

**Test**: Walk a hand-crafted `Cdi` struct with known structure; assert visitor is called for every eventid with correct ancestor slice.

---

### Layer 3: Identify Events exchange + BowtieCatalog builder (Tauri backend)

**File**: `app/src-tauri/src/commands/bowties.rs` (new)

**Step A — Identify Events query** (runs after all CDI reads complete):
```rust
/// Send IdentifyEventsAddressed to each known node (125 ms between sends);
/// collect Producer/Consumer Identified replies within collect_window_ms.
/// Returns node-level role map: event_id → { producers: Set<node_id>, consumers: Set<node_id> }.
///
/// Per JMRI EventTablePane.sendRequestEvents() pattern (OpenLCB_Java reference impl):
///   - Use addressed MTI (IdentifyEventsAddressed, 0x0488) not global broadcast
///   - 125 ms between sends to avoid CAN bus flooding
///   - Collect all replies in a single post-send window (500 ms default)
///   - ProducerIdentified (0x0544/0x0545/0x0547) → record nodeID as producer for eventID
///   - ConsumerIdentified (0x04C4/0x04C5/0x04C7) → record nodeID as consumer for eventID
///   - EventState in replies is present but IGNORED in Phase 1 (see research.md RQ-11)
async fn query_event_roles(
    node_ids: &[[u8; 8]],          // all known nodes from AppState
    send_delay_ms: u64,            // between addressed sends, default 125
    collect_window_ms: u64,        // reply window after last send, default 500
    state: &AppState,
) -> HashMap<[u8; 8], NodeRoles>
```

**Step B — Catalog builder**:
```rust
pub async fn build_bowtie_catalog(
    nodes: &[DiscoveredNode],
    event_roles: &HashMap<[u8; 8], NodeRoles>,
) -> BowtieCatalog
```

Algorithm (see research.md RQ-8):
1. Walk each node's CDI + config values alongside `event_roles` → produce `EventSlotEntry` with correct role (or `Ambiguous` for same-node inconclusive)
2. Group entries by `event_id_bytes`
3. For each group: build `BowtieCard` if ≥1 confirmed producer AND ≥1 confirmed consumer; include `ambiguous_entries`
4. Sort cards by `event_id_bytes` lexicographically
5. Store on `AppState.bowties_catalog`; emit `cdi-read-complete`

**Register the `get_bowties` Tauri command** (see contracts/tauri-commands.md).

**Trigger** at end of `read_all_config_values` in cdi.rs when `node_index + 1 == total_nodes`.

**Test** `build_bowtie_catalog` with mock `DiscoveredNode` + `NodeRoles` data covering:
- Two nodes, shared event ID, one producer + one consumer → 1 BowtieCard, ambiguous_entries empty
- Same event ID on two producers + one consumer on three nodes → 1 card, 2 producers
- Node replies both ProducerIdentified + ConsumerIdentified, heuristic resolves → classified correctly
- Node replies both, heuristic inconclusive → entry in `ambiguous_entries`
- Event ID only on producers (no consumer reply) → excluded
- Zero nodes → empty catalog

---

### Layer 4: Frontend stores and API wrapper

**Files**:
- `app/src/lib/stores/bowties.ts` (new)
- `app/src/lib/api/tauri.ts` (modify — add `getBowties()`)

```typescript
// stores/bowties.ts
export const bowtieCatalogStore = writable<BowtieCatalog | null>(null);
export const cdiReadCompleteStore = writable<boolean>(false);
export const usedInMap: Readable<Map<string, BowtieCard>> = derived(
  bowtieCatalogStore,
  ($catalog) => {
    const map = new Map<string, BowtieCard>();
    for (const card of $catalog?.bowties ?? []) {
      map.set(card.event_id_hex, card);
    }
    return map;
  }
);
```

Listen for `cdi-read-complete` Tauri event in `+layout.svelte` (or root page) and update both stores.

---

### Layer 5: Svelte components

**Directory**: `app/src/lib/components/Bowtie/`

Build in this order:

1. **`ElementEntry.svelte`** — renders one `EventSlotEntry` (node name + element label). Props: `entry: EventSlotEntry`. Write Vitest test.

2. **`ConnectorArrow.svelte`** — centre column. Props: `eventIdHex: string`. Renders right-pointing arrow (CSS or SVG) with `eventIdHex` label beneath. Write Vitest test.

3. **`EmptyState.svelte`** — shown when `catalog.bowties` is empty (FR-006). Renders illustration placeholder + "No connections yet — click + New Connection to link a producer to a consumer". Write Vitest test.

4. **`BowtieCard.svelte`** — three-column layout. Props: `card: BowtieCard`. Left column = producer `ElementEntry` stack; centre = `ConnectorArrow`; right = consumer `ElementEntry` stack. If `card.ambiguous_entries` is non-empty, add a fourth section below the three columns labelled "Unknown role — needs clarification" listing those entries with an explanation tooltip. Write Vitest test covering: single + multiple producers/consumers, and card with ambiguous entries.

---

### Layer 6: Bowties route page

**File**: `app/src/routes/bowties/+page.svelte` (new)

- Subscribe to `cdiReadCompleteStore` — show disabled state if `false`.
- Subscribe to `bowtieCatalogStore` — show `EmptyState` if `bowties` empty, else vertical scroll list of `BowtieCard`.
- Read `?highlight=<eventIdHex>` query param on mount → scroll to and highlight the matching card (FR-009).

---

### Layer 7: Tab navigation

Add a Bowties tab/link to the existing navigation surface (alongside Config and Traffic). The tab MUST be visually disabled (greyed out label, non-clickable) when `cdiReadCompleteStore` is `false` (FR-013).

---

### Layer 8: Cross-reference in EventSlotRow

**File**: `app/src/lib/components/ElementCardDeck/EventSlotRow.svelte` (modify)

Add optional `usedIn: UsedInRef | null` prop. When truthy, render below the event ID value:
```
Used in: [connection name]  ← navigable link
```
Clicking calls `goto('/bowties?highlight=' + usedIn.eventIdHex)`.

The parent `SegmentView` or `ElementCard` should look up `usedIn` from the `usedInMap` derived store using the slot's current event ID hex.

---

## Running tests

```bash
# Rust (lcc-rs + Tauri backend)
cd lcc-rs && cargo test
cd app/src-tauri && cargo test

# Frontend
cd app && npm test
```

---

## Acceptance checklist

Before marking implementation complete, verify each acceptance scenario from spec.md manually against a live LCC network or the mock test harness:

- [ ] Bowtie cards appear for all shared event IDs (US1 AC1)
- [ ] Card layout shows producers left, consumers right, event ID below arrow (US1 AC2)
- [ ] Multi-node bowtie (2 producers + 1 consumer) in one card (US1 AC3)
- [ ] Tab rebuilds after config refresh without manual action (US1 AC5)
- [ ] Empty state shown correctly (US2 AC1)
- [ ] Empty state transitions correctly on refresh (US2 AC2)
- [ ] "Used in" link present in Config tab for participating slots (US3 AC1)
- [ ] Clicking "Used in" navigates to and highlights the bowtie card (US3 AC2)
- [ ] No "Used in" link for unmatched slots (US3 AC3)
