//! Sole spawner and owner of `PeerSession` actors.
//!
//! The registry runs a background task that subscribes to the transport
//! inbound broadcast and qualifies frames by MTI:
//! - `VerifiedNode` (0x19170) — full NodeID payload.
//! - `InitializationComplete` (0x19100) — full NodeID payload.
//! - `AliasMapDefinition` (0x10701) — full NodeID payload.
//!
//! Frames that carry only an alias never trigger session creation.
//!
//! Repeat observations of the same NodeID dispatch `PeerCommand::AliasChanged`
//! to the existing session — never a re-spawn (ADR-0016 invariant 1 & 4).
//!
//! Concurrency: `tokio::sync::RwLock<HashMap<NodeID, PeerSessionHandle>>`.
//! The read side clones the handle and drops the guard before any `.await` on
//! it (guarded against `tokio-rwlock-self-deadlock`).

use crate::peer_session::{
    source_alias, InboundEvent, PeerCommand, PeerSession, PeerSessionHandle, INBOUND_CAPACITY,
};
use crate::protocol::mti::MTI;
use crate::transport_actor::{ReceivedMessage, TransportHandle};
use crate::types::NodeID;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

/// A registry entry bundling the peer's handle with the JoinHandles of the
/// two spawned tasks (main session `run()` + optional health-forwarder).
///
/// The tasks capture `TransportHandle` clones and cannot terminate on their
/// own after disconnect (self-referential Arc closure — see ADR-0016
/// §2026-07-14 extension). `shutdown()` aborts both to break the cycle.
struct RegistryEntry {
    handle: PeerSessionHandle,
    session_task: JoinHandle<()>,
    health_forwarder_task: Option<JoinHandle<()>>,
    /// The peer's current alias — the key under which its inbound route is
    /// registered in `RegistryInner::routes`. Updated on AMD/AMR churn so the
    /// route can be re-keyed atomically (S7 D1=A / T6).
    alias: u16,
}

/// An inbound route: the sender half of a peer session's per-peer channel plus
/// a drop counter (S7 D1=A). The demux forwards frames sourced from the peer's
/// alias into `tx`; on channel overflow it drops the frame and increments
/// `pending_drops`, coalescing the loss into an `InboundEvent::Lagged` marker
/// delivered before the next successful frame. `pending_drops` is mutated only
/// by the single demux task.
struct Route {
    tx: mpsc::Sender<InboundEvent>,
    pending_drops: Arc<AtomicU64>,
}

/// Sole owner of `PeerSession` actors, keyed by NodeID.
pub struct PeerSessionRegistry {
    inner: Arc<RegistryInner>,
    /// Spawn-watcher task handle. Held so `shutdown()` can abort it and
    /// release the `Arc<RegistryInner>` clone it captured — otherwise the
    /// watcher's transitively-held `TransportHandle` keeps the transport
    /// broadcast channel alive forever (self-referential Arc closure).
    spawn_watcher: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Inbound-demux task handle (S7 D1=A). Drains the transport broadcast and
    /// forwards each frame to the destination peer's per-peer channel keyed by
    /// source alias. Held so `shutdown()` can abort it and release its
    /// captured `Arc<RegistryInner>` (same self-referential Arc obligation as
    /// the spawn-watcher).
    demux_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

struct RegistryInner {
    sessions: RwLock<HashMap<NodeID, RegistryEntry>>,
    /// Per-peer inbound routes keyed by source alias (S7 D1=A). Read by the
    /// demux task on the hot path; written when sessions spawn / re-key /
    /// tear down. Lock ordering: acquire `sessions` before `routes` whenever
    /// both are needed (the demux only ever takes `routes`).
    routes: RwLock<HashMap<u16, Route>>,
    transport: TransportHandle,
    our_alias: RwLock<u16>,
}

impl PeerSessionRegistry {
    /// Construct a registry and start the spawn-watcher task.
    ///
    /// `our_alias` is the local NodeID's alias used as the source for
    /// addressed outbound frames from sessions.
    pub fn new(transport: TransportHandle, our_alias: u16) -> Arc<Self> {
        let inner = Arc::new(RegistryInner {
            sessions: RwLock::new(HashMap::new()),
            routes: RwLock::new(HashMap::new()),
            transport: transport.clone(),
            our_alias: RwLock::new(our_alias),
        });

        let watcher = tokio::spawn(spawn_watcher_loop(inner.clone(), transport.subscribe_all()));
        let demux = tokio::spawn(demux_loop(inner.clone(), transport.subscribe_all()));

        Arc::new(Self {
            inner,
            spawn_watcher: Mutex::new(Some(watcher)),
            demux_task: Mutex::new(Some(demux)),
        })
    }

