//! Transport actor — sole owner of the underlying LCC transport.
//!
//! Outbound writes use a dual path:
//! - Normal path: callers enqueue frames on an `mpsc` channel; the writer task
//!   drains it and writes to the serial port.  Low overhead for background traffic.
//! - Direct path (`send_direct`): callers lock `Arc<Mutex<WriteHalf>>` and write
//!   inline, bypassing the mpsc queue and writer-task wakeup.  Used by
//!   `BatchReader` to eliminate the ~13 ms Windows tokio scheduler hop between
//!   receiving a datagram reply and writing the ACK / next request.
//!
//! Architecture:
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  transport_actor (two tokio tasks, own the transport)   │
//! │                                                         │
//! │   reader task ──► broadcast::Sender<ReceivedMessage>    │
//! │                                                         │
//! │   mpsc::Receiver<OutboundFrame>  ┐                      │
//! │                                  ├─► Arc<Mutex<Writer>> │
//! │   send_direct (caller inline)  ──┘                      │
//! └─────────────────────────────────────────────────────────┘
//!           ▲                        │
//!           │ subscribe()            │ send() / send_direct()
//!           │                        ▼
//!    broadcast::Receiver      mpsc::Sender / Arc<Mutex<Writer>>
//! ```

use crate::protocol::{GridConnectFrame, MTI};
use crate::transport::{LccTransport, TransportReader, TransportWriter};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, watch, RwLock};
use tokio::task::JoinHandle;

/// Channel capacity for the broadcast (inbound) channel.
const BROADCAST_CAPACITY: usize = 2048;

/// Channel capacity for the outbound mpsc channel.
const OUTBOUND_CAPACITY: usize = 64;

/// Per-frame send timeout for serial (SPROG USB-LCC and similar TTY-backed)
/// transports. Exceeding this publishes `TransportHealth::Wedged` on the
/// health watch and drops the current frame instead of blocking the writer
/// loop indefinitely.
pub const SERIAL_SEND_TIMEOUT: Duration = Duration::from_millis(500);

/// Per-frame send timeout for TCP transports. TCP path is more forgiving than
/// serial (kernel-level buffering, socket-level retry), so the wedge threshold
/// is 4× higher than serial.
pub const TCP_SEND_TIMEOUT: Duration = Duration::from_millis(2000);

/// Health of the outbound transport writer, published on a `watch` channel by
/// the writer task. `PeerSession` (S2+) and the UI connection-status surface
/// subscribe; `TransportHandle::send()` short-circuits when the observed
/// state is `Wedged`.
///
/// `Degraded` is reserved for future high-latency-but-not-wedged conditions
/// and is not emitted in the S1 implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportHealth {
    /// Recent sends completed within their per-transport timeout.
    Healthy,
    /// Reserved for future: high-latency-but-not-wedged conditions.
    /// Not currently emitted.
    Degraded { reason: String },
    /// A recent send exceeded the transport's `send_timeout()`. The writer
    /// dropped that frame and continues draining; the underlying transport
    /// may or may not recover.
    Wedged { reason: String },
}

/// A message received from (or sent to) the LCC network with metadata.
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    /// The GridConnect frame
    pub frame: GridConnectFrame,
    /// Timestamp when received
    pub timestamp: std::time::Instant,
}

/// Cheap-to-clone handle for sending frames and subscribing to inbound traffic.
///
/// All fields are `Arc`-backed so cloning is just reference-count bumps.
#[derive(Clone)]
pub struct TransportHandle {
    /// Send a frame to the network (goes to the actor's outbound queue).
    tx: mpsc::Sender<GridConnectFrame>,
    /// Broadcast sender — kept as `Sender` so callers can call `.subscribe()`.
    all_tx: broadcast::Sender<ReceivedMessage>,
    /// MTI-specific broadcast senders for efficient filtering.
    mti_senders: Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>>,
    /// Direct access to the serial write half, shared with the writer task.
    /// `None` when constructed via `from_parts` (legacy bridge path).
    /// When `Some`, `send_direct` writes inline without going through the mpsc
    /// queue, eliminating the writer-task-wakeup latency (~13 ms on Windows).
    direct_writer: Option<Arc<tokio::sync::Mutex<Box<dyn TransportWriter>>>>,
    /// Observability counter incremented on every `send_direct` call.
    /// Used by tests and diagnostics to confirm the direct-write path is
    /// actually being taken by latency-sensitive code (e.g. CDI download).
    direct_write_count: Arc<AtomicUsize>,
    /// Transport-health watch sender. `None` when constructed via `from_parts`
    /// (legacy bridge path — the bridge does not publish health).
    /// Cloning the handle shares the sender; publishing goes through the
    /// writer task, subscription via `subscribe_health()`.
    health_tx: Option<watch::Sender<TransportHealth>>,
}

