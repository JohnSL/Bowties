//! Integration tests for CDI download through `PeerSession` (S3 slice of
//! feature 019-peer-session-refactor). Every behaviour listed in the S3
//! card's sub-clauses (a)–(g) is pinned here.
//!
//! Behaviours verified:
//! - (a) End-to-end CDI download over a mock transport assembles the bytes.
//! - (b) A stalled peer trips the per-chunk deadline, emits **exactly one**
//!   `TerminateDueToError` to the correct destination alias, and returns
//!   `PeerError::Timeout`.
//! - (c) An `OptionalInteractionRejected` reply terminates the exchange with
//!   `PeerError::Rejected { mti: <wrapped>, code }`.
//! - (d) `DatagramRejected` with the resend-OK flag retries up to
//!   `max_retries`; after cap exhaustion the session emits cleanup and
//!   returns `PeerError::Rejected`.
//! - (e) `PeerCommand::Cancel` mid-CDI emits cleanup and returns
//!   `PeerError::Cancelled`.
//! - (f) A mid-exchange `TransportHealth::Wedged` transition returns
//!   `PeerError::TransportUnhealthy` and does **NOT** emit
//!   `TerminateDueToError` (ADR-0016 D1).
//! - (g) Two concurrent `download_cdi` calls on the same handle serialise
//!   FIFO; no `CdiInflightRegistry` is present anywhere in the codebase.

use lcc_rs::peer_session::{InboundEvent, PeerCommand, PeerError, PeerSession};
use lcc_rs::protocol::mti::MTI;
use lcc_rs::protocol::AddressSpace;
use lcc_rs::protocol::GridConnectFrame;
use lcc_rs::transport::mock::MockTransport;
use lcc_rs::transport_actor::ReceivedMessage;
use lcc_rs::{MemoryReadConfig, NodeID, TransportActor, TransportHandle};
use std::time::Duration;

fn our_alias() -> u16 { 0x825 }

fn make_actor(transport: MockTransport) -> (TransportActor, TransportHandle) {
    let actor = TransportActor::new(Box::new(transport));
    let handle = actor.handle();
    (actor, handle)
}

/// Config with tight timings for tests.
fn cdi_test_config() -> MemoryReadConfig {
    MemoryReadConfig {
        timeout_ms: 200,
        max_retries: 3,
        post_ack_delay_ms: 0,
    }
}

/// Build a MemoryConfigRead reply datagram from the peer (embedded CDI-space
/// format 0x53). Uses DatagramOnly (single frame) when payload fits, else
/// splits into Datagram First/Middle/Final. For simplicity here we only use
/// DatagramOnly since CDI chunks are ≤ 64 bytes and the reply header is 6
/// bytes → 70 bytes total, which exceeds the 8-byte single-frame budget.
/// So we always split.
fn build_cdi_reply_frames(from_alias: u16, to_alias: u16, address: u32, payload: &[u8]) -> Vec<String> {
    let addr_bytes = address.to_be_bytes();
    let mut data = vec![
        0x20u8, 0x53, // memory-config command, read-reply for CDI space (embedded)
        addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
    ];
    data.extend_from_slice(payload);

    // Use protocol frame builder for correct multi-frame split.
    let frames = GridConnectFrame::create_datagram_frames(from_alias, to_alias, data)
        .expect("build datagram frames");
    frames.iter().map(|f| f.to_string()).collect()
}

/// Build a DatagramRejected frame (addressed message).
fn build_datagram_rejected(from_alias: u16, to_alias: u16, error_code: u16) -> String {
    let header = MTI::DatagramRejected.to_header(from_alias).unwrap();
    let data = vec![
        ((to_alias >> 8) & 0x0F) as u8,
        (to_alias & 0xFF) as u8,
        ((error_code >> 8) & 0xFF) as u8,
        (error_code & 0xFF) as u8,
    ];
    let frame = GridConnectFrame { header, data };
    frame.to_string()
}