    /// Construct an empty registry with no spawn-watcher (test-only helper).
    #[doc(hidden)]
    pub fn new_empty_for_test(transport: TransportHandle, our_alias: u16) -> Arc<Self> {
        let inner = Arc::new(RegistryInner {
            sessions: RwLock::new(HashMap::new()),
            routes: RwLock::new(HashMap::new()),
            transport,
            our_alias: RwLock::new(our_alias),
        });
        // No-op tasks so we can still hold JoinHandles in the struct.
        let watcher = tokio::spawn(async {});
        let demux = tokio::spawn(async {});
        Arc::new(Self {
            inner,
            spawn_watcher: Mutex::new(Some(watcher)),
            demux_task: Mutex::new(Some(demux)),
        })
    }

    /// Return an existing handle for `node_id`, if a session has been spawned.
    ///
    /// Cheap: read lock + clone. The read guard is dropped before returning.
    pub async fn get(&self, node_id: NodeID) -> Option<PeerSessionHandle> {
        let sessions = self.inner.sessions.read().await;
        sessions.get(&node_id).map(|e| e.handle.clone())
    }

    /// Update `our_alias` used for outbound frames from sessions spawned
    /// after this call. Sessions already spawned retain their captured value.
    pub async fn set_our_alias(&self, our_alias: u16) {
        *self.inner.our_alias.write().await = our_alias;
    }

    /// Remove a session by NodeID. Used when a peer is explicitly forgotten.
    /// Aborts the session's spawned tasks so their captured `TransportHandle`
    /// clones drop promptly.
    pub async fn remove(&self, node_id: NodeID) {
        let entry = {
            let mut sessions = self.inner.sessions.write().await;
            sessions.remove(&node_id)
        };
        if let Some(entry) = entry {
            // Drop the peer's inbound route so the demux stops forwarding.
            self.inner.routes.write().await.remove(&entry.alias);
            entry.session_task.abort();
            let _ = entry.session_task.await;
            if let Some(h) = entry.health_forwarder_task {
                h.abort();
                let _ = h.await;
            }
        }
    }

    /// Clear all sessions (e.g. transport disconnect).
    ///
    /// Aborts each entry's session + health-forwarder tasks so their
    /// captured `TransportHandle` clones drop. Callers that only want to
    /// evict entries from the map should use `remove` or `shutdown` — the
    /// clear-without-abort semantic is retained for the test-helper path
    /// but is not used in production.
    pub async fn clear(&self) {
        let entries: Vec<RegistryEntry> = {
            let mut sessions = self.inner.sessions.write().await;
            sessions.drain().map(|(_, e)| e).collect()
        };
        self.inner.routes.write().await.clear();
        for entry in entries {
            entry.session_task.abort();
            let _ = entry.session_task.await;
            if let Some(h) = entry.health_forwarder_task {
                h.abort();
                let _ = h.await;
            }
        }
    }

