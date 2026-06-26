# Feature Specification: Channel State on Connect — Eager Resolution & "No Config" Indicator

**Feature Branch**: `017-channel-state-on-connect`
**Created**: 2026-06-26
**Status**: Design complete, ready for slicing
**Input**: When a layout with saved nodes is opened and the user connects to the bus, occupancy indicators for block-occupancy channels should go live immediately — no need to read CDI for nodes whose configuration is already stored in the layout file. When a channel exists but we genuinely have no config data for its node, the indicator should communicate that distinct state.

## Context

Spec 016 delivered live occupancy indicators. The data flow works end-to-end: PCER events → event state store → derived channel state → ChannelCard indicator. But on a layout with saved nodes, occupancy stays at the "unknown" state until the user manually triggers a CDI read for *any* node — even nodes that have nothing to do with the channels (e.g., locomotive nodes).

Investigation traced this to a timing gap in the route's resolve effect:

1. Layout open populates the frontend `nodeTreeStore` and the backend `node_registry.saved_trees` with the saved CDI trees.
2. User clicks **Connect** → `layoutStore.isConnected` flips true → the `$effect` fires *immediately*.
3. The backend hasn't received `lcc-node-discovered` yet, so no live proxies exist, so `resolve_channel_event_ids` returns empty for every channel.
4. `lcc-node-discovered` lands → `register_node` spawns the live proxy and seeds it with the saved tree (see [`node_registry.rs#L100`](../../bowties-core/src/node_registry.rs)).
5. **The frontend `$effect` does not re-run.** Its reactive reads — `channelsStore.channels`, `layoutStore.isConnected`, `nodeTreeStore.trees.size` — none change. Discovery completion is invisible to it.
6. Manually triggering "Read Additional" eventually emits `node-tree-updated`, which replaces the `nodeTreeStore.trees` Map ref and unsticks the effect.

The fix is narrow: re-resolve when the **live roster changes**, not when the tree store changes. Resolution semantically depends on "does a live proxy exist for this channel's node?" — `register_node` is the right reactive boundary.

A second, related improvement: the existing "unknown ○" indicator conflates two genuinely different conditions:

- **Unknown** — we know the channel's event IDs but no PCER event has arrived yet (bus is quiet, or the block has never been touched).
- **No config data** — we cannot even resolve the channel's event IDs (the owning node isn't on the bus, or its saved tree was partial).

These look identical today but mean very different things to a user staring at a layout panel.

## Key Concepts

### Resolution Reactive Boundary

The resolve effect's job is "given the current set of channels and the current set of live nodes, what event IDs can we resolve right now?" The dep that matters is the **live roster**, not the tree store. The current tree-store dep is a proxy for "fresh node has been CDI-read" — it works for fresh nodes but misses saved ones whose trees were preloaded.

The new boundary: the effect reads `nodeRoster.liveEntries.length` (or an equivalent signal that changes on each new live node) and re-runs `resolveChannelEventIds` each time. The backend already does the right thing — `proxy.get_config_tree()` returns the seeded saved tree the moment the proxy exists.

### Channel Indicator State Model

The indicator now has **four** states, with predicates resolvable from the existing `resolvedEventIds` map and event store:

| State | Predicate | Visual | Tooltip |
|---|---|---|---|
| No config data | `resolvedEventIds` has no entry for the channel OR entry has neither `occupied` nor `clear` | ⊘ hollow circle with diagonal slash (or ○ with small `?`) — gray | "Configuration not available for this node" |
| Unknown | event IDs resolved, neither event seen in store | ○ hollow circle — light gray border | "Unknown — no events received" |
| Clear | clear event most recent | ● teal-green `#009e73` | "Clear" |
| Occupied | occupied event most recent | ● vermillion `#d55e00` (larger) | "Occupied" |

The "no config data" state is reachable in three scenarios:

1. **Transient on connect** (small with Slice 1, but never zero — there's always a window between flip-to-connected and first discovery).
2. **Saved layout viewed offline** where a node was partial-captured or never captured.
3. **Channel exists for a node not currently on the bus** (placeholder, off-line saved-only node).

## User Scenarios & Testing

### US1 — Instant Occupancy on Reconnect (Priority: P1)

A layout owner opens a layout with two BOD nodes and two locomotive nodes saved from a previous session. They click Connect. Within ~1 second of the bus session establishing, all BOD channel indicators show their correct state — without the user touching "Read Additional" or any CDI-read action for the locomotive nodes.

**Acceptance Criteria:**

1. After connecting to a bus where every channel's owning node is discovered and registered, channel indicators reflect live state within ~1s — regardless of whether other (non-channel-bearing) nodes have had their CDI read.
2. No `read_all_config_values` IPC is required to be invoked for channels of saved nodes to resolve.
3. As each live node is discovered and registered, channels on that node update from "no config data" → "unknown" (or directly to clear/occupied if events have already arrived).

### US2 — "No Config Data" State Communicated Distinctly (Priority: P2)

A layout owner opens a layout offline. One of the saved BOD nodes was previously partial-captured (the tree exists but lacks the producer event IDs at the required indices). That node's channels show a "no config data" indicator distinct from "unknown," with a tooltip explaining the cause.

**Acceptance Criteria:**

1. A channel with no resolvable event IDs displays the **No config data** indicator (⊘ or equivalent — distinct from ○ unknown).
2. The tooltip on the indicator clearly distinguishes this from "no events received yet."
3. The indicator is colorblind-safe: shape + color + tooltip together convey the state without relying solely on color.
4. When the channel's node is later registered and its saved tree resolves the event IDs, the indicator transitions to "unknown" (or directly to clear/occupied) without the user doing anything.

### US3 — Existing Behavior Preserved (Priority: P1)

Existing Spec 016 behavior continues to work end-to-end: PCER events still flow, derive logic still resolves clear vs occupied vs unknown, disconnect still clears the ledger, retroactive resolution on channel creation still works.

**Acceptance Criteria:**

1. All Spec 016 acceptance criteria continue to pass.
2. No regression in `channelState.test.ts`, `eventStateOrchestrator.test.ts`, or the route disconnect test.

## Deferred

- **LCC Identify Producer / Identify Events on connect** — could prime initial occupancy state without waiting for environmental PCER events. Useful follow-on; out of scope here.
- **Persisting resolved event IDs with the channel** — would let channels light up without any backend round-trip at all. Larger persistence-shape change; out of scope. Earlier discussion noted this as Option B; deferred in favor of Option E (this spec) for now.
- **Treatment of placeholder-only nodes** — placeholders never go on the bus; their channels always remain in "no config data." This falls out naturally from US2 and needs no special handling.

## Key Entities

| Entity | Description |
|--------|-------------|
| Live roster signal | Existing `nodeRoster.liveEntries` — the set of nodes currently registered with live proxies. The new resolution effect reads its length (or an equivalent change-tracking signal). |
| Resolved event ID map | Existing `resolvedEventIds: Map<channelId, { occupied?, clear? }>`. A missing entry or one with both fields undefined means "no config data" for that channel. |
| Indicator state | Existing `OccupancyState` enum extended from `'unknown' \| 'clear' \| 'occupied'` to include `'no-config'`. |