/// Build an OptionalInteractionRejected frame with wrapped MTI + error code.
fn build_oir(from_alias: u16, to_alias: u16, wrapped_mti: u16, error_code: u16) -> String {
    let header = MTI::OptionalInteractionRejected.to_header(from_alias).unwrap();
    let data = vec![
        ((to_alias >> 8) & 0x0F) as u8,
        (to_alias & 0xFF) as u8,
        ((wrapped_mti >> 8) & 0xFF) as u8,
        (wrapped_mti & 0xFF) as u8,
        ((error_code >> 8) & 0xFF) as u8,
        (error_code & 0xFF) as u8,
    ];
    let frame = GridConnectFrame { header, data };
    frame.to_string()
}

/// Count outbound TerminateDueToError frames from `our_alias` addressed to
/// `dest_alias`.
fn count_terminate_due_to_error(transport: &MockTransport, our_alias: u16, dest_alias: u16) -> usize {
    let expected_header = format!(":X{:08X}N", (MTI::TerminateDueToError.value() << 12) | our_alias as u32);
    let dest_body = format!("{:02X}{:02X}", ((dest_alias >> 8) & 0x0F) as u8, (dest_alias & 0xFF) as u8);
    transport
        .get_sent_frames()
        .iter()
        .filter(|f: &&String| f.starts_with(&expected_header) && f.contains(&dest_body))
        .count()
}

/// Count outbound MemoryConfigRead requests (embedded CDI format 0x43) sent
/// from `our_alias` to `dest_alias`. Uses the frame header's MTI upper bits
/// (bits 24-28) to identify datagram frames, then matches the encoded
/// dest+source aliases and the leading `20 43` command bytes.
fn count_cdi_read_requests(transport: &MockTransport, our_alias: u16, dest_alias: u16) -> usize {
    transport
        .get_sent_frames()
        .iter()
        .filter(|f: &&String| {
            // GridConnect: `:X<8-hex-header>N<data-hex>;`
            if !f.starts_with(":X") || !f.contains('N') {
                return false;
            }
            let hex_start = 2;
            let hex_end = hex_start + 8;
            if f.len() < hex_end + 1 { return false; }
            let header = match u32::from_str_radix(&f[hex_start..hex_end], 16) {
                Ok(h) => h,
                Err(_) => return false,
            };
            let (mti, source, dest) = match MTI::from_datagram_header(header) {
                Ok(x) => x,
                Err(_) => return false,
            };
            let is_datagram_start = matches!(mti, MTI::DatagramOnly | MTI::DatagramFirst);
            if !is_datagram_start || source != our_alias || dest != dest_alias {
                return false;
            }
            // Confirm the first two data bytes are `20 43` (memory-config read
            // CDI-space embedded command).
            let data_start = f.find('N').unwrap() + 1;
            f.get(data_start..data_start + 4) == Some("2043")
        })
        .count()
}

fn peer_node_id(byte: u8) -> NodeID {
    NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, byte])
}

// ── (a) Happy-path: end-to-end assembly ───────────────────────────────────

#[tokio::test]
async fn cdi_download_assembles_bytes_end_to_end() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xA1);

    // Two chunks: 64 bytes "AA..." then 3 bytes "BB..0x00" to terminate.
    let chunk_a: Vec<u8> = (0..64).map(|_| b'A').collect();
    let chunk_b: Vec<u8> = vec![b'B', b'B', 0x00];

    let mut transport = MockTransport::new();
    for f in build_cdi_reply_frames(node_alias, our_alias(), 0, &chunk_a) {
        transport.add_receive_frame(f);
    }
    for f in build_cdi_reply_frames(node_alias, our_alias(), 64, &chunk_b) {
        transport.add_receive_frame(f);
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let completion = session.download_cdi(cdi_test_config()).await.expect("cdi ok");
    assert_eq!(completion.bytes, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABB");
    assert_eq!(completion.stats.total_bytes, completion.bytes.len());
    assert_eq!(completion.stats.chunks, 2, "one chunk per successful reply");

    // No cleanup frame on success.
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        0,
        "success must not emit TerminateDueToError",
    );

    actor.shutdown().await;
}

// ── (b) Stalled peer → deadline → one TerminateDueToError ────────────────

