//! Event router for LCC message broadcasting to frontend

use lcc_rs::{MessageDispatcher, ReceivedMessage, MTI};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};

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
    pub mti: Option<String>,
    pub source_alias: Option<u16>,
    pub timestamp: String,
}

/// Event router that subscribes to dispatcher and emits Tauri events
pub struct EventRouter {
    app: AppHandle,
    dispatcher: Arc<Mutex<MessageDispatcher>>,
    router_task: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EventRouter {
    /// Create a new event router
    pub fn new(app: AppHandle, dispatcher: Arc<Mutex<MessageDispatcher>>) -> Self {
        Self {
            app,
            dispatcher,
            router_task: None,
            shutdown_tx: None,
        }
    }

    /// Start the event router
    pub fn start(&mut self) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        
        let app = self.app.clone();
        let dispatcher = self.dispatcher.clone();
        
        let handle = tokio::spawn(async move {
            Self::router_loop(app, dispatcher, shutdown_rx).await;
        });
        
        self.router_task = Some(handle);
        self.shutdown_tx = Some(shutdown_tx);
    }

    /// Main router loop
    async fn router_loop(
        app: AppHandle,
        dispatcher: Arc<Mutex<MessageDispatcher>>,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
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

        loop {
            tokio::select! {
                // Check for shutdown
                _ = &mut shutdown_rx => {
                    break;
                }
                
                // Handle all messages for monitor
                Ok(msg) = all_rx.recv() => {
                    Self::handle_all_messages(&app, msg);
                }
                
                // Handle node discovery
                Ok(msg) = verified_node_rx.recv() => {
                    Self::handle_node_discovered(&app, msg);
                }
            }
        }
    }

    /// Handle all message events (for monitor window)
    fn handle_all_messages(app: &AppHandle, msg: ReceivedMessage) {
        let mti_str = msg.frame.get_mti()
            .ok()
            .map(|(mti, _)| format!("{:?}", mti));
        
        let source_alias = msg.frame.get_mti()
            .ok()
            .map(|(_, alias)| alias);

        let event = MessageReceivedEvent {
            frame: msg.frame.to_string(),
            mti: mti_str,
            source_alias,
            timestamp: chrono::Utc::now().to_rfc3339(),
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
