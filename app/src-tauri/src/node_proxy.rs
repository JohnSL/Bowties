//! Per-node actor (NodeProxy) that owns all state for a single discovered LCC node.
//!
//! Each proxy runs as a `tokio::spawn` task with an mpsc mailbox.  Callers
//! interact through a cloneable [`NodeProxyHandle`] that sends messages and
//! receives replies via oneshot channels.

use lcc_rs::{
    CdiData, ConnectionStatus, DiscoveredNode, NodeAlias, NodeID, PIPStatus, ProtocolFlags,
    SNIPData, SNIPStatus, TransportHandle,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Semaphore};

use crate::node_tree::NodeConfigTree;

// ── Types ────────────────────────────────────────────────────────────────────

type SnipResult = Result<(Option<SNIPData>, SNIPStatus), String>;
type PipResult = Result<(Option<ProtocolFlags>, PIPStatus), String>;

// ── ProxyMessage ─────────────────────────────────────────────────────────────

/// Messages accepted by a NodeProxy actor.
pub enum ProxyMessage {
    // ── Quick queries (handled inline, <100ms) ────────────────────────────
    QuerySnip {
        reply: oneshot::Sender<SnipResult>,
    },
    QueryPip {
        reply: oneshot::Sender<PipResult>,
    },
    GetSnapshot {
        reply: oneshot::Sender<DiscoveredNode>,
    },

    // ── CDI ───────────────────────────────────────────────────────────────
    GetCdiData {
        reply: oneshot::Sender<Option<CdiData>>,
    },
    SetCdiData {
        cdi_data: CdiData,
    },
    GetCdiParsed {
        reply: oneshot::Sender<Option<lcc_rs::cdi::Cdi>>,
    },
    SetCdiParsed {
        cdi: lcc_rs::cdi::Cdi,
    },

    // ── Config values ─────────────────────────────────────────────────────
    GetConfigValues {
        reply: oneshot::Sender<HashMap<String, [u8; 8]>>,
    },
    SetConfigValues {
        values: HashMap<String, [u8; 8]>,
    },
    MergeConfigValues {
        values: HashMap<String, [u8; 8]>,
    },

    // ── Config tree ───────────────────────────────────────────────────────
    GetConfigTree {
        reply: oneshot::Sender<Option<NodeConfigTree>>,
    },
    SetConfigTree {
        tree: NodeConfigTree,
    },
    UpdateConfigTree {
        update_fn: Box<dyn FnOnce(&mut NodeConfigTree) + Send>,
    },

    // ── Status updates (external — from EventRouter or commands) ──────────
    UpdateAlias {
        alias: u16,
    },
    UpdateSnip {
        snip_data: Option<SNIPData>,
        status: SNIPStatus,
    },
    UpdatePip {
        pip_flags: Option<ProtocolFlags>,
        status: PIPStatus,
    },
    UpdateConnectionStatus {
        status: ConnectionStatus,
        last_verified: Option<chrono::DateTime<chrono::Utc>>,
    },

    // ── Internal: spawned query completion (wakes parked callers) ─────────
    SnipQueryDone {
        result: SnipResult,
    },
    PipQueryDone {
        result: PipResult,
    },

    // ── Lifecycle ─────────────────────────────────────────────────────────
    NodeReinitialised,
    Shutdown,
}

// ── NodeProxy (actor) ────────────────────────────────────────────────────────

/// Per-node actor owning all state for a single discovered LCC node.
pub struct NodeProxy {
    // Identity
    node_id: NodeID,
    alias: u16,

    // Transport
    transport_handle: TransportHandle,
    our_alias: u16,

    // Cached protocol state (was in AppState.nodes / DiscoveredNode)
    snip: Option<SNIPData>,
    snip_status: SNIPStatus,
    pip_flags: Option<ProtocolFlags>,
    pip_status: PIPStatus,
    connection_status: ConnectionStatus,
    last_seen: chrono::DateTime<chrono::Utc>,
    last_verified: Option<chrono::DateTime<chrono::Utc>>,

