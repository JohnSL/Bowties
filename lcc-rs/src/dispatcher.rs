//! Message dispatcher for persistent LCC message monitoring
//!
//! The dispatcher runs a background task that continuously reads frames from the transport
//! and broadcasts them to multiple subscribers via channels.
//!
//! Architecture: A `TapTransport` wrapper intercepts EVERY send() and receive() call on the
//! underlying transport, regardless of which code path made the call (dispatcher, snip module,
//! memory config, etc.). This ensures all bidirectional traffic appears in the broadcast channel.

use crate::protocol::{GridConnectFrame, MTI};
use crate::transport::LccTransport;
use crate::transport_actor::TransportHandle;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use std::collections::HashMap;

/// A transparent transport wrapper that taps every send/receive and broadcasts to a channel.
/// This ensures ALL traffic is captured regardless of which code path calls the transport.
struct TapTransport {
    inner: Box<dyn LccTransport>,
    tx: broadcast::Sender<ReceivedMessage>,
}

#[async_trait::async_trait]
impl LccTransport for TapTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> crate::Result<()> {
        let result = self.inner.send(frame).await;
        if result.is_ok() {
            let _ = self.tx.send(ReceivedMessage {
                frame: frame.clone(),
                timestamp: std::time::Instant::now(),
            });
        }
        result
    }

    async fn receive(&mut self, timeout_ms: u64) -> crate::Result<Option<GridConnectFrame>> {
        let result = self.inner.receive(timeout_ms).await;
        if let Ok(Some(ref frame)) = result {
            let _ = self.tx.send(ReceivedMessage {
                frame: frame.clone(),
                timestamp: std::time::Instant::now(),
            });
        }
        result
    }

    async fn close(&mut self) -> crate::Result<()> {
        self.inner.close().await
    }
}

/// Channel capacity for broadcast channels
const CHANNEL_CAPACITY: usize = 256;

/// A no-op transport used as a placeholder when the dispatcher is backed by
/// a `TransportHandle` (the actor owns the real transport).
struct NullTransport;

#[async_trait::async_trait]
impl LccTransport for NullTransport {
    async fn send(&mut self, _frame: &GridConnectFrame) -> crate::Result<()> {
        Err(crate::Error::Transport("NullTransport: use TransportHandle".to_string()))
    }
    async fn receive(&mut self, _timeout_ms: u64) -> crate::Result<Option<GridConnectFrame>> {
        Ok(None)
    }
    async fn close(&mut self) -> crate::Result<()> {
        Ok(())
    }
}

/// Filters for subscribing to specific message types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MessageFilter {
    /// All messages
    All,
    /// Messages with a specific MTI
    Mti(MTI),
    /// Messages from a specific source alias
    SourceAlias(u16),
    /// Messages to a specific destination alias
    DestAlias(u16),
}

/// A message received from the LCC network with metadata
///
/// Re-exported from `transport_actor` for backward compatibility.
pub use crate::transport_actor::ReceivedMessage;

/// Message dispatcher that runs a persistent listener and broadcasts frames
pub struct MessageDispatcher {
    /// Broadcast sender for all messages
    all_tx: broadcast::Sender<ReceivedMessage>,
    /// Map of MTI-specific broadcast senders
    mti_senders: Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>>,
    /// Handle to the background listener task
    listener_handle: Option<JoinHandle<()>>,
    /// Shutdown signal
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Transport reference for sending
    transport: Arc<Mutex<Box<dyn LccTransport>>>,
    /// D11: Alias-to-NodeID map maintained from AMD/AMR frames
    alias_map: Arc<RwLock<HashMap<u16, [u8; 6]>>>,
    /// When backed by a TransportActor, delegate send/subscribe through the handle.
    handle: Option<TransportHandle>,
}