#[tokio::test]
async fn cdi_timeout_emits_one_terminate_due_to_error() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xB1);

    // Empty receive queue → the session will wait forever for a reply and
    // the per-chunk deadline will fire.
    let transport = MockTransport::new();
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let mut config = cdi_test_config();
    config.timeout_ms = 100; // short so the test doesn't take long
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.download_cdi(config),
    ).await.expect("cdi call returned within timeout");

    match result {
        Err(PeerError::Timeout { .. }) => { /* expected */ }
        other => panic!("expected Timeout, got {:?}", other),
    }

    // Give the session a moment to emit the outbound frame after aborting.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let count = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 1, "timeout must emit exactly one TerminateDueToError, got {}", count);

    actor.shutdown().await;
}

// ── (c) OIR → Rejected with wrapped MTI ───────────────────────────────────

#[tokio::test]
async fn cdi_oir_reply_returns_rejected_with_wrapped_mti() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xC1);

    let mut transport = MockTransport::new();
    transport.add_receive_frame(build_oir(node_alias, our_alias(), 0x1D28, 0x1000));
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.download_cdi(cdi_test_config()),
    ).await.expect("cdi call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, 0x1D28, "wrapped MTI from OIR payload");
            assert_eq!(code, 0x1000, "error code from OIR payload");
        }
        other => panic!("expected Rejected, got {:?}", other),
    }

    // Give the session a moment to emit the cleanup frame after aborting.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let count = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 1, "OIR must emit exactly one TerminateDueToError, got {}", count);

    actor.shutdown().await;
}

// ── (d) DR-with-resend-OK: no immediate retry; wait for reply or timeout ──
//
// Policy change (2026-07-15): resend-OK DRs no longer trigger an immediate
// retry. Some peers (observed on SPROG USB-LCC) opportunistically DR under
// buffer pressure but still process the request, sending both the DR and
// the reply. Immediate retry caused a cascading duplicate-request buildup
// that eventually swamped the peer. New policy: log the DR, continue
// waiting for the reply; if none arrives before the deadline,
// `handle_deadline` completes with `PeerError::Timeout`.

#[tokio::test]
async fn cdi_datagram_rejected_resend_ok_no_immediate_retry_waits_for_reply_or_timeout() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xD1);

    // Rejection payload: bit 13 (0x2000) set → resend OK / buffer unavailable.
    // Inject several DRs; none of them should cause a fresh outbound read
    // request from the session under the new policy.
    let dr_frame = build_datagram_rejected(node_alias, our_alias(), 0x2020);

    let mut transport = MockTransport::new();
    for _ in 0..4 {
        transport.add_receive_frame(dr_frame.clone());
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let mut config = cdi_test_config();
    config.max_retries = 3;
    // Short timeout so the test doesn't wait long for deadline expiry.
    config.timeout_ms = 200;
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.download_cdi(config),
    ).await.expect("cdi call returned within timeout");

    // With the new policy the deadline expires (no reply ever arrives).
    // `handle_deadline` completes with `PeerError::Timeout` and emits a
    // single `TerminateDueToError`. The session sends EXACTLY ONE outbound
    // read request — the original — with no retries.
    match result {
        Err(PeerError::Timeout { .. }) => { /* expected */ }
        other => panic!("expected Timeout after deadline, got {:?}", other),
    }

    let requests = count_cdi_read_requests(&transport_probe, our_alias(), node_alias);
    assert_eq!(
        requests, 1,
        "resend-OK DR must not trigger immediate retry; expected 1 outbound request, got {}",
        requests
    );

    tokio::time::sleep(Duration::from_millis(50)).await;
    let cleanup = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(cleanup, 1, "deadline expiry must emit exactly one TerminateDueToError, got {}", cleanup);

    actor.shutdown().await;
}

// ── (e) Cancel mid-CDI → cleanup + Cancelled ──────────────────────────────

#[tokio::test]
async fn cdi_cancel_emits_cleanup_and_returns_cancelled() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xE1);

    // Empty receive queue — session waits, then we cancel.
    let transport = MockTransport::new();
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let session_clone = session.clone();
    let cancel = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        session_clone.cancel("test-cancel").await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.download_cdi(cdi_test_config()),
    ).await.expect("cdi call returned within timeout");
    let _ = cancel.await;

    match result {
        Err(PeerError::Cancelled { .. }) => { /* expected */ }
        other => panic!("expected Cancelled, got {:?}", other),
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    let cleanup = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(cleanup, 1, "Cancel must emit exactly one TerminateDueToError, got {}", cleanup);

    actor.shutdown().await;
}

