# Implementation Plan: Live Channel State

**Spec**: [spec.md](spec.md)
**Created**: 2026-06-25

## Architecture Assessment

### Module Impact

| Module | Change | Rationale |
|--------|--------|-----------|
| `lcc-rs/protocol/mti.rs` | Add `ProducerConsumerEventReport` (0x195B4) to MTI enum | Protocol completeness — PCER is a core LCC message type not yet in the enum |
| `app/src-tauri/events/router.rs` | Subscribe to PCER MTI, emit `lcc-event-state` Tauri event | Extends existing MTI subscription pattern (VerifiedNode, InitComplete) |
| `bowties-core` (new function) | `resolve_channel_events()`: hardware ref + profile + config tree → event ID pair | Pure domain function; hides CDI tree navigation behind small interface |
| `app/src-tauri/commands/` (new) | `resolve_channel_event_ids` IPC command | Thin IPC wrapper around bowties-core resolver |
| `bowties-core/profile/types.rs` | Add `EventMapping` to `ChannelInputMapping` | Profile schema extension for event leaf index declarations |
| Profile YAML files | Add `eventMapping` to BOD-family `channelInputs` | Data-only change; declares which producer leaves are occupied/clear |
| `app/src/lib/stores/` (new) | `eventState.svelte.ts` — session-scoped event ledger | Transient store; records all PCER events with timestamps |
| `app/src/lib/stores/` or `utils/` (new) | Channel state derivation logic | Pure function: (channel event IDs, event store) → occupied/clear/unknown |
| `app/src/lib/components/Railroad/ChannelRow.svelte` | Add occupancy indicator (○/●) | Rendering concern; reads derived state |
| `app/src/lib/components/Railroad/RailroadPanel.svelte` | Wire event store subscription lifecycle | Lifecycle concern; subscribe on mount/connect, clear on disconnect |

### Architectural Decisions

**D1: General-purpose event state store (not per-channel subscription)**

The event store records ALL PCER events, not just those matching known channels. Channel state is derived at display time by joining the event ledger with resolved event IDs. This provides:
- Retroactive state: channels created after events are received show correct state immediately.
- Channel-type independence: future channel types (signals, turnouts) use the same store.
- Alignment with the "always listening" backlog item.
- No subscription lifecycle management per channel.

Alternative rejected: per-channel event subscription with write-time filtering. Lost retroactivity, required subscription lifecycle per channel, coupled the event pipeline to channel knowledge.

**D2: Profile-declared event leaf mapping**

The board profile declares which CDI producer event leaf indices correspond to which channel states (e.g., `occupied: producerLeafIndex 0`, `clear: producerLeafIndex 1`). This keeps the resolution function generic — no hardcoded CDI structure assumptions.

Channel types define abstract states; profiles map states to CDI positions. The resolution function follows the mapping.

**D3: Frontend-resolved event matching (Approach B from design discussion)**

The backend forwards raw PCER events (event ID hex + timestamp). The frontend calls `resolve_channel_event_ids` once to get the mapping, then matches incoming events locally. This keeps the backend stateless for this feature — the EventRouter doesn't maintain a channel-specific lookup table.

**D4: Transient store, not layout data**

The event state store is ephemeral — populated while connected, cleared on disconnect, never persisted. This follows the traffic monitor pattern. It is a separate store from `channelsStore` (which owns persistent channel data with ADR-0012 draft layer semantics).

### Placement Compliance

| Concern | Layer | Placement Rule |
|---------|-------|---------------|
| PCER parsing / MTI routing | `lcc-rs` | Protocol behavior → lcc-rs ✓ |
| Event ID resolution from profile + tree | `bowties-core` | Domain logic → bowties-core ✓ |
| PCER event forwarding via Tauri | `app/src-tauri/events` | Backend coordination → src-tauri ✓ |
| Event state ledger | `app/src/lib/stores` | Transient frontend state → stores ✓ |
| State derivation | `app/src/lib/utils` or inline | Pure computation → utils or component ✓ |
| Indicator rendering | `app/src/lib/components` | Rendering → components ✓ |

### ADR Compliance

No conflicts with existing ADRs. Key alignments:
- **ADR-0012** (draft layer): Event state is NOT a draft layer — it's transient session state with no save/discard semantics.
- **ADR-0004** (layout facade): Event state is consumed directly by components, not merged into the layout facade. It has no layout-persistence concern.

## Vertical Slices

### S1: Live occupancy indicators (HITL)

**Intent**: End-to-end live occupancy state display on block-occupancy channel rows.

**Layers touched**: lcc-rs → backend EventRouter → backend command → bowties-core domain → frontend store → frontend component

**User-visible change**: Channel rows in Railroad tab show ○/● indicators reflecting real-time block occupancy from the LCC bus. Defining channels after events arrived shows correct state immediately (retroactive).

**Architecture note**: Establishes three new patterns:
1. PCER MTI subscription in EventRouter (extends existing MTI subscription pattern)
2. Channel-to-event resolution via profile-declared event mapping in bowties-core
3. Transient event state store on frontend (session-scoped, channel-unaware)

**Acceptance criteria**:
- PCER events (MTI 0x195B4) are received and forwarded as Tauri events
- Profile `eventMapping` declares producer leaf indices for BOD-family channels
- `resolve_channel_event_ids` returns correct event IDs from config tree + profile
- Event state store records all PCER events with timestamps
- ChannelRow shows ○ (unknown/clear) or ● red (occupied) derived from event store
- Retroactive state works: create channel after events received → correct indicator

**Classification**: HITL — introduces new event pipeline, new domain function, new transient store pattern. First instance of live bus state driving UI outside the traffic monitor.

### S2: Lifecycle and edge cases (AFK)

**Intent**: Robust lifecycle management for the event pipeline and indicators.

**Layers touched**: frontend store → frontend component → orchestration

**User-visible change**: Indicators clear on disconnect. Show unknown when config not yet loaded. Recompute event IDs when connector daughter board changes.

**Acceptance criteria**:
- Event store clears on bus disconnect; indicators revert to ○
- On reconnect, event store starts fresh
- Channels on nodes without loaded config show ○ (unknown), not an error
- Placeholder node channels always show ○ (cannot have live state)
- Changing a connector's daughter board recomputes event ID mapping for affected channels

**Classification**: AFK — follows established patterns from S1. Lifecycle management within existing stores and event pipeline.