impl MessageDispatcher {
    /// Create a new message dispatcher with a persistent listener
    ///
    /// The dispatcher takes ownership of the transport and starts a background
    /// task that continuously reads frames and broadcasts them to subscribers.
    pub fn new(transport: Box<dyn LccTransport>) -> Self {
        let (all_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let mti_senders = Arc::new(RwLock::new(HashMap::new()));
        
        // Wrap transport in TapTransport so ALL sends/receives are broadcast,
        // regardless of which code path (dispatcher, snip, memory config) makes the call.
        let tap = TapTransport { inner: transport, tx: all_tx.clone() };
        let transport = Arc::new(Mutex::new(Box::new(tap) as Box<dyn LccTransport>));
        
        Self {
            all_tx,
            mti_senders,
            listener_handle: None,
            shutdown_tx: None,
            transport,
            alias_map: Arc::new(RwLock::new(HashMap::new())),
            handle: None,
        }
    }

    /// Create a dispatcher facade backed by a `TransportHandle`.
    ///
    /// The `TransportActor` owns the real transport; this dispatcher delegates
    /// `subscribe_*`, `send`, and alias-map queries to the handle / actor.
    /// `start()` is a no-op on a handle-backed dispatcher; the actor's reader
    /// loop is already running.
    pub fn from_handle(handle: TransportHandle) -> Self {
        // Create a dummy TapTransport-less transport (never used for I/O).
        // subscribe/send go through the handle.
        let all_tx = handle.all_tx_clone();
        let mti_senders = handle.mti_senders_clone();

        Self {
            all_tx,
            mti_senders,
            listener_handle: None,
            shutdown_tx: None,
            transport: Arc::new(Mutex::new(Box::new(NullTransport) as Box<dyn LccTransport>)),
            alias_map: Arc::new(RwLock::new(HashMap::new())),
            handle: Some(handle),
        }
    }

    /// Start the background listener task
    ///
    /// No-op when this dispatcher is backed by a `TransportHandle` — the actor's
    /// reader loop is already running.
    pub fn start(&mut self) {
        if self.handle.is_some() {
            return;
        }

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        
        let mti_senders = self.mti_senders.clone();
        let transport = self.transport.clone();
        let alias_map = self.alias_map.clone();
        
        // Spawn background task
        let handle = tokio::spawn(async move {
            Self::listener_loop(transport, mti_senders, alias_map, shutdown_rx).await;
        });
        
        self.listener_handle = Some(handle);
        self.shutdown_tx = Some(shutdown_tx);
    }

    /// Background listener loop
    /// 
    /// Drives transport.receive() so frames are read from the network.
    /// TapTransport handles broadcasting all frames to all_tx automatically.
    /// This loop only needs to route frames to MTI-specific subscribers.
    async fn listener_loop(
        transport: Arc<Mutex<Box<dyn LccTransport>>>,
        mti_senders: Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>>,
        alias_map: Arc<RwLock<HashMap<u16, [u8; 6]>>>,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        loop {
            // Check for shutdown signal (Ok = explicit send, Closed = sender dropped)
            match shutdown_rx.try_recv() {
                Ok(()) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => break,
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
            }

            // Drive receive - TapTransport broadcasts to all_tx automatically.
            // Keep the poll timeout SHORT (1ms) so the transport mutex is released
            // frequently enough that send() callers (e.g. read_memory) are not blocked
            // for the full poll duration.
            let frame_result = {
                let mut transport = transport.lock().await;
                transport.receive(1).await // 1ms timeout — release lock frequently
            };

            match frame_result {
                Ok(Some(frame)) => {
                    // TapTransport already broadcast to all_tx.
                    // Route to MTI-specific subscribers here.
                    if let Ok((mti, alias)) = frame.get_mti() {
                        // D11: Maintain alias map from AMD/AMR frames
                        match mti {
                            MTI::AliasMapDefinition => {
                                if frame.data.len() >= 6 {
                                    let mut node_id = [0u8; 6];
                                    node_id.copy_from_slice(&frame.data[0..6]);
                                    let mut map = alias_map.write().await;
                                    map.insert(alias, node_id);
                                }
                            }
                            MTI::AliasMapReset => {
                                let mut map = alias_map.write().await;
                                map.remove(&alias);
                            }
                            _ => {}
                        }

                        let senders = mti_senders.read().await;
                        if let Some(tx) = senders.get(&mti) {
                            eprintln!(
                                "[Dispatcher] routing {:?} frame (alias=0x{:03X}, data_len={}) to MTI channel",
                                mti, alias, frame.data.len()
                            );
                            let msg = ReceivedMessage {
                                frame,
                                timestamp: std::time::Instant::now(),
                            };
                            let _ = tx.send(msg);
                        }
                    }
                }
                Ok(None) => {
                    // Timeout, no frame this iteration
                }
                Err(e) => {
                    // Connection error - log and break
                    eprintln!("Dispatcher: Connection error: {}", e);
                    break;
                }
            }

            // Yield between iterations so other tasks (senders) can acquire the
            // transport lock.  Without this, in single-threaded Tokio runtimes
            // the listener re-locks immediately, starving send() callers.
            // In multi-threaded runtimes this is essentially a no-op.
            tokio::task::yield_now().await;
        }
    }

    /// Subscribe to all messages
    pub fn subscribe_all(&self) -> broadcast::Receiver<ReceivedMessage> {
        self.all_tx.subscribe()
    }

    /// Subscribe to messages with a specific MTI
    pub async fn subscribe_mti(&self, mti: MTI) -> broadcast::Receiver<ReceivedMessage> {
        let mut senders = self.mti_senders.write().await;
        
        let tx = senders.entry(mti).or_insert_with(|| {
            let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
            tx
        });
        
        tx.subscribe()
    }

    /// Send a frame to the LCC network
    /// When backed by a TransportHandle, delegates to the actor.
    /// Otherwise, TapTransport automatically broadcasts the frame to all_tx on send.
    pub async fn send(&self, frame: &GridConnectFrame) -> Result<(), crate::Error> {
        if let Some(ref handle) = self.handle {
            return handle.send(frame).await;
        }
        let mut transport = self.transport.lock().await;
        transport.send(frame).await
    }

    /// Get a reference to the transport for direct access
    ///
    /// This should be used sparingly - prefer using send() and subscribe methods
    pub fn transport(&self) -> Arc<Mutex<Box<dyn LccTransport>>> {
        self.transport.clone()
    }

    /// Create a `TransportHandle` backed by this dispatcher's channels and transport.
    ///
    /// This is a Phase 2/3 bridge: the handle's `send()` goes through an mpsc channel
    /// to a spawned task that forwards to the transport via the mutex, and `subscribe_all()`
    /// / `subscribe_mti()` use the existing broadcast channels.
    pub fn transport_handle(&self) -> TransportHandle {
        let transport = self.transport.clone();
        let (tx, mut rx) = mpsc::channel::<GridConnectFrame>(64);
        let all_tx = self.all_tx.clone();

        // Spawn a bridge task that forwards outbound frames to the transport.
        tokio::spawn(async move {
            while let Some(frame) = rx.recv().await {
                let mut t = transport.lock().await;
                if let Err(e) = t.send(&frame).await {
                    eprintln!("[TransportHandle bridge] send error: {}", e);
                    break;
                }
            }
        });

        TransportHandle::from_parts(tx, all_tx, self.mti_senders.clone())
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

    /// Stop the background listener and cleanup
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        if let Some(handle) = self.listener_handle.take() {
            let _ = handle.await;
        }

        // Send a graceful TCP FIN so the remote hub sees EOF (readLine → null)
        // rather than RST (SocketException: Connection reset).  The listener
        // task has fully exited above, so there is no contention on the lock.
        let mut transport = self.transport.lock().await;
        let _ = transport.close().await;
    }

    /// Check if the listener is running
    pub fn is_running(&self) -> bool {
        if self.handle.is_some() {
            return true; // Actor manages its own lifetime
        }
        self.listener_handle.as_ref().map_or(false, |h| !h.is_finished())
    }
}

impl Drop for MessageDispatcher {
    fn drop(&mut self) {
        // Send shutdown signal if still active
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
    async fn test_dispatcher_broadcasts_all_messages() {
        let mut transport = MockTransport::new();
        transport.add_receive_frame(":X195B4001N;".to_string());
        
        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();
        
        let mut rx = dispatcher.subscribe_all();
        
        // Give listener time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Should receive the frame
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.frame.to_string(), ":X195B4001N;");
        
        dispatcher.shutdown().await;
    }