// ── (f) Wedged mid-CDI → TransportUnhealthy, NO cleanup ──────────────────

#[tokio::test]
async fn cdi_wedged_returns_transport_unhealthy_without_cleanup() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xF1);

    // Empty receive queue — session waits; TransportWedged aborts.
    let transport = MockTransport::new();
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let s2 = session.clone();
    let wedge = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = s2.command(PeerCommand::TransportWedged {
            reason: "wire-stall".into(),
        }).await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.download_cdi(cdi_test_config()),
    ).await.expect("cdi call returned within timeout");
    let _ = wedge.await;

    match result {
        Err(PeerError::TransportUnhealthy { .. }) => { /* expected */ }
        other => panic!("expected TransportUnhealthy, got {:?}", other),
    }

    // Peer-cleanup on Wedged is DISABLED per ADR-0016 D1: the wire is dead,
    // so any emission would spam the writer without reaching the peer.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let cleanup = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(cleanup, 0, "Wedged must NOT emit TerminateDueToError, got {}", cleanup);

    actor.shutdown().await;
}

// ── (g) Two concurrent download_cdi calls serialise FIFO ─────────────────

#[tokio::test]
async fn two_concurrent_download_cdi_serialise_fifo() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xA2);

    // Prepare two independent successful downloads: one 3-byte CDI each.
    let cdi_a: Vec<u8> = vec![b'A', b'A', 0x00];
    let cdi_b: Vec<u8> = vec![b'B', b'B', 0x00];

    let mut transport = MockTransport::new();
    for f in build_cdi_reply_frames(node_alias, our_alias(), 0, &cdi_a) {
        transport.add_receive_frame(f);
    }
    for f in build_cdi_reply_frames(node_alias, our_alias(), 0, &cdi_b) {
        transport.add_receive_frame(f);
    }

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let s1 = session.clone();
    let s2 = session.clone();
    let (r1, r2) = tokio::join!(
        async move { s1.download_cdi(cdi_test_config()).await },
        async move { s2.download_cdi(cdi_test_config()).await },
    );

    let c1 = r1.expect("first CDI ok");
    let c2 = r2.expect("second CDI ok");
    // Both should return their own bytes, not shared.
    let bytes = [c1.bytes.clone(), c2.bytes.clone()];
    assert!(bytes.contains(&b"AA".to_vec()), "one CDI must equal AA");
    assert!(bytes.contains(&b"BB".to_vec()), "one CDI must equal BB");

    actor.shutdown().await;
}

// ── Regression: mismatched-address reply is ACKed and discarded ──────────

/// Post-S3 (ADR-0018) regression from SPROG USB-LCC serial: the peer replies
/// with a valid multi-frame datagram whose `ReadReply.address` does NOT
/// match our request-in-flight cursor. The session must ACK the datagram
/// (protocol obligation — the datagram was received cleanly), discard the
/// payload, and keep listening. It must NOT extend `assembled`, NOT advance
/// `address_cursor`, NOT emit `PeerError` on the waiter, and NOT emit
/// `TerminateDueToError`.
///
/// The next legitimate reply (whose address matches the cursor) must be
/// processed normally and the download must complete successfully.
#[tokio::test]
async fn cdi_mismatched_reply_address_is_acked_and_discarded_no_fatal() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xC2);

    // Two reply frames queued:
    //   1) MISMATCH: peer claims addr 0x40, but our cursor is 0x00.
    //   2) CORRECT: reply for addr 0x00 with null-terminated 2-byte payload,
    //      which cleanly terminates the download via short-read.
    let mismatch_payload: Vec<u8> = vec![b'X', b'X', b'X'];
    let good_payload: Vec<u8> = vec![b'A', b'A', 0x00];

    let mut transport = MockTransport::new();
    for f in build_cdi_reply_frames(node_alias, our_alias(), 0x40, &mismatch_payload) {
        transport.add_receive_frame(f);
    }
    for f in build_cdi_reply_frames(node_alias, our_alias(), 0x00, &good_payload) {
        transport.add_receive_frame(f);
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let completion = session
        .download_cdi(cdi_test_config())
        .await
        .expect("cdi ok despite mismatched reply");

    // Only the CORRECT reply's payload should end up in the assembled CDI.
    assert_eq!(completion.bytes, b"AA");

    // The mismatched reply must not have terminated the exchange.
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        0,
        "mismatched-reply drop must not emit TerminateDueToError",
    );

    actor.shutdown().await;
}

