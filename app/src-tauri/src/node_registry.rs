//! Registry of NodeProxy actors — one per discovered LCC node.
//!
//! The registry is the single point of truth for "which nodes exist" and
//! provides lookup by NodeID.  It is stored in `AppState` behind an `Arc` so
//! that Tauri commands, the EventRouter, and background tasks can all reach it.

use lcc_rs::{DiscoveredNode, NodeID, TransportHandle};
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::node_proxy::{NodeProxy, NodeProxyHandle};

/// Thread-safe registry mapping NodeID → NodeProxyHandle.
pub struct NodeRegistry {
    proxies: RwLock<HashMap<NodeID, NodeProxyHandle>>,
    transport_handle: RwLock<Option<TransportHandle>>,
    our_alias: RwLock<u16>,
}

impl NodeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            proxies: RwLock::new(HashMap::new()),
            transport_handle: RwLock::new(None),
            our_alias: RwLock::new(0),
        }
    }

    /// Configure the transport used to spawn new proxies.
    pub async fn set_transport(&self, handle: TransportHandle, our_alias: u16) {
        *self.transport_handle.write().await = Some(handle);
        *self.our_alias.write().await = our_alias;
    }

    /// Get or create a proxy for the given node.
    ///
    /// If a proxy already exists for this NodeID, its handle is returned.
    /// Otherwise a new NodeProxy actor is spawned.
    pub async fn get_or_create(&self, node_id: NodeID, alias: u16) -> Result<NodeProxyHandle, String> {
        // Fast path: read lock
        {
            let proxies = self.proxies.read().await;
            if let Some(handle) = proxies.get(&node_id) {
                return Ok(handle.clone());
            }
        }

        // Slow path: write lock + create
        let mut proxies = self.proxies.write().await;
        // Double-check after acquiring write lock
        if let Some(handle) = proxies.get(&node_id) {
            return Ok(handle.clone());
        }

        let transport = self.transport_handle.read().await;
        let transport = transport
            .as_ref()
            .ok_or_else(|| "No transport configured — not connected".to_string())?;
        let our_alias = *self.our_alias.read().await;

        let proxy_handle = NodeProxy::spawn(node_id, alias, transport.clone(), our_alias);
        proxies.insert(node_id, proxy_handle.clone());
        Ok(proxy_handle)
    }

    /// Look up an existing proxy by NodeID. Returns None if not registered.
    pub async fn get(&self, node_id: &NodeID) -> Option<NodeProxyHandle> {
        self.proxies.read().await.get(node_id).cloned()
    }

    /// Look up an existing proxy by alias. Returns None if not registered.
    pub async fn get_by_alias(&self, alias: u16) -> Option<NodeProxyHandle> {
        self.proxies
            .read()
            .await
            .values()
            .find(|h| h.alias == alias)
            .cloned()
    }

    /// Return snapshot of all node proxies' cached DiscoveredNode data.
    pub async fn get_all_snapshots(&self) -> Vec<DiscoveredNode> {
        let proxies = self.proxies.read().await;
        let mut snapshots = Vec::with_capacity(proxies.len());
        for handle in proxies.values() {
            if let Ok(snap) = handle.get_snapshot().await {
                snapshots.push(snap);
            }
        }
        snapshots
    }

    /// Return handles for all registered proxies.
    pub async fn get_all_handles(&self) -> Vec<NodeProxyHandle> {
        self.proxies.read().await.values().cloned().collect()
    }

    /// Number of registered nodes.
    pub async fn len(&self) -> usize {
        self.proxies.read().await.len()
    }

    /// Shut down all proxy actors and clear the registry.
    pub async fn shutdown_all(&self) {
        let mut proxies = self.proxies.write().await;
        for (_, handle) in proxies.drain() {
            handle.shutdown().await;
        }
        *self.transport_handle.write().await = None;
    }

    /// Remove a single proxy by NodeID, shutting down its actor.
    pub async fn remove(&self, node_id: &NodeID) {
        let mut proxies = self.proxies.write().await;
        if let Some(handle) = proxies.remove(node_id) {
            handle.shutdown().await;
        }
    }
}
