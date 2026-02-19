//! Application state management for Bowties Tauri application

use lcc_rs::{LccConnection, DiscoveredNode, MessageDispatcher};
use crate::events::EventRouter;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};

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
            dispatcher: Arc::new(RwLock::new(None)),
            event_router: Arc::new(RwLock::new(None)),
            nodes: Arc::new(RwLock::new(Vec::new())),
            host: Arc::new(RwLock::new("localhost".to_string())),
            port: Arc::new(RwLock::new(12021)),
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
        
        if let Some(disp) = dispatcher {
            *self.dispatcher.write().await = Some(disp.clone());
            
            // Start event router
            let mut router = EventRouter::new(app, disp);
            router.start();
            *self.event_router.write().await = Some(router);
        }
    }

    /// Disconnect and cleanup
    pub async fn disconnect(&self) {
        // Stop event router
        if let Some(mut router) = self.event_router.write().await.take() {
            router.stop().await;
        }
        
        // Clear dispatcher
        *self.dispatcher.write().await = None;
        
        // Clear connection
        *self.connection.write().await = None;
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
