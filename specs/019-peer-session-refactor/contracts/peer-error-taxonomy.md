# Contract: `PeerError` Taxonomy & Frontend Mapping

**Module**: `lcc-rs::peer_session::PeerError`
**Consumer boundary**: Tauri command return types → SvelteKit frontend

---

## Invariants

1. **Every existing error variant surfaced by current Tauri commands remains available with the same string discriminant** (FR-018). No frontend code that pattern-matches on error strings breaks.
2. **New variants may be added**, but only additively — existing frontend fallback branches continue to handle them as generic errors.
3. **Every terminal error condition on an active exchange emits `TerminateDueToError` to the peer** before completing the reply (FR-009). This is not observable to the frontend but is observable in traces.

---

## Variant table

| Variant | Serde tag | Emitted when | Peer cleanup? | Frontend handling (existing) |
|---------|-----------|-------------|---------------|------------------------------|
| `Timeout { operation, elapsed }` | `"Timeout"` | Our timeout wall-clock exceeded on the exchange | YES | Existing timeout branch. |
| `PeerReinitialised` | `"PeerReinitialised"` | Peer emitted VNI/InitComplete during our active exchange | NO (peer already reset) | NEW variant — frontend falls through to generic-error banner. Frontend enhancement out of scope. |
| `AliasChanged { old, new }` | `"AliasChanged"` | Peer's alias changed mid-exchange (AMR + new AMD) | NO (old alias invalid) | NEW variant — same generic fallback. |
| `TransportUnhealthy { health }` | `"TransportUnhealthy"` | Transport writer emitted `Wedged`/`Degraded` health, blocking the send | N/A | NEW variant — frontend generic fallback; future slice may surface as connection status. |
| `Rejected { mti, code }` | `"Rejected"` | `DatagramRejected` (permanent, resend-OK=0) or `OptionalInteractionRejected` | YES | Existing rejection branch; string message includes code. |
| `Cancelled { reason }` | `"Cancelled"` | Caller invoked `handle.cancel(...)` or `PeerCommand::Cancel` | YES | Existing cancel branch (currently comes from `cdi_download_cancel`). |
| `NotSupported { operation }` | `"NotSupported"` | Peer's PIP indicates the required capability is absent | NO (never sent) | Existing capability branch. |
| `Protocol(msg)` | `"Protocol"` | Malformed frame or exchange-loop invariant violation | YES (defensive) | Existing malformed-response branch. |

---

## Retry classification (per FR-011)

Not exposed as a distinct error variant — retries happen inside the session before returning. Retry policy:

- **Transient `DatagramRejected` with resend-OK bit set** (`code & 0x1000 == 0`): retry current chunk, increment `retry_state.attempts_at_current_address`, up to `max_attempts` (default 3).
- **Permanent `DatagramRejected` without resend-OK** (`code & 0x1000 != 0`): terminal → `PeerError::Rejected`.
- **`OptionalInteractionRejected`**: terminal → `PeerError::Rejected` (per FR-010).
- **Our timeout**: terminal → `PeerError::Timeout`. Peer cleanup emitted.

---

## Standards references

- `TerminateDueToError` (MTI `0x0A08`): per TN-9.7.2.1 §2.5 — sent when abandoning an exchange the peer believes is still open.
- `OptionalInteractionRejected` (MTI `0x1068`): per TN-9.7.3.2 §3.4 — treated as terminal for the requested exchange (no automatic retry — this is our peer telling us it will not participate).
- `DatagramRejected` (MTI `0x1048`): per TN-9.7.3.2 §2.3 — error code bit 12 (`0x1000`) distinguishes permanent (set) from transient (clear).