    // CDI (was in CDI_PARSE_CACHE + node.cdi)
    cdi_data: Option<CdiData>,
    cdi_parsed: Option<lcc_rs::cdi::Cdi>,

    // Config values (was in AppState.config_value_cache)
    config_values: HashMap<String, [u8; 8]>,

    // Config tree (was in AppState.node_trees)
    config_tree: Option<NodeConfigTree>,

    // In-flight SNIP query: Some(waiters) while a network query is running
    snip_waiters: Option<Vec<oneshot::Sender<SnipResult>>>,
    // In-flight PIP query
    pip_waiters: Option<Vec<oneshot::Sender<PipResult>>>,
}

impl NodeProxy {
    /// Spawn the actor, returning a handle for communication.
    pub fn spawn(
        node_id: NodeID,
        alias: u16,
        transport_handle: TransportHandle,
        our_alias: u16,
    ) -> NodeProxyHandle {
        let (tx, rx) = mpsc::channel(64);
        let mailbox_tx = tx.clone();

        let proxy = NodeProxy {
            node_id,
            alias,
            transport_handle,
            our_alias,
            snip: None,
            snip_status: SNIPStatus::Unknown,
            pip_flags: None,
            pip_status: PIPStatus::Unknown,
            connection_status: ConnectionStatus::Connected,
            last_seen: chrono::Utc::now(),
            last_verified: None,
            cdi_data: None,
            cdi_parsed: None,
            config_values: HashMap::new(),
            config_tree: None,
            snip_waiters: None,
            pip_waiters: None,
        };

        let task = tokio::spawn(proxy.run(rx, mailbox_tx));

        NodeProxyHandle {
            node_id,
            alias,
            tx,
            _task: Arc::new(task),
        }
    }

    /// Main actor loop.
    async fn run(
        mut self,
        mut rx: mpsc::Receiver<ProxyMessage>,
        mailbox_tx: mpsc::Sender<ProxyMessage>,
    ) {
        while let Some(msg) = rx.recv().await {
            match msg {
                // ── Queries with dedup ────────────────────────────────────
                ProxyMessage::QuerySnip { reply } => {
                    self.handle_query_snip(reply, &mailbox_tx);
                }
                ProxyMessage::QueryPip { reply } => {
                    self.handle_query_pip(reply, &mailbox_tx);
                }

                // ── Spawned query completions ────────────────────────────
                ProxyMessage::SnipQueryDone { result } => {
                    // Update cached state
                    if let Ok((ref data, status)) = result {
                        self.snip = data.clone();
                        self.snip_status = status;
                        self.last_seen = chrono::Utc::now();
                    }
                    // Wake all parked callers
                    if let Some(waiters) = self.snip_waiters.take() {
                        for w in waiters {
                            let _ = w.send(result.clone());
                        }
                    }
                }
                ProxyMessage::PipQueryDone { result } => {
                    if let Ok((ref data, status)) = result {
                        self.pip_flags = data.clone();
                        self.pip_status = status;
                    }
                    if let Some(waiters) = self.pip_waiters.take() {
                        for w in waiters {
                            let _ = w.send(result.clone());
                        }
                    }
                }

                // ── Snapshot ─────────────────────────────────────────────
                ProxyMessage::GetSnapshot { reply } => {
                    let _ = reply.send(self.snapshot());
                }

                // ── CDI data ─────────────────────────────────────────────
                ProxyMessage::GetCdiData { reply } => {
                    let _ = reply.send(self.cdi_data.clone());
                }
                ProxyMessage::SetCdiData { cdi_data } => {
                    self.cdi_data = Some(cdi_data);
                }
                ProxyMessage::GetCdiParsed { reply } => {
                    let _ = reply.send(self.cdi_parsed.clone());
                }
                ProxyMessage::SetCdiParsed { cdi } => {
                    self.cdi_parsed = Some(cdi);
                }

                // ── Config values ────────────────────────────────────────
                ProxyMessage::GetConfigValues { reply } => {
                    let _ = reply.send(self.config_values.clone());
                }
                ProxyMessage::SetConfigValues { values } => {
                    self.config_values = values;
                }
                ProxyMessage::MergeConfigValues { values } => {
                    self.config_values.extend(values);
                }

                // ── Config tree ──────────────────────────────────────────
                ProxyMessage::GetConfigTree { reply } => {
                    let _ = reply.send(self.config_tree.clone());
                }
                ProxyMessage::SetConfigTree { tree } => {
                    self.config_tree = Some(tree);
                }
                ProxyMessage::UpdateConfigTree { update_fn } => {
                    if let Some(ref mut tree) = self.config_tree {
                        update_fn(tree);
                    }
                }

                // ── External state updates ───────────────────────────────
                ProxyMessage::UpdateAlias { alias } => {
                    self.alias = alias;
                }
                ProxyMessage::UpdateSnip { snip_data, status } => {
                    self.snip = snip_data;
                    self.snip_status = status;
                    self.last_seen = chrono::Utc::now();
                }
                ProxyMessage::UpdatePip { pip_flags, status } => {
                    self.pip_flags = pip_flags;
                    self.pip_status = status;
                }
                ProxyMessage::UpdateConnectionStatus { status, last_verified } => {
                    self.connection_status = status;
                    if let Some(lv) = last_verified {
                        self.last_verified = Some(lv);
                        self.last_seen = lv;
                    }
                }

                // ── Lifecycle ────────────────────────────────────────────
                ProxyMessage::NodeReinitialised => {
                    // Volatile protocol state — clear on reinit so it gets
                    // re-queried from the (possibly rebooted) node.
                    self.snip = None;
                    self.snip_status = SNIPStatus::Unknown;
                    self.pip_flags = None;
                    self.pip_status = PIPStatus::Unknown;
                    // CDI XML, config values, and config tree are all backed
                    // by NV memory — they survive a node reboot and stay
                    // valid across reinit.  Clearing them here would force
                    // set_modified_value / get_node_tree to rebuild from CDI
                    // without values, zeroing every field the user hasn't
                    // just edited.
                }
                ProxyMessage::Shutdown => {
                    break;
                }
            }
        }
    }

