//! Application state management for Bowties Tauri application

use lcc_rs::{LccConnection, SNIPData, TransportHandle};
use crate::commands::{ConnectionConfig};
use crate::events::EventRouter;
use crate::node_registry::NodeRegistry;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::collections::HashMap;
use tokio::sync::{RwLock, Mutex};
use crate::node_key::NodeKey;

// Re-export NodeRoles from bowties-core (now owns the canonical definition).
pub use bowties_core::node_tree::NodeRoles;

// Re-export bowtie catalog types from bowties-core (now owns the canonical definitions).
// These are re-exported so downstream code (and Tauri command return types) can reference
// them through `crate::state::*` without needing a direct bowties-core dependency.
#[allow(unused_imports)]
pub use bowties_core::bowtie::types::{BowtieState, EventSlotEntry, BowtieCard, BowtieCatalog};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLayoutMode {
    OfflineFile,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    TargetLayoutBus,
    BenchOtherBus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveLayoutContext {
    pub layout_id: String,
    pub root_path: String,
    pub mode: ActiveLayoutMode,
    pub captured_at: Option<String>,
    pub pending_offline_change_count: usize,
    /// Node keys for every snapshot in the active layout. Real nodes use
    /// the canonical 12-hex form (e.g. `"050201020200"`); placeholders use
    /// `"placeholder:<uuidv4>"`. Populated when the layout is opened; used
    /// for bus-match overlap scoring.
    #[serde(default)]
    pub layout_node_keys: Vec<NodeKey>,
}

// ─────────────────────────────────────────────────────────────────────────────

// ── Application state ─────────────────────────────────────────────────────

/// Runtime tuning parameters loaded from `tuning.toml` in the app data directory.
/// All fields have sensible defaults from lcc-rs constants.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TuningConfig {
    /// Per-attempt timeout (ms) waiting for a memory-config read reply.
    pub read_timeout_ms: u64,
    /// Maximum retries when a node rejects with the resend-OK flag.
    pub max_datagram_retries: u32,
    /// Delay (ms) after ACK-ing a reply before sending the next request.
    /// Gives CAN gateways time to finish forwarding the ACK frame.
    pub post_ack_delay_ms: u64,
}

impl Default for TuningConfig {
    fn default() -> Self {
        Self {
            read_timeout_ms: lcc_rs::constants::READ_MEMORY_TIMEOUT_MS,
            max_datagram_retries: lcc_rs::constants::MAX_DATAGRAM_RETRIES,
            post_ack_delay_ms: lcc_rs::constants::DEFAULT_POST_ACK_DELAY_MS,
        }
    }
}

impl TuningConfig {
    /// Load from a `tuning.toml` file at `path`. Returns defaults if file is
    /// missing or cannot be parsed.
    pub fn load_from_dir(dir: &std::path::Path) -> Self {
        let path = dir.join("tuning.toml");
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                match toml::from_str::<TuningConfig>(&contents) {
                    Ok(cfg) => {
                        eprintln!("[tuning] Loaded from {}", path.display());
                        cfg
                    }
                    Err(e) => {
                        eprintln!("[tuning] WARNING: Failed to parse {}: {} — using defaults", path.display(), e);
                        Self::default()
                    }
                }
            }
            Err(_) => Self::default(),
        }
    }

    /// Convert to lcc-rs `MemoryReadConfig`.
    pub fn to_memory_read_config(&self) -> lcc_rs::MemoryReadConfig {
        lcc_rs::MemoryReadConfig {
            timeout_ms: self.read_timeout_ms,
            max_retries: self.max_datagram_retries,
            post_ack_delay_ms: self.post_ack_delay_ms,
        }
    }
}

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

    /// Active layout context for either legacy single-file layout or offline directory layout.
    pub active_layout: Arc<RwLock<Option<ActiveLayoutContext>>>,

    /// Runtime cache of pending offline changes for the active layout.
    /// Synced to disk on each modification; cleared when layout is closed.
    pub offline_changes_cache: Arc<RwLock<Vec<crate::layout::offline_changes::OfflineChange>>>,

    /// User-selected sync mode when bus match is uncertain.
    pub sync_mode: Arc<RwLock<Option<SyncMode>>>,

    /// Single in-memory owner of the open layout (saved → captured → drafts).
    /// Owns the persisted-in-memory projection of the layout file: bowties,
    /// channels, facilities, offline changes, and per-node snapshot/CDI/tree.
    /// Source of truth for the save flow's per-node snapshot building and for
    /// the offline catalog rebuild. See ADR-0015.
    pub layout_state: Arc<RwLock<Option<bowties_core::layout::state::LayoutState>>>,

    // ── Spec 008: Structure profile cache ─────────────────────────────────

    /// Loaded structure profiles keyed by `ProfileKey` (manufacturer::model).
    /// `None` entry means "looked up but not found" (avoids re-scanning).
    pub profiles: crate::profile::ProfileCache,

    // ── Diagnostics ───────────────────────────────────────────────────────

    /// Ring-buffer diagnostic log (most recent 2000 lines, timestamped).
    pub diag_log: crate::diagnostics::DiagLog,

    /// Aggregate diagnostic statistics (updated as operations complete).
    pub diag_stats: crate::diagnostics::DiagStats,

    /// Bounded ring buffer of recent frame activity (last 100 frames).
    pub frame_ring: crate::diagnostics::FrameRing,

    /// Runtime tuning parameters (loaded from `tuning.toml` at startup).
    pub tuning: TuningConfig,
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
            active_layout: Arc::new(RwLock::new(None)),
            offline_changes_cache: Arc::new(RwLock::new(Vec::new())),
            sync_mode: Arc::new(RwLock::new(None)),
            layout_state: Arc::new(RwLock::new(None)),

            profiles: Arc::new(RwLock::new(HashMap::new())),
            diag_log: crate::diagnostics::new_diag_log(),
            diag_stats: crate::diagnostics::new_diag_stats(),
            frame_ring: crate::diagnostics::new_frame_ring(),
            tuning: TuningConfig::default(),
        }
    }

    /// Return a `MemoryReadConfig` from the current tuning parameters.
    pub fn memory_read_config(&self) -> lcc_rs::MemoryReadConfig {
        self.tuning.to_memory_read_config()
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
            let mut router = EventRouter::from_handle(app, h.clone(), our_alias, self.node_registry.clone(), self.diag_stats.clone(), self.frame_ring.clone());
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
        *self.sync_mode.write().await = None;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
