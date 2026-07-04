//! Per-node actor (LiveNodeProxy) that owns all state for a single discovered LCC node,
//! and a polymorphic [`NodeProxyHandle`] enum for uniform dispatch over live
//! and synthesized (placeholder) nodes.
//!
//! Live proxies run as `tokio::spawn` tasks with an mpsc mailbox.  Callers
//! interact through a cloneable [`NodeProxyHandle`] that sends messages and
//! receives replies via oneshot channels.  Synthesized proxies hold
//! factory-produced state directly (no actor, no mailbox).

use lcc_rs::{
    CdiData, ConnectionStatus, DiscoveredNode, NodeAlias, NodeID, PIPStatus, ProtocolFlags,
    SNIPData, SNIPStatus, TransportHandle,
};
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

// ── LiveNodeProxy (actor) ────────────────────────────────────────────────────

/// Per-node actor owning all state for a single discovered LCC node.
pub struct LiveNodeProxy {
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



    // In-flight SNIP query: Some(waiters) while a network query is running
    snip_waiters: Option<Vec<oneshot::Sender<SnipResult>>>,
    // In-flight PIP query
    pip_waiters: Option<Vec<oneshot::Sender<PipResult>>>,
}

impl LiveNodeProxy {
    /// Spawn the actor, returning a handle for communication.
    pub fn spawn(
        node_id: NodeID,
        alias: u16,
        transport_handle: TransportHandle,
        our_alias: u16,
    ) -> LiveNodeProxyHandle {
        let (tx, rx) = mpsc::channel(64);
        let mailbox_tx = tx.clone();

        let proxy = LiveNodeProxy {
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
            snip_waiters: None,
            pip_waiters: None,
        };

        let task = tokio::spawn(proxy.run(rx, mailbox_tx));

        LiveNodeProxyHandle {
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

                // ── Config tree (removed — lives in LayoutState, ADR-0015) ──
                ProxyMessage::GetConfigTree { reply } => {
                    let _ = reply.send(None);
                }
                ProxyMessage::SetConfigTree { tree: _ } => {
                    // No-op: tree ownership moved to LayoutState.
                }
                ProxyMessage::UpdateConfigTree { update_fn: _ } => {
                    // No-op: tree ownership moved to LayoutState.
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
    ///
    /// `cdi` is always `None` on a `LiveNodeProxy` snapshot: persistent CDI
    /// state lives in `LayoutState` (ADR-0015). Callers needing CDI bytes
    /// must consult `LayoutState::cdi_xml`. Synthesized placeholders still
    /// carry CDI in their own struct field (see ADR-0009's 2026-06-28
    /// amendment).
    fn snapshot(&self) -> DiscoveredNode {
        DiscoveredNode {
            node_id: self.node_id,
            alias: NodeAlias::new(self.alias).unwrap_or_else(|_| NodeAlias::new(1).unwrap()),
            snip_data: self.snip.clone(),
            snip_status: self.snip_status,
            connection_status: self.connection_status,
            last_verified: self.last_verified,
            last_seen: self.last_seen,
            cdi: None,
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

// ── LiveNodeProxyHandle ──────────────────────────────────────────────────────

/// Cloneable handle for communicating with a LiveNodeProxy actor.
#[derive(Clone)]
pub struct LiveNodeProxyHandle {
    pub node_id: NodeID,
    pub alias: u16,
    tx: mpsc::Sender<ProxyMessage>,
    _task: Arc<tokio::task::JoinHandle<()>>,
}

impl LiveNodeProxyHandle {
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

// ── SynthesizedNodeProxy ─────────────────────────────────────────────────────

/// Passive state holder for a placeholder node synthesized from a bundled
/// profile.  No actor, no mailbox — the factory populates this struct and the
/// enum dispatches reads directly from its fields.
///
/// Constructed by the placeholder factory (S8.10); this slice only defines the
/// type so the `NodeProxyHandle` enum can be exhaustive.
#[derive(Clone)]
pub struct SynthesizedNodeProxy {
    /// The `"placeholder:<uuid>"` key that identifies this placeholder.
    pub node_key: String,
    /// Profile stem that sourced this placeholder's CDI (e.g. `"Mustangpeak-Engineering_TurnoutBoss"`).
    pub profile_stem: String,
    /// SNIP-equivalent identity (manufacturer, model, user name/desc from profile).
    pub snip: Option<SNIPData>,
    /// Raw CDI XML loaded from the bundled profile.
    pub cdi_data: Option<CdiData>,
    /// Parsed CDI tree.
    pub cdi_parsed: Option<lcc_rs::cdi::Cdi>,
    /// Assembled config tree (populated after tree build).
    pub config_tree: Option<NodeConfigTree>,
    /// Producer-identified events for this placeholder (typically empty).
    pub producer_identified_events: Vec<String>,
}

// ── NodeProxyHandle (polymorphic enum) ───────────────────────────────────────

/// Polymorphic handle for interacting with a node — either a live node on the
/// bus (`Live`) or a synthesized placeholder (`Synthesized`).
///
/// All read/write paths dispatch through this enum.  Callers do not branch on
/// variant; the enum's methods provide a uniform interface.
#[derive(Clone)]
pub enum NodeProxyHandle {
    /// A live node connected via CAN/TCP, backed by a `LiveNodeProxy` actor.
    Live(LiveNodeProxyHandle),
    /// A placeholder node synthesized from a bundled profile.  No actor.
    Synthesized(SynthesizedNodeProxy),
}

impl NodeProxyHandle {
    /// The node's `NodeID`.  Returns `Some` for live nodes, `None` for placeholders.
    pub fn node_id(&self) -> Option<NodeID> {
        match self {
            Self::Live(h) => Some(h.node_id),
            Self::Synthesized(_) => None,
        }
    }

    /// The CAN alias for addressed traffic.  Only meaningful for live nodes.
    pub fn alias(&self) -> u16 {
        match self {
            Self::Live(h) => h.alias,
            Self::Synthesized(_) => 0,
        }
    }

    /// The `NodeKey` string — canonical NodeID for live nodes, `"placeholder:<uuid>"` for synthesized.
    pub fn node_key(&self) -> String {
        match self {
            Self::Live(h) => h.node_id.to_canonical(),
            Self::Synthesized(s) => s.node_key.clone(),
        }
    }

    /// Query SNIP data (live nodes only).
    pub async fn query_snip(&self) -> Result<(Option<SNIPData>, SNIPStatus), String> {
        match self {
            Self::Live(h) => h.query_snip().await,
            Self::Synthesized(s) => Ok((s.snip.clone(), SNIPStatus::Complete)),
        }
    }

    /// Query PIP flags (live nodes only).
    pub async fn query_pip(&self) -> Result<(Option<ProtocolFlags>, PIPStatus), String> {
        match self {
            Self::Live(h) => h.query_pip().await,
            Self::Synthesized(_) => Ok((None, PIPStatus::Complete)),
        }
    }

    /// Get a `DiscoveredNode` snapshot from current cached state.
    pub async fn get_snapshot(&self) -> Result<DiscoveredNode, String> {
        match self {
            Self::Live(h) => h.get_snapshot().await,
            Self::Synthesized(s) => {
                // Build a minimal DiscoveredNode for the synthesized proxy.
                // The NodeID uses a zero placeholder since synthesized nodes
                // have no real bus identity.
                Ok(DiscoveredNode {
                    node_id: NodeID::new([0; 6]),
                    alias: NodeAlias::new(1).unwrap(),
                    snip_data: s.snip.clone(),
                    snip_status: SNIPStatus::Complete,
                    connection_status: ConnectionStatus::Connected,
                    last_verified: None,
                    last_seen: chrono::Utc::now(),
                    cdi: s.cdi_data.clone(),
                    pip_flags: None,
                    pip_status: PIPStatus::Complete,
                })
            }
        }
    }

    /// Get cached CDI data.
    ///
    /// Returns the synthesized proxy's bundled CDI for placeholders. Returns
    /// `Ok(None)` for live proxies — their CDI lives in `LayoutState`, not on
    /// the actor (ADR-0015; ADR-0009's 2026-06-28 amendment).
    pub async fn get_cdi_data(&self) -> Result<Option<CdiData>, String> {
        match self {
            Self::Live(_) => Ok(None),
            Self::Synthesized(s) => Ok(s.cdi_data.clone()),
        }
    }

    /// Get cached parsed CDI.
    ///
    /// Same ownership rules as [`Self::get_cdi_data`]: synthesized placeholders
    /// carry the parse; live proxies return `Ok(None)` and callers must parse
    /// from `LayoutState`'s XML.
    pub async fn get_cdi_parsed(&self) -> Result<Option<lcc_rs::cdi::Cdi>, String> {
        match self {
            Self::Live(_) => Ok(None),
            Self::Synthesized(s) => Ok(s.cdi_parsed.clone()),
        }
    }

    /// Get cached config tree.
    ///
    /// Returns `Ok(None)` for live proxies — their config tree lives in
    /// `LayoutState`, not on the actor (ADR-0015). Synthesized placeholders
    /// carry their own tree (they have no LayoutState entry until promoted).
    pub async fn get_config_tree(&self) -> Result<Option<NodeConfigTree>, String> {
        match self {
            Self::Live(_) => Ok(None),
            Self::Synthesized(s) => Ok(s.config_tree.clone()),
        }
    }

    /// Update SNIP data (external update, e.g. from EventRouter).
    pub async fn update_snip(
        &self,
        snip_data: Option<SNIPData>,
        status: SNIPStatus,
    ) -> Result<(), String> {
        match self {
            Self::Live(h) => h.update_snip(snip_data, status).await,
            Self::Synthesized(_) => Err("Cannot update SNIP on a synthesized node".into()),
        }
    }

    /// Update the destination alias used for subsequent addressed traffic.
    pub async fn update_alias(&self, alias: u16) -> Result<(), String> {
        match self {
            Self::Live(h) => h.update_alias(alias).await,
            Self::Synthesized(_) => Err("Cannot update alias on a synthesized node".into()),
        }
    }

    /// Update PIP flags.
    pub async fn update_pip(
        &self,
        pip_flags: Option<ProtocolFlags>,
        status: PIPStatus,
    ) -> Result<(), String> {
        match self {
            Self::Live(h) => h.update_pip(pip_flags, status).await,
            Self::Synthesized(_) => Err("Cannot update PIP on a synthesized node".into()),
        }
    }

    /// Update connection status.
    pub async fn update_connection_status(
        &self,
        status: ConnectionStatus,
        last_verified: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), String> {
        match self {
            Self::Live(h) => h.update_connection_status(status, last_verified).await,
            Self::Synthesized(_) => {
                Err("Cannot update connection status on a synthesized node".into())
            }
        }
    }

    /// Notify the proxy that the node has reinitialised (live nodes only).
    pub async fn node_reinitialised(&self) -> Result<(), String> {
        match self {
            Self::Live(h) => h.node_reinitialised().await,
            Self::Synthesized(_) => Ok(()), // no-op for synthesized
        }
    }

    /// Shut down the proxy actor (live nodes) or no-op (synthesized).
    pub async fn shutdown(&self) {
        match self {
            Self::Live(h) => h.shutdown().await,
            Self::Synthesized(_) => {} // no actor to shut down
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_tree::{
        ConfigNode, ConfigValue, LeafNode, LeafType, NodeConfigTree, SegmentNode,
    };
    use std::collections::HashMap;

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
            unknown_variants: Vec::new(),
            profile_applied: false,
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

    /// Live proxy no longer holds config_tree (ADR-0015) — get_config_tree
    /// returns None.
    #[tokio::test]
    async fn live_proxy_config_tree_returns_none() {
        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]);
        let handle = NodeProxyHandle::Live(LiveNodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001));

        let tree = handle.get_config_tree().await.unwrap();
        assert!(tree.is_none(), "live proxy config_tree lives in LayoutState, not on the actor");

        handle.shutdown().await;
    }

    /// UpdateAlias must be reflected in subsequent snapshots.
    ///
    /// The event router calls update_alias when a node's CAN alias changes
    /// after reconnection; downstream code reads the alias from snapshots.
    #[tokio::test]
    async fn update_alias_reflected_in_snapshot() {
        let node_id = NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]);
        let handle = NodeProxyHandle::Live(LiveNodeProxy::spawn(node_id, 0x100, dummy_transport_handle(), 0x001));

        let snap_before = handle.get_snapshot().await.unwrap();
        assert_eq!(snap_before.alias.value(), 0x100);

        handle.update_alias(0x200).await.unwrap();

        let snap_after = handle.get_snapshot().await.unwrap();
        assert_eq!(snap_after.alias.value(), 0x200);

        handle.shutdown().await;
    }
}
