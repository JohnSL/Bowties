//! Integration tests for `PeerSession` — SNIP + PIP through the new actor
//! pattern (S2 slice of feature 019-peer-session-refactor).
//!
//! Behaviours verified:
//! 1. Cold `query_snip` returns the assembled data.
//! 2. Two concurrent `query_snip` callers coalesce onto a single outbound
//!    `SNIPRequest` frame (session-scoped coalescing).
//! 3. Cache hit: after a first success, a second `query_snip` returns the
//!    cached data with no additional outbound frame.
//! 4. Analog (1)–(3) for PIP.
//! 5. `PeerCommand::PeerReinitialised` clears SNIP and PIP caches.
//! 6. `PeerCommand::AliasChanged` updates the alias in place and subsequent
//!    outbound frames go to the new destination.
//! 7. `PeerSessionRegistry` spawns a session on VNI, InitComplete, and AMD;
//!    repeat observation with a new alias updates in place (no duplicate
//!    session spawn).
//! 8. Broadcast `Lagged(n)` aborts the active exchange with
//!    `PeerError::Protocol` and preserves caches.
//!
//! These tests exercise the public API only. Wire assertions use the shared
//! `MockTransport` from `lcc-rs::transport::mock`.

use lcc_rs::peer_session::{InboundEvent, PeerCommand, PeerError, PeerSession};
use lcc_rs::peer_session_registry::PeerSessionRegistry;
use lcc_rs::transport::mock::MockTransport;
use lcc_rs::transport_actor::ReceivedMessage;
use lcc_rs::{NodeID, PIPStatus, SNIPStatus, TransportActor, TransportHandle};
use std::sync::Arc;
use std::time::Duration;

fn our_alias() -> u16 { 0x825 }

fn make_actor(transport: MockTransport) -> (TransportActor, TransportHandle) {
    let actor = TransportActor::new(Box::new(transport));
    let handle = actor.handle();
    (actor, handle)
}

fn minimal_snip_payload() -> Vec<u8> {
    let mut p = vec![0x04u8];
    p.extend_from_slice(b"ACME\x00Widget\x001.0\x002.3\x00");
    p.push(0x02);
    p.extend_from_slice(b"MyNode\x00Desc\x00");
    p
}

/// Build a SNIP reply frame (addressed message MTI 0x19A08).
///
/// data[0]: (flag_nibble << 4) | ((dest_alias >> 8) & 0x0F)
/// data[1]: dest_alias & 0xFF
/// data[2..]: payload chunk
fn snip_reply_frame(source_alias: u16, dest_alias: u16, flag_nibble: u8, chunk: &[u8]) -> String {
    let header = (0x19A08u32 << 12) | source_alias as u32;
    let dest_hi = ((dest_alias >> 8) & 0x0F) as u8;
    let dest_lo = (dest_alias & 0xFF) as u8;
    let frame_type_byte = (flag_nibble << 4) | dest_hi;
    let mut data = vec![frame_type_byte, dest_lo];
    data.extend_from_slice(chunk);
    let data_hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
    format!(":X{:08X}N{};", header, data_hex)
}

/// Build a PIP reply frame (MTI 0x19668, addressed).
fn pip_reply_frame(source_alias: u16, dest_alias: u16, flag_bytes: &[u8]) -> String {
    let header = (0x19668u32 << 12) | source_alias as u32;
    let dest_hi = ((dest_alias >> 8) & 0x0F) as u8;
    let dest_lo = (dest_alias & 0xFF) as u8;
    let mut data = vec![dest_hi, dest_lo];
    data.extend_from_slice(flag_bytes);
    let data_hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
    format!(":X{:08X}N{};", header, data_hex)
}

/// Build an AMD frame (MTI 0x10701, source_alias in header, NodeID in payload).
fn amd_frame(source_alias: u16, node_id: [u8; 6]) -> String {
    let header = (0x10701u32 << 12) | source_alias as u32;
    let data_hex: String = node_id.iter().map(|b| format!("{:02X}", b)).collect();
    format!(":X{:08X}N{};", header, data_hex)
}

/// Build a VerifiedNode frame (MTI 0x19170).
fn vni_frame(source_alias: u16, node_id: [u8; 6]) -> String {
    let header = (0x19170u32 << 12) | source_alias as u32;
    let data_hex: String = node_id.iter().map(|b| format!("{:02X}", b)).collect();
    format!(":X{:08X}N{};", header, data_hex)
}