impl TransportHandle {
    /// Send a frame to the LCC network.
    ///
    /// Returns immediately once the frame is queued; the actor task performs
    /// the actual I/O. Short-circuits with `Error::TransportUnhealthy` when
    /// the observed transport health is `Wedged` — no frame is enqueued.
    pub async fn send(&self, frame: &GridConnectFrame) -> Result<()> {
        if let Some(ref health_tx) = self.health_tx {
            if let TransportHealth::Wedged { ref reason } = *health_tx.borrow() {
                return Err(Error::TransportUnhealthy(reason.clone()));
            }
        }
        self.tx.send(frame.clone()).await.map_err(|_| {
            Error::Transport("Transport actor shut down".to_string())
        })
    }

    /// Subscribe to **all** inbound (and echoed outbound) frames.
    pub fn subscribe_all(&self) -> broadcast::Receiver<ReceivedMessage> {
        self.all_tx.subscribe()
    }

    /// Subscribe to frames matching a specific MTI.
    ///
    /// The actor routes frames to per-MTI channels; if no channel exists yet
    /// for the requested MTI one is created on the fly.
    pub async fn subscribe_mti(&self, mti: MTI) -> broadcast::Receiver<ReceivedMessage> {
        let mut senders = self.mti_senders.write().await;
        let tx = senders.entry(mti).or_insert_with(|| {
            let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
            tx
        });
        tx.subscribe()
    }

    /// Send a frame directly to the serial writer, bypassing the mpsc queue.
    ///
    /// When a `direct_writer` is available (i.e. the handle was created by
    /// `TransportActor::new`), this locks the writer mutex and writes inline,
    /// eliminating the writer-task-wakeup latency of the mpsc path.  Falls
    /// back to the mpsc path when no direct writer is present.
    ///
    /// The inline send participates in the transport-health seam via the
    /// shared `send_frame_with_timeout` helper: a stall here publishes
    /// `TransportHealth::Wedged` and returns `Error::TransportUnhealthy`.
    pub async fn send_direct(&self, frame: &GridConnectFrame) -> Result<()> {
        self.direct_write_count.fetch_add(1, Ordering::Relaxed);
        if let Some(ref writer_lock) = self.direct_writer {
            {
                let mut writer = writer_lock.lock().await;
                send_frame_with_timeout(&mut **writer, frame, self.health_tx.as_ref()).await?;
            }
            // Echo to the broadcast channel so the traffic monitor sees it.
            let _ = self.all_tx.send(ReceivedMessage {
                frame: frame.clone(),
                timestamp: std::time::Instant::now(),
            });
            Ok(())
        } else {
            self.send(frame).await
        }
    }

    /// Construct a `TransportHandle` from pre-existing channel components.
    pub fn from_parts(
        tx: mpsc::Sender<GridConnectFrame>,
        all_tx: broadcast::Sender<ReceivedMessage>,
        mti_senders: Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>>,
    ) -> Self {
        Self {
            tx,
            all_tx,
            mti_senders,
            direct_writer: None,
            direct_write_count: Arc::new(AtomicUsize::new(0)),
            health_tx: None,
        }
    }

    /// Number of times `send_direct` has been invoked on this handle (or any
    /// clone). Backed by an `Arc<AtomicUsize>`, so clones share the counter.
    pub fn direct_write_count(&self) -> usize {
        self.direct_write_count.load(Ordering::Relaxed)
    }

    /// Subscribe to transport-health transitions. Returns a `watch::Receiver`
    /// whose current value is the latest published `TransportHealth`. Callers
    /// typically use `rx.borrow()` to read the current state and
    /// `rx.changed().await` to await the next transition.
    ///
    /// Returns `None` for handles constructed via `from_parts` (the legacy
    /// bridge path does not publish health).
    pub fn subscribe_health(&self) -> Option<watch::Receiver<TransportHealth>> {
        self.health_tx.as_ref().map(|tx| tx.subscribe())
    }

