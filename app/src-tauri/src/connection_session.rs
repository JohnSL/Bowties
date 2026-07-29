//! Connection-scoped resource aggregate (Option B).
//!
//! `ConnectionSession` owns every resource that exists only while a single
//! LCC connection is active: the transport connection itself, its transport
//! handle, the event router, the peer-session registry, the active
//! connection config, and the supervisor task that watches for unexpected
//! transport termination. `NodeRegistry` stays app-scoped (survives across
//! connections) and is passed in explicitly wherever session cleanup needs
//! to shut down its live actors.

use std::sync::Arc;
use tokio::sync::{watch, Mutex, RwLock};
use tokio::task::JoinHandle;

use lcc_rs::peer_session_registry::PeerSessionRegistry;
use lcc_rs::{LccConnection, TransportHandle, TransportTermination};

use crate::commands::ConnectionConfig;
use crate::events::EventRouter;
use crate::node_registry::NodeRegistry;

/// Payload emitted to the frontend as `lcc-connection-lost` when the
/// transport terminates unexpectedly. Never emitted for explicit disconnect.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionLostPayload {
    pub reason: String,
}

/// Connection-scoped resources, valid from a successful `connect_lcc` until
/// either explicit `disconnect_lcc` or unexpected transport termination
/// claims and cleans it up.
pub struct ConnectionSession {
    pub connection: Arc<Mutex<LccConnection>>,
    pub transport_handle: Option<TransportHandle>,
    pub event_router: Option<EventRouter>,
    pub sessions: Option<Arc<PeerSessionRegistry>>,
    pub active_config: ConnectionConfig,
    supervisor: Option<JoinHandle<()>>,
}

impl ConnectionSession {
    pub fn new(
        connection: Arc<Mutex<LccConnection>>,
        transport_handle: Option<TransportHandle>,
        event_router: Option<EventRouter>,
        sessions: Option<Arc<PeerSessionRegistry>>,
        active_config: ConnectionConfig,
    ) -> Self {
        Self {
            connection,
            transport_handle,
            event_router,
            sessions,
            active_config,
            supervisor: None,
        }
    }

    /// Attach the termination-supervisor task spawned for this session.
    pub fn attach_supervisor(&mut self, handle: JoinHandle<()>) {
        self.supervisor = Some(handle);
    }

    /// Abort the termination-supervisor task, if one was spawned. Called only
    /// from the explicit-disconnect path — the supervisor never aborts its
    /// own task; by the time it would reach that code it has already claimed
    /// the session (see `spawn_termination_supervisor`).
    pub fn abort_supervisor(&mut self) {
        if let Some(handle) = self.supervisor.take() {
            handle.abort();
        }
    }

    /// Run the full connection-scoped cleanup sequence exactly once: stop the
    /// event router, shut down `NodeRegistry`'s live actors, close the
    /// connection/transport, and shut down the peer-session registry.
    ///
    /// Consumes `self` so a session can only be cleaned up once.
    pub async fn cleanup(mut self, node_registry: &NodeRegistry) {
        if let Some(mut router) = self.event_router.take() {
            router.stop().await;
        }

        node_registry.shutdown_all().await;

        {
            let mut conn = self.connection.lock().await;
            conn.shutdown_responders().await;
            let _ = conn.close().await;
        }

        if let Some(sessions) = self.sessions.take() {
            // Clears the sessions map AND aborts the spawn-watcher so its
            // captured `TransportHandle` is released. Without the watcher
            // abort the transport broadcast channel stays alive and on
            // Windows serial reconnect the OS handle surfaces `COM7: Access
            // is denied`.
            sessions.shutdown().await;
        }
    }
}

/// Atomically claim (take) the active session out of `slot`. Whichever
/// caller wins the race gets `Some`; a concurrent caller gets `None` and
/// should return promptly without performing cleanup.
pub async fn claim_session(
    slot: &Arc<RwLock<Option<ConnectionSession>>>,
) -> Option<ConnectionSession> {
    slot.write().await.take()
}

/// Spawn the transport-termination supervisor for a newly established
/// session. Watches `term_rx` for the terminal `ReaderError` notification; on
/// the first (and only) unexpected termination it atomically claims the
/// session out of `slot`, runs cleanup exactly once, then calls `on_lost`
/// exactly once with the human-readable reason. Never fires for explicit
/// `disconnect()` — that path claims the session first via `claim_session`
/// and aborts this task via `abort_supervisor`.
///
/// Checks `term_rx`'s current value via `borrow_and_update()` *before*
/// awaiting `changed()`: a `watch::Receiver` created via `subscribe()` starts
/// with its "last seen" version set to whatever value is already published,
/// so `changed()` alone would never wake for a termination that happened
/// before (or racing) subscription. Checking the current value first — and
/// only awaiting `changed()` when nothing terminal has landed yet — handles
/// late subscription without a polling/yield loop.
pub fn spawn_termination_supervisor(
    slot: Arc<RwLock<Option<ConnectionSession>>>,
    node_registry: Arc<NodeRegistry>,
    mut term_rx: watch::Receiver<Option<TransportTermination>>,
    on_lost: impl Fn(String) + Send + Sync + 'static,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let reason = loop {
            if let Some(TransportTermination::ReaderError { reason }) =
                term_rx.borrow_and_update().clone()
            {
                break reason;
            }
            if term_rx.changed().await.is_err() {
                // Sender dropped (transport shut down through the normal
                // disconnect path) — nothing unexpected to report.
                return;
            }
        };
        if let Some(session) = claim_session(&slot).await {
            session.cleanup(&node_registry).await;
            on_lost(reason);
        }
    })
}