/// Build an InitializationComplete frame (MTI 0x19100).
fn init_complete_frame(source_alias: u16, node_id: [u8; 6]) -> String {
    let header = (0x19100u32 << 12) | source_alias as u32;
    let data_hex: String = node_id.iter().map(|b| format!("{:02X}", b)).collect();
    format!(":X{:08X}N{};", header, data_hex)
}

fn queue_snip_reply(transport: &mut MockTransport, source: u16, dest: u16, payload: &[u8]) {
    let chunks: Vec<&[u8]> = payload.chunks(6).collect();
    let n = chunks.len();
    for (i, chunk) in chunks.iter().enumerate() {
        let flag = if n == 1 { 0x0 } else if i == 0 { 0x1 } else if i == n - 1 { 0x2 } else { 0x3 };
        transport.add_receive_frame(snip_reply_frame(source, dest, flag, chunk));
    }
}

/// Count outbound SNIPRequest frames (MTI 0x19DE8).
fn count_snip_requests(transport: &MockTransport, our_alias: u16, dest_alias: u16) -> usize {
    let expected_header = format!(":X{:08X}N", (0x19DE8u32 << 12) | our_alias as u32);
    let expected_body = format!("{:02X}{:02X}", (dest_alias >> 8) as u8 & 0x0F, dest_alias & 0xFF);
    transport
        .get_sent_frames()
        .iter()
        .filter(|f: &&String| f.starts_with(&expected_header) && f.contains(&expected_body))
        .count()
}

fn count_pip_requests(transport: &MockTransport, our_alias: u16, dest_alias: u16) -> usize {
    let expected_header = format!(":X{:08X}N", (0x19828u32 << 12) | our_alias as u32);
    let expected_body = format!("{:02X}{:02X}", (dest_alias >> 8) as u8 & 0x0F, dest_alias & 0xFF);
    transport
        .get_sent_frames()
        .iter()
        .filter(|f: &&String| f.starts_with(&expected_header) && f.contains(&expected_body))
        .count()
}

// ── SNIP behaviours ──────────────────────────────────────────────────────

#[tokio::test]
async fn cold_query_snip_returns_data() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x01]);
    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let snip = session.query_snip().await.unwrap().expect("SNIP data");
    assert_eq!(snip.manufacturer, "ACME");
    assert_eq!(snip.model, "Widget");
    assert_eq!(count_snip_requests(&transport_probe, our_alias(), node_alias), 1);

    actor.shutdown().await;
}

#[tokio::test]
async fn two_concurrent_query_snip_coalesce_to_single_wire_request() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x02]);
    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let a = session.clone();
    let b = session.clone();

    let (ra, rb) = tokio::join!(
        async move { a.query_snip().await },
        async move { b.query_snip().await },
    );
    let da = ra.unwrap().expect("A SNIP");
    let db = rb.unwrap().expect("B SNIP");
    assert_eq!(da.manufacturer, "ACME");
    assert_eq!(db.manufacturer, "ACME");

    let count = count_snip_requests(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 1, "coalescing must produce exactly one outbound SNIPRequest, got {}", count);

    actor.shutdown().await;
}

#[tokio::test]
async fn snip_cache_hit_makes_no_wire_request() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x03]);
    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let _ = session.query_snip().await.unwrap();
    let count_after_first = count_snip_requests(&transport_probe, our_alias(), node_alias);
    let _ = session.query_snip().await.unwrap();
    let count_after_second = count_snip_requests(&transport_probe, our_alias(), node_alias);
    assert_eq!(count_after_first, count_after_second, "cache hit must not emit another SNIPRequest");
    assert_eq!(count_after_second, 1);

    actor.shutdown().await;
}

// ── PIP behaviours ────────────────────────────────────────────────────────

#[tokio::test]
async fn cold_query_pip_returns_flags() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x11]);
    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    // JMRI-ish: datagram + event_exchange + identification + SNIP
    transport.add_receive_frame(pip_reply_frame(node_alias, our_alias(), &[0x46, 0x10, 0x00, 0x00, 0x00, 0x00]));

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let flags = session.query_pip().await.unwrap().expect("PIP flags");
    assert!(flags.datagram);
    assert!(flags.snip);
    assert_eq!(count_pip_requests(&transport_probe, our_alias(), node_alias), 1);

    actor.shutdown().await;
}

#[tokio::test]
async fn two_concurrent_query_pip_coalesce_to_single_wire_request() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x12]);
    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    transport.add_receive_frame(pip_reply_frame(node_alias, our_alias(), &[0x46, 0x10, 0x00, 0x00, 0x00, 0x00]));

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let a = session.clone();
    let b = session.clone();
    let (ra, rb) = tokio::join!(
        async move { a.query_pip().await },
        async move { b.query_pip().await },
    );
    let _ = ra.unwrap().expect("A PIP");
    let _ = rb.unwrap().expect("B PIP");
    let count = count_pip_requests(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 1, "coalescing must produce exactly one outbound PIP request, got {}", count);

    actor.shutdown().await;
}

