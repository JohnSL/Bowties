# PCER Implementation Reference

> **Status: Historical — implemented.** PCER (event transport) is implemented in `lcc-rs`. Retain as implementation reference for the lcc-rs event transport layer.

Implementation notes for adding ProducerConsumerEventReport (event transport) to lcc-rs.

## Protocol

**MTI**: `0x05B4` (ProducerConsumerEventReport)
- Global message (not addressed) — header carries source alias only
- Payload: 8-byte EventID
- Optional additional bytes after the EventID (rare; used by some extensions)
- Multi-frame PCER uses `PCERfirst` / `PCERmiddle` / `PCERlast` frame types when payload exceeds single CAN frame

## Related MTIs (already in mti.rs)

| MTI | Value | Direction | Purpose |
|-----|-------|-----------|---------|
| IdentifyConsumers | 0x198F4 | Request | "Who consumes this event?" — carries 8-byte EventID |
| ConsumerIdentifiedValid | 0x194C4 | Reply | "I consume it, currently valid" |
| ConsumerIdentifiedInvalid | 0x194C5 | Reply | "I consume it, currently invalid" |
| ConsumerIdentifiedUnknown | 0x194C7 | Reply | "I consume it, state unknown" |
| IdentifyProducers | 0x19914 | Request | "Who produces this event?" |
| ProducerIdentifiedValid | 0x19544 | Reply | "I produce it, currently valid" |
| ProducerIdentifiedInvalid | 0x19545 | Reply | |
| ProducerIdentifiedUnknown | 0x19547 | Reply | |
| IdentifyEventsGlobal | 0x19970 | Request | "Report all your events" |
| IdentifyEventsAddressed | 0x19968 | Request | "Report all your events" (addressed to specific node) |
| ConsumerRangeIdentified | 0x194A4 | Reply | "I consume events in this range" |
| ProducerRangeIdentified | 0x19524 | Reply | "I produce events in this range" |

## What needs to be added to lcc-rs

### 1. MTI variant
Add `ProducerConsumerEventReport` to the `MTI` enum in `mti.rs` with value `0x05B4`. Also need the multi-frame variants if supporting payloads > 6 bytes (unlikely for initial implementation).

### 2. Event monitor (passive receive)
Subscribe to PCER frames via the dispatcher's broadcast channel. Each PCER frame contains:
- Source alias in CAN header (resolve to NodeID via dispatcher alias map)
- 8-byte EventID in payload

New struct:
```rust
pub struct EventReport {
    pub source: NodeID,
    pub source_alias: u16,
    pub event_id: EventID,
    pub timestamp: Instant,
}
```

### 3. Event send (active produce)
Build and send a PCER frame:
```rust
pub async fn send_event(&self, event_id: &EventID) -> Result<()>
```
Simple: construct frame with MTI 0x05B4, our alias, and the 8-byte EventID as data.

### 4. IdentifyConsumers/IdentifyProducers responses
If Bowties itself consumes or produces events, it should respond to these queries. For a monitor-only mode this is optional — we can just listen.

### 5. Tauri integration
- New command: `listen_events` → starts an event monitor, returns events via Tauri event channel
- New command: `send_event(event_id)` → produces a PCER
- Frontend: event log panel showing source node, event ID, timestamp, and decoded meaning (well-known events, or CDI-derived name if available)

## Well-known event IDs

| Event ID | Meaning |
|----------|---------|
| 01.00.00.00.00.00.FF.FF | Emergency off |
| 01.00.00.00.00.00.FF.FE | Clear emergency off |
| 01.00.00.00.00.00.FF.FD | Emergency stop |
| 01.00.00.00.00.00.FF.FC | Clear emergency stop |
| 01.00.00.00.00.00.FF.F8 | New log entry |
| 01.00.00.00.00.00.FE.00 | Ident button pressed |
| 01.01.00.00.00.00.02.01 | Duplicate node ID detected |
| 01.01.00.00.00.00.03.03 | Node is a train |
| 01.01.00.00.00.00.03.04 | Node is a traction proxy |

## OpenLCB_Java reference: BitProducerConsumer pattern

The Java implementation uses a `BitProducerConsumer` class that pairs two EventIDs (on/off) with a boolean `VersionedValue`. Key behaviors:
- **Flags** control role: `IS_PRODUCER` (send PCER on state change), `IS_CONSUMER` (update state on received PCER), `QUERY_AT_STARTUP` (send IdentifyProducers on connect)
- **Anti-echo**: `setFromOwner()` updates state without triggering the outbound PCER callback
- **State tracking**: replies to IdentifyConsumers/IdentifyProducers with Valid/Invalid/Unknown based on current boolean value
- **VersionedValue**: version counter prevents out-of-order updates from overwriting newer state

## Use cases for Bowties

1. **Event monitor panel** — live log of all PCER traffic with source node name (from SNIP), event ID, and decoded meaning. Simplest to implement, highest value.
2. **Live bowtie state** — color bowtie arrows based on which events are currently Valid vs Invalid. Requires listening to ProducerIdentified/ConsumerIdentified replies.
3. **Test event button** — send a specific event to verify wiring. Useful during commissioning.
4. **Well-known event decode** — match event IDs against the table above for human-readable display.

## Suggested implementation order

1. Add PCER MTI to `mti.rs`
2. Add passive event listener in dispatcher (subscribe to PCER MTI, decode EventID, emit to broadcast channel)
3. Add Tauri event-monitor command + frontend panel
4. Add `send_event` command for testing
5. Add live bowtie state (optional, needs ProducerIdentified/ConsumerIdentified tracking)
