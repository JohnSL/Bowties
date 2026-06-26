# Feature Specification: Live Channel State — Event State Store & Occupancy Indicators

**Feature Branch**: `016-live-channel-state`
**Created**: 2026-06-25
**Status**: Design complete, ready for slicing
**Input**: Show real-time occupancy state on block-occupancy channel rows in the Railroad tab, powered by a general-purpose event state store that records all PCER events received from the LCC bus.

## Context

Spec 015 delivered information channels — a typed, named inventory of layout I/O (initially block-occupancy channels from BOD-family daughter boards) displayed in the Railroad tab. But the inventory is static: it shows names and hardware references, not live state.

This feature adds live occupancy indicators to channel rows. When connected to the bus, each block-occupancy channel shows whether its block is currently occupied (● red circle) or clear (○ empty circle), updated in real-time from Producer/Consumer Event Report (PCER) messages on the LCC bus.

The enabling infrastructure is a **general-purpose event state store** — a session-scoped ledger that records every PCER event received, regardless of whether it's attached to a channel. Channel state is derived at display time by joining the event ledger with channel-to-event-ID mappings. This design delivers retroactive state: if a channel is defined after events have already been received, the indicator immediately shows the correct state based on the last-seen events.

This also implements the "LCC Event Driver: switch to always listening" backlog item.

## Key Concepts

### Event State Store

A session-scoped, transient store that records every PCER event received from the LCC bus. The store maintains a map from event ID (hex string) to the timestamp of the most recent occurrence. It is:

- **Channel-unaware** — records all events, not just those matching known channels.
- **Session-scoped** — populated while connected to the bus, cleared on disconnect. Not persisted.
- **Retroactive** — events received before any channel references them are still available when channels are later defined or configured.

### Channel-to-Event-ID Resolution

A pure domain function that maps a channel's hardware reference to its event IDs using the board's profile and config tree. The profile declares which CDI leaves correspond to which channel states:

- The **channel type** defines the abstract states (e.g., block-occupancy has `occupied` and `clear`).
- The **board profile** maps those states to specific CDI producer event leaf positions within the channel's hardware scope (e.g., "for BOD inputs, `occupied` is producer leaf index 0, `clear` is producer leaf index 1").
- The **resolution function** follows the profile mapping: navigate the config tree to the declared leaf position, read the configured event ID value.

### Channel State Derivation

A read-time computation: given a channel's resolved event IDs (occupied=A, clear=B) and the event state store, determine current state:

- If `lastSeen(A) > lastSeen(B)` → occupied
- If `lastSeen(B) > lastSeen(A)` → clear
- If neither seen → unknown

This is a pure function, not a subscription. The derivation runs at render time, making it naturally reactive when either the event store or channel list changes.

## User Scenarios & Testing

### US1 — Live Occupancy Indicator (Priority: P1)

A layout owner has a Tower-LCC with a BOD-8 connected. They open the Railroad tab and see their block-occupancy channels. Each channel row shows a small state indicator. When a train enters a detection block, the indicator changes from ○ (unknown/clear) to ● vermillion (occupied). When the train leaves, it changes to ● teal-green (clear).

**Visual design — three states with Okabe-Ito colorblind-safe palette:**

| State | Indicator | Color | Size | Tooltip |
|-------|-----------|-------|------|---------|
| Unknown | ○ hollow circle (gray border) | No fill | 8px | "Unknown — no events received" |
| Clear | ● filled circle | Teal-green `#009e73` | 8px | "Clear" |
| Occupied | ● filled circle | Vermillion `#d55e00` | 10px | "Occupied" |

Three perceptual channels distinguish states: shape (hollow vs filled), color (Okabe-Ito palette), and size (occupied is larger). All three states are distinguishable in grayscale and under deuteranopia, protanopia, and tritanopia. Tooltips provide a text fallback. See [mockup 5](../proposals/app-ux-vision/app-ux-vision-mockups.html).

**Acceptance Criteria:**
1. Each block-occupancy channel row displays a three-state indicator: ○ hollow (unknown), ● teal-green (clear), or ● vermillion (occupied).
2. State updates within ~100ms of PCER event arrival (no perceptible lag).
3. All three states have distinct shapes or fills — accessible without relying solely on color.
4. Each indicator shows a tooltip describing the state on hover.

### US2 — Retroactive State on Channel Creation (Priority: P1)

A layout owner connects to the bus with no channels defined. PCER events arrive from BOD boards. The owner then selects a BOD-8 daughter board on a connector, creating channels. The new channels immediately show correct occupancy state based on events already received.

**Acceptance Criteria:**
1. Channels created after PCER events are already in the event store show the correct state without waiting for new events.
2. No explicit "refresh" action needed — state resolves automatically on channel creation.

### US3 — State Clears on Disconnect (Priority: P2)

When the user disconnects from the bus, all indicators revert to the unknown state (○ hollow). On reconnection, indicators remain unknown until new PCER events arrive.

**Acceptance Criteria:**
1. All channel indicators show ○ hollow (unknown) when not connected to the bus.
2. On reconnect, the event store starts fresh — no stale state from the previous session.

## Profile Schema Extension

The `channelInputs` section in daughter board metadata gains an `eventMapping` field that declares which producer event leaf indices correspond to which channel states:

```yaml
channelInputs:
  - channelType: "block-occupancy"
    inputs: [1, 2, 3, 4]
    eventMapping:
      occupied: { producerLeafIndex: 0 }
      clear: { producerLeafIndex: 1 }
```

- `producerLeafIndex` is the 0-based index within the producer event group (Event#2) for the channel's Line in the CDI.
- This mapping is board-profile-specific — different boards may use different leaf orderings.

## Deferred

- **Event monitor UI** — a full event log viewer. The event state store could power this, but no UI beyond channel indicators is in scope.
- **Non-occupancy channel states** — signal aspects, turnout position, etc. Same event store, different channel types and derivation logic. Future feature.
- **Offline/placeholder state** — placeholder nodes have no live bus connection and cannot show live state. Indicators always show unknown for placeholders.
- **Connector change recomputation** — when a connector's daughter board changes, channel event mappings should recompute. Deferred to S2 or a follow-up.

## Key Entities

| Entity | Description |
|--------|-------------|
| Event State Store | Session-scoped map: EventID (hex) → last-seen timestamp. Channel-unaware. |
| Event Mapping | Profile-declared mapping from channel type states to CDI producer event leaf indices. |
| Channel State | Derived value: occupied / clear / unknown. Computed from event store + resolved event IDs. |
| PCER | Producer/Consumer Event Report — LCC message (MTI 0x195B4) carrying an 8-byte event ID. |