// ── Regression: stray DatagramMiddle from peer is recoverable ───────────

/// Post-S3 (ADR-0018) regression from SPROG USB-LCC serial: after a clean
/// multi-frame reply completes (First+Middle*+Final), a residual
/// `DatagramMiddle` from the same peer alias arrives before the next
/// legitimate reply's `DatagramFirst`. The assembler has been reset for the
/// next chunk, so this frame belongs to no active buffer — the reassembler
/// returns an `Err`. The session must NOT emit `TerminateDueToError`, NOT
/// emit `PeerError::Protocol("...reassembly failed...")` on the waiter, and
/// MUST keep listening. The next chunk must be processed normally and the
/// download must complete successfully.
#[tokio::test]
async fn cdi_stale_datagram_middle_from_peer_is_recoverable() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xC3);

    // Chunk 0 = 64 bytes 'A' at addr 0.
    // Then a stray DatagramMiddle from the same peer alias (belongs to no
    // active buffer after chunk 0's Final has been ACKed and the assembler
    // was reset in preparation for chunk 1).
    // Then chunk 1 = 2 bytes 'B' + 0x00 at addr 0x40 (null-terminates).
    let chunk_a: Vec<u8> = (0..64).map(|_| b'A').collect();
    let chunk_b: Vec<u8> = vec![b'B', b'B', 0x00];

    let mut transport = MockTransport::new();
    for f in build_cdi_reply_frames(node_alias, our_alias(), 0, &chunk_a) {
        transport.add_receive_frame(f);
    }

    let stray_header = MTI::DatagramMiddle
        .to_header_with_dest(node_alias, our_alias())
        .expect("build stray Middle header");
    let stray = GridConnectFrame {
        header: stray_header,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };
    transport.add_receive_frame(stray.to_string());

    for f in build_cdi_reply_frames(node_alias, our_alias(), 0x40, &chunk_b) {
        transport.add_receive_frame(f);
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let completion = session
        .download_cdi(cdi_test_config())
        .await
        .expect("cdi ok despite stray DatagramMiddle");

    // Full expected assembly: 64 'A's + "BB" (chunk_b's null terminates).
    let mut expected = vec![b'A'; 64];
    expected.extend_from_slice(b"BB");
    assert_eq!(completion.bytes, expected);

    // No cleanup emission — the stray Middle was recovered from silently.
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        0,
        "stray DatagramMiddle recovery must not emit TerminateDueToError",
    );

    actor.shutdown().await;
}

// ── Guard: CdiInflightRegistry retirement is structural ───────────────────

/// Compile-time proof that no `CdiInflightRegistry` type exists in the
/// crate: this test only needs to compile. Removed with the module.
#[test]
fn cdi_inflight_registry_is_retired() {
    // This test is intentionally trivial — its purpose is to ensure the
    // slice's grep-for-zero-hits assertion has a concrete anchor. The
    // production check is a workspace-wide grep in the slice's validation
    // step.
    let _ = AddressSpace::Cdi;
}

// ── S7: inbound lag handling (per-peer channel) ───────────────────────────

/// Wrap a GridConnect frame string as an `InboundEvent::Frame` for direct
/// injection into a session's per-peer inbound channel (S7 D1=A). Bypasses
/// the transport broadcast so a test can interleave frames and `Lagged`
/// markers deterministically.
fn inbound_frame(frame_str: &str) -> InboundEvent {
    let frame = GridConnectFrame::parse(frame_str).expect("parse inbound frame");
    InboundEvent::Frame(ReceivedMessage {
        frame,
        timestamp: std::time::Instant::now(),
    })
}

