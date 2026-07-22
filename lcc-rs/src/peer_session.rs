//! Per-peer session actor.
//!
//! One `PeerSession` runs per remote NodeID. It owns every protocol
//! interaction with that peer: exchange state, coalesced query waiters,
//! cached SNIP/PIP results, alias renegotiation.
//!
//! Callers dispatch via `PeerSessionHandle` typed methods that construct a
//! `PeerCommand` + oneshot and await the reply. Sole-spawner is
//! [`crate::peer_session_registry::PeerSessionRegistry`]; test code may use
//! [`PeerSession::spawn`] directly.
//!
//! See ADR-0016 for invariants (single ACK owner per peer, single outbound
//! sender per peer, single active exchange per peer, sole-spawner registry,
//! cache-on-`PeerReinitialised`, in-place `AliasChanged`).

use crate::datagram_reader::MemoryReadConfig;
use crate::protocol::datagram::DatagramAssembler;
use crate::protocol::frame::GridConnectFrame;
use crate::protocol::memory_config::{AddressSpace, MemoryConfigCmd, ReadReply};
use crate::protocol::mti::MTI;
use crate::transport_actor::{ReceivedMessage, TransportHandle, TransportHealth};
use crate::types::{NodeID, PIPStatus, ProtocolFlags, SNIPData, SNIPStatus};
use crate::{snip::parse_snip_payload, Error};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant as StdInstant;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{Duration, Instant as TokioInstant};

/// SNIP round-trip cap (matches legacy free-function behaviour).
pub const SNIP_TIMEOUT: Duration = Duration::from_secs(5);
/// Silence window that closes a multi-frame SNIP response.
pub const SNIP_SILENCE_TIMEOUT: Duration = Duration::from_millis(100);
/// PIP round-trip cap.
pub const PIP_TIMEOUT: Duration = Duration::from_secs(2);
/// PIP inter-frame silence window (matches legacy free-function behaviour).
pub const PIP_SILENCE_TIMEOUT: Duration = Duration::from_millis(200);

/// Per-chunk read size for CDI downloads (matches legacy behaviour).
const CDI_CHUNK_SIZE: u8 = 64;
/// Hard cap on assembled CDI size before we abort (matches legacy 10MB limit).
const CDI_MAX_BYTES: usize = 10 * 1024 * 1024;

const COMMAND_CAPACITY: usize = 64;

/// Per-peer inbound channel capacity (S7 D1=A). Bounds how many frames the
/// registry demux may buffer for one session before it must drop + coalesce
/// the loss into an [`InboundEvent::Lagged`] marker.
pub const INBOUND_CAPACITY: usize = 256;

/// An inbound delivery to a `PeerSession` over its per-peer channel (S7 D1=A).
///
/// The `PeerSessionRegistry` demux drains the transport broadcast and forwards
/// each frame routed to this peer as [`InboundEvent::Frame`]. When the
/// per-peer channel overflows, the demux drops frames and coalesces the loss
/// into an [`InboundEvent::Lagged`] marker delivered before the next frame —
/// the session treats it exactly like the former
/// `broadcast::error::RecvError::Lagged(n)` signal.
#[derive(Debug, Clone)]
pub enum InboundEvent {
    /// A frame routed to this peer's session.
    Frame(ReceivedMessage),
    /// `n` inbound frames were dropped before this point due to per-peer
    /// channel overflow (or an upstream broadcast lag).
    Lagged(u64),
}

/// Extract the source alias from a frame's header for per-peer routing.
///
/// Datagram frames carry the source alias in the datagram header; addressed
/// and global frames carry it in the standard MTI header. Shared by the
/// registry demux (`PeerSessionRegistry`) and the test-convenience forwarder
/// in [`PeerSession::spawn`] so both route by exactly the same rule. Returns
/// `None` for frames whose header decodes as neither.
pub(crate) fn source_alias(frame: &GridConnectFrame) -> Option<u16> {
    if let Ok((mti, src, _dest)) = MTI::from_datagram_header(frame.header) {
        if matches!(
            mti,
            MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal
        ) {
            return Some(src);
        }
    }
    MTI::from_header(frame.header).ok().map(|(_, src)| src)
}

pub type SnipResult = Result<Option<SNIPData>, PeerError>;
pub type PipResult = Result<Option<ProtocolFlags>, PeerError>;
pub type CdiResult = Result<CdiCompletion, PeerError>;

/// Result of a single-datagram memory read primitive (S4 D1=A).
///
/// Returns the raw reply payload bytes plus per-frame timing metadata,
/// reusing [`crate::discovery::MemoryReadTiming`] so the diagnostics boundary
/// stays identical to the pre-refactor `read_memory_timed` shape (FR-018).
pub type MemoryReadResult = Result<(Vec<u8>, crate::discovery::MemoryReadTiming), PeerError>;

/// Result of a single memory write primitive (S4 D1=A). Success carries no
/// payload — a `DatagramReceivedOk` (RequestWithNoReply) means the write
/// applied.
pub type MemoryWriteResult = Result<(), PeerError>;

/// Typed error surfaced across the `PeerSession` API.
///
/// Serde-serialised with a stable `type` string tag so the frontend can
/// pattern-match without ambiguity (FR-018).
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PeerError {
    #[error("timeout during {operation} after {elapsed_ms}ms")]
    Timeout {
        operation: String,
        elapsed_ms: u64,
    },

    #[error("peer reinitialised during active exchange")]
    PeerReinitialised,

    #[error("peer alias changed from {old:03X} to {new:03X}")]
    AliasChanged { old: u16, new: u16 },

    #[error("transport unhealthy: {reason}")]
    TransportUnhealthy { reason: String },

    #[error("peer rejected: mti=0x{mti:05X} code=0x{code:04X}")]
    Rejected { mti: u32, code: u16 },

    #[error("cancelled: {reason}")]
    Cancelled { reason: String },

    #[error("operation not supported: {operation}")]
    NotSupported { operation: String },

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("peer session not connected")]
    NotConnected,
}

impl PeerError {
    /// Fault-nature classifier for the peer-cleanup contract (ADR-0018).
    ///
    /// Returns `true` when this error represents a fault on **our** side while
    /// the wire is still live — meaning we must release the peer's exchange
    /// state by emitting exactly one `TerminateDueToError`. This is the single
    /// source of truth for the `abort_active` cleanup gate (S7 D2 / T2),
    /// replacing the former `matches!(err, Cancelled | Rejected)` allowlist so
    /// that lag-recovery exhaustion (`Protocol`) also cleans up.
    ///
    /// Excluded (return `false`):
    /// - `TransportUnhealthy` — the wire is dead; a cleanup frame cannot land.
    /// - `PeerReinitialised` / `AliasChanged` — peer-initiated; the peer has
    ///   already released or replaced its exchange state.
    /// - `NotSupported` / `NotConnected` — no exchange ever reached the wire.
    ///
    /// Note on `Timeout`: CDI timeout cleanup is emitted directly in
    /// `handle_deadline` (which does not route through `abort_active`), so
    /// classifying `Timeout` as our-fault-live-wire here is semantically
    /// correct without causing a double-emit — no live code path calls
    /// `abort_active` with a `Timeout`.
    pub fn is_our_fault_live_wire(&self) -> bool {
        matches!(
            self,
            PeerError::Cancelled { .. }
                | PeerError::Rejected { .. }
                | PeerError::Protocol(_)
                | PeerError::Timeout { .. }
        )
    }
}

impl From<Error> for PeerError {
    fn from(err: Error) -> Self {
        match err {
            Error::TransportUnhealthy(reason) => PeerError::TransportUnhealthy { reason },
            other => PeerError::Protocol(other.to_string()),
        }
    }
}

/// Successful CDI download payload returned by `PeerSessionHandle::download_cdi`.
///
/// Amends the peer-session contract per ADR-0018 (S3 D2 outcome): the CDI
/// exchange returns its assembled bytes plus per-chunk timing metadata
/// synchronously — no `broadcast::Sender<CdiProgress>` argument and no Tauri
/// `cdi-progress` event surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdiCompletion {
    /// Assembled CDI XML bytes (up to and excluding the first null terminator).
    pub bytes: Vec<u8>,
    /// Per-chunk timing + retry stats.
    pub stats: CdiStats,
}

/// Per-chunk statistics recorded during a CDI download.
///
/// Mirrors `CdiDownloadStats` at the diagnostics boundary (FR-018).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CdiStats {
    /// Assembled CDI byte count.
    pub total_bytes: usize,
    /// Number of successful chunks read.
    pub chunks: usize,
    /// Per-chunk elapsed time in milliseconds (index 0 = first chunk).
    pub chunk_durations_ms: Vec<u32>,
    /// Total DR-with-resend-OK retries across all chunks.
    pub total_retries: usize,
    /// Total wall-clock duration from first chunk request to completion.
    pub total_duration_ms: u64,
}

/// Commands accepted by a `PeerSession`.
///
/// Public variants come from external callers via `PeerSessionHandle`.
/// Registry-internal variants (`PeerReinitialised`, `AliasChanged`,
/// `TransportWedged`) are dispatched by the registry's spawn-watcher and the
/// session's own health-forwarder task, but the type is public so tests can
/// construct them.
pub enum PeerCommand {
    QuerySnip {
        reply: oneshot::Sender<SnipResult>,
    },
    QueryPip {
        reply: oneshot::Sender<PipResult>,
    },
    /// Read the peer's full CDI over the Memory Configuration Protocol
    /// (address space 0xFF). Assembled bytes plus per-chunk stats are
    /// returned on `reply`. See ADR-0018.
    DownloadCdi {
        config: MemoryReadConfig,
        reply: oneshot::Sender<CdiResult>,
    },
    /// Read `count` bytes from `space` at `address` via a single Memory
    /// Configuration read datagram exchange (S4 D1=A). Reply bytes + timing
    /// return on `reply`. `count` must be 1..=64 (single datagram).
    ReadMemory {
        space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
        reply: oneshot::Sender<MemoryReadResult>,
    },
    /// Write `data` to `space` at `address` via a single Memory Configuration
    /// write datagram exchange (S4 D1=A). Uses the RequestWithNoReply
    /// pattern: `DatagramReceivedOk` = success. `data` must be 1..=64 bytes
    /// (single datagram; the handle chunks larger writes).
    WriteMemory {
        space: u8,
        address: u32,
        data: Vec<u8>,
        timeout_ms: u64,
        reply: oneshot::Sender<MemoryWriteResult>,
    },
    Cancel {
        reason: String,
    },
    PeerReinitialised,
    AliasChanged {
        new_alias: u16,
    },
    /// Internal: forwarded from the transport health watch by a per-session
    /// task. Aborts the active exchange with `PeerError::TransportUnhealthy`
    /// and does NOT emit `TerminateDueToError` (ADR-0016 D1 — peer cleanup
    /// requires a live wire).
    TransportWedged {
        reason: String,
    },
}

