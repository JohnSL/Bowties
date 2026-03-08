//! Application state management for Bowties Tauri application

use lcc_rs::{LccConnection, DiscoveredNode, MessageDispatcher, SNIPData};
use crate::commands::{ConnectionConfig};
use crate::events::EventRouter;
use crate::node_tree::NodeConfigTree;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::collections::{HashMap, HashSet};
use tokio::sync::{RwLock, Mutex};

// ── Feature 006: Bowtie catalog types ─────────────────────────────────────

/// Protocol-level producer/consumer ground truth from the Identify Events exchange.
///
/// Keyed in `AppState.event_roles` by `event_id_hex` (dotted-hex notation).
/// Populated by sending `IdentifyEventsAddressed` to each known node (125 ms
/// between sends) and collecting `ProducerIdentified` / `ConsumerIdentified` replies.
#[derive(Debug, Clone, Default)]
pub struct NodeRoles {
    /// Node IDs (dotted-hex) that replied ProducerIdentified for this event.
    pub producers: HashSet<String>,
    /// Node IDs (dotted-hex) that replied ConsumerIdentified for this event.
    pub consumers: HashSet<String>,
}

// ── Bowtie catalog types (defined here to avoid circular deps with commands::bowties) ──

/// A single classified event ID configuration field from one node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EventSlotEntry {
    /// Node identifier (dotted-hex)
    pub node_id: String,
    /// Human-readable node name
    pub node_name: String,
    /// CDI element path from segment root
    pub element_path: Vec<String>,
    /// Display label (CDI name → description first sentence → slash-joined path)
    pub element_label: String,
    /// Full CDI <description> text for this slot (None when absent).
    /// Forwarded to the frontend so users can read the raw description when the
    /// role is Ambiguous and decide how to classify the slot.
    pub element_description: Option<String>,
    /// The 8-byte event ID stored in this slot
    pub event_id: [u8; 8],
    /// Classified role (only Producer or Consumer here; Ambiguous goes to ambiguous_entries)
    pub role: lcc_rs::EventRole,
}

/// One shared event ID with ≥1 confirmed producer and ≥1 confirmed consumer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BowtieCard {
    /// Dotted-hex event ID (unique key, default header)
    pub event_id_hex: String,
    /// Raw 8-byte event ID (for sorting/comparisons)
    pub event_id_bytes: [u8; 8],
    /// Confirmed producer slots (≥1)
    pub producers: Vec<EventSlotEntry>,
    /// Confirmed consumer slots (≥1)
    pub consumers: Vec<EventSlotEntry>,
    /// Slots whose role could not be determined
    pub ambiguous_entries: Vec<EventSlotEntry>,
    /// User-assigned name (None in this phase)
    pub name: Option<String>,
}

/// Complete in-memory collection of discovered bowties for the current session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BowtieCatalog {
    /// Bowtie cards sorted by event_id_bytes
    pub bowties: Vec<BowtieCard>,
    /// ISO 8601 timestamp of when this catalog was built
    pub built_at: String,
    /// Number of nodes included in the catalog build
    pub source_node_count: usize,
    /// Total event slots scanned across all nodes
    pub total_slots_scanned: usize,
}

// ─────────────────────────────────────────────────────────────────────────────

// ── Application state ─────────────────────────────────────────────────────

/// Global application state shared across Tauri commands
#[derive(Clone)]
pub struct AppState {
    /// LCC network connection with dispatcher (optional, None if not connected)
    pub connection: Arc<RwLock<Option<Arc<Mutex<LccConnection>>>>>,
    
    /// Message dispatcher for persistent listening
    pub dispatcher: Arc<RwLock<Option<Arc<Mutex<MessageDispatcher>>>>>,
    
    /// Event router for frontend notifications
    pub event_router: Arc<RwLock<Option<EventRouter>>>,
    
    /// Cache of discovered nodes
    pub nodes: Arc<RwLock<Vec<DiscoveredNode>>>,
    
    /// Cancellation token for config reading operations (T012)
    pub config_read_cancel: Arc<AtomicBool>,

    /// Active connection configuration (None when not connected)
    pub active_connection: Arc<RwLock<Option<ConnectionConfig>>>,

    // ── Feature 006: Bowtie catalog fields ────────────────────────────────

    /// Finished bowtie catalog built after CDI reads + Identify Events exchange.
    /// `None` until the first `cdi-read-complete` cycle completes.
    pub bowties_catalog: Arc<RwLock<Option<BowtieCatalog>>>,

    /// Node-level producer/consumer roles from the Identify Events exchange.
    /// Key = event_id_hex (e.g. "05.02.01.02.03.00.00.01").
    /// Populated by `query_event_roles` in `commands/bowties.rs`.
    pub event_roles: Arc<RwLock<HashMap<String, NodeRoles>>>,