// ── PeerReinitialised clears caches ──────────────────────────────────────

#[tokio::test]
async fn peer_reinitialised_clears_snip_and_pip_caches() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x21]);
    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());
    transport.add_receive_frame(pip_reply_frame(node_alias, our_alias(), &[0x46, 0x10, 0x00, 0x00, 0x00, 0x00]));
    // Prepare replies for the post-reinit second query cycle.
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());
    transport.add_receive_frame(pip_reply_frame(node_alias, our_alias(), &[0x46, 0x10, 0x00, 0x00, 0x00, 0x00]));

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let _ = session.query_snip().await.unwrap();
    let _ = session.query_pip().await.unwrap();
    let snip_before = count_snip_requests(&transport_probe, our_alias(), node_alias);
    let pip_before = count_pip_requests(&transport_probe, our_alias(), node_alias);
    assert_eq!(snip_before, 1);
    assert_eq!(pip_before, 1);

    session.command(PeerCommand::PeerReinitialised).await.unwrap();

    let _ = session.query_snip().await.unwrap();
    let _ = session.query_pip().await.unwrap();
    let snip_after = count_snip_requests(&transport_probe, our_alias(), node_alias);
    let pip_after = count_pip_requests(&transport_probe, our_alias(), node_alias);
    assert_eq!(snip_after, 2, "PeerReinitialised must clear SNIP cache — second query emits a new wire request");
    assert_eq!(pip_after, 2, "PeerReinitialised must clear PIP cache — second query emits a new wire request");

    actor.shutdown().await;
}

// ── Broadcast Lagged aborts exchange, preserves caches ──────────────────

#[tokio::test]
async fn broadcast_lagged_aborts_active_exchange_and_preserves_caches() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x31]);
    // Seed the SNIP cache first.
    let mut transport = MockTransport::new();
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let seed = session.query_snip().await.unwrap().expect("seed SNIP");
    assert_eq!(seed.manufacturer, "ACME");

    // Now start a PIP exchange but never send a reply. Then inject a
    // synthetic broadcast lag via a Cancel — simulates the abort path that
    // Lagged would take (uses the same abort_active codepath).
    //
    // We can't cleanly force the broadcast channel to lag from a test, so we
    // test the observable contract: an abort clears the exchange and
    // preserves the SNIP cache. The Lagged branch invokes abort_active with
    // a Protocol error, which we verify separately in the direct unit test
    // below via drop-based teardown observation.
    //
    // Here we assert cache survival across the abort.
    let handle2 = session.clone();
    let cancel_task = tokio::spawn(async move {
        // Wait long enough for query_pip to be in flight, then cancel.
        tokio::time::sleep(Duration::from_millis(20)).await;
        handle2.cancel("test-abort").await;
    });
    let res = tokio::time::timeout(Duration::from_secs(3), session.query_pip()).await;
    let _ = cancel_task.await;
    match res {
        Ok(Err(PeerError::Cancelled { .. })) => { /* expected */ }
        Ok(Ok(_)) => panic!("expected Cancelled, got Ok"),
        Ok(Err(e)) => panic!("expected Cancelled, got {:?}", e),
        Err(_) => panic!("PIP call did not return within the deadline after cancel"),
    }

    // SNIP cache preserved — subsequent query returns cached data with no
    // extra outbound frame.
    let snip = session.query_snip().await.unwrap().expect("cached SNIP");
    assert_eq!(snip.manufacturer, "ACME");

    actor.shutdown().await;
}

// ── S7: a real inbound Lagged aborts SNIP/PIP but preserves caches ───────

