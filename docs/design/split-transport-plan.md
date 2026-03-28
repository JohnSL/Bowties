# Split-Transport Implementation Plan

## Problem

Reader and writer loops in `TransportActor` share `Arc<tokio::sync::Mutex<Box<dyn LccTransport>>>`. Reader holds the mutex during 1ms timeout polls, blocking the writer from sending. This adds ~1-2ms to every round-trip and causes the measured performance regression.

## Solution

Split each transport into independent read/write halves. Reader task owns its half exclusively (blocks until data, no timeout, no mutex). Writer task owns its half exclusively (drains mpsc, no mutex).

## Architecture After

```
connect_with_dispatcher():
  1. transport = TcpTransport::connect() → Box<dyn LccTransport>
  2. AliasAllocator::allocate(&mut transport)  // uses unsplit transport
  3. TransportActor::new(transport)           // calls transport.into_halves() internally
       ├── reader_loop(reader_half)  ← owns read half, blocks on receive(), select! for shutdown
       └── writer_loop(writer_half)  ← owns write half, drains mpsc
```

## New Traits (in `tcp.rs`)

```rust
#[async_trait]
pub trait TransportReader: Send {
    /// Receive a single frame. Blocks until data arrives or an error (including
    /// ConnectionClosed) occurs. No timeout — the caller uses `tokio::select!`
    /// with a shutdown signal instead.
    async fn receive(&mut self) -> Result<GridConnectFrame>;
}

#[async_trait]
pub trait TransportWriter: Send {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
}
```

Add to `LccTransport`:
```rust
fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>);
```

## Per-Transport Splitting

| Transport | Split mechanism | Reader half | Writer half |
|---|---|---|---|
| **TCP** | `TcpStream::into_split()` → zero-cost `OwnedReadHalf`/`OwnedWriteHalf` | `BufReader<OwnedReadHalf>` + `String` buffer, same `fill_buf`/`consume` logic | `OwnedWriteHalf`, `write_all` + `flush` |
| **GridConnect serial** | `tokio::io::split(port)` → `ReadHalf`/`WriteHalf` | `ReadHalf<SerialStream>` + `Vec<u8>` read_buf, same byte-by-byte logic | `WriteHalf<SerialStream>`, `write_all` + `flush` |
| **SLCAN serial** | `tokio::io::split(port)` → same | Same as GridConnect but `\r`-delimited + SLCAN decode | Same + `close()` sends `"C\r"` |
| **Mock** | Hand off existing `Arc<Mutex>` refs | `Arc<Mutex<VecDeque<String>>>` | `Arc<Mutex<Vec<String>>>` |

**Note on serial split**: `tokio::io::split()` uses internal `Arc<Mutex>`, but the lock is only held during `poll_read`/`poll_write` syscalls (microseconds), not during timeouts. This is a non-issue.

## TransportActor Changes

**`new()`**: Calls `transport.into_halves()`, removes `Arc<Mutex>` entirely.

**`reader_loop()`**: Uses `tokio::select!` for graceful shutdown:
```rust
loop {
    tokio::select! {
        _ = &mut shutdown_rx => break,
        result = reader.receive() => {
            match result {
                Ok(frame) => { /* broadcast + MTI route + alias map */ }
                Err(e) => { eprintln!(...); break; }
            }
        }
    }
}
```

**`writer_loop()`**: No mutex, just `rx.recv()` → `writer.send()`.

## Cancellation Safety

All reader implementations are cancellation-safe under `tokio::select!`:
- **TCP**: `fill_buf()` preserves buffered data when dropped
- **Serial**: `read_exact` on 1 byte is atomic; accumulation buffer lives in the struct
- **Mock**: `VecDeque::pop_front` is instant

## Unchanged

- `LccTransport` trait's `send`/`receive`/`close` methods stay for `AliasAllocator::allocate()` (pre-actor)
- `TransportHandle` API — no changes
- All `discovery.rs` `_with_handle` methods — no changes
- All Tauri consumers — no changes

## Implementation Steps

1. Define `TransportReader` + `TransportWriter` traits, add `into_halves()` to `LccTransport`
2. Implement for `TcpTransport` (`TcpTransportReader`/`TcpTransportWriter`)
3. Implement for `GridConnectSerialTransport`
4. Implement for `SlcanSerialTransport`
5. Implement for `MockTransport`
6. Rewrite `TransportActor::new()`, `reader_loop()`, `writer_loop()` — remove `Arc<Mutex>`
7. Run all 326 tests, fix any breakage
