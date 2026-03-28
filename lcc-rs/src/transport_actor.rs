//! Transport actor — sole owner of the underlying LCC transport.
//!
//! All network I/O goes through channels; no mutex around the transport.
//!
//! Architecture:
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  transport_actor (single tokio task, owns transport)    │
//! │                                                         │
//! │   recv loop ──► broadcast::Sender<ReceivedMessage>      │
//! │                                                         │
//! │   mpsc::Receiver<OutboundFrame> ──► transport.send()    │
//! └─────────────────────────────────────────────────────────┘
//!           ▲                        │
//!           │ subscribe()            │ send(frame)
//!           │                        ▼
//!    broadcast::Receiver      mpsc::Sender<OutboundFrame>
//! ```

use crate::protocol::{GridConnectFrame, MTI};
use crate::transport::{LccTransport, TransportReader, TransportWriter};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;

/// Channel capacity for the broadcast (inbound) channel.
const BROADCAST_CAPACITY: usize = 2048;

/// Channel capacity for the outbound mpsc channel.
const OUTBOUND_CAPACITY: usize = 64;

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
}

impl TransportHandle {
    /// Send a frame to the LCC network.
    ///
    /// Returns immediately once the frame is queued; the actor task performs
    /// the actual I/O.
    pub async fn send(&self, frame: &GridConnectFrame) -> Result<()> {
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
        }
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

        let handle = TransportHandle {
            tx: outbound_tx,
            all_tx: all_tx.clone(),
            mti_senders: mti_senders.clone(),
        };

        let alias_map = Arc::new(RwLock::new(HashMap::new()));

        let (reader, writer) = transport.into_halves();

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
            tokio::spawn(Self::writer_loop(writer, outbound_rx, all_tx.clone()))
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
    async fn writer_loop(
        mut writer: Box<dyn TransportWriter>,
        mut rx: mpsc::Receiver<GridConnectFrame>,
        all_tx: broadcast::Sender<ReceivedMessage>,
    ) {
        while let Some(frame) = rx.recv().await {
            if let Err(e) = writer.send(&frame).await {
                eprintln!("TransportActor writer: send error: {}", e);
                break;
            }
            // Echo sent frames to the broadcast channel so the traffic monitor sees them.
            let _ = all_tx.send(ReceivedMessage {
                frame,
                timestamp: std::time::Instant::now(),
            });
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
}