    /// Build a DiscoveredNode snapshot from current state.
    fn snapshot(&self) -> DiscoveredNode {
        DiscoveredNode {
            node_id: self.node_id,
            alias: NodeAlias::new(self.alias).unwrap_or_else(|_| NodeAlias::new(1).unwrap()),
            snip_data: self.snip.clone(),
            snip_status: self.snip_status,
            connection_status: self.connection_status,
            last_verified: self.last_verified,
            last_seen: self.last_seen,
            cdi: self.cdi_data.clone(),
            pip_flags: self.pip_flags.clone(),
            pip_status: self.pip_status,
        }
    }

    /// Handle SNIP query: return cached data, or perform network query with dedup.
    fn handle_query_snip(
        &mut self,
        reply: oneshot::Sender<SnipResult>,
        mailbox_tx: &mpsc::Sender<ProxyMessage>,
    ) {
        // Cache hit — already have a definitive answer
        if self.snip_status == SNIPStatus::Complete || self.snip_status == SNIPStatus::Timeout {
            let _ = reply.send(Ok((self.snip.clone(), self.snip_status)));
            return;
        }

        // Already in flight — park this caller alongside existing waiters
        if let Some(ref mut waiters) = self.snip_waiters {
            waiters.push(reply);
            return;
        }

        // First request — spawn the network query
        self.snip_waiters = Some(vec![reply]);
        let handle = self.transport_handle.clone();
        let our_alias = self.our_alias;
        let dest_alias = self.alias;
        let tx = mailbox_tx.clone();

        tokio::spawn(async move {
            let semaphore = Arc::new(Semaphore::new(1));
            let result = lcc_rs::query_snip(&handle, our_alias, dest_alias, semaphore)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(ProxyMessage::SnipQueryDone { result }).await;
        });
    }

