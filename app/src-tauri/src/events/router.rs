//! Event router for LCC message broadcasting to frontend

use lcc_rs::{TransportHandle, ReceivedMessage, MTI};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use crate::node_key::NodeKey;
use crate::node_registry::NodeRegistry;
use crate::traffic::DecodedMessage;
use crate::diagnostics::{DiagStats, FrameRing, NodeDiscoveryStat, FRAME_RING_CAPACITY, FrameEntry};

/// Event payloads sent to the frontend

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDiscoveredEvent {
    pub node_id: NodeKey,
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

/// Payload for lcc-event-state events (PCER)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventStateEvent {
    pub event_id: String,
    pub timestamp: String,
}

/// Event router that subscribes to transport handle and emits Tauri events
pub struct EventRouter {
    app: AppHandle,
    /// Transport handle for direct channel access
    handle: Option<TransportHandle>,
    router_task: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    our_alias: u16,
    registry: Arc<NodeRegistry>,
    diag_stats: DiagStats,
    frame_ring: FrameRing,
}

impl EventRouter {
    /// Create a new event router backed by a TransportHandle
    pub fn from_handle(app: AppHandle, handle: TransportHandle, our_alias: u16, registry: Arc<NodeRegistry>, diag_stats: DiagStats, frame_ring: FrameRing) -> Self {
        Self {
            app,
            handle: Some(handle),
            router_task: None,
            shutdown_tx: None,
            our_alias,
            registry,
            diag_stats,
            frame_ring,
        }
    }

    /// Start the event router.
    ///
    /// Subscriptions are set up **synchronously** (before `tokio::spawn`) so that by the
    /// time this function returns all broadcast receivers are in place.  This eliminates
    /// the race where `probe_nodes` could be called before the spawned task had a chance
    /// to run and subscribe, causing `VerifiedNode` replies to be silently dropped.
    pub async fn start(&mut self) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        
        // Subscribe BEFORE spawning — guaranteed in place when start() returns.
        let (all_rx, verified_node_rx, init_complete_rx, pcer_rx) = if let Some(ref h) = self.handle {
            (
                h.subscribe_all(),
                h.subscribe_mti(MTI::VerifiedNode).await,
                h.subscribe_mti(MTI::InitializationComplete).await,
                h.subscribe_mti(MTI::ProducerConsumerEventReport).await,
            )
        } else {
            eprintln!("[EventRouter] No transport handle — cannot start");
            return;
        };

        eprintln!("[EventRouter] Subscribed to message channels (alias=0x{:03X})", self.our_alias);

        let app = self.app.clone();
        let our_alias = self.our_alias;
        let registry = self.registry.clone();
        let diag_stats = self.diag_stats.clone();
        let frame_ring = self.frame_ring.clone();
        
        let handle = tokio::spawn(async move {
            Self::router_loop(app, all_rx, verified_node_rx, init_complete_rx, pcer_rx, our_alias, registry, diag_stats, frame_ring, shutdown_rx).await;
        });
        