/// The in-flight exchange (S2 supports SNIP + PIP; S3 adds CDI).
enum ActiveExchange {
    SnipQuery {
        payload: Vec<u8>,
        receiving: bool,
        received_first_frame: bool,
        deadline: TokioInstant,
    },
    PipQuery {
        deadline: TokioInstant,
    },
    /// CDI download exchange (ADR-0018).
    ///
    /// Owns the full multi-chunk state machine that used to live in the
    /// `datagram_read_exchange` free function: request assembly, DR-with-
    /// resend-OK retry, OIR-terminal classification, and null-terminator
    /// short-read termination.
    CdiDownload {
        /// Effective per-chunk deadline (moved forward on each new chunk
        /// request or DatagramReceivedOk timeout extension).
        deadline: TokioInstant,
        /// Byte cursor into the peer's CDI address space (0xFF).
        address_cursor: u32,
        /// DR-with-resend-OK retry counter for the current chunk only.
        chunk_retry_count: u32,
        /// Cumulative inbound-lag recovery counter for the whole download
        /// (S7 D2=C). Distinct from `chunk_retry_count` (per-chunk DR-resend).
        /// NOT reset on chunk advance, so a sustained lag storm terminates
        /// deterministically once it reaches `config.max_retries`.
        lag_recovery_count: u32,
        /// Reassembly buffer for the current chunk's reply datagram.
        assembler: DatagramAssembler,
        /// Accumulated CDI bytes across chunks (excluding null terminator).
        assembled: Vec<u8>,
        /// Wall-clock start of the current chunk's request-send.
        chunk_start: StdInstant,
        /// Overall stats snapshot in progress; finalised on completion.
        stats: CdiStats,
        /// Wall-clock start of the whole download (for `total_duration_ms`).
        download_start: StdInstant,
        /// Tuning parameters for this download (per-chunk timeout, retry cap,
        /// post-ACK delay). Captured at start so mid-download reconfiguration
        /// cannot desync the chunk loop.
        config: MemoryReadConfig,
        /// Waiter to notify on completion.
        waiter: oneshot::Sender<CdiResult>,
    },
    /// Single-datagram memory read exchange (S4 D1=A).
    ///
    /// Essentially one CDI chunk with an arbitrary address space + count.
    /// Owns: reply-identity guard, single ACK ownership, DR-with-resend-OK
    /// (wait, no immediate retry), OIR-terminal classification, and per-frame
    /// timing capture.
    MemoryRead {
        /// Per-op deadline (moved forward on `DatagramReceivedOk` extension).
        deadline: TokioInstant,
        /// Address space byte (0x00–0xFF).
        space: u8,
        /// Read start address.
        address: u32,
        /// Requested byte count (1..=64).
        count: u8,
        /// Cumulative inbound-lag recovery counter (S7 D2=C bounded recovery).
        lag_recovery_count: u32,
        /// Reassembly buffer for the reply datagram.
        assembler: DatagramAssembler,
        /// Wall-clock start of the request send (first_frame_latency + total).
        request_start: StdInstant,
        /// Timestamp of the first reply frame, set when it arrives.
        first_frame_at: Option<StdInstant>,
        /// Timestamps of each reply frame for inter-frame gap capture.
        frame_times: Vec<StdInstant>,
        /// Per-op tuning (timeout, retry cap).
        config: MemoryReadConfig,
        /// Waiter to notify on completion.
        waiter: oneshot::Sender<MemoryReadResult>,
    },
    /// Single-datagram memory write exchange (S4 D1=A).
    ///
    /// RequestWithNoReply pattern: `DatagramReceivedOk` (without reply-pending)
    /// = success. Retries on resend-OK DR up to `WRITE_MEMORY_MAX_RETRIES`.
    MemoryWrite {
        /// Per-op deadline.
        deadline: TokioInstant,
        /// Address space byte.
        space: u8,
        /// Write address.
        address: u32,
        /// Payload to write (1..=64 bytes).
        data: Vec<u8>,
        /// Resend-OK DR retry counter.
        retry_count: u32,
        /// Cumulative inbound-lag recovery counter (S7 D2=C).
        lag_recovery_count: u32,
        /// Per-op timeout in milliseconds (for deadline refresh on resend).
        timeout_ms: u64,
        /// Waiter to notify on completion.
        waiter: oneshot::Sender<MemoryWriteResult>,
    },
}

/// A pending memory operation queued behind the active exchange (S4 D1=A).
/// A single unified FIFO queue preserves strict issue order across mixed
/// read/write operations so concurrent callers on one handle serialise
/// deterministically (spec acceptance: concurrent read/write serialise FIFO).
enum PendingMemOp {
    Read {
        space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
        reply: oneshot::Sender<MemoryReadResult>,
    },
    Write {
        space: u8,
        address: u32,
        data: Vec<u8>,
        timeout_ms: u64,
        reply: oneshot::Sender<MemoryWriteResult>,
    },
}

/// Cheap-to-clone dispatch surface for callers.
#[derive(Clone)]
pub struct PeerSessionHandle {
    node_id: NodeID,
    commands: mpsc::Sender<PeerCommand>,
}

impl PeerSessionHandle {
    /// The peer's NodeID (for logging).
    pub fn node_id(&self) -> NodeID {
        self.node_id
    }

    /// Query SNIP data. Cached results return immediately; concurrent callers
    /// coalesce onto a single wire exchange.
    pub async fn query_snip(&self) -> SnipResult {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(PeerCommand::QuerySnip { reply: tx })
            .await
            .map_err(|_| PeerError::NotConnected)?;
        rx.await.map_err(|_| PeerError::NotConnected)?
    }

    /// Query PIP flags.
    pub async fn query_pip(&self) -> PipResult {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(PeerCommand::QueryPip { reply: tx })
            .await
            .map_err(|_| PeerError::NotConnected)?;
        rx.await.map_err(|_| PeerError::NotConnected)?
    }

    /// Cancel the active exchange (if any). Fire-and-forget.
    pub async fn cancel(&self, reason: impl Into<String>) {
        let _ = self
            .commands
            .send(PeerCommand::Cancel {
                reason: reason.into(),
            })
            .await;
    }

    /// Download the peer's CDI (address space 0xFF) with the given tuning.
    ///
    /// Multiple concurrent calls on the same handle are queued FIFO by the
    /// session actor — there is no external `CdiInflightRegistry` in the
    /// codebase; the per-peer serialisation is structural (ADR-0018).
    pub async fn download_cdi(&self, config: MemoryReadConfig) -> CdiResult {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(PeerCommand::DownloadCdi { config, reply: tx })
            .await
            .map_err(|_| PeerError::NotConnected)?;
        rx.await.map_err(|_| PeerError::NotConnected)?
    }

    /// Read `count` bytes from `space` at `address` via a single Memory
    /// Configuration read datagram exchange. Serialised per-peer behind any
    /// active exchange (structural, ADR-0016). `count` must be 1..=64.
    pub async fn read_memory(
        &self,
        space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> MemoryReadResult {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(PeerCommand::ReadMemory {
                space,
                address,
                count,
                timeout_ms,
                reply: tx,
            })
            .await
            .map_err(|_| PeerError::NotConnected)?;
        rx.await.map_err(|_| PeerError::NotConnected)?
    }

    /// Write `data` to `space` at `address`. Data longer than 64 bytes is
    /// automatically chunked into sequential ≤64-byte single-datagram writes
    /// (mirroring the legacy `LccConnection::write_memory` contract); each
    /// chunk serialises through the actor's FIFO. Success requires every
    /// chunk to complete.
    pub async fn write_memory(
        &self,
        space: u8,
        address: u32,
        data: Vec<u8>,
        timeout_ms: u64,
    ) -> MemoryWriteResult {
        if data.is_empty() {
            return Err(PeerError::Protocol("write data cannot be empty".into()));
        }
        let mut offset: usize = 0;
        while offset < data.len() {
            let chunk_size = (data.len() - offset).min(64);
            let chunk = data[offset..offset + chunk_size].to_vec();
            let chunk_address = address + offset as u32;
            let (tx, rx) = oneshot::channel();
            self.commands
                .send(PeerCommand::WriteMemory {
                    space,
                    address: chunk_address,
                    data: chunk,
                    timeout_ms,
                    reply: tx,
                })
                .await
                .map_err(|_| PeerError::NotConnected)?;
            rx.await.map_err(|_| PeerError::NotConnected)??;
            offset += chunk_size;
        }
        Ok(())
    }

    /// Raw command dispatch (used by the registry for internal control frames).
    pub async fn command(&self, cmd: PeerCommand) -> Result<(), PeerError> {
        self.commands
            .send(cmd)
            .await
            .map_err(|_| PeerError::NotConnected)
    }
}

/// The per-peer actor.
pub struct PeerSession {
    node_id: NodeID,
    alias: u16,
    our_alias: u16,
    transport: TransportHandle,
    inbound: mpsc::Receiver<InboundEvent>,
    /// False once the inbound channel's senders have all dropped; the run
    /// loop then stops polling inbound. The session's lifetime is governed by
    /// its command channel, not the inbound channel.
    inbound_open: bool,
    commands: mpsc::Receiver<PeerCommand>,

    snip_cache: Option<SNIPData>,
    snip_status: SNIPStatus,
    snip_waiters: Vec<oneshot::Sender<SnipResult>>,
    pip_cache: Option<ProtocolFlags>,
    pip_status: PIPStatus,
    pip_waiters: Vec<oneshot::Sender<PipResult>>,