    /// Config value cache: actual event ID bytes read from each CDI slot.
    /// Outer key = node_id_hex; inner key = element_path joined by "/".
    /// Populated as `read_all_config_values` completes for each node.
    /// Consulted by `build_bowtie_catalog` to identify the correct CDI slot
    /// for each event ID (precise match, fallback to heuristic if missing).
    pub config_value_cache: Arc<RwLock<HashMap<String, HashMap<String, [u8; 8]>>>>,

    // ── Spec 007: Unified node configuration trees ────────────────────────

    /// Canonical per-node tree merging CDI structure, absolute addresses,
    /// config values, and event roles.  Built once after CDI parse, then
    /// progressively enriched by `merge_config_values` / `merge_event_roles`.
    /// Key = node_id_hex.
    pub node_trees: Arc<RwLock<HashMap<String, NodeConfigTree>>>,

    // ── Spec 008: Structure profile cache ─────────────────────────────────

    /// Loaded structure profiles keyed by `ProfileKey` (manufacturer::model).
    /// `None` entry means "looked up but not found" (avoids re-scanning).
    pub profiles: crate::profile::ProfileCache,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            dispatcher: Arc::new(RwLock::new(None)),
            event_router: Arc::new(RwLock::new(None)),
            nodes: Arc::new(RwLock::new(Vec::new())),
            active_connection: Arc::new(RwLock::new(None)),
            config_read_cancel: Arc::new(AtomicBool::new(false)),
            bowties_catalog: Arc::new(RwLock::new(None)),
            event_roles: Arc::new(RwLock::new(HashMap::new())),
            config_value_cache: Arc::new(RwLock::new(HashMap::new())),
            node_trees: Arc::new(RwLock::new(HashMap::new())),
            profiles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if connected to LCC network
    pub async fn is_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Set the LCC connection (dispatcher-based)
    pub async fn set_connection_with_dispatcher(
        &self,
        connection: Arc<Mutex<LccConnection>>,
        app: tauri::AppHandle,
    ) {
        // Set connection
        *self.connection.write().await = Some(connection.clone());
        
        // Get dispatcher from connection
        let dispatcher = {
            let conn = connection.lock().await;
            conn.dispatcher()
        };
        
        // Get our alias from connection for event routing
        let our_alias = {
            let conn = connection.lock().await;
            conn.our_alias().value()
        };
        
        if let Some(disp) = dispatcher {
            *self.dispatcher.write().await = Some(disp.clone());
            
            // Start event router with our alias for direction detection
            let mut router = EventRouter::new(app, disp, our_alias);
            router.start();
            *self.event_router.write().await = Some(router);
        }
        
        // Configure SNIP data and start responding to discovery queries
        {
            let mut conn = connection.lock().await;
            
            // Set SNIP data (Manufacturer blank, Model "Bowties::LCC", Software version from app)
            let snip_data = SNIPData {
                manufacturer: "JohnSL".to_string(),
                model: "Bowties::LCC".to_string(),
                hardware_version: String::new(),
                software_version: env!("CARGO_PKG_VERSION").to_string(),
                user_name: String::new(),
                user_description: String::new(),
            };
            conn.set_snip_data(snip_data);
            
            // Start responding to discovery queries (Verify Node ID Global)
            let _ = conn.start_responding_to_queries();
            
            // Start responding to SNIP requests
            let _ = conn.start_responding_to_snip_requests();
        }
    }

    /// Disconnect and cleanup
    pub async fn disconnect(&self) {
        // Stop event router
        if let Some(mut router) = self.event_router.write().await.take() {
            router.stop().await;
        }
        
        // Shutdown dispatcher (signals background task to exit, releasing the serial port)
        if let Some(disp_arc) = self.dispatcher.write().await.take() {
            disp_arc.lock().await.shutdown().await;
        }
        
        // Clear connection and active config
        *self.connection.write().await = None;
        *self.active_connection.write().await = None;
    }

    /// Get all cached nodes
    pub async fn get_nodes(&self) -> Vec<DiscoveredNode> {
        self.nodes.read().await.clone()
    }

    /// Update the nodes cache
    pub async fn set_nodes(&self, nodes: Vec<DiscoveredNode>) {
        *self.nodes.write().await = nodes;
    }

    /// Add a single node to the cache (deduplicates by node_id)
    #[allow(dead_code)]
    pub async fn add_node(&self, node: DiscoveredNode) {
        let mut nodes = self.nodes.write().await;
        
        // Check if node already exists
        let exists = nodes.iter().any(|n| n.node_id == node.node_id);
        
        if !exists {
            nodes.push(node);
        }
    }

    /// Update a specific node in the cache
    pub async fn update_node(&self, node_id: lcc_rs::NodeID, update_fn: impl FnOnce(&mut DiscoveredNode)) {
        let mut nodes = self.nodes.write().await;
        
        if let Some(node) = nodes.iter_mut().find(|n| n.node_id == node_id) {
            update_fn(node);
        }
    }

    /// Clear all cached nodes
    #[allow(dead_code)]
    pub async fn clear_nodes(&self) {
        self.nodes.write().await.clear();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