        self.router_task = Some(handle);
        self.shutdown_tx = Some(shutdown_tx);
    }

    /// Main router loop — receivers are created by `start()` before this task is spawned.
    async fn router_loop(
        app: AppHandle,
        mut all_rx: tokio::sync::broadcast::Receiver<lcc_rs::ReceivedMessage>,
        mut verified_node_rx: tokio::sync::broadcast::Receiver<lcc_rs::ReceivedMessage>,
        mut init_complete_rx: tokio::sync::broadcast::Receiver<lcc_rs::ReceivedMessage>,
        mut pcer_rx: tokio::sync::broadcast::Receiver<lcc_rs::ReceivedMessage>,
        our_alias: u16,
        registry: Arc<NodeRegistry>,
        diag_stats: DiagStats,
        frame_ring: FrameRing,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        eprintln!("[EventRouter] Router loop started with our_alias=0x{:03X}", our_alias);

        loop {
            tokio::select! {
                // Check for shutdown
                _ = &mut shutdown_rx => {
                    eprintln!("[EventRouter] Shutdown signal received");
                    break;
                }
                
                // Handle all messages for monitor
                Ok(msg) = all_rx.recv() => {
                    // Record frame in the ring buffer for diagnostics.
                    {
                        // Determine direction: frames from our alias are TX (echoed), others are RX.
                        let direction = if let Ok((_, src_alias)) = msg.frame.get_mti() {
                            if src_alias == our_alias { "tx" } else { "rx" }
                        } else {
                            "rx"
                        };
                        let timestamp_ms = {
                            let stats = diag_stats.read().await;
                            stats.connected_at
                                .map(|t| chrono::Utc::now().signed_duration_since(t).num_milliseconds().max(0) as u64)
                                .unwrap_or(0)
                        };
                        if let Ok(mut ring) = frame_ring.try_lock() {
                            if ring.len() >= FRAME_RING_CAPACITY {
                                ring.pop_front();
                            }
                            ring.push_back(FrameEntry {
                                direction: direction.to_string(),
                                timestamp_ms,
                                frame: msg.frame.to_string(),
                            });
                        }
                    }
                    Self::handle_all_messages(&app, msg, our_alias);
                }
                
                // Handle node discovery via VerifyNodeGlobal replies
                Ok(msg) = verified_node_rx.recv() => {
                    Self::handle_node_discovered(&app, &registry, &diag_stats, msg, our_alias).await;
                }

                // Handle nodes that join mid-session (they announce via InitializationComplete)
                // D15: Emit lcc-node-reinitialized so the frontend can refresh cached data.
                Ok(msg) = init_complete_rx.recv() => {
                    Self::handle_node_reinitialized(&app, &registry, msg, our_alias).await;
                }

                // Handle Producer/Consumer Event Report — emit to frontend for live state
                Ok(msg) = pcer_rx.recv() => {
                    Self::handle_pcer(&app, msg, our_alias);
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
    async fn handle_node_discovered(app: &AppHandle, registry: &NodeRegistry, diag_stats: &DiagStats, msg: ReceivedMessage, our_alias: u16) {
        eprintln!(
            "[EventRouter] handle_node_discovered: frame={} data_len={}",
            msg.frame.to_string(),
            msg.frame.data.len()
        );
        // Parse VerifiedNode response — accept >= 6 bytes so nodes that include
        // extra reserved/type bytes (some implementations send 8 bytes) are not
        // silently ignored.
        if msg.frame.data.len() >= 6 {
            if let Ok((_, alias)) = msg.frame.get_mti() {
                // Ignore echoes of our own VerifiedNode responses
                if alias == our_alias {
                    eprintln!("[EventRouter] ignoring own VerifiedNode (alias=0x{:03X})", alias);
                    return;
                }
                // Node ID is in the first 6 bytes. Emit a `NodeKey::Live(...)`
                // which serializes as the canonical 12-hex uppercase form
                // (ADR-0010 — NodeKey is the wire-form identity for nodes).
                let node_id_bytes: [u8; 6] = msg.frame.data[0..6].try_into().unwrap_or([0; 6]);
                let parsed_node_id = lcc_rs::NodeID::new(node_id_bytes);
                let node_key = NodeKey::from_node_id(parsed_node_id);

                let event = NodeDiscoveredEvent {
                    node_id: node_key.clone(),
                    alias,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };

                // Record discovery timing in diagnostics
                {
                    let stats = diag_stats.read().await;
                    let ms_after = stats.connected_at
                        .map(|t| chrono::Utc::now().signed_duration_since(t).num_milliseconds().max(0) as u64)
                        .unwrap_or(0);
                    drop(stats);
                    let mut stats = diag_stats.write().await;
                    // Only record if this node hasn't been seen before
                    if !stats.discovery.nodes.iter().any(|n| n.node_id == node_key.to_string()) {
                        stats.discovery.nodes.push(NodeDiscoveryStat {
                            node_id: node_key.to_string(),
                            snip_name: None,
                            ms_after_connect: ms_after,
                            snip_query_duration_ms: None,
                        });
                    }
                }

                // Auto-register proxy for this node
                let _ = registry.get_or_create(parsed_node_id, alias).await;

                eprintln!("[EventRouter] emitting lcc-node-discovered: node_id={} alias=0x{:03X}", event.node_id, event.alias);
                // Emit to frontend
                let _ = app.emit("lcc-node-discovered", event);
            }
        } else {
            eprintln!("[EventRouter] handle_node_discovered: ignoring frame with {} data bytes (expected >= 6): {}", msg.frame.data.len(), msg.frame.to_string());
        }
    }

    /// D15: Handle InitializationComplete — emit both node-discovered (for new nodes)
    /// and node-reinitialized (so the frontend can refresh cached SNIP/PIP/CDI).
    async fn handle_node_reinitialized(app: &AppHandle, registry: &NodeRegistry, msg: ReceivedMessage, our_alias: u16) {
        if msg.frame.data.len() >= 6 {
            if let Ok((_, alias)) = msg.frame.get_mti() {
                if alias == our_alias {
                    return;
                }
                let node_id_bytes: [u8; 6] = msg.frame.data[0..6].try_into().unwrap_or([0; 6]);
                let parsed_node_id = lcc_rs::NodeID::new(node_id_bytes);
                let node_key = NodeKey::from_node_id(parsed_node_id);

                // Auto-register proxy and signal reinitialization
                if let Ok(proxy) = registry.get_or_create(parsed_node_id, alias).await {
                    let _ = proxy.node_reinitialised().await;
                }

                // Always emit node-discovered so new nodes get added
                let event = NodeDiscoveredEvent {
                    node_id: node_key,
                    alias,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = app.emit("lcc-node-discovered", event.clone());

                // Also emit reinitialized so frontend refreshes stale cache
                let _ = app.emit("lcc-node-reinitialized", event);
            }
        }
    }

    /// Handle Producer/Consumer Event Report — extract 8-byte event ID, emit to frontend.
    fn handle_pcer(app: &AppHandle, msg: ReceivedMessage, our_alias: u16) {
        // PCER frames carry 8 bytes of event ID in the data field
        if msg.frame.data.len() >= 8 {
            if let Ok((_, alias)) = msg.frame.get_mti() {
                // Ignore echoes of our own PCER events
                if alias == our_alias {
                    return;
                }
                // Event ID is the 8-byte payload → canonical contiguous hex (ADR-0010)
                let event_bytes: [u8; 8] = msg.frame.data[0..8].try_into().unwrap_or([0; 8]);
                let event_id = lcc_rs::EventID::new(event_bytes).to_canonical();

                let event = EventStateEvent {
                    event_id,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = app.emit("lcc-event-state", event);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovered_event_serializes_node_id_as_canonical_string() {
        let id = lcc_rs::NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9]);
        let event = NodeDiscoveredEvent {
            node_id: NodeKey::from_node_id(id),
            alias: 0x123,
            timestamp: "2026-05-31T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["nodeId"], "020157000002D9");
        assert_eq!(json["alias"], 0x123);
    }
}
