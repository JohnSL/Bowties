# Traffic Monitor Architecture

This document describes the full pipeline for the LCC traffic monitor feature, intended to orient a future session for making decoding or UX changes.

## Pipeline Overview

```
CAN frame arrives/departs
        │
        ▼
TapTransport (lcc-rs/src/dispatcher.rs)
  Intercepts every send() and receive() on the raw transport.
  Broadcasts ReceivedMessage to a tokio broadcast channel (all_tx).
        │
        ▼
EventRouter subscriber (app/src-tauri/src/events/router.rs)
  Subscribes to all_tx via dispatcher.subscribe_all().
  Calls DecodedMessage::decode() on each frame.
  Emits "lcc-message-received" Tauri event to the frontend.
        │
        ▼
TrafficMonitor.svelte (app/src/lib/components/TrafficMonitor.svelte)
  Listens for "lcc-message-received" via @tauri-apps/api event.
  Stamps each message with a seq number and pushes to $state array.
  Renders in a keyed {#each} block using seq as the key.
```

## Key Files

| File | Purpose |
|------|---------|
| `lcc-rs/src/dispatcher.rs` | `TapTransport` wrapper + `MessageDispatcher`. Where all frames are intercepted. |
| `app/src-tauri/src/traffic/mod.rs` | `DecodedMessage::decode()`. All MTI naming and payload formatting. |
| `app/src-tauri/src/events/router.rs` | Subscribes to dispatcher, decodes frames, emits Tauri events. |
| `app/src/lib/components/TrafficMonitor.svelte` | Full UI component (list, pause/clear/raw/auto-scroll buttons). |
| `app/src/lib/api/types.ts` | `TrafficMessage` interface — the shape of the Tauri event payload. |
| `specs/004-read-node-config/traffic/Bowties_async_blink.txt` | Ground truth capture showing ~126 expected messages with S:/R: labels. |

## Data Flow Details

### 1. TapTransport (lcc-rs/src/dispatcher.rs)

`MessageDispatcher::new()` wraps the raw `Box<dyn LccTransport>` in a `TapTransport`:

```rust
let tap = TapTransport { inner: transport, tx: all_tx.clone() };
let transport = Arc::new(Mutex::new(Box::new(tap) as Box<dyn LccTransport>));
```

`TapTransport` implements `LccTransport` and intercepts every call:

```rust
async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
    let result = self.inner.send(frame).await;
    if result.is_ok() { self.tx.send(ReceivedMessage { frame: frame.clone(), ... }); }
    result
}
async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
    let result = self.inner.receive(timeout_ms).await;
    if let Ok(Some(ref frame)) = result { self.tx.send(ReceivedMessage { ... }); }
    result
}
```

This means ALL code paths (discovery, SNIP, datagram, memory config) are captured because they all share the same `Arc<Mutex<Box<dyn LccTransport>>>`.

### 2. Decoder (app/src-tauri/src/traffic/mod.rs)

`DecodedMessage::decode(frame, our_alias)` returns:

```rust
pub struct DecodedMessage {
    pub timestamp: String,       // "HH:MM:SS.mmm" (UTC chrono format)
    pub direction: String,       // "S" if source_alias == our_alias, else "R"
    pub mti_name: String,        // format!("{:?}", mti) — Rust Debug name of MTI enum variant
    pub source_alias: u16,
    pub dest_alias: Option<u16>,
    pub node_id: Option<String>, // Set for VerifiedNode and InitializationComplete
    pub decoded_payload: String, // Human-readable payload or hex dump
    pub raw_frame: String,       // Full GridConnect frame string e.g. ":X19490AAAN;"
}
```

**Direction detection:** source alias of the frame is compared to `our_alias` (hardcoded `0xAAA` in `LccConnection::connect_with_dispatcher`).

**MTI name:** Uses Rust Debug format (`format!("{:?}", mti)`). If the MTI enum variant is `VerifyNodeGlobal`, the name will be `"VerifyNodeGlobal"`.

**Dest alias extraction:** Addressed messages (SNIPRequest, VerifyNodeAddressed, etc.) extract dest from header bits 27–16. Datagram messages extract dest from bits 23–12.

**Payload decoding** per MTI:
- `VerifiedNode`, `InitializationComplete`: formats 6-byte node ID as `XX.XX.XX.XX.XX.XX`
- `SNIPRequest`: static string `"SNIP Request"`
- `SNIPResponse`: decodes SNIP multi-frame (first/middle/final markers + manufacturer name from first null-terminated string)
- `DatagramReceivedOk`, `DatagramRejected`: formats flags byte
- Everything else: hex dump of data bytes, or `"(no data)"`

