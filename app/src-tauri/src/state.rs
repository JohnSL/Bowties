//! Application state management for Bowties Tauri application

use lcc_rs::{LccConnection, SNIPData, TransportHandle};
use crate::commands::{ConnectionConfig};
use crate::events::EventRouter;
use crate::node_registry::NodeRegistry;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::collections::{HashMap, HashSet};
use tokio::sync::{RwLock, Mutex};

// ── Feature 006: Bowtie catalog types ─────────────────────────────────────

/// Protocol-level producer/consumer ground truth from the Identify Events exchange.
///
/// Returned by `query_event_roles`, keyed by raw event_id bytes.
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

/// Bowtie state reflecting current element membership.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum BowtieState {
    Active,
    Incomplete,
    Planning,
}

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
    /// User-assigned name (None = unnamed, show event_id_hex as header)
    pub name: Option<String>,
    /// User-assigned tags from layout metadata
    #[serde(default)]
    pub tags: Vec<String>,
    /// Derived state based on element membership
    #[serde(default = "default_bowtie_state")]
    pub state: BowtieState,
}

fn default_bowtie_state() -> BowtieState {
    BowtieState::Active
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
    /// LCC network connection (optional, None if not connected)
    pub connection: Arc<RwLock<Option<Arc<Mutex<LccConnection>>>>>,

    /// Transport handle for direct channel-based communication
    pub transport_handle: Arc<RwLock<Option<TransportHandle>>>,
    
    /// Event router for frontend notifications
    pub event_router: Arc<RwLock<Option<EventRouter>>>,

    /// Per-node proxy registry — the canonical source of per-node state.
    pub node_registry: Arc<NodeRegistry>,
    
    /// Cancellation token for config reading operations (T012)
    pub config_read_cancel: Arc<AtomicBool>,

    /// Cancellation flag for CDI download operations.
    pub cdi_download_cancel: Arc<AtomicBool>,

    /// Active connection configuration (None when not connected)
    pub active_connection: Arc<RwLock<Option<ConnectionConfig>>>,

    // ── Feature 006: Bowtie catalog fields ────────────────────────────────

    /// Finished bowtie catalog built after CDI reads + Identify Events exchange.
    /// `None` until the first `cdi-read-complete` cycle completes.
    pub bowties_catalog: Arc<RwLock<Option<BowtieCatalog>>>,



    // ── Spec 008: Structure profile cache ─────────────────────────────────

    /// Loaded structure profiles keyed by `ProfileKey` (manufacturer::model).
    /// `None` entry means "looked up but not found" (avoids re-scanning).
    pub profiles: crate::profile::ProfileCache,

    // ── Diagnostics ───────────────────────────────────────────────────────

    /// Ring-buffer diagnostic log (most recent 2000 lines, timestamped).
    pub diag_log: crate::diagnostics::DiagLog,

    /// Aggregate diagnostic statistics (updated as operations complete).
    pub diag_stats: crate::diagnostics::DiagStats,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            transport_handle: Arc::new(RwLock::new(None)),
            event_router: Arc::new(RwLock::new(None)),
            node_registry: Arc::new(NodeRegistry::new()),
            active_connection: Arc::new(RwLock::new(None)),
            config_read_cancel: Arc::new(AtomicBool::new(false)),
            cdi_download_cancel: Arc::new(AtomicBool::new(false)),
            bowties_catalog: Arc::new(RwLock::new(None)),

            profiles: Arc::new(RwLock::new(HashMap::new())),
            diag_log: crate::diagnostics::new_diag_log(),
            diag_stats: crate::diagnostics::new_diag_stats(),
        }
    }

    /// Check if connected to LCC network
    pub async fn is_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Set the LCC connection
    pub async fn set_connection_with_dispatcher(
        &self,
        connection: Arc<Mutex<LccConnection>>,
        app: tauri::AppHandle,
    ) {
        // Set connection
        *self.connection.write().await = Some(connection.clone());
        
        // Get transport handle from connection
        let handle = {
            let conn = connection.lock().await;
            conn.transport_handle().cloned()
        };
        
        // Get our alias from connection for event routing
        let our_alias = {
            let conn = connection.lock().await;
            conn.our_alias().value()
        };

        // Store transport handle
        if let Some(ref h) = handle {
            *self.transport_handle.write().await = Some(h.clone());
            // Configure node registry so proxies can be spawned
            self.node_registry.set_transport(h.clone(), our_alias).await;
        }

        // Start event router
        if let Some(ref h) = handle {
            let mut router = EventRouter::from_handle(app, h.clone(), our_alias, self.node_registry.clone());
            router.start().await;
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

        // Shut down all node proxy actors
        self.node_registry.shutdown_all().await;

        // Abort background responder tasks (VerifyNodeGlobal + SNIP responders) that
        // were spawned by LccConnection.  These tasks hold channel handles;
        // if they are not aborted here they keep the transport alive.
        // Then close the connection (shuts down the TransportActor if present).
        if let Some(conn_arc) = self.connection.read().await.as_ref().cloned() {
            let mut conn = conn_arc.lock().await;
            conn.shutdown_responders().await;
            let _ = conn.close().await;
        }
        
        // Clear transport handle
        *self.transport_handle.write().await = None;
        
        // Clear connection and active config
        *self.connection.write().await = None;
        *self.active_connection.write().await = None;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