    /// FIFO queue of pending CDI downloads waiting for the active exchange
    /// (any variant) to complete. Each entry carries its own tuning config
    /// so a per-call `MemoryReadConfig` override survives queueing.
    cdi_pending: VecDeque<(MemoryReadConfig, oneshot::Sender<CdiResult>)>,

    /// Unified FIFO queue of pending single-datagram memory reads/writes
    /// waiting for the active exchange (any variant) to complete. A single
    /// queue preserves strict issue order across mixed read/write ops (S4).
    mem_pending: VecDeque<PendingMemOp>,

    active: Option<ActiveExchange>,
    last_known_wedge_reason: Option<String>,
}

impl PeerSession {
    /// Spawn a new session and return only the handle. Discards both task
    /// JoinHandles — safe for tests but **not** for production teardown.
    /// Production callers use [`Self::spawn_with_tasks`] (used by
    /// `PeerSessionRegistry`).
    ///
    /// The session runs on a dedicated tokio task and terminates when the
    /// command channel closes (i.e. all handles dropped and the registry
    /// removed its entry).
    pub fn spawn(
        node_id: NodeID,
        alias: u16,
        our_alias: u16,
        transport: TransportHandle,
    ) -> PeerSessionHandle {
        // Test/back-compat forwarder: mirror the transport broadcast into a
        // fresh per-peer channel. Production wiring uses the registry demux
        // (`PeerSessionRegistry`), which routes by source alias and re-keys on
        // AMD/AMR churn. This forwarder deliberately does NOT filter by source
        // alias: it has no alias-update channel, and the session's
        // destination check (`dest == our_alias`) already discards frames not
        // addressed to us — which is sufficient for the single-peer test
        // harness and keeps the forwarder correct across an `AliasChanged`.
        let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_CAPACITY);
        let mut bcast = transport.subscribe_all();
        tokio::spawn(async move {
            loop {
                match bcast.recv().await {
                    Ok(msg) => {
                        if inbound_tx.send(InboundEvent::Frame(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        if inbound_tx.send(InboundEvent::Lagged(n)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        let (handle, _session_task, _health_task) =
            Self::spawn_with_tasks(node_id, alias, our_alias, transport, inbound_rx);
        handle
    }

    /// Spawn a new session and return the handle plus the JoinHandles of the
    /// two spawned tasks (main `run()` + optional health-forwarder).
    ///
    /// Callers **must** retain the returned JoinHandles and abort them on
    /// teardown; otherwise the health-forwarder's captured `TransportHandle`
    /// clone keeps the transport broadcast + watch channels alive
    /// indefinitely (self-referential Arc closure — see ADR-0016 §2026-07-14
    /// extension). `PeerSessionRegistry` is the sole production caller and
    /// stores the JoinHandles in its entry table for later abort.
    pub fn spawn_with_tasks(
        node_id: NodeID,
        alias: u16,
        our_alias: u16,
        transport: TransportHandle,
        inbound: mpsc::Receiver<InboundEvent>,
    ) -> (
        PeerSessionHandle,
        tokio::task::JoinHandle<()>,
        Option<tokio::task::JoinHandle<()>>,
    ) {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CAPACITY);

        // Spawn a health-forwarder task that pushes `TransportWedged` into the
        // command channel on `Healthy → Wedged` transitions. This decouples
        // the actor loop from `watch::Receiver` borrowing.
        let health_task = if let Some(mut health_rx) = transport.subscribe_health() {
            let cmd_tx_health = cmd_tx.clone();
            Some(tokio::spawn(async move {
                // If the wire is already wedged at construction, forward once.
                let initial_wedge = {
                    let snap = health_rx.borrow_and_update();
                    if let TransportHealth::Wedged { ref reason } = *snap {
                        Some(reason.clone())
                    } else {
                        None
                    }
                };
                if let Some(reason) = initial_wedge {
                    let _ = cmd_tx_health
                        .send(PeerCommand::TransportWedged { reason })
                        .await;
                }
                while health_rx.changed().await.is_ok() {
                    let snapshot = {
                        let snap = health_rx.borrow_and_update();
                        (*snap).clone()
                    };
                    if let TransportHealth::Wedged { reason } = snapshot {
                        if cmd_tx_health
                            .send(PeerCommand::TransportWedged { reason })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }))
        } else {
            None
        };

        let session = PeerSession {
            node_id,
            alias,
            our_alias,
            transport,
            inbound,
            inbound_open: true,
            commands: cmd_rx,
            snip_cache: None,
            snip_status: SNIPStatus::Unknown,
            snip_waiters: Vec::new(),
            pip_cache: None,
            pip_status: PIPStatus::Unknown,
            pip_waiters: Vec::new(),
            cdi_pending: VecDeque::new(),
            mem_pending: VecDeque::new(),
            active: None,
            last_known_wedge_reason: None,
        };

        let session_task = tokio::spawn(session.run());

        let handle = PeerSessionHandle {
            node_id,
            commands: cmd_tx,
        };
        (handle, session_task, health_task)
    }

    async fn run(mut self) {
        loop {
            // Compute the deadline for the current active exchange (if any).
            // SNIP switches to a shorter silence-window once the first frame
            // arrives; the exchange stores the appropriate absolute deadline.
            let deadline = match &self.active {
                Some(ActiveExchange::SnipQuery { deadline, .. }) => Some(*deadline),
                Some(ActiveExchange::PipQuery { deadline, .. }) => Some(*deadline),
                Some(ActiveExchange::CdiDownload { deadline, .. }) => Some(*deadline),
                Some(ActiveExchange::MemoryRead { deadline, .. }) => Some(*deadline),
                Some(ActiveExchange::MemoryWrite { deadline, .. }) => Some(*deadline),
                None => None,
            };

            tokio::select! {
                biased;

                cmd = self.commands.recv() => {
                    match cmd {
                        Some(command) => self.handle_command(command).await,
                        None => break,
                    }
                }

                // Inbound is polled only while an exchange is active: a
                // per-peer channel buffers frames that arrive between exchanges
                // (bounded by `INBOUND_CAPACITY`; overflow coalesces into a
                // `Lagged` marker), and stale frames are tolerated by the
                // reply-identity guard + stray-`DatagramMiddle` recovery in
                // `on_cdi_frame`. Draining while idle would instead discard a
                // reply that the mock harness pre-queues before its request.
                frame_res = self.inbound.recv(), if self.inbound_open && self.active.is_some() => {
                    match frame_res {
                        Some(InboundEvent::Frame(msg)) => self.handle_inbound_frame(&msg).await,
                        Some(InboundEvent::Lagged(n)) => self.handle_inbound_lag(n).await,
                        None => {
                            // All senders (registry demux / test forwarder)
                            // dropped. Stop polling inbound; the command
                            // channel still governs the session's lifetime.
                            self.inbound_open = false;
                        }
                    }
                }

                _ = async move {
                    match deadline {
                        Some(d) => tokio::time::sleep_until(d).await,
                        None => std::future::pending::<()>().await,
                    }
                }, if self.active.is_some() => {
                    self.handle_deadline().await;
                }
            }
        }

        self.abort_active(PeerError::Cancelled { reason: "session shutdown".into() }).await;
        for w in self.snip_waiters.drain(..) { let _ = w.send(Err(PeerError::NotConnected)); }
        for w in self.pip_waiters.drain(..) { let _ = w.send(Err(PeerError::NotConnected)); }
        for (_, w) in self.cdi_pending.drain(..) { let _ = w.send(Err(PeerError::NotConnected)); }
        for op in self.mem_pending.drain(..) {
            match op {
                PendingMemOp::Read { reply, .. } => { let _ = reply.send(Err(PeerError::NotConnected)); }
                PendingMemOp::Write { reply, .. } => { let _ = reply.send(Err(PeerError::NotConnected)); }
            }
        }
    }

    async fn handle_command(&mut self, cmd: PeerCommand) {
        match cmd {
            PeerCommand::QuerySnip { reply } => {
                if matches!(self.snip_status, SNIPStatus::Complete | SNIPStatus::Timeout) {
                    let _ = reply.send(Ok(self.snip_cache.clone()));
                    return;
                }
                // Fast-path: if the last observed wedge is still current, fail
                // immediately. (S3+ may swap this for a peek at the health
                // receiver; for S2 we rely on the forwarder to have emitted
                // TransportWedged before this command runs.)
                self.snip_waiters.push(reply);
                if self.active.is_some() { return; }
                self.start_snip_exchange().await;
            }
            PeerCommand::QueryPip { reply } => {
                if matches!(self.pip_status, PIPStatus::Complete | PIPStatus::Timeout) {
                    let _ = reply.send(Ok(self.pip_cache.clone()));
                    return;
                }
                self.pip_waiters.push(reply);
                if self.active.is_some() { return; }
                self.start_pip_exchange().await;
            }
            PeerCommand::DownloadCdi { config, reply } => {
                if self.active.is_some() {
                    // Queue behind the current exchange; served FIFO on completion.
                    self.cdi_pending.push_back((config, reply));
                    return;
                }
                self.start_cdi_exchange(config, reply).await;
            }
            PeerCommand::ReadMemory { space, address, count, timeout_ms, reply } => {
                if self.active.is_some() {
                    self.mem_pending.push_back(PendingMemOp::Read {
                        space, address, count, timeout_ms, reply,
                    });
                    return;
                }
                self.start_read_memory_exchange(space, address, count, timeout_ms, reply).await;
            }
            PeerCommand::WriteMemory { space, address, data, timeout_ms, reply } => {
                if self.active.is_some() {
                    self.mem_pending.push_back(PendingMemOp::Write {
                        space, address, data, timeout_ms, reply,
                    });
                    return;
                }
                self.start_write_memory_exchange(space, address, data, timeout_ms, reply).await;
            }
            PeerCommand::Cancel { reason } => {
                self.abort_active(PeerError::Cancelled { reason }).await;
            }
            PeerCommand::PeerReinitialised => {
                self.abort_active(PeerError::PeerReinitialised).await;
                self.snip_cache = None;
                self.snip_status = SNIPStatus::Unknown;
                self.pip_cache = None;
                self.pip_status = PIPStatus::Unknown;
                // Pending CDIs assume the previous peer state; discard.
                for (_, w) in self.cdi_pending.drain(..) {
                    let _ = w.send(Err(PeerError::PeerReinitialised));
                }
                // Pending memory ops likewise assume the prior peer; discard.
                for op in self.mem_pending.drain(..) {
                    match op {
                        PendingMemOp::Read { reply, .. } => { let _ = reply.send(Err(PeerError::PeerReinitialised)); }
                        PendingMemOp::Write { reply, .. } => { let _ = reply.send(Err(PeerError::PeerReinitialised)); }
                    }
                }
            }
            PeerCommand::AliasChanged { new_alias } => {
                if new_alias != self.alias {
                    let old = self.alias;
                    self.alias = new_alias;
                    if self.active.is_some() {
                        self.abort_active(PeerError::AliasChanged { old, new: new_alias }).await;
                    }
                }
            }
            PeerCommand::TransportWedged { reason } => {
                self.last_known_wedge_reason = Some(reason.clone());
                if self.active.is_some() {
                    self.abort_active(PeerError::TransportUnhealthy { reason }).await;
                }
            }
        }
    }

    async fn start_snip_exchange(&mut self) {
        let request = match GridConnectFrame::from_addressed_mti(
            MTI::SNIPRequest,
            self.our_alias,
            self.alias,
            vec![],
        ) {
            Ok(f) => f,
            Err(e) => {
                self.complete_snip(Err(PeerError::Protocol(format!(
                    "failed to build SNIPRequest: {}", e
                )))).await;
                return;
            }
        };

        if let Err(e) = self.transport.send(&request).await {
            self.complete_snip(Err(PeerError::from(e))).await;
            return;
        }

        self.snip_status = SNIPStatus::InProgress;
        self.active = Some(ActiveExchange::SnipQuery {
            payload: Vec::new(),
            receiving: false,
            received_first_frame: false,
            deadline: TokioInstant::now() + SNIP_TIMEOUT,
        });
    }

    async fn start_pip_exchange(&mut self) {
        let request = match GridConnectFrame::from_addressed_mti(
            MTI::ProtocolSupportInquiry,
            self.our_alias,
            self.alias,
            vec![],
        ) {
            Ok(f) => f,
            Err(e) => {
                self.complete_pip(Err(PeerError::Protocol(format!(
                    "failed to build ProtocolSupportInquiry: {}", e
                )))).await;
                return;
            }
        };

        if let Err(e) = self.transport.send(&request).await {
            self.complete_pip(Err(PeerError::from(e))).await;
            return;
        }

        self.pip_status = PIPStatus::InProgress;
        self.active = Some(ActiveExchange::PipQuery {
            deadline: TokioInstant::now() + PIP_TIMEOUT,
        });
    }

    async fn handle_inbound_frame(&mut self, msg: &ReceivedMessage) {
        let frame = &msg.frame;

        // Datagram frames (DatagramOnly/First/Middle/Final) encode the
        // destination alias in the header, not the data. Try that decoding
        // first — a match here also proves the frame IS a datagram.
        //
        // Routing by source alias is owned by the registry demux (S7 D1=A):
        // every `InboundEvent::Frame` delivered here is already for this peer,
        // so the former `dg_source != self.alias` filter is dropped (it would
        // otherwise drop correctly-routed frames during the AMD/AMR alias
        // window, before this session has processed its `AliasChanged`).
        // The destination check stays — it distinguishes datagrams addressed
        // to us from datagrams the peer sent to a third party.
        if let Ok((dg_mti, _dg_source, dg_dest)) = MTI::from_datagram_header(frame.header) {
            if matches!(
                dg_mti,
                MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal
            ) {
                if dg_dest != self.our_alias {
                    return;
                }
                match &self.active {
                    Some(ActiveExchange::CdiDownload { .. }) => self.on_cdi_frame(dg_mti, frame).await,
                    Some(ActiveExchange::MemoryRead { .. }) => self.on_memory_read_frame(dg_mti, frame).await,
                    Some(ActiveExchange::MemoryWrite { .. }) => self.on_memory_write_frame(dg_mti, frame).await,
                    _ => {}
                }
                return;
            }
        }

        // Addressed / global frames: source alias in header, destination
        // alias (for addressed frames) is in data[0..2]. Source routing is
        // the demux's responsibility (see above); only the destination check
        // remains here.
        let (mti, _source) = match MTI::from_header(frame.header) {
            Ok(x) => x,
            Err(_) => return,
        };

        if frame.data.len() >= 2 {
            let dest_in_frame = ((frame.data[0] as u16 & 0x0F) << 8) | frame.data[1] as u16;
            if dest_in_frame != self.our_alias { return; }
        }

        match &self.active {
            Some(ActiveExchange::SnipQuery { .. }) => self.on_snip_frame(mti, frame).await,
            Some(ActiveExchange::PipQuery { .. }) => self.on_pip_frame(mti, frame).await,
            Some(ActiveExchange::CdiDownload { .. }) => self.on_cdi_frame(mti, frame).await,
            Some(ActiveExchange::MemoryRead { .. }) => self.on_memory_read_frame(mti, frame).await,
            Some(ActiveExchange::MemoryWrite { .. }) => self.on_memory_write_frame(mti, frame).await,
            None => {}
        }
    }

    /// Handle a coalesced inbound-lag signal (`n` frames dropped upstream).
    ///
    /// Policy (S7 D2=C):
    /// - **CDI download**: bounded in-place recovery — reset the current
    ///   chunk's assembler and re-issue the in-flight read (idempotent; the
    ///   reply-identity guard discards stale replies). Bound with the
    ///   dedicated `lag_recovery_count` (distinct from the per-chunk DR-resend
    ///   `chunk_retry_count`); on exhaustion abort through the fault-nature
    ///   cleanup gate, which emits exactly one `TerminateDueToError`.
    /// - **SNIP/PIP**: keep the S2 D3 abort-and-continue (caches preserved).
    /// - **Idle**: ignore — there is no exchange to recover.
    async fn handle_inbound_lag(&mut self, n: u64) {
        enum LagAction {
            Recover,
            RecoverRead,
            RecoverWrite,
            Abort(String),
            Ignore,
        }
        // Decide without holding a `&mut self.active` borrow across an await.
        let action = match &mut self.active {
            Some(ActiveExchange::CdiDownload {
                lag_recovery_count,
                config,
                ..
            }) => {
                if *lag_recovery_count < config.max_retries {
                    *lag_recovery_count += 1;
                    LagAction::Recover
                } else {
                    LagAction::Abort(format!(
                        "inbound lag recovery exhausted after {} attempts (dropped {} frames)",
                        config.max_retries, n
                    ))
                }
            }
            Some(ActiveExchange::MemoryRead {
                lag_recovery_count,
                config,
                ..
            }) => {
                if *lag_recovery_count < config.max_retries {
                    *lag_recovery_count += 1;
                    LagAction::RecoverRead
                } else {
                    LagAction::Abort(format!(
                        "inbound lag recovery exhausted after {} attempts (dropped {} frames)",
                        config.max_retries, n
                    ))
                }
            }
            Some(ActiveExchange::MemoryWrite {
                lag_recovery_count,
                ..
            }) => {
                if *lag_recovery_count < crate::constants::WRITE_MEMORY_MAX_RETRIES {
                    *lag_recovery_count += 1;
                    LagAction::RecoverWrite
                } else {
                    LagAction::Abort(format!(
                        "inbound lag recovery exhausted after {} attempts (dropped {} frames)",
                        crate::constants::WRITE_MEMORY_MAX_RETRIES, n
                    ))
                }
            }
            Some(ActiveExchange::SnipQuery { .. }) | Some(ActiveExchange::PipQuery { .. }) => {
                LagAction::Abort(format!("inbound lag: dropped {} frames", n))
            }
            None => LagAction::Ignore,
        };
        match action {
            // Reset the assembler + re-issue the current chunk read.
            LagAction::Recover => self.send_next_chunk_request().await,
            LagAction::RecoverRead => self.send_read_request().await,
            LagAction::RecoverWrite => self.send_write_request().await,
            LagAction::Abort(reason) => self.abort_active(PeerError::Protocol(reason)).await,
            LagAction::Ignore => {}
        }
    }

    async fn on_snip_frame(&mut self, mti: MTI, frame: &GridConnectFrame) {
        if mti == MTI::OptionalInteractionRejected {
            self.snip_status = SNIPStatus::NotSupported;
            self.complete_snip(Ok(None)).await;
            return;
        }
        if mti != MTI::SNIPResponse { return; }
        if frame.data.len() < 2 {
            self.complete_snip(Err(PeerError::Protocol(format!(
                "SNIP frame data too short: {} bytes",
                frame.data.len()
            )))).await;
            return;
        }

        let frame_type = frame.data[0] & 0xF0;
        let chunk_bytes = &frame.data[2..];
        let chunk: Vec<u8> = chunk_bytes.to_vec();

        // We need to end the mutable borrow of `self.active` before calling
        // `complete_snip`. Split into a compute-then-decide pattern.
        let mut completed: Option<Result<Option<SNIPData>, PeerError>> = None;

        if let Some(ActiveExchange::SnipQuery {
            payload,
            receiving,
            received_first_frame,
            deadline,
        }) = &mut self.active {
            *received_first_frame = true;
            // Once we've seen a real SNIP frame, shorten the deadline to the
            // inter-frame silence window.
            *deadline = TokioInstant::now() + SNIP_SILENCE_TIMEOUT;
            match frame_type {
                0x10 => {
                    payload.clear();
                    payload.extend_from_slice(&chunk);
                    *receiving = true;
                }
                0x30 => {
                    if !*receiving {
                        completed = Some(Err(PeerError::Protocol(
                            "SNIP middle frame received without first frame".into(),
                        )));
                    } else {
                        payload.extend_from_slice(&chunk);
                    }
                }
                0x20 => {
                    if !*receiving {
                        completed = Some(Err(PeerError::Protocol(
                            "SNIP final frame received without first frame".into(),
                        )));
                    } else {
                        payload.extend_from_slice(&chunk);
                        let final_payload = payload.clone();
                        completed = Some(match parse_snip_payload(&final_payload) {
                            Ok(data) => Ok(Some(data)),
                            Err(e) => Err(PeerError::Protocol(e.to_string())),
                        });
                    }
                }
                0x00 => {
                    payload.extend_from_slice(&chunk);
                    *receiving = true;
                }
                _ => { /* ignore */ }
            }
        }

        if let Some(result) = completed {
            self.complete_snip(result).await;
        }
    }

    async fn on_pip_frame(&mut self, mti: MTI, frame: &GridConnectFrame) {
        if mti == MTI::OptionalInteractionRejected {
            self.pip_status = PIPStatus::NotSupported;
            self.complete_pip(Ok(None)).await;
            return;
        }
        if mti != MTI::ProtocolSupportReply { return; }
        if frame.data.len() < 2 {
            self.complete_pip(Err(PeerError::Protocol(format!(
                "PIP reply frame too short: {} bytes",
                frame.data.len()
            )))).await;
            return;
        }
        let flags = ProtocolFlags::from_bytes(&frame.data[2..]);
        self.complete_pip(Ok(Some(flags))).await;
    }

    // ── CDI download exchange (ADR-0018) ─────────────────────────────────

    /// Start a fresh CDI download exchange. Called from `handle_command`
    /// when no other exchange is active, and from `drain_parked_after_completion`
    /// to service a queued waiter.
    async fn start_cdi_exchange(
        &mut self,
        config: MemoryReadConfig,
        waiter: oneshot::Sender<CdiResult>,
    ) {
        let now_std = StdInstant::now();
        self.active = Some(ActiveExchange::CdiDownload {
            deadline: TokioInstant::now() + Duration::from_millis(config.timeout_ms),
            address_cursor: 0,
            chunk_retry_count: 0,
            lag_recovery_count: 0,
            assembler: DatagramAssembler::new(),
            assembled: Vec::new(),
            chunk_start: now_std,
            stats: CdiStats::default(),
            download_start: now_std,
            config,
            waiter,
        });
        // Send the first read request for the current cursor. Failures here
        // complete the exchange immediately with `PeerError::from(...)`.
        self.send_next_chunk_request().await;
    }

    /// Build and send the MemoryConfigRead datagram for the current
    /// `address_cursor`. Also resets the per-chunk deadline and timing
    /// baseline. If the transport rejects the send, the exchange completes
    /// with a translated `PeerError`.
    async fn send_next_chunk_request(&mut self) {
        // Take the config + values we need out of the borrow to avoid
        // holding `&mut self.active` across an async send.
        let (frames, timeout_ms) = match &self.active {
            Some(ActiveExchange::CdiDownload { config, address_cursor, chunk_retry_count: _, .. }) => {
                match MemoryConfigCmd::build_read(
                    self.our_alias,
                    self.alias,
                    AddressSpace::Cdi,
                    *address_cursor,
                    CDI_CHUNK_SIZE,
                ) {
                    Ok(f) => (f, config.timeout_ms),
                    Err(e) => {
                        // Complete with a Protocol error (do not emit
                        // TerminateDueToError — we never got on the wire).
                        self.complete_cdi(Err(PeerError::Protocol(format!(
                            "failed to build MemoryConfigRead: {}", e
                        )))).await;
                        return;
                    }
                }
            }
            _ => {
                return;
            }
        };

        // Refresh the deadline + chunk_start + reassembler before sending.
        // `chunk_retry_count` is intentionally NOT reset here — resets happen
        // at chunk-advance points (start of exchange or after a successful
        // reply). This lets the DR-with-resend-OK path call
        // `send_next_chunk_request` to retry without losing its counter.
        let now_tokio = TokioInstant::now();
        let now_std = StdInstant::now();
        if let Some(ActiveExchange::CdiDownload {
            deadline, chunk_start, assembler, ..
        }) = &mut self.active
        {
            *deadline = now_tokio + Duration::from_millis(timeout_ms);
            *chunk_start = now_std;
            *assembler = DatagramAssembler::new();
        }

        for frame in &frames {
            if let Err(e) = self.transport.send(frame).await {
                // Transport wedged mid-send: skip cleanup emission per D1.
                self.complete_cdi(Err(PeerError::from(e))).await;
                return;
            }
        }
    }

    /// Handle a frame that arrived while a CDI exchange is active.
    ///
    /// Called from `handle_inbound_frame` after alias filtering. Owns:
    ///
    /// - Reply-datagram reassembly + address advance / null-terminator
    ///   short-read termination.
    /// - DR-with-resend-OK retry up to `config.max_retries` per chunk.
    /// - OIR-terminal classification with wrapped-MTI + error-code decode.
    /// - Per-chunk timing capture and total-retry accumulation.
    async fn on_cdi_frame(&mut self, mti: MTI, frame: &GridConnectFrame) {
        // Terminal peer rejection (TN-9.7.3.2 §3.4): OIR ends the exchange
        // regardless of the current cursor. Payload = wrapped MTI (2 bytes)
        // + error code (2 bytes) + optional message.
        if mti == MTI::OptionalInteractionRejected {
            let (wrapped_mti, error_code) = if frame.data.len() >= 6 {
                let mti = ((frame.data[2] as u32) << 8) | frame.data[3] as u32;
                let code = ((frame.data[4] as u16) << 8) | frame.data[5] as u16;
                (mti, code)
            } else if frame.data.len() >= 4 {
                let mti = ((frame.data[2] as u32) << 8) | frame.data[3] as u32;
                (mti, 0)
            } else {
                (0u32, 0u16)
            };
            // Peer-cleanup contract: emit TerminateDueToError once.
            self.emit_terminate_due_to_error(error_code).await;
            self.complete_cdi(Err(PeerError::Rejected {
                mti: wrapped_mti,
                code: error_code,
            })).await;
            return;
        }

        // DatagramRejected handling.
        //
        // Per LCC S-9.7.3.2 §2.3, a resend-OK DR (bit 13 set, e.g. 0x2020
        // "buffer unavailable") means the peer will NOT process this request
        // and the client MUST resend. In practice, some peers (observed on
        // SPROG USB-LCC) opportunistically DR when their buffer is under
        // pressure but *still process the request later*, sending both the
        // DR and the reply for the same request. If we retry immediately on
        // DR, we send a duplicate that the peer must also queue — this
        // cascades: each cycle adds one more pending request to the peer's
        // buffer until it overflows completely.
        //
        // Policy: on resend-OK DR, log it and **do not** retry immediately.
        // Continue waiting for the reply. If the peer never sends the reply,
        // the exchange deadline will fire and `handle_deadline` will emit
        // `TerminateDueToError` and complete with `PeerError::Timeout` —
        // the same terminal path as any other slow-peer failure.
        //
        // On non-resend-OK DR (bit 13 clear), the peer is signalling
        // permanent rejection. Emit peer cleanup and complete immediately.
        if mti == MTI::DatagramRejected {
            let error_code = if frame.data.len() >= 4 {
                ((frame.data[2] as u16) << 8) | frame.data[3] as u16
            } else {
                0
            };
            let resend_ok = (error_code & 0x2000) != 0;

            match &self.active {
                Some(ActiveExchange::CdiDownload { .. }) => {}
                _ => return,
            }

            if resend_ok {
                // Fall through / return — do NOT resend. Continue waiting
                // for the peer's actual reply on the next select! iteration.
                return;
            } else {
                self.emit_terminate_due_to_error(error_code).await;
                self.complete_cdi(Err(PeerError::Rejected {
                    mti: MTI::DatagramRejected.value(),
                    code: error_code,
                })).await;
            }
            return;
        }

        // DatagramReceivedOk: honour the timeout-extension flags (low 4 bits
        // = 2^n seconds). Not a reply — just extends our patience.
        if mti == MTI::DatagramReceivedOk {
            let flags = if frame.data.len() >= 3 { frame.data[2] } else { 0 };
            let timeout_exp = flags & 0x0F;
            if timeout_exp > 0 {
                let extended_ms = (1u64 << timeout_exp) * 1000;
                if let Some(ActiveExchange::CdiDownload { deadline, .. }) = &mut self.active {
                    let new_deadline = TokioInstant::now() + Duration::from_millis(extended_ms);
                    if new_deadline > *deadline {
                        *deadline = new_deadline;
                    }
                }
            }
            return;
        }

        // Reply datagram frames (DatagramOnly/First/Middle/Final): route
        // through the reassembler; on complete-datagram we ACK, parse the
        // read reply, advance the cursor, and either send the next chunk or
        // finish.
        if !matches!(
            mti,
            MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal
        ) {
            return;
        }

        let complete_data = match &mut self.active {
            Some(ActiveExchange::CdiDownload { assembler, .. }) => {
                match assembler.handle_frame(frame) {
                    Ok(Some(data)) => data,
                    Ok(None) => return, // more frames expected
                    Err(_) => {
                        // Recoverable: a stray datagram frame (e.g. a
                        // residual `DatagramMiddle` after chunk-N's Final
                        // from SPROG USB-LCC) belongs to no active buffer.
                        // Reset the reassembler in place so the next
                        // legitimate `DatagramFirst` re-establishes state,
                        // and keep the exchange in-flight. Do NOT emit
                        // `TerminateDueToError` — the peer did nothing
                        // fatally wrong.
                        assembler.clear_source(self.alias);
                        return;
                    }
                }
            }
            _ => return,
        };

        // Send the ACK before parsing so the peer can release its exchange
        // state promptly. Ownership: this is the sole ACK owner per
        // ADR-0016 for CDI replies.
        if let Ok(ack) = DatagramAssembler::send_acknowledgment(self.our_alias, self.alias) {
            let _ = self.transport.send(&ack).await;
        }

        // Post-ACK pacing: give the serial gateway time to forward the
        // ACK on CAN before the next request arrives via USB. Required
        // for SPROG USB-LCC v1.4 firmware, which changed internal FTDI
        // buffer management and is sensitive to rapid back-to-back USB
        // OUT transactions. JMRI avoids the issue via natural Java
        // processing overhead (~8ms median between reply and ACK).
        // Harmless for adapters that don't need it (delay is only
        // applied during CDI downloads, not general traffic).
        let post_ack_delay_ms = match &self.active {
            Some(ActiveExchange::CdiDownload { config, .. }) => config.post_ack_delay_ms,
            _ => 0,
        };
        if post_ack_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(post_ack_delay_ms)).await;
        }

        // Parse the memory-config reply.
        let reply = match MemoryConfigCmd::parse_read_reply(&complete_data) {
            Ok(r) => r,
            Err(e) => {
                let err = PeerError::Protocol(format!("CDI reply parse failed: {}", e));
                self.emit_terminate_due_to_error(0x0200).await;
                self.complete_cdi(Err(err)).await;
                return;
            }
        };

        match reply {
            ReadReply::Success { address: reply_address, data: chunk, .. } => {
                // Reply-identity check: the peer's reply address must match
                // the request-in-flight cursor. SPROG USB-LCC has been
                // observed emitting replies for a stale address after a
                // retry; those must be ACKed (already done above) but
                // otherwise discarded so the exchange survives.
                let expected_cursor = match &self.active {
                    Some(ActiveExchange::CdiDownload { address_cursor, .. }) => *address_cursor,
                    _ => return,
                };
                if reply_address != expected_cursor {
                    return;
                }

                // Address-out-of-bounds / null-terminator detection.
                let (assembled_len, chunk_len, hit_null_pos) = {
                    let ActiveExchange::CdiDownload { assembled, .. } =
                        self.active.as_mut().expect("active is CdiDownload here")
                    else {
                        unreachable!("active variant checked above");
                    };
                    let null_pos = chunk.iter().position(|&b| b == 0x00);
                    match null_pos {
                        Some(pos) => {
                            assembled.extend_from_slice(&chunk[..pos]);
                        }
                        None => {
                            assembled.extend_from_slice(&chunk);
                        }
                    }
                    (assembled.len(), chunk.len(), null_pos)
                };

                if assembled_len > CDI_MAX_BYTES {
                    let err = PeerError::Protocol(format!(
                        "CDI exceeds {}MB size limit",
                        CDI_MAX_BYTES / (1024 * 1024)
                    ));
                    self.emit_terminate_due_to_error(0x0200).await;
                    self.complete_cdi(Err(err)).await;
                    return;
                }

                // Capture per-chunk timing.
                let chunk_ms = match &mut self.active {
                    Some(ActiveExchange::CdiDownload { chunk_start, stats, .. }) => {
                        let elapsed = chunk_start.elapsed().as_millis() as u32;
                        stats.chunks += 1;
                        stats.chunk_durations_ms.push(elapsed);
                        elapsed
                    }
                    _ => 0,
                };
                let _ = chunk_ms;

                // Terminate on null or empty chunk (short read).
                if hit_null_pos.is_some() || chunk_len == 0 {
                    self.complete_cdi(Ok(())).await;
                    return;
                }

                // Advance cursor and issue the next chunk request.
                if let Some(ActiveExchange::CdiDownload {
                    address_cursor, chunk_retry_count, ..
                }) = &mut self.active
                {
                    *address_cursor += chunk_len as u32;
                    *chunk_retry_count = 0;
                }
                self.send_next_chunk_request().await;
            }
            ReadReply::Failed { error_code, .. } => {
                // Peer reports read failure. Some codes (e.g. 0x1082 "address
                // out of bounds") are natural short-read terminators for CDI.
                // Match datagram_reader.rs: treat 0x1082 as clean termination.
                if error_code == 0x1082 {
                    self.complete_cdi(Ok(())).await;
                } else {
                    self.emit_terminate_due_to_error(error_code).await;
                    self.complete_cdi(Err(PeerError::Rejected {
                        mti: MTI::DatagramRejected.value(),
                        code: error_code,
                    })).await;
                }
            }
        }
    }

    /// Finalise a CDI exchange. On `Ok(())` the assembled bytes + stats
    /// stored in the `ActiveExchange::CdiDownload` variant are delivered to
    /// the waiter; on `Err` the error is delivered directly.
    async fn complete_cdi(&mut self, terminal: Result<(), PeerError>) {
        let Some(exchange) = self.active.take() else { return; };
        let ActiveExchange::CdiDownload {
            assembled,
            mut stats,
            waiter,
            download_start,
            ..
        } = exchange
        else {
            // Wrong variant — this should never happen because callers gate
            // on `active` being CdiDownload. Put it back and abort.
            self.active = Some(exchange);
            return;
        };

        match terminal {
            Ok(()) => {
                stats.total_bytes = assembled.len();
                stats.total_duration_ms = download_start.elapsed().as_millis() as u64;
                let _ = waiter.send(Ok(CdiCompletion {
                    bytes: assembled,
                    stats,
                }));
            }
            Err(err) => {
                let _ = waiter.send(Err(err));
            }
        }
        self.drain_parked_after_completion().await;
    }

    // ── Single-datagram memory read exchange (S4 D1=A) ───────────────────

    /// Start a fresh single-datagram memory read exchange.
    async fn start_read_memory_exchange(
        &mut self,
        space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
        waiter: oneshot::Sender<MemoryReadResult>,
    ) {
        let config = MemoryReadConfig {
            timeout_ms,
            ..MemoryReadConfig::default()
        };
        self.active = Some(ActiveExchange::MemoryRead {
            deadline: TokioInstant::now() + Duration::from_millis(timeout_ms),
            space,
            address,
            count,
            lag_recovery_count: 0,
            assembler: DatagramAssembler::new(),
            request_start: StdInstant::now(),
            first_frame_at: None,
            frame_times: Vec::new(),
            config,
            waiter,
        });
        self.send_read_request().await;
    }

    /// Build + send the read request for the active `MemoryRead`. Resets the
    /// deadline, request timing baseline, and reassembler (idempotent re-issue
    /// for lag recovery). Completes the exchange on a build/send failure.
    async fn send_read_request(&mut self) {
        let (frames, timeout_ms) = match &self.active {
            Some(ActiveExchange::MemoryRead { space, address, count, config, .. }) => {
                let addr_space = match AddressSpace::from_u8(*space) {
                    Ok(s) => s,
                    Err(e) => {
                        self.complete_memory_read(Err(PeerError::Protocol(format!(
                            "invalid address space 0x{:02X}: {}", space, e
                        )))).await;
                        return;
                    }
                };
                match MemoryConfigCmd::build_read(self.our_alias, self.alias, addr_space, *address, *count) {
                    Ok(f) => (f, config.timeout_ms),
                    Err(e) => {
                        self.complete_memory_read(Err(PeerError::Protocol(format!(
                            "failed to build MemoryConfigRead: {}", e
                        )))).await;
                        return;
                    }
                }
            }
            _ => return,
        };

        let now_tokio = TokioInstant::now();
        let now_std = StdInstant::now();
        if let Some(ActiveExchange::MemoryRead {
            deadline, request_start, assembler, first_frame_at, frame_times, ..
        }) = &mut self.active
        {
            *deadline = now_tokio + Duration::from_millis(timeout_ms);
            *request_start = now_std;
            *assembler = DatagramAssembler::new();
            *first_frame_at = None;
            frame_times.clear();
        }

        for frame in &frames {
            if let Err(e) = self.transport.send_direct(frame).await {
                self.complete_memory_read(Err(PeerError::from(e))).await;
                return;
            }
        }
    }

    /// Handle a frame while a single-datagram memory read is active. Mirrors
    /// `on_cdi_frame`'s single-chunk logic: OIR/DR terminal classification,
    /// timeout-extension, sole-ACK ownership, reply-identity guard, and
    /// per-frame timing capture.
    async fn on_memory_read_frame(&mut self, mti: MTI, frame: &GridConnectFrame) {
        if mti == MTI::OptionalInteractionRejected {
            let (wrapped_mti, error_code) = if frame.data.len() >= 6 {
                let mti = ((frame.data[2] as u32) << 8) | frame.data[3] as u32;
                let code = ((frame.data[4] as u16) << 8) | frame.data[5] as u16;
                (mti, code)
            } else if frame.data.len() >= 4 {
                let mti = ((frame.data[2] as u32) << 8) | frame.data[3] as u32;
                (mti, 0)
            } else {
                (0u32, 0u16)
            };
            self.emit_terminate_due_to_error(error_code).await;
            self.complete_memory_read(Err(PeerError::Rejected { mti: wrapped_mti, code: error_code })).await;
            return;
        }

        if mti == MTI::DatagramRejected {
            let error_code = if frame.data.len() >= 4 {
                ((frame.data[2] as u16) << 8) | frame.data[3] as u16
            } else {
                0
            };
            let resend_ok = (error_code & 0x2000) != 0;
            if !matches!(&self.active, Some(ActiveExchange::MemoryRead { .. })) {
                return;
            }
            if resend_ok {
                // Do NOT retry immediately — wait for the reply or deadline
                // (same cascade-avoidance policy as CDI).
                return;
            }
            self.emit_terminate_due_to_error(error_code).await;
            self.complete_memory_read(Err(PeerError::Rejected {
                mti: MTI::DatagramRejected.value(),
                code: error_code,
            })).await;
            return;
        }

        if mti == MTI::DatagramReceivedOk {
            let flags = if frame.data.len() >= 3 { frame.data[2] } else { 0 };
            let timeout_exp = flags & 0x0F;
            if timeout_exp > 0 {
                let extended_ms = (1u64 << timeout_exp) * 1000;
                if let Some(ActiveExchange::MemoryRead { deadline, .. }) = &mut self.active {
                    let new_deadline = TokioInstant::now() + Duration::from_millis(extended_ms);
                    if new_deadline > *deadline {
                        *deadline = new_deadline;
                    }
                }
            }
            return;
        }

        if !matches!(
            mti,
            MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal
        ) {
            return;
        }

        // Capture per-frame timing before reassembly.
        let now = StdInstant::now();
        if let Some(ActiveExchange::MemoryRead { first_frame_at, frame_times, .. }) = &mut self.active {
            if first_frame_at.is_none() {
                *first_frame_at = Some(now);
            }
            frame_times.push(now);
        }

        let complete_data = match &mut self.active {
            Some(ActiveExchange::MemoryRead { assembler, .. }) => {
                match assembler.handle_frame(frame) {
                    Ok(Some(data)) => data,
                    Ok(None) => return,
                    Err(_) => {
                        assembler.clear_source(self.alias);
                        return;
                    }
                }
            }
            _ => return,
        };

        // Sole ACK owner for this reply datagram.
        if let Ok(ack) = DatagramAssembler::send_acknowledgment(self.our_alias, self.alias) {
            let _ = self.transport.send_direct(&ack).await;
        }

        let reply = match MemoryConfigCmd::parse_read_reply(&complete_data) {
            Ok(r) => r,
            Err(e) => {
                let err = PeerError::Protocol(format!("read reply parse failed: {}", e));
                self.emit_terminate_due_to_error(0x0200).await;
                self.complete_memory_read(Err(err)).await;
                return;
            }
        };

        match reply {
            ReadReply::Success { address: reply_address, data, .. } => {
                let expected = match &self.active {
                    Some(ActiveExchange::MemoryRead { address, .. }) => *address,
                    _ => return,
                };
                if reply_address != expected {
                    // ACKed above; discard stale reply and keep waiting.
                    return;
                }
                let timing = match &self.active {
                    Some(ActiveExchange::MemoryRead { request_start, first_frame_at, frame_times, .. }) => {
                        let first_frame_latency_ms = first_frame_at
                            .map(|t| t.duration_since(*request_start).as_millis() as u64)
                            .unwrap_or(0);
                        let mut frame_gaps_ms = Vec::new();
                        for w in frame_times.windows(2) {
                            frame_gaps_ms.push(w[1].duration_since(w[0]).as_millis() as u32);
                        }
                        crate::discovery::MemoryReadTiming {
                            first_frame_latency_ms,
                            frame_gaps_ms,
                            total_duration_ms: request_start.elapsed().as_millis() as u64,
                            frame_count: frame_times.len() as u8,
                        }
                    }
                    _ => return,
                };
                self.complete_memory_read(Ok((data, timing))).await;
            }
            ReadReply::Failed { error_code, .. } => {
                self.emit_terminate_due_to_error(error_code).await;
                self.complete_memory_read(Err(PeerError::Rejected {
                    mti: MTI::DatagramRejected.value(),
                    code: error_code,
                })).await;
            }
        }
    }

    /// Finalise a memory read exchange: deliver the terminal result to the
    /// waiter and service the next parked op.
    async fn complete_memory_read(&mut self, result: MemoryReadResult) {
        let Some(exchange) = self.active.take() else { return; };
        let ActiveExchange::MemoryRead { waiter, .. } = exchange else {
            self.active = Some(exchange);
            return;
        };
        let _ = waiter.send(result);
        self.drain_parked_after_completion().await;
    }

    // ── Single-datagram memory write exchange (S4 D1=A) ──────────────────

    /// Start a fresh single-datagram memory write exchange.
    async fn start_write_memory_exchange(
        &mut self,
        space: u8,
        address: u32,
        data: Vec<u8>,
        timeout_ms: u64,
        waiter: oneshot::Sender<MemoryWriteResult>,
    ) {
        self.active = Some(ActiveExchange::MemoryWrite {
            deadline: TokioInstant::now() + Duration::from_millis(timeout_ms),
            space,
            address,
            data,
            retry_count: 0,
            lag_recovery_count: 0,
            timeout_ms,
            waiter,
        });
        self.send_write_request().await;
    }

    /// Build + send the write datagram for the active `MemoryWrite`. Refreshes
    /// the deadline (used for the initial send and resend-OK retries).
    async fn send_write_request(&mut self) {
        let (frames, timeout_ms) = match &self.active {
            Some(ActiveExchange::MemoryWrite { space, address, data, timeout_ms, .. }) => {
                let addr_space = match AddressSpace::from_u8(*space) {
                    Ok(s) => s,
                    Err(e) => {
                        self.complete_memory_write(Err(PeerError::Protocol(format!(
                            "invalid address space 0x{:02X}: {}", space, e
                        )))).await;
                        return;
                    }
                };
                match MemoryConfigCmd::build_write(self.our_alias, self.alias, addr_space, *address, data) {
                    Ok(f) => (f, *timeout_ms),
                    Err(e) => {
                        self.complete_memory_write(Err(PeerError::Protocol(format!(
                            "failed to build MemoryConfigWrite: {}", e
                        )))).await;
                        return;
                    }
                }
            }
            _ => return,
        };

        if let Some(ActiveExchange::MemoryWrite { deadline, .. }) = &mut self.active {
            *deadline = TokioInstant::now() + Duration::from_millis(timeout_ms);
        }

        for frame in &frames {
            if let Err(e) = self.transport.send(frame).await {
                self.complete_memory_write(Err(PeerError::from(e))).await;
                return;
            }
        }
    }

    /// Handle a frame while a single-datagram memory write is active.
    /// RequestWithNoReply: `DatagramReceivedOk` (without reply-pending) means
    /// the write applied. Resend-OK DR retries up to `WRITE_MEMORY_MAX_RETRIES`;
    /// non-resend DR / OIR are terminal; a reply-pending write reply carries
    /// the final result.
    async fn on_memory_write_frame(&mut self, mti: MTI, frame: &GridConnectFrame) {
        if mti == MTI::OptionalInteractionRejected {
            let error_code = if frame.data.len() >= 6 {
                ((frame.data[4] as u16) << 8) | frame.data[5] as u16
            } else {
                0
            };
            self.emit_terminate_due_to_error(error_code).await;
            self.complete_memory_write(Err(PeerError::Rejected {
                mti: MTI::OptionalInteractionRejected.value(),
                code: error_code,
            })).await;
            return;
        }

        if mti == MTI::DatagramRejected {
            let error_code = if frame.data.len() >= 4 {
                ((frame.data[2] as u16) << 8) | frame.data[3] as u16
            } else {
                0
            };
            let resend_ok = (error_code & 0x2000) != 0;
            let should_retry = match &mut self.active {
                Some(ActiveExchange::MemoryWrite { retry_count, .. }) => {
                    if resend_ok && *retry_count < crate::constants::WRITE_MEMORY_MAX_RETRIES {
                        *retry_count += 1;
                        true
                    } else {
                        false
                    }
                }
                _ => return,
            };
            if should_retry {
                self.send_write_request().await;
            } else {
                self.emit_terminate_due_to_error(error_code).await;
                self.complete_memory_write(Err(PeerError::Rejected {
                    mti: MTI::DatagramRejected.value(),
                    code: error_code,
                })).await;
            }
            return;
        }

        if mti == MTI::DatagramReceivedOk {
            let flags = if frame.data.len() >= 3 { frame.data[2] } else { 0 };
            let reply_pending = flags & 0x80 != 0;
            let timeout_exp = flags & 0x0F;
            if timeout_exp > 0 {
                let extended_ms = (1u64 << timeout_exp) * 1000;
                if let Some(ActiveExchange::MemoryWrite { deadline, .. }) = &mut self.active {
                    let new_deadline = TokioInstant::now() + Duration::from_millis(extended_ms);
                    if new_deadline > *deadline {
                        *deadline = new_deadline;
                    }
                }
            }
            if !reply_pending {
                self.complete_memory_write(Ok(())).await;
            }
            // reply_pending: keep waiting for the write-reply datagram below.
            return;
        }

        // Write-reply datagram (reply-pending path): command 0x20, reply byte;
        // error bit 0x08 set → failure. ACK then complete.
        if matches!(mti, MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal) {
            if frame.data.len() >= 2 && frame.data[0] == 0x20 {
                let reply_cmd = frame.data[1];
                if let Ok(ack) = DatagramAssembler::send_acknowledgment(self.our_alias, self.alias) {
                    let _ = self.transport.send(&ack).await;
                }
                if reply_cmd & 0x08 != 0 {
                    let error_code = if frame.data.len() >= 4 {
                        ((frame.data[2] as u16) << 8) | frame.data[3] as u16
                    } else {
                        0
                    };
                    self.complete_memory_write(Err(PeerError::Rejected {
                        mti: MTI::DatagramRejected.value(),
                        code: error_code,
                    })).await;
                } else {
                    self.complete_memory_write(Ok(())).await;
                }
            }
        }
    }

    /// Finalise a memory write exchange: deliver the terminal result and
    /// service the next parked op.
    async fn complete_memory_write(&mut self, result: MemoryWriteResult) {
        let Some(exchange) = self.active.take() else { return; };
        let ActiveExchange::MemoryWrite { waiter, .. } = exchange else {
            self.active = Some(exchange);
            return;
        };
        let _ = waiter.send(result);
        self.drain_parked_after_completion().await;
    }

    async fn handle_deadline(&mut self) {
        match self.active.take() {
            Some(ActiveExchange::SnipQuery { payload, received_first_frame, .. }) => {
                // Silence-terminated DatagramOnly assembly: parse whatever we
                // buffered.
                if received_first_frame && !payload.is_empty() {
                    if let Ok(data) = parse_snip_payload(&payload) {
                        self.snip_cache = Some(data.clone());
                        self.snip_status = SNIPStatus::Complete;
                        for w in self.snip_waiters.drain(..) {
                            let _ = w.send(Ok(Some(data.clone())));
                        }
                        self.drain_parked_after_completion().await;
                        return;
                    }
                }
                self.snip_status = SNIPStatus::Timeout;
                for w in self.snip_waiters.drain(..) { let _ = w.send(Ok(None)); }
                self.drain_parked_after_completion().await;
            }
            Some(ActiveExchange::PipQuery { .. }) => {
                self.pip_status = PIPStatus::Timeout;
                for w in self.pip_waiters.drain(..) { let _ = w.send(Ok(None)); }
                self.drain_parked_after_completion().await;
            }
            Some(ActiveExchange::CdiDownload { waiter, download_start, .. }) => {
                // Peer-cleanup contract (TN-9.7.2.1): emit exactly one
                // TerminateDueToError before releasing the exchange.
                self.emit_terminate_due_to_error(0x0200).await;
                let elapsed_ms = download_start.elapsed().as_millis() as u64;
                let _ = waiter.send(Err(PeerError::Timeout {
                    operation: "download_cdi".into(),
                    elapsed_ms,
                }));
                self.drain_parked_after_completion().await;
            }
            Some(ActiveExchange::MemoryRead { waiter, request_start, .. }) => {
                self.emit_terminate_due_to_error(0x0200).await;
                let elapsed_ms = request_start.elapsed().as_millis() as u64;
                let _ = waiter.send(Err(PeerError::Timeout {
                    operation: "read_memory".into(),
                    elapsed_ms,
                }));
                self.drain_parked_after_completion().await;
            }
            Some(ActiveExchange::MemoryWrite { waiter, .. }) => {
                self.emit_terminate_due_to_error(0x0200).await;
                let _ = waiter.send(Err(PeerError::Timeout {
                    operation: "write_memory".into(),
                    elapsed_ms: 0,
                }));
                self.drain_parked_after_completion().await;
            }
            None => {}
        }
    }

    async fn complete_snip(&mut self, result: SnipResult) {
        self.active = None;
        match &result {
            Ok(Some(data)) => {
                self.snip_cache = Some(data.clone());
                self.snip_status = SNIPStatus::Complete;
            }
            Ok(None) => {
                if matches!(self.snip_status, SNIPStatus::InProgress) {
                    self.snip_status = SNIPStatus::Timeout;
                }
            }
            Err(_) => {
                self.snip_status = SNIPStatus::Error;
            }
        }
        for w in self.snip_waiters.drain(..) {
            let _ = w.send(result.clone());
        }
        self.drain_parked_after_completion().await;
    }
    async fn complete_pip(&mut self, result: PipResult) {
        self.active = None;
        match &result {
            Ok(Some(flags)) => {
                self.pip_cache = Some(*flags);
                self.pip_status = PIPStatus::Complete;
            }
            Ok(None) => {
                if matches!(self.pip_status, PIPStatus::InProgress) {
                    self.pip_status = PIPStatus::Timeout;
                }
            }
            Err(_) => {
                self.pip_status = PIPStatus::Error;
            }
        }
        for w in self.pip_waiters.drain(..) {
            let _ = w.send(result.clone());
        }
        self.drain_parked_after_completion().await;
    }

    /// Peer-cleanup contract for on-our-fault failures. Emits
    /// `TerminateDueToError` addressed to the peer with the given error code
    /// (TN-9.7.2.1). Fire-and-forget: if the transport can't accept the
    /// frame, the error surfaces to the caller via the exchange's Err arm.
    ///
    /// `error_code` is a 16-bit value written into the frame data as
    /// big-endian bytes after the destination alias.
    async fn emit_terminate_due_to_error(&self, error_code: u16) {
        let header = match MTI::TerminateDueToError.to_header(self.our_alias) {
            Ok(h) => h,
            Err(_) => return,
        };
        let data = vec![
            (((self.alias >> 8) & 0x0F) as u8),
            (self.alias & 0xFF) as u8,
            ((error_code >> 8) & 0xFF) as u8,
            (error_code & 0xFF) as u8,
        ];
        let frame = GridConnectFrame { header, data };
        let _ = self.transport.send(&frame).await;
    }

    async fn abort_active(&mut self, err: PeerError) {
        match self.active.take() {
            Some(ActiveExchange::SnipQuery { .. }) => {
                self.snip_status = SNIPStatus::Error;
                for w in self.snip_waiters.drain(..) {
                    let _ = w.send(Err(err.clone()));
                }
            }
            Some(ActiveExchange::PipQuery { .. }) => {
                self.pip_status = PIPStatus::Error;
                for w in self.pip_waiters.drain(..) {
                    let _ = w.send(Err(err.clone()));
                }
            }
            Some(ActiveExchange::CdiDownload { waiter, .. }) => {
                // Peer-cleanup emission is gated on fault nature via the
                // single-source-of-truth classifier (ADR-0018 §Peer-cleanup
                // contract; S7 T2). Our-fault-live-wire faults (cancel,
                // terminal rejection, lag-recovery-exhaustion `Protocol`)
                // emit exactly one `TerminateDueToError`. Wire-dead faults
                // (`TransportUnhealthy`/Wedged) and peer-initiated events
                // (`PeerReinitialised`, `AliasChanged`) skip emission — the
                // peer has either lost the wire or already released its
                // exchange state.
                if err.is_our_fault_live_wire() {
                    self.emit_terminate_due_to_error(0x0200).await;
                }
                let _ = waiter.send(Err(err.clone()));
            }
            Some(ActiveExchange::MemoryRead { waiter, .. }) => {
                if err.is_our_fault_live_wire() {
                    self.emit_terminate_due_to_error(0x0200).await;
                }
                let _ = waiter.send(Err(err.clone()));
            }
            Some(ActiveExchange::MemoryWrite { waiter, .. }) => {
                if err.is_our_fault_live_wire() {
                    self.emit_terminate_due_to_error(0x0200).await;
                }
                let _ = waiter.send(Err(err.clone()));
            }
            None => {}
        }
    }

    /// After the active exchange completes, kick off the next parked query
    /// type if it has waiters and no cached answer yet.
    async fn drain_parked_after_completion(&mut self) {
        if self.active.is_some() { return; }
        // Queued CDIs take priority over parked SNIP/PIP so a caller-issued
        // download does not starve behind a background SNIP retry.
        //
        // The recursion path complete_cdi → drain_parked → start_cdi →
        // send_next_chunk_request → complete_cdi (on early failure) forms an
        // async recursion cycle; Box::pin breaks the cycle so the compiler
        // can size the future.
        if let Some((config, waiter)) = self.cdi_pending.pop_front() {
            Box::pin(self.start_cdi_exchange(config, waiter)).await;
            return;
        }
        // Then queued single-datagram memory ops, strict FIFO across mixed
        // read/write so concurrent callers on one handle serialise in issue
        // order. Box::pin breaks the async recursion cycle (complete_* →
        // drain → start_* → send_* → complete_* on early failure).
        if let Some(op) = self.mem_pending.pop_front() {
            match op {
                PendingMemOp::Read { space, address, count, timeout_ms, reply } => {
                    Box::pin(self.start_read_memory_exchange(space, address, count, timeout_ms, reply)).await;
                }
                PendingMemOp::Write { space, address, data, timeout_ms, reply } => {
                    Box::pin(self.start_write_memory_exchange(space, address, data, timeout_ms, reply)).await;
                }
            }
            return;
        }
        // SNIP first if both parked.
        if !self.snip_waiters.is_empty()
            && !matches!(self.snip_status, SNIPStatus::Complete | SNIPStatus::Timeout)
        {
            let transport = self.transport.clone();
            let our_alias = self.our_alias;
            let alias = self.alias;
            match GridConnectFrame::from_addressed_mti(MTI::SNIPRequest, our_alias, alias, vec![]) {
                Ok(f) => {
                    tokio::spawn(async move { let _ = transport.send(&f).await; });
                    self.snip_status = SNIPStatus::InProgress;
                    self.active = Some(ActiveExchange::SnipQuery {
                        payload: Vec::new(),
                        receiving: false,
                        received_first_frame: false,
                        deadline: TokioInstant::now() + SNIP_TIMEOUT,
                    });
                    return;
                }
                Err(e) => {
                    let err = PeerError::Protocol(e.to_string());
                    for w in self.snip_waiters.drain(..) { let _ = w.send(Err(err.clone())); }
                }
            }
        }
        if !self.pip_waiters.is_empty()
            && !matches!(self.pip_status, PIPStatus::Complete | PIPStatus::Timeout)
        {
            let transport = self.transport.clone();
            let our_alias = self.our_alias;
            let alias = self.alias;
            match GridConnectFrame::from_addressed_mti(
                MTI::ProtocolSupportInquiry,
                our_alias,
                alias,
                vec![],
            ) {
                Ok(f) => {
                    tokio::spawn(async move { let _ = transport.send(&f).await; });
                    self.pip_status = PIPStatus::InProgress;
                    self.active = Some(ActiveExchange::PipQuery {
                        deadline: TokioInstant::now() + PIP_TIMEOUT,
                    });
                }
                Err(e) => {
                    let err = PeerError::Protocol(e.to_string());
                    for w in self.pip_waiters.drain(..) { let _ = w.send(Err(err.clone())); }
                }
            }
        }
    }
}

// Fields kept only for future extension (S3+) — suppress dead_code today.
#[allow(dead_code)]
impl PeerSession {
    fn _keep_alive(&self) -> (NodeID, Option<&str>) {
        (self.node_id, self.last_known_wedge_reason.as_deref())
    }
}

#[cfg(test)]
mod classifier_tests {
    use super::PeerError;

    #[test]
    fn our_fault_live_wire_covers_live_wire_faults() {
        // Faults on our side while the wire is still live MUST emit peer
        // cleanup (TerminateDueToError). `Protocol` covers lag-recovery
        // exhaustion (S7 D2=C); `Timeout` reaches this classifier only
        // conceptually — CDI timeout is emitted in `handle_deadline`.
        assert!(PeerError::Cancelled { reason: "x".into() }.is_our_fault_live_wire());
        assert!(PeerError::Rejected { mti: 0x1D28, code: 0x1000 }.is_our_fault_live_wire());
        assert!(PeerError::Protocol("inbound lag recovery exhausted".into()).is_our_fault_live_wire());
        assert!(PeerError::Timeout { operation: "download_cdi".into(), elapsed_ms: 10 }.is_our_fault_live_wire());
    }

    #[test]
    fn our_fault_live_wire_excludes_wire_dead_and_peer_initiated() {
        // Wire-dead (Wedged) and peer-initiated events (reinit, alias change)
        // MUST NOT emit cleanup — the peer either lost the wire or already
        // released its exchange state.
        assert!(!PeerError::TransportUnhealthy { reason: "wedged".into() }.is_our_fault_live_wire());
        assert!(!PeerError::PeerReinitialised.is_our_fault_live_wire());
        assert!(!PeerError::AliasChanged { old: 0x3AE, new: 0x4C1 }.is_our_fault_live_wire());
        assert!(!PeerError::NotSupported { operation: "x".into() }.is_our_fault_live_wire());
        assert!(!PeerError::NotConnected.is_our_fault_live_wire());
    }
}