### 3. EventRouter (app/src-tauri/src/events/router.rs)

`MessageReceivedEvent` (emitted as `"lcc-message-received"`):

```rust
pub struct MessageReceivedEvent {
    pub frame: String,
    pub mti: Option<String>,
    pub source_alias: Option<u16>,
    pub timestamp: String,
    pub direction: Option<String>,
    pub decoded_payload: Option<String>,
    pub node_id: Option<String>,
    pub dest_alias: Option<u16>,
}
```

All fields are `Option` for forward compatibility.  
`eprintln!` debug logging is still present in the router loop and `handle_all_messages`.

### 4. TrafficMessage TypeScript interface (app/src/lib/api/types.ts)

```typescript
export interface TrafficMessage {
    frame: string;
    mti: string | null;
    sourceAlias: number | null;
    timestamp: string;
    direction: string | null;   // "S" or "R"
    decodedPayload: string | null;
    nodeId: string | null;
    destAlias: number | null;
}
```

Rust `snake_case` fields are serialized to TypeScript `camelCase` via `#[serde(rename_all = "camelCase")]`.

### 5. TrafficMonitor.svelte

**State:**
- `messages: DisplayMessage[]` — circular buffer capped at 500 entries
- `DisplayMessage` extends `TrafficMessage` with `seq: number`
- `nextId: number` — module-level counter, increments on every `addMessage()`, never reset

**Key rendering pattern:**
```svelte
{#each messages as msg (msg.seq)}
```
`msg.seq` is used as the unique key instead of any content-based composite to avoid `each_key_duplicate` errors from duplicate frames arriving at the same millisecond.

**Controls:** Pause/Resume, Clear, Raw/Parsed toggle, Auto-scroll toggle.

**Column layout per row:**
1. Timestamp (24ch) — from backend `HH:MM:SS.mmm`
2. Direction (1ch) — `S:` green / `R:` blue
3. Source → Dest aliases (28ch) — formatted as `0xAAA → 0xC41`
4. MTI name (40ch) — purple
5. Decoded payload (flex) — or raw frame string in raw mode

## Common Change Scenarios

### Change what a specific MTI displays

Edit `DecodedMessage::decode_payload()` in `app/src-tauri/src/traffic/mod.rs`. Add a new match arm for the MTI variant. MTI variants come from `lcc-rs/src/protocol/mti.rs`.

### Add a new column to the UI

1. Add the field to `DecodedMessage` in `app/src-tauri/src/traffic/mod.rs`
2. Map it onto `MessageReceivedEvent` in `app/src-tauri/src/events/router.rs`
3. Add to `TrafficMessage` interface in `app/src/lib/api/types.ts`
4. Render in the `{#each}` block in `TrafficMonitor.svelte`

### Change how direction is determined

Direction is set in `DecodedMessage::decode()` by comparing `source_alias == our_alias`. `our_alias` is `0xAAA` (hardcoded in `lcc-rs/src/discovery.rs` `LccConnection::connect_with_dispatcher`). The router passes it through from `state.rs`.

### Filter messages

Filtering should be done in `TrafficMonitor.svelte` at the display level, not in the backend, to preserve all raw data in the buffer. Add a `$derived` array that filters `messages` and use that in `{#each}`.

### Remove debug logging

`eprintln!` calls exist in:
- `app/src-tauri/src/events/router.rs` — `router_loop` start and `handle_all_messages`
- `lcc-rs/src/dispatcher.rs` — `listener_loop` error path (intentional)

## Known Issues / Notes

- `MTI::from_header()` is in `lcc-rs/src/protocol/mti.rs`. If a frame has an unrecognised header, `DecodedMessage` returns `direction: "?"` and `mti_name: "Unknown"`.
- SNIP responses arrive as multiple CAN frames (first/middle/final). The decoder handles each frame individually — there is no multi-frame reassembly; each frame gets its own row.
- Datagram memory reads produce many identical middle frames (`FF FF FF FF FF FF FF FF`) — this was the original cause of the `each_key_duplicate` crash, now fixed by the `seq` counter.
- `our_alias` is hardcoded as `0xAAA`; if nodes share this alias there would be misclassified directions.
