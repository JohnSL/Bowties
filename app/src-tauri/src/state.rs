//! Application state management for Bowties Tauri application

use lcc_rs::{LccConnection, DiscoveredNode};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global application state shared across Tauri commands
#[derive(Clone)]
pub struct AppState {
    /// LCC network connection (optional, None if not connected)
    pub connection: Arc<RwLock<Option<LccConnection>>>,
    
    /// Cache of discovered nodes
    pub nodes: Arc<RwLock<Vec<DiscoveredNode>>>,
    
    /// Connection host
    pub host: Arc<RwLock<String>>,
    
    /// Connection port
    pub port: Arc<RwLock<u16>>,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            nodes: Arc::new(RwLock::new(Vec::new())),
            host: Arc::new(RwLock::new("localhost".to_string())),
            port: Arc::new(RwLock::new(12021)),
        }
    }

    /// Check if connected to LCC network
    pub async fn is_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Get mutable access to the connection
    /// This is used for operations that need to modify the transport
    #[allow(dead_code)]
    pub async fn with_connection<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut LccConnection) -> R,
    {
        let mut conn_guard = self.connection.write().await;
        conn_guard.as_mut().map(f)
    }

    /// Set the LCC connection
    pub async fn set_connection(&self, connection: Option<LccConnection>) {
        *self.connection.write().await = connection;
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