    /// Clone of the broadcast sender (for bridging to legacy `MessageDispatcher`).
    pub fn all_tx_clone(&self) -> broadcast::Sender<ReceivedMessage> {
        self.all_tx.clone()
    }

    /// Clone of the MTI senders map (for bridging to legacy `MessageDispatcher`).
    pub fn mti_senders_clone(&self) -> Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>> {
        self.mti_senders.clone()
    }
}

/// Manages the actor task lifetime.
pub struct TransportActor {
    /// The handle callers use to interact with the actor.
    handle: TransportHandle,
    /// Handle to the background reader task.
    reader_handle: Option<JoinHandle<()>>,
    /// Handle to the background writer task.
    writer_handle: Option<JoinHandle<()>>,
    /// Shutdown signal for the reader task (drop to stop).
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// D11: Alias-to-NodeID map maintained from AMD/AMR frames.
    alias_map: Arc<RwLock<HashMap<u16, [u8; 6]>>>,
}

impl TransportActor {
    /// Create a new actor that takes exclusive ownership of the transport.
    ///
    /// Splits the transport into independent read/write halves and spawns
    /// a reader task and a writer task. No mutex is needed.
    pub fn new(transport: Box<dyn LccTransport>) -> Self {
        let (all_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (outbound_tx, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        let mti_senders: Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let alias_map = Arc::new(RwLock::new(HashMap::new()));

        let (reader, writer) = transport.into_halves();
        let direct_writer = Arc::new(tokio::sync::Mutex::new(writer));

        let (health_tx, _health_rx) = watch::channel(TransportHealth::Healthy);

        let handle = TransportHandle {
            tx: outbound_tx,
            all_tx: all_tx.clone(),
            mti_senders: mti_senders.clone(),
            direct_writer: Some(direct_writer.clone()),
            direct_write_count: Arc::new(AtomicUsize::new(0)),
            health_tx: Some(health_tx.clone()),
        };

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let reader_handle = {
            let all_tx = all_tx.clone();
            let mti_senders = mti_senders.clone();
            let alias_map = alias_map.clone();
            tokio::spawn(Self::reader_loop(
                reader,
                all_tx,
                mti_senders,
                alias_map,
                shutdown_rx,
            ))
        };

        let writer_handle = {
            tokio::spawn(Self::writer_loop(
                direct_writer,
                outbound_rx,
                all_tx.clone(),
                health_tx,
            ))
        };

        Self {
            handle,
            reader_handle: Some(reader_handle),
            writer_handle: Some(writer_handle),
            shutdown_tx: Some(shutdown_tx),
            alias_map,
        }
    }

    /// Get a cheap clone of the transport handle.
    pub fn handle(&self) -> TransportHandle {
        self.handle.clone()
    }

    /// D11: Look up the NodeID associated with an alias.
    pub async fn lookup_alias(&self, alias: u16) -> Option<[u8; 6]> {
        let map = self.alias_map.read().await;
        map.get(&alias).copied()
    }

    /// D11: Get a snapshot of the current alias map.
    pub async fn alias_map_snapshot(&self) -> HashMap<u16, [u8; 6]> {
        let map = self.alias_map.read().await;
        map.clone()
    }

    /// Reader loop: blocks on `reader.receive()` and broadcasts inbound frames.
    /// Uses `tokio::select!` with a shutdown signal for graceful termination.
    async fn reader_loop(
        mut reader: Box<dyn TransportReader>,
        all_tx: broadcast::Sender<ReceivedMessage>,
        mti_senders: Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>>,
        alias_map: Arc<RwLock<HashMap<u16, [u8; 6]>>>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                result = reader.receive() => {
                    match result {
                        Ok(frame) => {
                            let msg = ReceivedMessage {
                                frame: frame.clone(),
                                timestamp: std::time::Instant::now(),
                            };

                            // Broadcast to all subscribers.
                            let _ = all_tx.send(msg.clone());

                            // Route to MTI-specific channels + maintain alias map.
                            if let Ok((mti, alias)) = frame.get_mti() {
                                // D11: Maintain alias map from AMD/AMR frames.
                                match mti {
                                    MTI::AliasMapDefinition => {
                                        if frame.data.len() >= 6 {
                                            let mut node_id = [0u8; 6];
                                            node_id.copy_from_slice(&frame.data[0..6]);
                                            alias_map.write().await.insert(alias, node_id);
                                        }
                                    }
                                    MTI::AliasMapReset => {
                                        alias_map.write().await.remove(&alias);
                                    }
                                    _ => {}
                                }

                                let senders = mti_senders.read().await;
                                if let Some(tx) = senders.get(&mti) {
                                    let _ = tx.send(msg);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("TransportActor reader: connection error: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Writer loop: drains the outbound mpsc channel and sends frames via the writer.
    /// Also broadcasts sent frames to the all_tx channel so subscribers see both directions.
    /// The writer is behind an `Arc<Mutex>` so `send_direct` callers can also write
    /// inline without going through this loop.
    ///
    /// Every send is wrapped in the writer's per-transport `send_timeout()`
    /// via `send_frame_with_timeout`; a timeout publishes `TransportHealth::Wedged`
    /// on the shared watch channel and continues draining rather than blocking
    /// on the failed frame.
    async fn writer_loop(
        writer: Arc<tokio::sync::Mutex<Box<dyn TransportWriter>>>,
        mut rx: mpsc::Receiver<GridConnectFrame>,
        all_tx: broadcast::Sender<ReceivedMessage>,
        health_tx: watch::Sender<TransportHealth>,
    ) {
        while let Some(frame) = rx.recv().await {
            let send_result = {
                let mut w = writer.lock().await;
                send_frame_with_timeout(&mut **w, &frame, Some(&health_tx)).await
            };
            match send_result {
                Ok(()) => {
                    // Echo sent frames to the broadcast channel so the traffic monitor sees them.
                    let _ = all_tx.send(ReceivedMessage {
                        frame,
                        timestamp: std::time::Instant::now(),
                    });
                }
                Err(Error::TransportUnhealthy(_)) => {
                    // Wedged: current frame was dropped; the health seam has been
                    // updated. Continue draining — the underlying transport may
                    // recover on a subsequent frame.
                }
                Err(e) => {
                    eprintln!("TransportActor writer: send error: {}", e);
                    break;
                }
            }
        }
    }

    /// Graceful shutdown: stops both reader and writer tasks and closes the transport.
    pub async fn shutdown(&mut self) {
        // Signal reader to stop.
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(h) = self.reader_handle.take() {
            let _ = h.await;
        }

        // The writer loop waits on mpsc::recv() which won't return None while
        // any TransportHandle clone holds a sender.  Abort it explicitly.
        if let Some(h) = self.writer_handle.take() {
            h.abort();
            let _ = h.await;
        }
    }

    /// Check if the actor tasks are still running.
    pub fn is_running(&self) -> bool {
        self.reader_handle
            .as_ref()
            .map_or(false, |h| !h.is_finished())
    }
}

impl Drop for TransportActor {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Send a frame through the given writer bounded by the writer's own
/// `send_timeout()`. Publishes `TransportHealth::Wedged` on timeout and
/// `TransportHealth::Healthy` after a subsequent success (dedup handled by
/// the `watch` channel's `send_if_modified` semantics).
///
/// Shared by `writer_loop` (mpsc drain) and `TransportHandle::send_direct`
/// (D2 — every writer-holding path participates in the health seam). This
/// helper survives the retirement of `send_direct` in S5.
///
/// On timeout, returns `Error::TransportUnhealthy(reason)` so the caller can
/// distinguish a wedge from a "hard" transport error. The current frame is
/// dropped; the caller decides whether to keep draining.
async fn send_frame_with_timeout(
    writer: &mut dyn TransportWriter,
    frame: &GridConnectFrame,
    health_tx: Option<&watch::Sender<TransportHealth>>,
) -> Result<()> {
    let timeout_dur = writer.send_timeout();
    match tokio::time::timeout(timeout_dur, writer.send(frame)).await {
        Ok(Ok(())) => {
            if let Some(tx) = health_tx {
                tx.send_if_modified(|state| {
                    if matches!(state, TransportHealth::Healthy) {
                        false
                    } else {
                        *state = TransportHealth::Healthy;
                        true
                    }
                });
            }
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(_elapsed) => {
            let reason = format!(
                "send timeout after {}ms: {}",
                timeout_dur.as_millis(),
                frame.to_string()
            );
            if let Some(tx) = health_tx {
                let new_reason = reason.clone();
                tx.send_if_modified(|state| match state {
                    TransportHealth::Wedged { reason: existing } if existing == &new_reason => false,
                    _ => {
                        *state = TransportHealth::Wedged { reason: new_reason };
                        true
                    }
                });
            }
            Err(Error::TransportUnhealthy(reason))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::GridConnectFrame;
    use crate::transport::mock::MockTransport;

    #[tokio::test]
    async fn test_actor_broadcasts_received_frames() {
        let mut transport = MockTransport::new();
        transport.add_receive_frame(":X195B4001N;".to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let mut rx = handle.subscribe_all();

        // Give the reader loop time to process.
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.frame.to_string(), ":X195B4001N;");

        actor.shutdown().await;
    }

    #[tokio::test]
    async fn test_actor_mti_filtering() {
        let mut transport = MockTransport::new();
        transport.add_receive_frame(":X19490001N;".to_string()); // VerifyNodeGlobal
        transport.add_receive_frame(":X19170001N010203040506;".to_string()); // VerifiedNode

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let mut rx = handle.subscribe_mti(MTI::VerifiedNode).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let msg = rx.recv().await.unwrap();
        let (mti, _) = msg.frame.get_mti().unwrap();
        assert_eq!(mti, MTI::VerifiedNode);

        actor.shutdown().await;
    }

    #[tokio::test]
    async fn test_actor_send_and_echo() {
        let transport = MockTransport::new();
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let mut rx = handle.subscribe_all();

        let frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            0xAAA,
            vec![],
        )
        .unwrap();
        handle.send(&frame).await.unwrap();

        // The writer loop echoes sent frames to the broadcast channel.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.frame.to_string(), frame.to_string());

        actor.shutdown().await;
    }

    #[tokio::test]
    async fn test_actor_alias_map_tracks_amd() {
        let amd_frame = GridConnectFrame::from_mti(
            MTI::AliasMapDefinition,
            0x123,
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        )
        .unwrap();

        let mut transport = MockTransport::new();
        transport.add_receive_frame(amd_frame.to_string());

        let mut actor = TransportActor::new(Box::new(transport));

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let node_id = actor.lookup_alias(0x123).await;
        assert_eq!(node_id, Some([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]));

        assert_eq!(actor.lookup_alias(0x999).await, None);

        actor.shutdown().await;
    }

    #[tokio::test]
    async fn test_actor_alias_map_removes_on_amr() {
        let amd_frame = GridConnectFrame::from_mti(
            MTI::AliasMapDefinition,
            0x456,
            vec![0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        )
        .unwrap();
        let amr_frame = GridConnectFrame::from_mti(
            MTI::AliasMapReset,
            0x456,
            vec![0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        )
        .unwrap();

        let mut transport = MockTransport::new();
        transport.add_receive_frame(amd_frame.to_string());
        transport.add_receive_frame(amr_frame.to_string());

        let mut actor = TransportActor::new(Box::new(transport));

        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

        assert_eq!(actor.lookup_alias(0x456).await, None);

        actor.shutdown().await;
    }

    // ---------- Transport health seam (S1) ----------

    /// Slack over SEND_TIMEOUT before we call the wedge test failed.
    const WEDGE_SLACK: tokio::time::Duration = tokio::time::Duration::from_millis(300);

    #[tokio::test]
    async fn writer_bounded_send_emits_wedged_on_stall() {
        // Behavior: a writer that never returns from send() must cause the
        // writer task to publish TransportHealth::Wedged on the watch channel
        // within SERIAL_SEND_TIMEOUT + slack, without deadlocking other
        // subscribers (subscribe_all still works).
        let transport = MockTransport::new();
        let stall = transport.stall_handle();
        stall.store(true, Ordering::Relaxed);

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let mut health_rx = handle.subscribe_health().expect("health seam wired");
        assert_eq!(*health_rx.borrow(), TransportHealth::Healthy);

        // Second subscriber to prove no deadlock on the seam.
        let _health_rx2 = handle.subscribe_health().expect("health seam wired");
        let _all_rx = handle.subscribe_all();

        let frame =
            GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0xAAA, vec![]).unwrap();
        handle.send(&frame).await.expect("send() must enqueue");

        let deadline = SERIAL_SEND_TIMEOUT + WEDGE_SLACK;
        tokio::time::timeout(deadline, health_rx.changed())
            .await
            .expect("no health transition within timeout+slack")
            .expect("health watch closed unexpectedly");

        match &*health_rx.borrow_and_update() {
            TransportHealth::Wedged { reason } => {
                assert!(
                    reason.contains("timeout"),
                    "expected timeout reason, got {reason}"
                );
            }
            other => panic!("expected Wedged, got {:?}", other),
        }

        actor.shutdown().await;
    }

    #[tokio::test]
    async fn writer_returns_to_healthy_after_stall_clears() {
        // Behavior: after a stall publishes Wedged, clearing the stall and
        // pushing another frame must publish Healthy exactly once
        // (send_if_modified dedups repeats).
        let transport = MockTransport::new();
        let stall = transport.stall_handle();
        stall.store(true, Ordering::Relaxed);

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut health_rx = handle.subscribe_health().expect("health seam wired");

        let frame =
            GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0xAAA, vec![]).unwrap();
        handle.send(&frame).await.expect("send() must enqueue");

        // Wait for Wedged.
        tokio::time::timeout(SERIAL_SEND_TIMEOUT + WEDGE_SLACK, health_rx.changed())
            .await
            .expect("no Wedged transition")
            .unwrap();
        assert!(matches!(
            *health_rx.borrow_and_update(),
            TransportHealth::Wedged { .. }
        ));

        // Clear the stall — the next drained frame should succeed and heal.
        stall.store(false, Ordering::Relaxed);
        // send() short-circuits while Wedged, so use the mpsc path via a
        // fresh handle clone that reads its own health borrow — first heal
        // requires the writer task to drain another frame. Enqueue via the
        // low-level channel by cloning the sender's underlying tx: since
        // TransportHandle::send() short-circuits on Wedged, drive the writer
        // by sending via the raw mpsc sender captured before we started.
        // Instead: bypass the short-circuit by directly pushing to the mpsc
        // through a second, health-less handle constructed from parts.
        // Simpler path: wait for the writer_loop to naturally re-poll — but
        // there's no queued frame. Push through the direct writer via
        // send_direct, which uses the shared helper and will now succeed.
        handle
            .send_direct(&frame)
            .await
            .expect("send_direct must succeed once stall cleared");

        tokio::time::timeout(WEDGE_SLACK, health_rx.changed())
            .await
            .expect("no Healthy transition after stall cleared")
            .unwrap();
        assert_eq!(*health_rx.borrow_and_update(), TransportHealth::Healthy);

        actor.shutdown().await;
    }

    #[tokio::test]
    async fn send_short_circuits_with_transport_unhealthy_when_wedged() {
        // Behavior: once health is Wedged, TransportHandle::send() must
        // return Err(Error::TransportUnhealthy(_)) without enqueuing.
        let transport = MockTransport::new();
        let stall = transport.stall_handle();
        stall.store(true, Ordering::Relaxed);

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut health_rx = handle.subscribe_health().expect("health seam wired");

        let frame =
            GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0xAAA, vec![]).unwrap();
        handle.send(&frame).await.expect("first send() enqueues");

        // Wait for Wedged.
        tokio::time::timeout(SERIAL_SEND_TIMEOUT + WEDGE_SLACK, health_rx.changed())
            .await
            .expect("no Wedged transition")
            .unwrap();

        // Subsequent send() must fail fast without waiting on mpsc capacity.
        let start = tokio::time::Instant::now();
        let result = handle.send(&frame).await;
        let elapsed = start.elapsed();
        match result {
            Err(Error::TransportUnhealthy(reason)) => {
                assert!(reason.contains("timeout"));
            }
            other => panic!("expected TransportUnhealthy, got {:?}", other),
        }
        assert!(
            elapsed < tokio::time::Duration::from_millis(50),
            "send() did not fail fast: took {:?}",
            elapsed
        );

        actor.shutdown().await;
    }

    #[tokio::test]
    async fn send_direct_wedges_and_returns_transport_unhealthy_on_stall() {
        // Behavior (D2): send_direct participates in the transport-health
        // seam via the shared send_frame_with_timeout helper. A stall on the
        // inline direct path must publish Wedged and return TransportUnhealthy.
        let transport = MockTransport::new();
        let stall = transport.stall_handle();
        stall.store(true, Ordering::Relaxed);

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut health_rx = handle.subscribe_health().expect("health seam wired");

        let frame =
            GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0xAAA, vec![]).unwrap();

        let result = handle.send_direct(&frame).await;
        match result {
            Err(Error::TransportUnhealthy(reason)) => {
                assert!(reason.contains("timeout"));
            }
            other => panic!("expected TransportUnhealthy from send_direct, got {:?}", other),
        }

        assert!(matches!(
            *health_rx.borrow_and_update(),
            TransportHealth::Wedged { .. }
        ));

        actor.shutdown().await;
    }
}