    /// Handle PIP query: return cached data, or perform network query with dedup.
    fn handle_query_pip(
        &mut self,
        reply: oneshot::Sender<PipResult>,
        mailbox_tx: &mpsc::Sender<ProxyMessage>,
    ) {
        // Cache hit
        if self.pip_status == PIPStatus::Complete || self.pip_status == PIPStatus::Timeout {
            let _ = reply.send(Ok((self.pip_flags.clone(), self.pip_status)));
            return;
        }

        // Already in flight — park this caller
        if let Some(ref mut waiters) = self.pip_waiters {
            waiters.push(reply);
            return;
        }

        // First request — spawn the network query
        self.pip_waiters = Some(vec![reply]);
        let handle = self.transport_handle.clone();
        let our_alias = self.our_alias;
        let dest_alias = self.alias;
        let tx = mailbox_tx.clone();

        tokio::spawn(async move {
            let semaphore = Arc::new(Semaphore::new(1));
            let result = lcc_rs::pip::query_pip(&handle, our_alias, dest_alias, semaphore)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(ProxyMessage::PipQueryDone { result }).await;
        });
    }
}

// ── NodeProxyHandle ──────────────────────────────────────────────────────────

/// Cloneable handle for communicating with a NodeProxy actor.
#[derive(Clone)]
pub struct NodeProxyHandle {
    pub node_id: NodeID,
    pub alias: u16,
    tx: mpsc::Sender<ProxyMessage>,
    _task: Arc<tokio::task::JoinHandle<()>>,
}