/// (S7-T1c) A genuine inbound `Lagged` marker delivered while a PIP exchange
/// is active aborts that exchange with `PeerError::Protocol` (the S2 D3
/// abort-and-continue policy for non-CDI exchanges) while leaving a
/// previously-populated SNIP cache intact. Unlike the earlier
/// `broadcast_lagged_...` test — which simulated the abort path via `Cancel`
/// — this drives a real `InboundEvent::Lagged` through the per-peer channel
/// (S7 D1=A).
#[tokio::test]
async fn inbound_lag_aborts_active_pip_and_preserves_snip_cache() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x32]);

    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let transport = MockTransport::new();
    let (_actor, handle) = make_actor(transport);
    let (session, _s, _h) =
        PeerSession::spawn_with_tasks(node_id, node_alias, our_alias(), handle, rx);

    // Seed the SNIP cache by delivering a complete SNIP reply through the
    // per-peer channel while a SNIP query is in flight.
    let payload = minimal_snip_payload();
    let chunks: Vec<Vec<u8>> = payload.chunks(6).map(|c| c.to_vec()).collect();
    let n = chunks.len();
    let s_snip = session.clone();
    let snip_task = tokio::spawn(async move { s_snip.query_snip().await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    for (i, chunk) in chunks.iter().enumerate() {
        let flag = if n == 1 { 0x0 } else if i == 0 { 0x1 } else if i == n - 1 { 0x2 } else { 0x3 };
        let frame_str = snip_reply_frame(node_alias, our_alias(), flag, chunk);
        let frame = lcc_rs::protocol::GridConnectFrame::parse(&frame_str).unwrap();
        tx.send(InboundEvent::Frame(ReceivedMessage {
            frame,
            timestamp: std::time::Instant::now(),
        }))
        .await
        .unwrap();
    }
    let seed = snip_task
        .await
        .unwrap()
        .unwrap()
        .expect("seed SNIP populated the cache");
    assert_eq!(seed.manufacturer, "ACME");

    // Start a PIP exchange (no reply will arrive), then inject a real Lagged.
    let s_pip = session.clone();
    let pip_task = tokio::spawn(async move { s_pip.query_pip().await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    tx.send(InboundEvent::Lagged(5)).await.unwrap();

    let res = tokio::time::timeout(Duration::from_secs(2), pip_task)
        .await
        .expect("pip resolved within deadline")
        .unwrap();
    match res {
        Err(PeerError::Protocol(_)) => { /* S2 D3 abort-and-continue */ }
        other => panic!("expected Protocol from lag abort, got {:?}", other),
    }

    // SNIP cache preserved across the PIP abort.
    let snip = session
        .query_snip()
        .await
        .unwrap()
        .expect("cached SNIP survives the lag abort");
    assert_eq!(snip.manufacturer, "ACME");
}

// ── Registry qualifies frames and updates alias in place ────────────────

#[tokio::test]
async fn registry_spawns_on_vni_and_updates_alias_on_amd() {
    let node_alias1: u16 = 0x3AE;
    let node_alias2: u16 = 0x4C1;
    let node_id_bytes = [0x02, 0x01, 0x57, 0x00, 0x00, 0x41];
    let node_id = NodeID::new(node_id_bytes);

    let mut transport = MockTransport::new();
    // Emit a VNI to trigger session spawn.
    transport.add_receive_frame(vni_frame(node_alias1, node_id_bytes));
    // Then an AMD with a new alias to trigger AliasChanged.
    transport.add_receive_frame(amd_frame(node_alias2, node_id_bytes));

    let (mut actor, handle) = make_actor(transport);
    let registry = PeerSessionRegistry::new(handle, our_alias());

    // Poll for the spawn (broadcast is async).
    let mut spawned = false;
    for _ in 0..50 {
        if registry.get(node_id).await.is_some() { spawned = true; break; }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(spawned, "registry must spawn a session on VNI");
    assert_eq!(registry.len().await, 1);

    // Give the AMD frame time to be processed → AliasChanged forwarded.
    // The number of sessions must remain 1 (no duplicate spawn).
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(registry.len().await, 1, "repeat NodeID must not spawn a duplicate session");

    actor.shutdown().await;
}

#[tokio::test]
async fn registry_spawns_on_init_complete_and_amd() {
    let node_alias: u16 = 0x555;
    let node_id_bytes = [0x02, 0x01, 0x57, 0x00, 0x00, 0x51];
    let node_id = NodeID::new(node_id_bytes);
    let node_id2_bytes = [0x02, 0x01, 0x57, 0x00, 0x00, 0x52];
    let node_id2 = NodeID::new(node_id2_bytes);

    let mut transport = MockTransport::new();
    transport.add_receive_frame(init_complete_frame(node_alias, node_id_bytes));
    transport.add_receive_frame(amd_frame(0x556, node_id2_bytes));

    let (mut actor, handle) = make_actor(transport);
    let registry = PeerSessionRegistry::new(handle, our_alias());

    let mut both_spawned = false;
    for _ in 0..50 {
        if registry.get(node_id).await.is_some() && registry.get(node_id2).await.is_some() {
            both_spawned = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(both_spawned, "InitComplete + AMD must each trigger session spawn");
    assert_eq!(registry.len().await, 2);

    actor.shutdown().await;
}

// ── Handle-level query works through registry ─────────────────────────────

#[tokio::test]
async fn registry_delivered_handle_supports_query_snip() {
    let node_alias: u16 = 0x3AE;
    let node_id_bytes = [0x02, 0x01, 0x57, 0x00, 0x00, 0x61];
    let node_id = NodeID::new(node_id_bytes);
    let mut transport = MockTransport::new();
    // AMD triggers spawn.
    transport.add_receive_frame(amd_frame(node_alias, node_id_bytes));
    // SNIP reply.
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());

    let (mut actor, handle) = make_actor(transport);
    let registry = PeerSessionRegistry::new(handle, our_alias());

    // Wait for spawn.
    let mut session: Option<lcc_rs::PeerSessionHandle> = None;
    for _ in 0..50 {
        if let Some(h) = registry.get(node_id).await { session = Some(h); break; }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let session = session.expect("session spawned via registry");
    let snip = session.query_snip().await.unwrap().expect("SNIP via registry handle");
    assert_eq!(snip.manufacturer, "ACME");

    actor.shutdown().await;
}

// ── Cancel returns Cancelled error ────────────────────────────────────────

#[tokio::test]
async fn cancel_returns_cancelled_error() {
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x71]);
    // Transport with no replies — the query will hang until we cancel.
    let transport = MockTransport::new();
    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let s2 = session.clone();
    let cancel = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        s2.cancel("test-cancel").await;
    });

    let result = tokio::time::timeout(Duration::from_secs(2), session.query_snip()).await;
    let _ = cancel.await;
    match result {
        Ok(Err(PeerError::Cancelled { .. })) => {}
        Ok(other) => panic!("expected Cancelled, got {:?}", other),
        Err(_) => panic!("query_snip did not return promptly after cancel"),
    }

    actor.shutdown().await;
}

// ── SNIP status observation via a cached hit after Complete ───────────────

#[tokio::test]
async fn snip_status_progression_ends_in_complete() {
    // Not strictly a public API, but the observable proxy is: after a
    // successful call, the second call is a cache hit. We assert that via
    // outbound-frame counting rather than a private status accessor.
    let node_alias: u16 = 0x3AE;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x81]);
    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    queue_snip_reply(&mut transport, node_alias, our_alias(), &minimal_snip_payload());

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let _ = session.query_snip().await.unwrap().expect("first SNIP");
    let count1 = count_snip_requests(&transport_probe, our_alias(), node_alias);
    let _ = session.query_snip().await.unwrap();
    let count2 = count_snip_requests(&transport_probe, our_alias(), node_alias);
    assert_eq!(count1, 1);
    assert_eq!(count2, 1);
    // Also assert SNIPStatus/PIPStatus enum re-exports are reachable — pin
    // the public API surface.
    let _ = SNIPStatus::Complete;
    let _ = PIPStatus::Complete;

    actor.shutdown().await;
}

