//! Registry of NodeProxy actors — one per discovered LCC node or placeholder.
//!
//! The registry is the single point of truth for "which nodes exist" and
//! provides lookup by [`NodeKey`]. It is stored in `AppState` behind an `Arc`
//! so that Tauri commands, the EventRouter, and background tasks can all
//! reach it.

use lcc_rs::{NodeID, TransportHandle};
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::node_key::NodeKey;
use crate::node_proxy::{LiveNodeProxy, NodeProxyHandle};
use crate::node_tree::NodeConfigTree;

/// Thread-safe registry mapping NodeKey → NodeProxyHandle.
pub struct NodeRegistry {
    proxies: RwLock<HashMap<NodeKey, NodeProxyHandle>>,
    transport_handle: RwLock<Option<TransportHandle>>,
    our_alias: RwLock<u16>,
    /// Config trees loaded from a saved layout, keyed by NodeKey.
    /// Populated during `open_layout_directory`, consumed during node
    /// discovery to seed freshly-spawned proxies so they start with the
    /// previously-captured config rather than an empty tree.
    /// Cleared on `close_layout` / `clear_saved_trees`.
    saved_trees: RwLock<HashMap<NodeKey, NodeConfigTree>>,
}

impl NodeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            proxies: RwLock::new(HashMap::new()),
            transport_handle: RwLock::new(None),
            our_alias: RwLock::new(0),
            saved_trees: RwLock::new(HashMap::new()),
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
    /// Otherwise a new LiveNodeProxy actor is spawned.
    pub async fn get_or_create(&self, node_id: NodeID, alias: u16) -> Result<NodeProxyHandle, String> {
        let key = NodeKey::from_node_id(node_id);

        // Fast path: read lock
        {
            let proxies = self.proxies.read().await;
            if let Some(handle) = proxies.get(&key) {
                if let Ok(snapshot) = handle.get_snapshot().await {
                    let current_alias = snapshot.alias.value();
                    if current_alias != alias {
                        eprintln!(
                            "[node-registry] updating alias for {} from 0x{:03X} to 0x{:03X}",
                            node_id, current_alias, alias,
                        );
                        let _ = handle.update_alias(alias).await;
                    }
                }
                return Ok(handle.clone());
            }
        }

        // Slow path: write lock + create
        let mut proxies = self.proxies.write().await;
        if let Some(handle) = proxies.get(&key) {
            if let Ok(snapshot) = handle.get_snapshot().await {
                let current_alias = snapshot.alias.value();
                if current_alias != alias {
                    eprintln!(
                        "[node-registry] updating alias for {} from 0x{:03X} to 0x{:03X}",
                        node_id, current_alias, alias,
                    );
                    let _ = handle.update_alias(alias).await;
                }
            }
            return Ok(handle.clone());
        }

        let transport = self.transport_handle.read().await;
        let transport = transport
            .as_ref()
            .ok_or_else(|| "No transport configured — not connected".to_string())?;
        let our_alias = *self.our_alias.read().await;

        let live_handle = LiveNodeProxy::spawn(node_id, alias, transport.clone(), our_alias);
        let handle = NodeProxyHandle::Live(live_handle);

        // Seed the proxy with a previously-saved config tree if one exists.
        // This makes the saved layout data the base layer for this node's
        // config, so `get_node_tree` returns complete data without requiring
        // a fresh bus read.
        if let Some(tree) = self.saved_trees.read().await.get(&key).cloned() {
            let _ = handle.set_config_tree(tree).await;
        }

        proxies.insert(key, handle.clone());
        Ok(handle)
    }

    /// Look up an existing proxy by NodeID. Returns None if not registered.
    pub async fn get(&self, node_id: &NodeID) -> Option<NodeProxyHandle> {
        self.get_by_node_key(&NodeKey::from_node_id(*node_id)).await
    }

    /// Look up an existing proxy by [`NodeKey`]. Returns None if not registered.
    pub async fn get_by_node_key(&self, node_key: &NodeKey) -> Option<NodeProxyHandle> {
        self.proxies.read().await.get(node_key).cloned()
    }

    /// Insert a pre-built proxy handle under the given NodeKey.
    pub async fn insert(&self, node_key: NodeKey, handle: NodeProxyHandle) {
        self.proxies.write().await.insert(node_key, handle);
    }

    /// Look up an existing proxy by alias. Returns None if not registered.
    pub async fn get_by_alias(&self, alias: u16) -> Option<NodeProxyHandle> {
        let handles: Vec<NodeProxyHandle> = self.proxies.read().await.values().cloned().collect();
        for handle in handles {
            if let Ok(snapshot) = handle.get_snapshot().await {
                if snapshot.alias.value() == alias {
                    return Some(handle);
                }
            }
        }
        None
    }

    /// Return snapshot of all node proxies' cached DiscoveredNode data.
    pub async fn get_all_snapshots(&self) -> Vec<lcc_rs::DiscoveredNode> {
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
        self.remove_by_key(&NodeKey::from_node_id(*node_id)).await;
    }

    /// Remove a single proxy by [`NodeKey`], shutting down its actor.
    pub async fn remove_by_key(&self, node_key: &NodeKey) {
        let mut proxies = self.proxies.write().await;
        if let Some(handle) = proxies.remove(node_key) {
            handle.shutdown().await;
        }
    }

    /// Drop every layout-scoped proxy from the registry.
    ///
    /// Today that means every `Placeholder` proxy: placeholders exist only
    /// because a layout author created them, so closing the layout must
    /// forget them. `Live` proxies are kept because the bus connection
    /// outlives the layout — the next probe / save can promote them again
    /// via an explicit `AddNode` delta.
    ///
    /// Also clears saved trees since the layout is being closed.
    ///
    /// Owned by `close_layout` (and any other "forget this layout" entry
    /// point). Pinned by `ADR-0011`'s 2026-05-31 extension.
    pub async fn clear_layout_scope(&self) {
        let to_remove: Vec<NodeKey> = {
            let proxies = self.proxies.read().await;
            proxies
                .keys()
                .filter(|k| k.is_placeholder())
                .cloned()
                .collect()
        };
        let mut proxies = self.proxies.write().await;
        for key in to_remove {
            if let Some(handle) = proxies.remove(&key) {
                handle.shutdown().await;
            }
        }
        self.saved_trees.write().await.clear();
    }

    /// Replace the saved trees cache with trees built from a layout's
    /// saved node snapshots. Called during `open_layout_directory`.
    pub async fn set_saved_trees(&self, trees: HashMap<NodeKey, NodeConfigTree>) {
        *self.saved_trees.write().await = trees;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_proxy::SynthesizedNodeProxy;
    use lcc_rs::TransportHandle;
    use std::collections::HashMap as StdHashMap;
    use std::sync::Arc;
    use tokio::sync::{broadcast, mpsc, RwLock as TokioRwLock};
    use uuid::Uuid;

    fn dummy_transport_handle() -> TransportHandle {
        let (tx, _rx) = mpsc::channel(1);
        let (all_tx, _) = broadcast::channel(1);
        let mti_senders = Arc::new(TokioRwLock::new(StdHashMap::new()));
        TransportHandle::from_parts(tx, all_tx, mti_senders)
    }

    fn synth_handle(node_key: &NodeKey) -> NodeProxyHandle {
        NodeProxyHandle::Synthesized(SynthesizedNodeProxy {
            node_key: node_key.to_string(),
            profile_stem: "test".to_string(),
            snip: None,
            cdi_data: None,
            cdi_parsed: None,
            config_values: StdHashMap::new(),
            config_tree: None,
            producer_identified_events: Vec::new(),
        })
    }

    #[tokio::test]
    async fn get_or_create_then_lookup_via_parsed_dotted_form_finds_proxy() {
        let registry = NodeRegistry::new();
        registry.set_transport(dummy_transport_handle(), 0x001).await;
        let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9]);
        registry.get_or_create(node_id, 0x100).await.unwrap();

        let key = NodeKey::parse("02.01.57.00.02.D9").unwrap();
        assert!(registry.get_by_node_key(&key).await.is_some());
    }

    #[tokio::test]
    async fn get_or_create_then_lookup_via_parsed_canonical_form_finds_proxy() {
        let registry = NodeRegistry::new();
        registry.set_transport(dummy_transport_handle(), 0x001).await;
        let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9]);
        registry.get_or_create(node_id, 0x100).await.unwrap();

        let key = NodeKey::parse("0201570002D9").unwrap();
        assert!(registry.get_by_node_key(&key).await.is_some());
    }

    #[tokio::test]
    async fn get_and_get_by_node_key_agree_for_live_nodes() {
        let registry = NodeRegistry::new();
        registry.set_transport(dummy_transport_handle(), 0x001).await;
        let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9]);
        registry.get_or_create(node_id, 0x100).await.unwrap();

        let by_id = registry.get(&node_id).await;
        let by_key = registry
            .get_by_node_key(&NodeKey::from_node_id(node_id))
            .await;
        assert!(by_id.is_some() && by_key.is_some());
    }

    #[tokio::test]
    async fn insert_then_get_finds_proxy() {
        let registry = NodeRegistry::new();
        let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9]);
        let live = LiveNodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001);
        let key = NodeKey::from_node_id(node_id);
        registry.insert(key, NodeProxyHandle::Live(live)).await;

        let parsed = NodeKey::parse("02.01.57.00.02.D9").unwrap();
        assert!(registry.get_by_node_key(&parsed).await.is_some());
    }

    #[tokio::test]
    async fn placeholder_key_round_trip_via_insert_and_get_by_node_key() {
        let registry = NodeRegistry::new();
        let uuid = Uuid::parse_str("11111111-2222-4333-8444-555555555555").unwrap();
        let key = NodeKey::placeholder(uuid);
        registry.insert(key, synth_handle(&key)).await;

        assert!(registry.get_by_node_key(&key).await.is_some());
    }

    #[tokio::test]
    async fn unrelated_key_returns_none() {
        let registry = NodeRegistry::new();
        let key = NodeKey::placeholder(Uuid::new_v4());
        assert!(registry.get_by_node_key(&key).await.is_none());
    }

    #[tokio::test]
    async fn clear_layout_scope_drops_placeholders_and_keeps_live() {
        let registry = NodeRegistry::new();
        registry.set_transport(dummy_transport_handle(), 0x001).await;

        let live_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9]);
        registry.get_or_create(live_id, 0x100).await.unwrap();

        let ph_key = NodeKey::placeholder(Uuid::new_v4());
        registry.insert(ph_key, synth_handle(&ph_key)).await;

        assert_eq!(registry.len().await, 2);

        registry.clear_layout_scope().await;

        assert_eq!(registry.len().await, 1, "live proxy must survive");
        assert!(registry.get(&live_id).await.is_some());
        assert!(registry.get_by_node_key(&ph_key).await.is_none());
    }

    #[tokio::test]
    async fn clear_layout_scope_clears_saved_trees() {
        let registry = NodeRegistry::new();
        let tree = crate::node_tree::NodeConfigTree {
            node_id: "0201570002D9".to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            segments: Vec::new(),
            unknown_variants: Vec::new(),
            profile_applied: false,
        };
        let key = NodeKey::parse("0201570002D9").unwrap();
        let mut trees = HashMap::new();
        trees.insert(key, tree);
        registry.set_saved_trees(trees).await;

        assert!(!registry.saved_trees.read().await.is_empty());
        registry.clear_layout_scope().await;
        assert!(registry.saved_trees.read().await.is_empty());
    }

    #[tokio::test]
    async fn get_or_create_seeds_proxy_with_saved_tree() {
        let registry = NodeRegistry::new();
        registry.set_transport(dummy_transport_handle(), 0x001).await;

        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x02, 0x00]);
        let key = NodeKey::from_node_id(node_id);

        // Pre-populate a saved tree with a segment containing a leaf
        let tree = crate::node_tree::NodeConfigTree {
            node_id: key.to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            segments: vec![crate::node_tree::SegmentNode {
                name: "Test".to_string(),
                origin: 0,
                description: None,
                space: 253,
                children: vec![],
            }],
            unknown_variants: Vec::new(),
            profile_applied: false,
        };
        let mut trees = HashMap::new();
        trees.insert(key, tree.clone());
        registry.set_saved_trees(trees).await;

        // When the node is created via get_or_create, the proxy should be seeded
        let handle = registry.get_or_create(node_id, 0x100).await.unwrap();
        let proxy_tree = handle.get_config_tree().await.unwrap();
        assert!(proxy_tree.is_some(), "proxy should have been seeded with saved tree");
        let proxy_tree = proxy_tree.unwrap();
        assert_eq!(proxy_tree.segments.len(), 1);
        assert_eq!(proxy_tree.segments[0].name, "Test");
    }

    #[tokio::test]
    async fn get_or_create_without_saved_tree_leaves_proxy_empty() {
        let registry = NodeRegistry::new();
        registry.set_transport(dummy_transport_handle(), 0x001).await;

        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x02, 0x00]);

        let handle = registry.get_or_create(node_id, 0x100).await.unwrap();
        let proxy_tree = handle.get_config_tree().await.unwrap();
        assert!(proxy_tree.is_none(), "proxy without saved tree should have no config tree");
    }
}
