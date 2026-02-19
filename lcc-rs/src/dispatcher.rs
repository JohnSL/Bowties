//! Message dispatcher for persistent LCC message monitoring
//!
//! The dispatcher runs a background task that continuously reads frames from the transport
//! and broadcasts them to multiple subscribers via channels.

use crate::protocol::{GridConnectFrame, MTI};
use crate::transport::LccTransport;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;
use std::collections::HashMap;

/// Channel capacity for broadcast channels
const CHANNEL_CAPACITY: usize = 256;

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
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    /// The GridConnect frame
    pub frame: GridConnectFrame,
    /// Timestamp when received
    pub timestamp: std::time::Instant,
}

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
}

impl MessageDispatcher {
    /// Create a new message dispatcher with a persistent listener
    ///
    /// The dispatcher takes ownership of the transport and starts a background
    /// task that continuously reads frames and broadcasts them to subscribers.
    pub fn new(transport: Box<dyn LccTransport>) -> Self {
        let (all_tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        let mti_senders = Arc::new(RwLock::new(HashMap::new()));
        let transport = Arc::new(Mutex::new(transport));
        
        Self {
            all_tx,
            mti_senders,
            listener_handle: None,
            shutdown_tx: None,
            transport,
        }
    }

    /// Start the background listener task
    pub fn start(&mut self) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        
        let all_tx = self.all_tx.clone();
        let mti_senders = self.mti_senders.clone();
        let transport = self.transport.clone();
        
        // Spawn background task
        let handle = tokio::spawn(async move {
            Self::listener_loop(transport, all_tx, mti_senders, shutdown_rx).await;
        });
        
        self.listener_handle = Some(handle);
        self.shutdown_tx = Some(shutdown_tx);
    }

    /// Background listener loop
    async fn listener_loop(
        transport: Arc<Mutex<Box<dyn LccTransport>>>,
        all_tx: broadcast::Sender<ReceivedMessage>,
        mti_senders: Arc<RwLock<HashMap<MTI, broadcast::Sender<ReceivedMessage>>>>,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        loop {
            // Check for shutdown signal
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            // Try to receive a frame with a short timeout
            let frame_result = {
                let mut transport = transport.lock().await;
                transport.receive(100).await // 100ms timeout
            };

            match frame_result {
                Ok(Some(frame)) => {
                    let msg = ReceivedMessage {
                        frame: frame.clone(),
                        timestamp: std::time::Instant::now(),
                    };

                    // Broadcast to all subscribers
                    let _ = all_tx.send(msg.clone());

                    // Broadcast to MTI-specific subscribers
                    if let Ok((mti, _)) = frame.get_mti() {
                        let senders = mti_senders.read().await;
                        if let Some(tx) = senders.get(&mti) {
                            let _ = tx.send(msg);
                        }
                    }
                }
                Ok(None) => {
                    // Timeout, continue
                    continue;
                }
                Err(e) => {
                    // Connection error - log and break
                    eprintln!("Dispatcher: Connection error: {}", e);
                    break;
                }
            }
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
    pub async fn send(&self, frame: &GridConnectFrame) -> Result<(), crate::Error> {
        let mut transport = self.transport.lock().await;
        transport.send(frame).await
    }

    /// Get a reference to the transport for direct access
    ///
    /// This should be used sparingly - prefer using send() and subscribe methods
    pub fn transport(&self) -> Arc<Mutex<Box<dyn LccTransport>>> {
        self.transport.clone()
    }

    /// Stop the background listener and cleanup
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        if let Some(handle) = self.listener_handle.take() {
            let _ = handle.await;
        }
    }

    /// Check if the listener is running
    pub fn is_running(&self) -> bool {
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
}