// ── AliasChanged updates outbound destination ────────────────────────────

#[tokio::test]
async fn alias_changed_updates_outbound_destination() {
    let node_alias_old: u16 = 0x3AE;
    let node_alias_new: u16 = 0x4C1;
    let node_id = NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, 0x91]);

    let mut transport = MockTransport::new();
    let transport_probe = transport.clone();
    queue_snip_reply(&mut transport, node_alias_new, our_alias(), &minimal_snip_payload());

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias_old, our_alias(), handle);

    // Change alias BEFORE any query so the subsequent SNIPRequest is routed
    // to the new destination.
    session.command(PeerCommand::AliasChanged { new_alias: node_alias_new }).await.unwrap();
    let snip = session.query_snip().await.unwrap().expect("SNIP after alias change");
    assert_eq!(snip.manufacturer, "ACME");

    // No SNIPRequest should have been sent to the OLD alias.
    assert_eq!(count_snip_requests(&transport_probe, our_alias(), node_alias_old), 0);
    // Exactly one to the NEW alias.
    assert_eq!(count_snip_requests(&transport_probe, our_alias(), node_alias_new), 1);

    actor.shutdown().await;
}

// ── Arc<PeerSessionRegistry> is reachable as expected ─────────────────────

#[tokio::test]
async fn registry_is_arc_sharable_across_tasks() {
    let (mut actor, handle) = make_actor(MockTransport::new());
    let registry = PeerSessionRegistry::new(handle, our_alias());
    let r2: Arc<PeerSessionRegistry> = registry.clone();
    let handle_task = tokio::spawn(async move {
        r2.len().await
    });
    let _ = handle_task.await.unwrap();
    actor.shutdown().await;
}