    /// Full-lifecycle teardown: clear the sessions map (aborting each
    /// session's tasks) and abort the spawn-watcher task so it releases its
    /// `Arc<RegistryInner>` clone.
    ///
    /// Every `PeerSession::spawn_with_tasks` spawns two tasks that hold
    /// `TransportHandle` clones (the main `run()` task and an optional
    /// health-forwarder that carries a `watch::Receiver` derived from the
    /// transport). Neither can terminate on its own after disconnect — the
    /// health-forwarder's captured `cmd_tx_health` keeps the session's
    /// command mpsc alive, and the session's `self.transport` keeps the
    /// transport's broadcast + watch alive that the health-forwarder is
    /// waiting on. Explicit abort breaks this cycle.
    ///
    /// On Windows serial reconnect, failing to abort surfaces as
    /// `COM7: Access is denied` because the writer task's OS handle never
    /// closes. Callers must invoke this before dropping the registry when
    /// the underlying transport is being torn down.
    pub async fn shutdown(&self) {
        // Drain the sessions map first so any newly-arriving frames don't
        // find a live entry to dispatch to.
        let entries: Vec<RegistryEntry> = {
            let mut sessions = self.inner.sessions.write().await;
            sessions.drain().map(|(_, e)| e).collect()
        };
        self.inner.routes.write().await.clear();
        for entry in entries {
            entry.session_task.abort();
            let _ = entry.session_task.await;
            if let Some(h) = entry.health_forwarder_task {
                h.abort();
                let _ = h.await;
            }
        }

        // Abort the spawn-watcher and demux tasks so they release their
        // captured `Arc<RegistryInner>` clones (self-referential Arc closure).
        let watcher = { self.spawn_watcher.lock().await.take() };
        if let Some(handle) = watcher {
            handle.abort();
            let _ = handle.await;
        }
        let demux = { self.demux_task.lock().await.take() };
        if let Some(handle) = demux {
            handle.abort();
            let _ = handle.await;
        }
    }

    /// Test helper: manually insert a handle under a NodeID (bypasses the
    /// spawn watcher and does not spawn a session task).
    ///
    /// The inserted entry has no owned JoinHandles — `shutdown` and
    /// `remove` are no-ops on it. Only used by unit tests that construct
    /// their own session handle out-of-band.
    #[doc(hidden)]
    pub async fn insert_for_test(&self, node_id: NodeID, handle: PeerSessionHandle) {
        // For test-inserted entries, spawn a dummy no-op task so the
        // RegistryEntry struct is well-formed. Aborting a completed task is
        // a no-op.
        let session_task = tokio::spawn(async {});
        let entry = RegistryEntry {
            handle,
            session_task,
            health_forwarder_task: None,
            alias: 0,
        };
        let mut sessions = self.inner.sessions.write().await;
        sessions.insert(node_id, entry);
    }

    /// Test helper: register an inbound route for `alias` and return the
    /// receiver end of the per-peer channel. Lets a registry-scoped test
    /// observe exactly which frames the demux forwards (S7-T7 construction
    /// window). Does not spawn a session.
    #[doc(hidden)]
    pub async fn insert_route_for_test(&self, alias: u16) -> mpsc::Receiver<InboundEvent> {
        let (tx, rx) = mpsc::channel(INBOUND_CAPACITY);
        self.inner.routes.write().await.insert(
            alias,
            Route {
                tx,
                pending_drops: Arc::new(AtomicU64::new(0)),
            },
        );
        rx
    }

    /// Test helper: manually spawn a session for `(node_id, alias)`.
    #[doc(hidden)]
    pub async fn spawn_for_test(&self, node_id: NodeID, alias: u16) -> PeerSessionHandle {
        let our_alias = *self.inner.our_alias.read().await;
        let (tx, rx) = mpsc::channel(INBOUND_CAPACITY);
        let (handle, session_task, health_forwarder_task) = PeerSession::spawn_with_tasks(
            node_id,
            alias,
            our_alias,
            self.inner.transport.clone(),
            rx,
        );
        let entry = RegistryEntry {
            handle: handle.clone(),
            session_task,
            health_forwarder_task,
            alias,
        };
        {
            let mut sessions = self.inner.sessions.write().await;
            let mut routes = self.inner.routes.write().await;
            routes.insert(
                alias,
                Route {
                    tx,
                    pending_drops: Arc::new(AtomicU64::new(0)),
                },
            );
            sessions.insert(node_id, entry);
        }
        handle
    }

