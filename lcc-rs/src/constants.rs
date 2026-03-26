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
/// allowing slower/bridged nodes (e.g. JMRI TCP gateway nodes) time to reply.
/// Set to 250ms to cover typical bridge latencies (observed ~165ms for a JMRI
/// TCP-bridged UWT-100 throttle on initial connect).
pub const DISCOVERY_SILENCE_THRESHOLD_MS: u64 = 250;

/// Maximum time (ms) to block on a single channel/transport poll tick inside
/// the discovery loop. Keeps the silence-threshold check responsive.
pub const DISCOVERY_POLL_INTERVAL_MS: u64 = 10;

/// Time (ms) to listen for alias conflicts after sending CID frames.
/// Per OpenLCB S-9.7.2.1: "nodes that may take time to set up a response
/// shall be given 400ms". JMRI/OpenLCB_Java use 400ms; Bowties previously
/// used 200ms which could cause false-positive alias claims on slower buses.
pub const ALIAS_CONFLICT_LISTEN_MS: u64 = 400;

/// Delay (ms) after TCP connect before starting alias allocation.
/// Bridges/gateways (e.g. JMRI LCC Hub, WifiTrax) may not be ready to
/// relay frames immediately after the TCP handshake. JMRI applies a similar
/// delay before beginning alias negotiation.
pub const CONNECTION_STABILIZATION_MS: u64 = 500;