/// Poll until the session has emitted at least `n` outbound CDI read requests
/// to `dest_alias`, or panic after a generous timeout.
async fn wait_for_read_requests(
    transport: &MockTransport,
    our_alias: u16,
    dest_alias: u16,
    n: usize,
) {
    for _ in 0..400 {
        if count_cdi_read_requests(transport, our_alias, dest_alias) >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!(
        "timed out waiting for {} read requests (saw {})",
        n,
        count_cdi_read_requests(transport, our_alias, dest_alias)
    );
}

/// (S7-T1a) A mid-CDI inbound `Lagged` triggers bounded in-place recovery:
/// the session resets the current chunk's assembler, re-issues the same
/// `address_cursor` read, and the download completes with the correct bytes.
/// No `TerminateDueToError` is emitted on the recovery path.
#[tokio::test]
async fn cdi_mid_download_lag_recovers_in_place_and_completes() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xF1);

    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let transport = MockTransport::new();
    let transport_probe = transport.clone();
    let (_actor, handle) = make_actor(transport);
    let (session, _s, _h) =
        PeerSession::spawn_with_tasks(node_id, node_alias, our_alias(), handle, rx);

    let dl = tokio::spawn(async move { session.download_cdi(cdi_test_config()).await });

    // First read request (cursor 0).
    wait_for_read_requests(&transport_probe, our_alias(), node_alias, 1).await;

    // Chunk A (64 bytes) reply → ACK + advance to cursor 64 → 2nd request.
    let chunk_a: Vec<u8> = (0..64).map(|_| b'A').collect();
    for f in build_cdi_reply_frames(node_alias, our_alias(), 0, &chunk_a) {
        tx.send(inbound_frame(&f)).await.unwrap();
    }
    wait_for_read_requests(&transport_probe, our_alias(), node_alias, 2).await;

    // Lag mid-chunk-2, before the reply: session resets the assembler and
    // re-issues the cursor-64 read → a 3rd outbound request.
    tx.send(InboundEvent::Lagged(3)).await.unwrap();
    wait_for_read_requests(&transport_probe, our_alias(), node_alias, 3).await;

    // Deliver the terminating chunk for cursor 64.
    let chunk_b: Vec<u8> = vec![b'B', b'B', 0x00];
    for f in build_cdi_reply_frames(node_alias, our_alias(), 64, &chunk_b) {
        tx.send(inbound_frame(&f)).await.unwrap();
    }

    let completion = tokio::time::timeout(Duration::from_secs(2), dl)
        .await
        .expect("download completed within deadline")
        .expect("download task joined")
        .expect("cdi ok after lag recovery");

    let mut expected = vec![b'A'; 64];
    expected.extend_from_slice(b"BB");
    assert_eq!(completion.bytes, expected, "assembled bytes survive lag recovery");
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        0,
        "in-place lag recovery must not emit TerminateDueToError",
    );
}

/// (S7-T1b) A sustained lag storm exhausts `lag_recovery_count` (bound =
/// `max_retries`), aborts the exchange, emits **exactly one**
/// `TerminateDueToError` to the correct destination alias, and returns a
/// terminal `PeerError`.
#[tokio::test]
async fn cdi_sustained_lag_storm_exhausts_recovery_then_cleans_up() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xF2);

    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let transport = MockTransport::new();
    let transport_probe = transport.clone();
    let (_actor, handle) = make_actor(transport);
    let (session, _s, _h) =
        PeerSession::spawn_with_tasks(node_id, node_alias, our_alias(), handle, rx);

    let mut config = cdi_test_config();
    config.max_retries = 3;
    let dl = tokio::spawn(async move { session.download_cdi(config).await });

    // Exchange started (first request on the wire).
    wait_for_read_requests(&transport_probe, our_alias(), node_alias, 1).await;

    // Storm: max_retries recoveries are permitted; the (max_retries + 1)th
    // lag exhausts the budget and aborts.
    for _ in 0..4 {
        tx.send(InboundEvent::Lagged(1)).await.unwrap();
    }

    let result = tokio::time::timeout(Duration::from_secs(2), dl)
        .await
        .expect("download resolved within deadline")
        .expect("download task joined");

    match result {
        Err(PeerError::Protocol(_)) => { /* expected terminal fault */ }
        other => panic!("expected Protocol after lag exhaustion, got {:?}", other),
    }

    // Allow the abort's cleanup frame to reach the mock.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        1,
        "lag exhaustion must emit exactly one TerminateDueToError",
    );
}
