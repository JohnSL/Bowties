//! Event router for LCC message broadcasting to frontend

use lcc_rs::{MessageDispatcher, ReceivedMessage, MTI};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use crate::traffic::DecodedMessage;

/// Event payloads sent to the frontend

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDiscoveredEvent {
    pub node_id: String,
    pub alias: u16,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageReceivedEvent {
    pub frame: String,
    pub header: Option<u32>,
    pub data_bytes: Option<Vec<u8>>,
    pub mti: Option<String>,
    /// Human-readable display label (e.g. "Datagram First") — display only, not for protocol logic
    pub mti_label: Option<String>,
    pub source_alias: Option<u16>,
    pub timestamp: String,
    /// Direction: "S" for sent, "R" for received
    pub direction: Option<String>,
    /// User-friendly summary for non-technical mode
    pub decoded_payload: Option<String>,
    /// Protocol-level details for advanced troubleshooting
    pub technical_details: Option<String>,
    /// Node ID if this is a VerifiedNode message
    pub node_id: Option<String>,
    /// Destination alias for addressed messages
    pub dest_alias: Option<u16>,
}

/// Event router that subscribes to dispatcher and emits Tauri events
pub struct EventRouter {
    app: AppHandle,
    dispatcher: Arc<Mutex<MessageDispatcher>>,
    router_task: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    our_alias: u16,
}

impl EventRouter {
    /// Create a new event router
    pub fn new(app: AppHandle, dispatcher: Arc<Mutex<MessageDispatcher>>, our_alias: u16) -> Self {
        Self {
            app,
            dispatcher,
            router_task: None,
            shutdown_tx: None,
            our_alias,
        }
    }

    /// Start the event router
    pub fn start(&mut self) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        
        let app = self.app.clone();
        let dispatcher = self.dispatcher.clone();
        let our_alias = self.our_alias;
        
        let handle = tokio::spawn(async move {
            Self::router_loop(app, dispatcher, our_alias, shutdown_rx).await;
        });
        
        self.router_task = Some(handle);
        self.shutdown_tx = Some(shutdown_tx);
    }

    /// Main router loop
    async fn router_loop(
        app: AppHandle,
        dispatcher: Arc<Mutex<MessageDispatcher>>,
        our_alias: u16,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        eprintln!("[EventRouter] Starting router loop with our_alias=0x{:03X}", our_alias);
        
        // Subscribe to all messages
        let mut all_rx = {
            let disp = dispatcher.lock().await;
            disp.subscribe_all()
        };

        // Subscribe to specific MTIs for node discovery
        let mut verified_node_rx = {
            let disp = dispatcher.lock().await;
            disp.subscribe_mti(MTI::VerifiedNode).await
        };

        eprintln!("[EventRouter] Subscribed to message channels");

        loop {
            tokio::select! {
                // Check for shutdown
                _ = &mut shutdown_rx => {
                    eprintln!("[EventRouter] Shutdown signal received");
                    break;
                }
                
                // Handle all messages for monitor
                Ok(msg) = all_rx.recv() => {
                    Self::handle_all_messages(&app, msg, our_alias);
                }
                
                // Handle node discovery
                Ok(msg) = verified_node_rx.recv() => {
                    Self::handle_node_discovered(&app, msg);
                }
            }
        }
    }

    /// Handle all message events (for monitor window)
    fn handle_all_messages(app: &AppHandle, msg: ReceivedMessage, our_alias: u16) {
        // Decode the message with full parsing
        let decoded = DecodedMessage::decode(&msg.frame, our_alias);
        
        let event = MessageReceivedEvent {
            frame: msg.frame.to_string(),
            header: Some(msg.frame.header),
            data_bytes: Some(msg.frame.data.clone()),
            mti: Some(decoded.mti_name.clone()),
            mti_label: Some(decoded.mti_label.clone()),
            source_alias: Some(decoded.source_alias),
            timestamp: decoded.timestamp,
            direction: Some(decoded.direction),
            decoded_payload: Some(decoded.decoded_payload),
            technical_details: Some(decoded.technical_details),
            node_id: decoded.node_id,
            dest_alias: decoded.dest_alias,
        };

        // Emit to frontend
        let _ = app.emit("lcc-message-received", event);
    }

    /// Handle node discovered events
    fn handle_node_discovered(app: &AppHandle, msg: ReceivedMessage) {
        // Parse VerifiedNode response
        if msg.frame.data.len() == 6 {
            if let Ok((_, alias)) = msg.frame.get_mti() {
                // Node ID is in the data
                let node_id_bytes: [u8; 6] = msg.frame.data.as_slice().try_into().unwrap_or([0; 6]);
                let node_id = format!(
                    "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
                    node_id_bytes[0],
                    node_id_bytes[1],
                    node_id_bytes[2],
                    node_id_bytes[3],
                    node_id_bytes[4],
                    node_id_bytes[5]
                );

                let event = NodeDiscoveredEvent {
                    node_id,
                    alias,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };

                // Emit to frontend
                let _ = app.emit("lcc-node-discovered", event);
            }
        }
    }

    /// Stop the event router
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        if let Some(handle) = self.router_task.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for EventRouter {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}
