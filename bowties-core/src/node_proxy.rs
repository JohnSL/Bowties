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
    QueryOutcome, SNIPData, SNIPStatus, TransportHandle,
};
use lcc_rs::peer_session_registry::PeerSessionRegistry;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Semaphore};

use crate::node_tree::NodeConfigTree;

// ── Types ────────────────────────────────────────────────────────────────────

type SnipResult = Result<(Option<SNIPData>, SNIPStatus), String>;
type PipResult = Result<(Option<ProtocolFlags>, PIPStatus), String>;

/// Whether a cached query status is a final answer the proxy may serve
/// without asking the peer session again. Mirrors `PeerSession`'s policy
/// (`QueryOutcome::Complete` / `NotSupported` are final; `Timeout` is not),
/// so the proxy never holds a stricter finality rule than the session.
trait CachedQueryStatus: Copy {
    fn is_final(self) -> bool;
}

impl CachedQueryStatus for SNIPStatus {
    fn is_final(self) -> bool {
        matches!(self, SNIPStatus::Complete | SNIPStatus::NotSupported)
    }
}

impl CachedQueryStatus for PIPStatus {
    fn is_final(self) -> bool {
        matches!(self, PIPStatus::Complete | PIPStatus::NotSupported)
    }
}

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

    // Optional peer-session registry (S2 introduction, ADR-0016 D2).
    // When `Some`, protocol calls fetch a `PeerSessionHandle` per call and
    // delegate; the proxy retains no cached handle. When `None` (tests, or
    // pre-S2 code paths before the registry is wired), the proxy falls back
    // to its in-actor coalescing + free-function shim.
    sessions: Option<Arc<PeerSessionRegistry>>,

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
    /// Spawn the actor without a peer-session registry (fallback in-actor
    /// coalescing path). Preserved so existing test code and pre-S2 wiring
    /// continues to work.
    pub fn spawn(
        node_id: NodeID,
        alias: u16,
        transport_handle: TransportHandle,
        our_alias: u16,
    ) -> LiveNodeProxyHandle {
        Self::spawn_with_sessions(node_id, alias, transport_handle, our_alias, None)
    }

    /// Spawn the actor with the peer-session registry supplied (S2+ path).
    ///
    /// When `sessions` is `Some`, `query_snip`/`query_pip` fetch a
    /// `PeerSessionHandle` from the registry per call (ADR-0016 D2). No
    /// cached handle is held on the proxy — the registry is the sole source
    /// of truth.
    pub fn spawn_with_sessions(
        node_id: NodeID,
        alias: u16,
        transport_handle: TransportHandle,
        our_alias: u16,
        sessions: Option<Arc<PeerSessionRegistry>>,
    ) -> LiveNodeProxyHandle {
        let (tx, rx) = mpsc::channel(64);
        let mailbox_tx = tx.clone();

        let proxy = LiveNodeProxy {
            node_id,
            alias,
            transport_handle,
            our_alias,
            sessions,
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
                    // get_node_tree to rebuild from CDI without values.
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
        if self.snip_status.is_final() {
            let _ = reply.send(Ok((self.snip.clone(), self.snip_status)));
            return;
        }

        // Already in flight — park this caller alongside existing waiters
        if let Some(ref mut waiters) = self.snip_waiters {
            waiters.push(reply);
            return;
        }

        // First request — spawn the network query.
        self.snip_waiters = Some(vec![reply]);
        let dest_alias = self.alias;
        let tx = mailbox_tx.clone();

        // Session path (ADR-0016 D2): fetch handle from registry per call.
        // If no session yet (registry hasn't observed the peer's identity
        // frame), fall back to the free-function shim.
        if let Some(sessions) = self.sessions.clone() {
            let node_id = self.node_id;
            let handle = self.transport_handle.clone();
            let our_alias = self.our_alias;
            tokio::spawn(async move {
                let result = if let Some(session) = sessions.get(node_id).await {
                    match session.query_snip().await {
                        Ok(QueryOutcome::Complete(data)) => Ok((Some(data), SNIPStatus::Complete)),
                        Ok(QueryOutcome::NotSupported) => Ok((None, SNIPStatus::NotSupported)),
                        Ok(QueryOutcome::Timeout) => Ok((None, SNIPStatus::Timeout)),
                        Err(e) => Err(e.to_string()),
                    }
                } else {
                    let semaphore = Arc::new(Semaphore::new(1));
                    lcc_rs::query_snip(&handle, our_alias, dest_alias, semaphore)
                        .await
                        .map_err(|e| e.to_string())
                };
                let _ = tx.send(ProxyMessage::SnipQueryDone { result }).await;
            });
            return;
        }

        // Fallback path (no registry configured): use the free-function shim.
        let handle = self.transport_handle.clone();
        let our_alias = self.our_alias;
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
        // Cache hit — already have a definitive answer
        if self.pip_status.is_final() {
            let _ = reply.send(Ok((self.pip_flags.clone(), self.pip_status)));
            return;
        }

        // Already in flight — park this caller
        if let Some(ref mut waiters) = self.pip_waiters {
            waiters.push(reply);
            return;
        }

        self.pip_waiters = Some(vec![reply]);
        let dest_alias = self.alias;
        let tx = mailbox_tx.clone();

        if let Some(sessions) = self.sessions.clone() {
            let node_id = self.node_id;
            let handle = self.transport_handle.clone();
            let our_alias = self.our_alias;
            tokio::spawn(async move {
                let result = if let Some(session) = sessions.get(node_id).await {
                    match session.query_pip().await {
                        Ok(QueryOutcome::Complete(flags)) => Ok((Some(flags), PIPStatus::Complete)),
                        Ok(QueryOutcome::NotSupported) => Ok((None, PIPStatus::NotSupported)),
                        Ok(QueryOutcome::Timeout) => Ok((None, PIPStatus::Timeout)),
                        Err(e) => Err(e.to_string()),
                    }
                } else {
                    let semaphore = Arc::new(Semaphore::new(1));
                    lcc_rs::pip::query_pip(&handle, our_alias, dest_alias, semaphore)
                        .await
                        .map_err(|e| e.to_string())
                };
                let _ = tx.send(ProxyMessage::PipQueryDone { result }).await;
            });
            return;
        }

        let handle = self.transport_handle.clone();
        let our_alias = self.our_alias;
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
    use std::collections::HashMap;

    /// Create a dummy TransportHandle that doesn't connect to anything.
    fn dummy_transport_handle() -> TransportHandle {
        let (tx, _rx) = mpsc::channel(1);
        let (all_tx, _) = tokio::sync::broadcast::channel(1);
        let mti_senders = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        TransportHandle::from_parts(tx, all_tx, mti_senders)
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

    // ── Bug #28: proxy SNIP/PIP cache finality ────────────────────────────

    use lcc_rs::transport::mock::MockTransport;
    use lcc_rs::TransportActor;
    use std::time::Duration;

    const PEER_ALIAS: u16 = 0x3AE;
    const OUR_ALIAS: u16 = 0x825;
    const PEER_ID: [u8; 6] = [0x02, 0x01, 0x57, 0x00, 0x00, 0x28];

    fn frame(mti: u32, source: u16, data: &[u8]) -> String {
        let hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
        format!(":X{:08X}N{};", (mti << 12) | source as u32, hex)
    }

    /// Outbound addressed requests with `mti` from us to the peer.
    fn count_requests(probe: &MockTransport, mti: u32) -> usize {
        let header = format!(":X{:08X}N", (mti << 12) | OUR_ALIAS as u32);
        let body = format!("{:02X}{:02X}", (PEER_ALIAS >> 8) as u8 & 0x0F, PEER_ALIAS & 0xFF);
        probe
            .get_sent_frames()
            .iter()
            .filter(|f| f.starts_with(&header) && f.contains(&body))
            .count()
    }

    const SNIP_REQUEST_MTI: u32 = 0x19DE8;
    const PIP_REQUEST_MTI: u32 = 0x19828;

    /// Multi-frame SNIP reply addressed to us.
    fn snip_reply() -> Vec<String> {
        let (hi, lo) = (((OUR_ALIAS >> 8) & 0x0F) as u8, (OUR_ALIAS & 0xFF) as u8);
        let payload = [0x04, b'A', 0, b'W', 0, b'1', 0, b'2', 0, 0x02, b'N', 0, b'D', 0];
        let chunks: Vec<&[u8]> = payload.chunks(6).collect();
        let n = chunks.len();
        chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let flag: u8 = if n == 1 { 0 } else if i == 0 { 1 } else if i == n - 1 { 2 } else { 3 };
                let mut d = vec![(flag << 4) | hi, lo];
                d.extend_from_slice(c);
                frame(0x19A08, PEER_ALIAS, &d)
            })
            .collect()
    }

    fn pip_reply() -> Vec<String> {
        vec![frame(
            0x19668,
            PEER_ALIAS,
            &[((OUR_ALIAS >> 8) & 0x0F) as u8, (OUR_ALIAS & 0xFF) as u8, 0x80, 0x00],
        )]
    }

    /// Queue `reply` frames once `wanted` requests are on the wire.
    fn reply_after_requests(
        probe: MockTransport,
        mti: u32,
        wanted: usize,
        reply: Vec<String>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut probe = probe;
            loop {
                if count_requests(&probe, mti) >= wanted {
                    for f in reply {
                        probe.add_receive_frame(f);
                    }
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
    }

    /// Live proxy backed by a real registry/session over a mock transport.
    async fn proxy_with_session() -> (NodeProxyHandle, MockTransport, TransportActor) {
        let mut transport = MockTransport::new();
        let probe = transport.clone();
        transport.add_receive_frame(frame(0x19170, PEER_ALIAS, &PEER_ID));
        let actor = TransportActor::new(Box::new(transport));
        let th = actor.handle();
        let registry = PeerSessionRegistry::new(th.clone(), OUR_ALIAS);
        let node_id = NodeID::new(PEER_ID);
        for _ in 0..50 {
            if registry.get(node_id).await.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(registry.get(node_id).await.is_some(), "session must spawn on VNI");
        let proxy = NodeProxyHandle::Live(LiveNodeProxy::spawn_with_sessions(
            node_id, PEER_ALIAS, th, OUR_ALIAS, Some(registry),
        ));
        (proxy, probe, actor)
    }

    /// After a Timeout the proxy must not answer from its cache: the next
    /// query reaches the session again, and a Complete answer is then cached.
    #[tokio::test]
    async fn snip_query_after_timeout_reaches_session_again() {
        let (proxy, probe, mut actor) = proxy_with_session().await;
        proxy.update_snip(None, SNIPStatus::Timeout).await.unwrap();
        let responder = reply_after_requests(probe.clone(), SNIP_REQUEST_MTI, 1, snip_reply());

        let (data, status) = tokio::time::timeout(Duration::from_secs(5), proxy.query_snip())
            .await
            .expect("query_snip finished")
            .unwrap();
        responder.abort();
        assert_eq!(status, SNIPStatus::Complete);
        assert!(data.is_some());
        assert_eq!(count_requests(&probe, SNIP_REQUEST_MTI), 1);

        // Complete is now final: served from cache, no new request.
        let (_, status) = proxy.query_snip().await.unwrap();
        assert_eq!(status, SNIPStatus::Complete);
        assert_eq!(count_requests(&probe, SNIP_REQUEST_MTI), 1);

        proxy.shutdown().await;
        actor.shutdown().await;
    }

    #[tokio::test]
    async fn pip_query_after_timeout_reaches_session_again() {
        let (proxy, probe, mut actor) = proxy_with_session().await;
        proxy.update_pip(None, PIPStatus::Timeout).await.unwrap();
        let responder = reply_after_requests(probe.clone(), PIP_REQUEST_MTI, 1, pip_reply());

        let (flags, status) = tokio::time::timeout(Duration::from_secs(5), proxy.query_pip())
            .await
            .expect("query_pip finished")
            .unwrap();
        responder.abort();
        assert_eq!(status, PIPStatus::Complete);
        assert!(flags.is_some());
        assert_eq!(count_requests(&probe, PIP_REQUEST_MTI), 1);

        let (_, status) = proxy.query_pip().await.unwrap();
        assert_eq!(status, PIPStatus::Complete);
        assert_eq!(count_requests(&probe, PIP_REQUEST_MTI), 1);

        proxy.shutdown().await;
        actor.shutdown().await;
    }

    /// Complete and NotSupported are final: served from the proxy cache with
    /// no new request on the wire.
    #[tokio::test]
    async fn final_snip_and_pip_statuses_are_served_from_cache() {
        let (proxy, probe, mut actor) = proxy_with_session().await;

        for status in [SNIPStatus::Complete, SNIPStatus::NotSupported] {
            proxy.update_snip(None, status).await.unwrap();
            let (_, got) = proxy.query_snip().await.unwrap();
            assert_eq!(got, status);
        }
        for status in [PIPStatus::Complete, PIPStatus::NotSupported] {
            proxy.update_pip(None, status).await.unwrap();
            let (_, got) = proxy.query_pip().await.unwrap();
            assert_eq!(got, status);
        }
        assert_eq!(count_requests(&probe, SNIP_REQUEST_MTI), 0);
        assert_eq!(count_requests(&probe, PIP_REQUEST_MTI), 0);

        proxy.shutdown().await;
        actor.shutdown().await;
    }
}
