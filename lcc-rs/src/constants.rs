//! Protocol-level timing and retry constants for LCC operations.

/// Per-attempt timeout (ms) when waiting for a Datagram Received OK after a
/// memory-config write. Three seconds matches the OpenLCB reference
/// implementation and typical node firmware.
pub const WRITE_MEMORY_TIMEOUT_MS: u64 = 3_000;

/// Maximum number of write attempts before returning an error.
pub const WRITE_MEMORY_MAX_RETRIES: u32 = 3;

/// Timeout (ms) when waiting for the Datagram Received OK that acknowledges an
/// Update Complete command. Nodes may spend several seconds flushing changes to
/// non-volatile storage before they reply, so this is set generously higher
/// than the per-write timeout.
pub const UPDATE_COMPLETE_TIMEOUT_MS: u64 = 10_000;

/// After the first VerifiedNode response during discovery, stop waiting once
/// this many milliseconds have elapsed with no further responses. This avoids
/// stalling on the full discovery timeout on a quiet network, while still
/// allowing slower nodes time to reply.
pub const DISCOVERY_SILENCE_THRESHOLD_MS: u64 = 100;

/// Maximum time (ms) to block on a single channel/transport poll tick inside
/// the discovery loop. Keeps the silence-threshold check responsive.
pub const DISCOVERY_POLL_INTERVAL_MS: u64 = 10;