impl NodeProxyHandle {
    /// Query SNIP data (cached or from network). Deduplicates concurrent requests.
    pub async fn query_snip(&self) -> Result<(Option<SNIPData>, SNIPStatus), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ProxyMessage::QuerySnip { reply: reply_tx })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?;
        reply_rx
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?
    }

    /// Query PIP flags (cached or from network). Deduplicates concurrent requests.
    pub async fn query_pip(&self) -> Result<(Option<ProtocolFlags>, PIPStatus), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ProxyMessage::QueryPip { reply: reply_tx })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?;
        reply_rx
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?
    }

    /// Get a DiscoveredNode snapshot from current cached state.
    pub async fn get_snapshot(&self) -> Result<DiscoveredNode, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ProxyMessage::GetSnapshot { reply: reply_tx })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?;
        reply_rx
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Get cached CDI data (raw XML).
    pub async fn get_cdi_data(&self) -> Result<Option<CdiData>, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ProxyMessage::GetCdiData { reply: reply_tx })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?;
        reply_rx
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Store CDI data in the proxy.
    pub async fn set_cdi_data(&self, cdi_data: CdiData) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::SetCdiData { cdi_data })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Get cached parsed CDI.
    pub async fn get_cdi_parsed(&self) -> Result<Option<lcc_rs::cdi::Cdi>, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ProxyMessage::GetCdiParsed { reply: reply_tx })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?;
        reply_rx
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Store parsed CDI in the proxy.
    pub async fn set_cdi_parsed(&self, cdi: lcc_rs::cdi::Cdi) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::SetCdiParsed { cdi })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Get cached config values (event ID bytes by element path).
    pub async fn get_config_values(&self) -> Result<HashMap<String, [u8; 8]>, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ProxyMessage::GetConfigValues { reply: reply_tx })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?;
        reply_rx
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Replace all config values in the proxy.
    pub async fn set_config_values(&self, values: HashMap<String, [u8; 8]>) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::SetConfigValues { values })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Merge additional config values into the proxy's cache.
    pub async fn merge_config_values(
        &self,
        values: HashMap<String, [u8; 8]>,
    ) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::MergeConfigValues { values })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Get cached config tree.
    pub async fn get_config_tree(&self) -> Result<Option<NodeConfigTree>, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ProxyMessage::GetConfigTree { reply: reply_tx })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())?;
        reply_rx
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Store a config tree in the proxy.
    pub async fn set_config_tree(&self, tree: NodeConfigTree) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::SetConfigTree { tree })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Apply a mutation to the config tree inside the proxy.
    pub async fn update_config_tree(
        &self,
        update_fn: impl FnOnce(&mut NodeConfigTree) + Send + 'static,
    ) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::UpdateConfigTree {
                update_fn: Box::new(update_fn),
            })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Update SNIP data in the proxy's cache (external update, e.g. from EventRouter).
    pub async fn update_snip(
        &self,
        snip_data: Option<SNIPData>,
        status: SNIPStatus,
    ) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::UpdateSnip { snip_data, status })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Update the destination alias used for subsequent addressed traffic.
    pub async fn update_alias(&self, alias: u16) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::UpdateAlias { alias })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Update PIP flags in the proxy's cache.
    pub async fn update_pip(
        &self,
        pip_flags: Option<ProtocolFlags>,
        status: PIPStatus,
    ) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::UpdatePip { pip_flags, status })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Update connection status.
    pub async fn update_connection_status(
        &self,
        status: ConnectionStatus,
        last_verified: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::UpdateConnectionStatus {
                status,
                last_verified,
            })
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Notify the proxy that the node has reinitialised (clears volatile cache).
    pub async fn node_reinitialised(&self) -> Result<(), String> {
        self.tx
            .send(ProxyMessage::NodeReinitialised)
            .await
            .map_err(|_| "NodeProxy actor stopped".to_string())
    }

    /// Shut down the proxy actor.
    pub async fn shutdown(&self) {
        let _ = self.tx.send(ProxyMessage::Shutdown).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_tree::{
        ConfigNode, ConfigValue, LeafNode, LeafType, NodeConfigTree, SegmentNode,
    };

    /// Create a dummy TransportHandle that doesn't connect to anything.
    fn dummy_transport_handle() -> TransportHandle {
        let (tx, _rx) = mpsc::channel(1);
        let (all_tx, _) = tokio::sync::broadcast::channel(1);
        let mti_senders = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        TransportHandle::from_parts(tx, all_tx, mti_senders)
    }

    /// Build a minimal tree with one event-ID leaf carrying a known value.
    fn tree_with_event_id(hex: &str, bytes: [u8; 8]) -> NodeConfigTree {
        NodeConfigTree {
            node_id: "05.02.01.02.03.00".into(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            segments: vec![SegmentNode {
                name: "Configuration".into(),
                description: None,
                origin: 0,
                space: 0xFD,
                children: vec![ConfigNode::Leaf(LeafNode {
                    name: "Event ID".into(),
                    description: None,
                    element_type: LeafType::EventId,
                    address: 100,
                    size: 8,
                    space: 0xFD,
                    path: vec!["seg:0".into(), "elem:0".into()],
                    value: Some(ConfigValue::EventId {
                        bytes,
                        hex: hex.into(),
                    }),
                    event_role: None,
                    constraints: None,
                    button_text: None,
                    dialog_text: None,
                    action_value: 0,
                    hint_slider: None,
                    hint_radio: false,
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
                })],
            }],
        }
    }

    /// Regression: NodeReinitialised must preserve config_tree and config_values.
    ///
    /// Previously the handler cleared both, which meant the next
    /// set_modified_value / get_node_tree rebuilt from CDI without values,
    /// zeroing every field the user hadn't just edited.
    #[tokio::test]
    async fn reinitialised_preserves_config_tree_and_values() {
        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]);
        let handle = NodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001);

        // Populate config values
        let mut vals = HashMap::new();
        vals.insert(
            "seg:0/elem:0".into(),
            [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xFF],
        );
        handle.set_config_values(vals.clone()).await.unwrap();

        // Populate config tree
        let tree = tree_with_event_id(
            "05.01.01.01.22.00.00.FF",
            [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xFF],
        );
        handle.set_config_tree(tree.clone()).await.unwrap();

        // Simulate node reinitialization (e.g. after Update Complete)
        handle.node_reinitialised().await.unwrap();

        // Config tree must survive
        let after_tree = handle.get_config_tree().await.unwrap();
        assert!(
            after_tree.is_some(),
            "config_tree must not be cleared on reinit"
        );
        let after_tree = after_tree.unwrap();
        assert_eq!(after_tree.segments.len(), 1);
        if let ConfigNode::Leaf(ref leaf) = after_tree.segments[0].children[0] {
            assert_eq!(
                leaf.value,
                Some(ConfigValue::EventId {
                    bytes: [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xFF],
                    hex: "05.01.01.01.22.00.00.FF".into(),
                }),
                "leaf value must survive reinit"
            );
        } else {
            panic!("expected a leaf node");
        }

        // Config values must survive
        let after_vals = handle.get_config_values().await.unwrap();
        assert_eq!(after_vals, vals, "config_values must not be cleared on reinit");

        // SNIP/PIP volatile state must be cleared (correct behavior)
        let snapshot = handle.get_snapshot().await.unwrap();
        assert_eq!(snapshot.snip_status, SNIPStatus::Unknown);
        assert_eq!(snapshot.pip_status, PIPStatus::Unknown);

        handle.shutdown().await;
    }

    /// MergeConfigValues must extend existing entries, not replace the map.
    ///
    /// The event-router and write pipeline both rely on additive merges to
    /// update individual event-ID values without losing unrelated entries.
    #[tokio::test]
    async fn merge_config_values_extends_existing() {
        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]);
        let handle = NodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001);

        let mut initial = HashMap::new();
        initial.insert("seg:0/elem:0".into(), [1u8; 8]);
        initial.insert("seg:0/elem:1".into(), [2u8; 8]);
        handle.set_config_values(initial).await.unwrap();

        let mut merge = HashMap::new();
        merge.insert("seg:0/elem:1".into(), [3u8; 8]); // overwrite
        merge.insert("seg:0/elem:2".into(), [4u8; 8]); // new entry
        handle.merge_config_values(merge).await.unwrap();

        let result = handle.get_config_values().await.unwrap();
        assert_eq!(result.len(), 3, "merge should add new keys");
        assert_eq!(result["seg:0/elem:0"], [1u8; 8], "untouched key preserved");
        assert_eq!(result["seg:0/elem:1"], [3u8; 8], "overlapping key updated");
        assert_eq!(result["seg:0/elem:2"], [4u8; 8], "new key added");

        handle.shutdown().await;
    }

    /// UpdateConfigTree must be a safe no-op when no tree has been set.
    ///
    /// Callers (e.g. set_modified_value) may send UpdateConfigTree
    /// optimistically; the actor must not panic if config_tree is None.
    #[tokio::test]
    async fn update_config_tree_noop_without_tree() {
        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]);
        let handle = NodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001);

        // No tree set — update should silently complete
        handle
            .update_config_tree(|tree| {
                tree.node_id = "should-not-be-reachable".into();
            })
            .await
            .unwrap();

        let tree = handle.get_config_tree().await.unwrap();
        assert!(tree.is_none(), "tree should still be None");

        handle.shutdown().await;
    }

    /// UpdateConfigTree applies the mutation when a tree exists.
    #[tokio::test]
    async fn update_config_tree_applies_mutation() {
        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]);
        let handle = NodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001);

        let tree = tree_with_event_id(
            "05.01.01.01.22.00.00.FF",
            [0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xFF],
        );
        handle.set_config_tree(tree).await.unwrap();

        handle
            .update_config_tree(|tree| {
                tree.segments[0].name = "Mutated".into();
            })
            .await
            .unwrap();

        let updated = handle.get_config_tree().await.unwrap().unwrap();
        assert_eq!(updated.segments[0].name, "Mutated");

        handle.shutdown().await;
    }

    /// UpdateAlias must be reflected in subsequent snapshots.
    ///
    /// The event router calls update_alias when a node's CAN alias changes
    /// after reconnection; downstream code reads the alias from snapshots.
    #[tokio::test]
    async fn update_alias_reflected_in_snapshot() {
        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]);
        let handle = NodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001);

        let snap_before = handle.get_snapshot().await.unwrap();
        assert_eq!(snap_before.alias.value(), 0x100);

        handle.update_alias(0x200).await.unwrap();

        let snap_after = handle.get_snapshot().await.unwrap();
        assert_eq!(snap_after.alias.value(), 0x200);

        handle.shutdown().await;
    }
}