    #[tokio::test]
    async fn test_dispatcher_mti_filtering() {
        let mut transport = MockTransport::new();
        transport.add_receive_frame(":X19490001N;".to_string()); // VerifyNodeGlobal
        transport.add_receive_frame(":X19170001N010203040506;".to_string()); // VerifiedNode
        
        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();
        
        let mut rx = dispatcher.subscribe_mti(MTI::VerifiedNode).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Should only receive VerifiedNode, not VerifyNodeGlobal
        let msg = rx.recv().await.unwrap();
        let (mti, _) = msg.frame.get_mti().unwrap();
        assert_eq!(mti, MTI::VerifiedNode);
        
        dispatcher.shutdown().await;
    }

    // --- D11: Alias map maintenance tests ---

    #[tokio::test]
    async fn test_alias_map_tracks_amd() {
        // AMD from alias 0x123 carries NodeID 01.02.03.04.05.06
        let amd_frame = GridConnectFrame::from_mti(
            MTI::AliasMapDefinition,
            0x123,
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ).unwrap();

        let mut transport = MockTransport::new();
        transport.add_receive_frame(amd_frame.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let node_id = dispatcher.lookup_alias(0x123).await;
        assert_eq!(node_id, Some([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]));

        // Unknown alias should return None
        assert_eq!(dispatcher.lookup_alias(0x999).await, None);

        dispatcher.shutdown().await;
    }

    #[tokio::test]
    async fn test_alias_map_removes_on_amr() {
        // First AMD, then AMR from the same alias
        let amd_frame = GridConnectFrame::from_mti(
            MTI::AliasMapDefinition,
            0x456,
            vec![0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        ).unwrap();
        let amr_frame = GridConnectFrame::from_mti(
            MTI::AliasMapReset,
            0x456,
            vec![0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        ).unwrap();

        let mut transport = MockTransport::new();
        transport.add_receive_frame(amd_frame.to_string());
        transport.add_receive_frame(amr_frame.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

        // After AMR, the alias should be gone
        assert_eq!(dispatcher.lookup_alias(0x456).await, None);

        dispatcher.shutdown().await;
    }

    #[tokio::test]
    async fn test_alias_map_snapshot() {
        let amd1 = GridConnectFrame::from_mti(
            MTI::AliasMapDefinition, 0x111, vec![1, 2, 3, 4, 5, 6],
        ).unwrap();
        let amd2 = GridConnectFrame::from_mti(
            MTI::AliasMapDefinition, 0x222, vec![7, 8, 9, 10, 11, 12],
        ).unwrap();

        let mut transport = MockTransport::new();
        transport.add_receive_frame(amd1.to_string());
        transport.add_receive_frame(amd2.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        let snap = dispatcher.alias_map_snapshot().await;
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[&0x111], [1, 2, 3, 4, 5, 6]);
        assert_eq!(snap[&0x222], [7, 8, 9, 10, 11, 12]);

        dispatcher.shutdown().await;
    }
}