/// Publish `session` into `slot` and spawn its termination supervisor, in
/// the one order that closes the publish/claim race: the session is stored
/// in `slot` *before* the supervisor is spawned, so the supervisor can never
/// see `None` for a session that is about to be published. This includes a
/// transport that already terminated before setup, or terminates during it
/// — `spawn_termination_supervisor`'s current-value check claims and cleans
/// the session as soon as it is spawned, and `AppState::is_connected()`
/// never observes it as live.
///
/// The supervisor's `JoinHandle` is attached back onto the now-published
/// session (for `abort_supervisor` on explicit disconnect) unless the
/// supervisor has *already* claimed (and is cleaning up, or has cleaned up)
/// the session by the time this function tries to attach it — in which case
/// the handle is returned instead of attached, since there is no session left
/// to attach it to. That handle is for an already-finishing task; dropping
/// it only detaches, it does not leave a forever-waiting task.
///
/// `term_rx` is `None` when the transport handle does not publish
/// termination (legacy bridge path) — the session is published with no
/// supervisor and this returns `None`.
pub async fn publish_session_and_supervise(
    slot: &Arc<RwLock<Option<ConnectionSession>>>,
    session: ConnectionSession,
    node_registry: Arc<NodeRegistry>,
    term_rx: Option<watch::Receiver<Option<TransportTermination>>>,
    on_lost: impl Fn(String) + Send + Sync + 'static,
) -> Option<JoinHandle<()>> {
    *slot.write().await = Some(session);

    let term_rx = term_rx?;
    let supervisor = spawn_termination_supervisor(slot.clone(), node_registry, term_rx, on_lost);

    match slot.write().await.as_mut() {
        Some(s) => {
            s.attach_supervisor(supervisor);
            None
        }
        None => Some(supervisor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lcc_rs::transport::mock::MockTransport;
    use lcc_rs::{NodeAlias, NodeID};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_node_id() -> NodeID {
        NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF])
    }

    fn test_config() -> ConnectionConfig {
        ConnectionConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            adapter_type: crate::layout::types::AdapterType::Tcp,
            host: None,
            port: None,
            serial_port: None,
            baud_rate: None,
            flow_control: Default::default(),
        }
    }

    /// Build a `ConnectionSession` backed by a real (mock-transport) LCC
    /// connection so `cleanup()` exercises its real code path, and a handle
    /// to force an unexpected reader failure on demand.
    async fn build_session() -> (ConnectionSession, Arc<std::sync::atomic::AtomicBool>) {
        let transport = MockTransport::new();
        let fail_handle = transport.fail_receive_handle();
        let alias = NodeAlias::new(0x123).unwrap();
        let connection = LccConnection::with_transport(Box::new(transport), test_node_id(), alias);
        let connection = Arc::new(Mutex::new(connection));
        let handle = {
            let conn = connection.lock().await;
            conn.transport_handle().cloned()
        };
        let session = ConnectionSession::new(connection, handle, None, None, test_config());
        (session, fail_handle)
    }

    #[tokio::test]
    async fn unexpected_termination_claims_session_once_and_emits_once() {
        let node_registry = Arc::new(NodeRegistry::new());
        let (session, fail_handle) = build_session().await;
        let term_rx = session
            .transport_handle
            .as_ref()
            .expect("mock transport exposes a transport handle")
            .subscribe_termination()
            .expect("full actor path publishes termination");
        let slot = Arc::new(RwLock::new(Some(session)));

        let emit_count = Arc::new(AtomicUsize::new(0));
        let emit_count_clone = emit_count.clone();
        let supervisor = spawn_termination_supervisor(
            slot.clone(),
            node_registry,
            term_rx,
            move |_reason| {
                emit_count_clone.fetch_add(1, Ordering::SeqCst);
            },
        );

        fail_handle.store(true, Ordering::Relaxed);
        supervisor.await.expect("supervisor task does not panic");

        assert_eq!(emit_count.load(Ordering::SeqCst), 1, "on_lost must fire exactly once");
        assert!(slot.read().await.is_none(), "session must be claimed (taken) exactly once");
    }

    #[tokio::test]
    async fn explicit_disconnect_aborts_supervisor_and_emits_zero() {
        let node_registry = Arc::new(NodeRegistry::new());
        let (mut session, _fail_handle) = build_session().await;
        let term_rx = session
            .transport_handle
            .as_ref()
            .unwrap()
            .subscribe_termination()
            .unwrap();

        let emit_count = Arc::new(AtomicUsize::new(0));
        let emit_count_clone = emit_count.clone();
        let supervisor = spawn_termination_supervisor(
            Arc::new(RwLock::new(None)), // never used: the disconnect path below claims first
            node_registry.clone(),
            term_rx,
            move |_reason| {
                emit_count_clone.fetch_add(1, Ordering::SeqCst);
            },
        );
        session.attach_supervisor(supervisor);

        // Explicit-disconnect path: claim the session directly (as
        // `AppState::disconnect` does via its own slot), abort the
        // supervisor, then run cleanup.
        session.abort_supervisor();
        session.cleanup(&node_registry).await;

        assert_eq!(emit_count.load(Ordering::SeqCst), 0, "explicit disconnect must never emit lcc-connection-lost");
    }

    #[tokio::test]
    async fn competing_claims_only_one_winner_cleans_up() {
        let (session, _fail_handle) = build_session().await;
        let slot = Arc::new(RwLock::new(Some(session)));

        let (a, b) = tokio::join!(claim_session(&slot), claim_session(&slot));
        let winners = [a.is_some(), b.is_some()];

        assert_eq!(
            winners.iter().filter(|w| **w).count(),
            1,
            "exactly one concurrent claim must win the session"
        );
        assert!(slot.read().await.is_none());
    }

    #[tokio::test]
    async fn late_subscription_to_already_terminal_channel_is_claimed_without_awaiting_changed() {
        let node_registry = Arc::new(NodeRegistry::new());
        let (session, fail_handle) = build_session().await;
        let handle = session
            .transport_handle
            .clone()
            .expect("mock transport exposes a transport handle");

        // Subscribe *before* the failure so this receiver's `changed()` can
        // observe it, and use it only to deterministically confirm the
        // termination has actually landed on the channel.
        let mut early_rx = handle.subscribe_termination().unwrap();
        fail_handle.store(true, Ordering::Relaxed);
        early_rx.changed().await.expect("termination is published");

        // Late subscription: this receiver's baseline is already the
        // terminal value, so only the pre-`changed()` current-value check
        // in `spawn_termination_supervisor` can surface it — a `changed()`
        // wait alone would never wake since nothing changes again.
        let late_rx = handle.subscribe_termination().unwrap();
        let slot = Arc::new(RwLock::new(Some(session)));

        let emit_count = Arc::new(AtomicUsize::new(0));
        let emit_count_clone = emit_count.clone();
        let supervisor = spawn_termination_supervisor(slot.clone(), node_registry, late_rx, move |_reason| {
            emit_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        supervisor.await.expect("supervisor task does not panic");

        assert_eq!(emit_count.load(Ordering::SeqCst), 1, "late-subscribed terminal state must still emit exactly once");
        assert!(slot.read().await.is_none(), "late-subscribed terminal state must still claim the session");
    }

    #[tokio::test]
    async fn publish_session_and_supervise_claims_prepublished_termination_never_leaves_live_session() {
        let node_registry = Arc::new(NodeRegistry::new());
        let (session, fail_handle) = build_session().await;
        let handle = session
            .transport_handle
            .clone()
            .expect("mock transport exposes a transport handle");

        // Fail and confirm publication *before* the session is ever
        // published to the slot — reproducing "transport already terminated
        // when supervision begins" rather than a race during setup.
        let mut early_rx = handle.subscribe_termination().unwrap();
        fail_handle.store(true, Ordering::Relaxed);
        early_rx.changed().await.expect("termination is published");
        let late_rx = handle.subscribe_termination().unwrap();

        let slot: Arc<RwLock<Option<ConnectionSession>>> = Arc::new(RwLock::new(None));
        let emit_count = Arc::new(AtomicUsize::new(0));
        let notify = Arc::new(tokio::sync::Notify::new());
        let emit_count_clone = emit_count.clone();
        let notify_clone = notify.clone();

        let maybe_handle = publish_session_and_supervise(
            &slot,
            session,
            node_registry,
            Some(late_rx),
            move |_reason| {
                emit_count_clone.fetch_add(1, Ordering::SeqCst);
                notify_clone.notify_one();
            },
        )
        .await;

        // Deterministically wait for the claim+cleanup+emit to finish
        // regardless of which side of the publish/attach race the
        // supervisor landed on: if it already raced ahead of the attach
        // step, its handle comes back directly; otherwise it is attached to
        // the (now claimed-and-gone) session and `notify` reports emission.
        match maybe_handle {
            Some(handle) => handle.await.expect("supervisor task does not panic"),
            None => notify.notified().await,
        }

        assert_eq!(emit_count.load(Ordering::SeqCst), 1, "on_lost must fire exactly once for a prepublished termination");
        assert!(
            slot.read().await.is_none(),
            "a prepublished termination must never leave a live session in the slot (AppState::is_connected() must be false)"
        );
    }
}

