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
                    // Volatile state — clear on reinit
                    self.snip = None;
                    self.snip_status = SNIPStatus::Unknown;
                    self.pip_flags = None;
                    self.pip_status = PIPStatus::Unknown;
                    self.config_values.clear();
                    self.config_tree = None;
                    // CDI XML is stable across reinit — keep it
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