    /// Number of registered sessions.
    pub async fn len(&self) -> usize {
        self.inner.sessions.read().await.len()
    }

    /// True if no sessions are registered.
    pub async fn is_empty(&self) -> bool {
        self.inner.sessions.read().await.is_empty()
    }

    /// Snapshot of every currently registered peer's session handle.
    ///
    /// Returned handles are cheap to clone and remain valid for the lifetime
    /// of their session task. Used by cross-cutting commands (e.g.
    /// `cancel_cdi_download`) that broadcast a `PeerCommand` to every peer
    /// while holding no long-lived locks.
    pub async fn snapshot_handles(&self) -> Vec<PeerSessionHandle> {
        let sessions = self.inner.sessions.read().await;
        sessions.values().map(|e| e.handle.clone()).collect()
    }
}

/// Spawn watcher: subscribes to the transport inbound broadcast and calls
/// `spawn` on qualifying frames (VNI, InitComplete, AMD).
async fn spawn_watcher_loop(
    inner: Arc<RegistryInner>,
    mut inbound: broadcast::Receiver<ReceivedMessage>,
) {
    loop {
        let msg = match inbound.recv().await {
            Ok(m) => m,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        };

        let frame = &msg.frame;
        let (mti, alias) = match MTI::from_header(frame.header) {
            Ok(x) => x,
            Err(_) => continue,
        };

        // Qualify by MTI. Only VNI, InitComplete, and AMD carry a full NodeID.
        let node_id = match mti {
            MTI::VerifiedNode | MTI::InitializationComplete => {
                // VNI + InitComplete: NodeID is the entire 6-byte payload.
                if frame.data.len() >= 6 {
                    let mut bytes = [0u8; 6];
                    bytes.copy_from_slice(&frame.data[0..6]);
                    NodeID::new(bytes)
                } else {
                    continue;
                }
            }
            MTI::AliasMapDefinition => {
                // AMD: NodeID is the 6-byte payload.
                if frame.data.len() >= 6 {
                    let mut bytes = [0u8; 6];
                    bytes.copy_from_slice(&frame.data[0..6]);
                    NodeID::new(bytes)
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        // Read-check first (per tokio-rwlock-self-deadlock guardrail).
        let existing = {
            let sessions = inner.sessions.read().await;
            sessions.get(&node_id).map(|e| (e.handle.clone(), e.alias))
        };

        if let Some((handle, old_alias)) = existing {
            // Repeat observation: update alias in place, never re-spawn.
            if old_alias != alias {
                // Re-key the inbound route atomically with the stored alias
                // (S7-T6). The channel sender is unchanged — only the routing
                // key moves — so no in-flight frame is lost. Because the
                // session no longer filters inbound frames by source alias
                // (S7-T5), the demux can begin delivering new-alias frames
                // immediately, before the session processes `AliasChanged`.
                {
                    let mut sessions = inner.sessions.write().await;
                    let mut routes = inner.routes.write().await;
                    if let Some(entry) = sessions.get_mut(&node_id) {
                        entry.alias = alias;
                    }
                    if let Some(route) = routes.remove(&old_alias) {
                        routes.insert(alias, route);
                    }
                }
                let _ = handle
                    .command(PeerCommand::AliasChanged { new_alias: alias })
                    .await;
            }
            continue;
        }

        // Build the session outside the write lock. The per-peer channel is
        // created here; the route is registered atomically with the entry
        // insert below. Frames arriving before that insert are dropped (the
        // documented construction-window contract, S7-T6).
        let our_alias = *inner.our_alias.read().await;
        let (tx, rx) = mpsc::channel(INBOUND_CAPACITY);
        let (new_handle, session_task, health_forwarder_task) = PeerSession::spawn_with_tasks(
            node_id,
            alias,
            our_alias,
            inner.transport.clone(),
            rx,
        );

        // Double-check under the write lock.
        {
            let mut sessions = inner.sessions.write().await;
            if let Some(existing) = sessions.get(&node_id).map(|e| e.handle.clone()) {
                // Someone raced us; use their handle and dispatch AliasChanged
                // on our own to keep the invariant. Abort the orphan tasks we
                // just spawned so their captured TransportHandle clones drop.
                // `tx`/`rx` drop with the aborted session — no route was
                // registered, so nothing to clean up.
                drop(sessions);
                session_task.abort();
                if let Some(h) = health_forwarder_task {
                    h.abort();
                }
                let _ = existing
                    .command(PeerCommand::AliasChanged { new_alias: alias })
                    .await;
            } else {
                // Register the route first, then the entry — both under the
                // sessions write guard so an observer taking `sessions` then
                // `routes` sees a consistent (entry + route) pair.
                {
                    let mut routes = inner.routes.write().await;
                    routes.insert(
                        alias,
                        Route {
                            tx,
                            pending_drops: Arc::new(AtomicU64::new(0)),
                        },
                    );
                }
                let entry = RegistryEntry {
                    handle: new_handle,
                    session_task,
                    health_forwarder_task,
                    alias,
                };
                sessions.insert(node_id, entry);
            }
        }
    }
}

/// Inbound demux loop (S7 D1=A): drains the transport broadcast and forwards
/// each frame to the destination peer's per-peer channel, keyed by the frame's
/// **source** alias (the peer that sent it). The bus broadcast is retained for
/// the spawn-watcher and future `EventRouter`; only per-peer delivery moves off
/// the shared ring, so cross-peer fan-out amplification lag becomes structurally
/// impossible.
async fn demux_loop(
    inner: Arc<RegistryInner>,
    mut inbound: broadcast::Receiver<ReceivedMessage>,
) {
    loop {
        match inbound.recv().await {
            Ok(msg) => route_frame(&inner, msg).await,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                // Upstream broadcast lag: the dropped frames cannot be
                // attributed to a specific peer, so signal every active route.
                // Over-signalling is safe — a session's lag recovery is an
                // idempotent re-request (S7 D2=C).
                let routes = inner.routes.read().await;
                for route in routes.values() {
                    route.pending_drops.fetch_add(n, Ordering::Relaxed);
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Forward one frame to its peer's channel. Delivers any owed `Lagged` marker
/// first so the session recovers before the next frame lands; on channel
/// overflow drops the frame and increments the peer's `pending_drops`.
async fn route_frame(inner: &Arc<RegistryInner>, msg: ReceivedMessage) {
    let Some(source) = source_alias(&msg.frame) else {
        return;
    };
    let route = {
        let routes = inner.routes.read().await;
        routes
            .get(&source)
            .map(|r| (r.tx.clone(), r.pending_drops.clone()))
    };
    let Some((tx, pending)) = route else {
        // No session registered for this alias — construction-window drop.
        return;
    };

    // Deliver any owed lag marker before the next frame.
    let owed = pending.load(Ordering::Relaxed);
    if owed > 0 {
        match tx.try_send(InboundEvent::Lagged(owed)) {
            Ok(()) => {
                pending.store(0, Ordering::Relaxed);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Still full — this frame is also lost; keep owing.
                pending.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Err(mpsc::error::TrySendError::Closed(_)) => return,
        }
    }

    match tx.try_send(InboundEvent::Frame(msg)) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(_)) => {
            pending.fetch_add(1, Ordering::Relaxed);
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {}
    }
}
