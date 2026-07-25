//! Integration tests for single-datagram config read + write through
//! `PeerSession` (S4 slice of feature 019-peer-session-refactor). Every
//! behaviour listed in the S4 card's sub-clauses (a)–(g) is pinned here.
//!
//! Behaviours verified:
//! - (a) `handle.read_memory(space, address, count)` over a mock peer returns
//!   the reply bytes + timing.
//! - (b) `handle.write_memory(space, address, data)` completes on
//!   `DatagramReceivedOk` (RequestWithNoReply).
//! - (c) A read whose peer stalls trips the per-op deadline, emits **exactly
//!   one** `TerminateDueToError`, and returns `PeerError::Timeout`.
//! - (d) `OptionalInteractionRejected` mid-read → `PeerError::Rejected { mti,
//!   code }`.
//! - (e) Two concurrent `read_memory` ops on one handle serialise FIFO: the
//!   second request is not sent until the first op's ACK — no interleave.
//! - (f) A concurrent `download_cdi` + `read_memory` to the SAME session
//!   produce exactly one ACK per reply datagram and no assembler
//!   cross-contamination (the 2026-07-18 collision regression).
//! - (g) Mid-read `TransportWedged` → `PeerError::TransportUnhealthy`, no
//!   cleanup emission (ADR-0016 D1).

use lcc_rs::peer_session::{PeerCommand, PeerError, PeerSession};
use lcc_rs::protocol::mti::MTI;
use lcc_rs::protocol::{
    build_datagram_received_ok_frame, build_datagram_rejected_frame,
    AddressSpace, GridConnectFrame, MemoryConfigCmd,
};
use lcc_rs::transport::mock::MockTransport;
use lcc_rs::{MemoryReadConfig, NodeID, TransportActor, TransportHandle};
use std::time::Duration;

fn our_alias() -> u16 { 0x825 }

fn make_actor(transport: MockTransport) -> (TransportActor, TransportHandle) {
    let actor = TransportActor::new(Box::new(transport));
    let handle = actor.handle();
    (actor, handle)
}

fn peer_node_id(byte: u8) -> NodeID {
    NodeID::new([0x02, 0x01, 0x57, 0x00, 0x00, byte])
}

/// Build a MemoryConfigRead SUCCESS reply datagram from the peer as GridConnect
/// wire strings. Delegates payload encoding to `MemoryConfigCmd::build_read_reply_success`
/// (the codec-pair partner of `MemoryConfigCmd::parse_read_reply`) and adds the
/// datagram framing on top; this wrapper exists only to return `Vec<String>`
/// for `MockTransport::add_receive_frame`.
fn build_read_reply_frames(
    from_alias: u16,
    to_alias: u16,
    space: AddressSpace,
    address: u32,
    payload: &[u8],
) -> Vec<String> {
    let data = MemoryConfigCmd::build_read_reply_success(space, address, payload);
    let frames = GridConnectFrame::create_datagram_frames(from_alias, to_alias, data)
        .expect("build datagram frames");
    frames.iter().map(|f| f.to_string()).collect()
}

/// Build a MemoryConfigRead FAILED reply datagram from the peer as GridConnect
/// wire strings. See `build_read_reply_frames` for the codec-pair rationale;
/// this variant carries a peer-reported read failure code inside an
/// otherwise-valid memory-config reply.
fn build_read_reply_failed_frames(
    from_alias: u16,
    to_alias: u16,
    space: AddressSpace,
    address: u32,
    error_code: u16,
) -> Vec<String> {
    let data = MemoryConfigCmd::build_read_reply_failed(space, address, error_code);
    let frames = GridConnectFrame::create_datagram_frames(from_alias, to_alias, data)
        .expect("build datagram frames");
    frames.iter().map(|f| f.to_string()).collect()
}

/// Build a DatagramReceivedOk frame from the peer addressed to us, as a
/// GridConnect wire string. Thin adapter over
/// `lcc_rs::protocol::build_datagram_received_ok_frame` for
/// `MockTransport::add_receive_frame`.
fn build_datagram_received_ok(from_alias: u16, to_alias: u16, flags: u8) -> String {
    build_datagram_received_ok_frame(from_alias, to_alias, flags).to_string()
}

/// Build an OptionalInteractionRejected frame with wrapped MTI + error code.
/// Byte-order authority: `lcc_rs::protocol::build_oir_payload`.
fn build_oir(from_alias: u16, to_alias: u16, wrapped_mti: u16, error_code: u16) -> String {
    let header = MTI::OptionalInteractionRejected.to_header(from_alias).unwrap();
    let data = lcc_rs::protocol::build_oir_payload(to_alias, error_code, wrapped_mti);
    GridConnectFrame { header, data }.to_string()
}

/// Build a DatagramRejected frame (addressed message) as a GridConnect wire
/// string. Thin adapter over `lcc_rs::protocol::build_datagram_rejected_frame`
/// for `MockTransport::add_receive_frame`.
fn build_datagram_rejected(from_alias: u16, to_alias: u16, error_code: u16) -> String {
    build_datagram_rejected_frame(from_alias, to_alias, error_code).to_string()
}

/// Count outbound TerminateDueToError frames from `our_alias` to `dest_alias`.
fn count_terminate_due_to_error(transport: &MockTransport, our_alias: u16, dest_alias: u16) -> usize {
    let expected_header = format!(":X{:08X}N", (MTI::TerminateDueToError.value() << 12) | our_alias as u32);
    let dest_body = format!("{:02X}{:02X}", ((dest_alias >> 8) & 0x0F) as u8, (dest_alias & 0xFF) as u8);
    transport
        .get_sent_frames()
        .iter()
        .filter(|f: &&String| f.starts_with(&expected_header) && f.contains(&dest_body))
        .count()
}

/// Count outbound DatagramReceivedOk (ACK) frames from `our_alias` to
/// `dest_alias`.
fn count_acks(transport: &MockTransport, our_alias: u16, dest_alias: u16) -> usize {
    let expected_header = format!(":X{:08X}N", (MTI::DatagramReceivedOk.value() << 12) | our_alias as u32);
    let dest_body = format!("{:02X}{:02X}", ((dest_alias >> 8) & 0x0F) as u8, (dest_alias & 0xFF) as u8);
    transport
        .get_sent_frames()
        .iter()
        .filter(|f: &&String| f.starts_with(&expected_header) && f.contains(&dest_body))
        .count()
}

/// Classify each outbound frame as a memory-config read REQUEST (datagram
/// start with `20 41` embedded config command) or an ACK (DatagramReceivedOk),
/// preserving order. Returns a compact string like "RAR A" mapping R=request,
/// A=ack, other=skip.
fn request_ack_sequence(transport: &MockTransport, our_alias: u16, dest_alias: u16) -> String {
    let ack_header = format!(":X{:08X}N", (MTI::DatagramReceivedOk.value() << 12) | our_alias as u32);
    let dest_body = format!("{:02X}{:02X}", ((dest_alias >> 8) & 0x0F) as u8, (dest_alias & 0xFF) as u8);
    let mut seq = String::new();
    for f in transport.get_sent_frames() {
        // ACK?
        if f.starts_with(&ack_header) && f.contains(&dest_body) {
            seq.push('A');
            continue;
        }
        // Read request? datagram-start frame with `2041` command bytes.
        if f.starts_with(":X") && f.contains('N') {
            let hex_end = 10;
            if f.len() >= hex_end + 1 {
                if let Ok(header) = u32::from_str_radix(&f[2..hex_end], 16) {
                    if let Ok((mti, source, dest)) = MTI::from_datagram_header(header) {
                        let is_start = matches!(mti, MTI::DatagramOnly | MTI::DatagramFirst);
                        if is_start && source == our_alias && dest == dest_alias {
                            let data_start = f.find('N').unwrap() + 1;
                            if f.get(data_start..data_start + 4) == Some("2041") {
                                seq.push('R');
                            }
                        }
                    }
                }
            }
        }
    }
    seq
}

// ── (a) read_memory happy path: bytes + timing ────────────────────────────

#[tokio::test]
async fn read_memory_returns_bytes_and_timing() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xA1);

    // Config space 0xFD, address 0x10, read 4 bytes → reply command 0x51.
    let payload: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let mut transport = MockTransport::new();
    for f in build_read_reply_frames(node_alias, our_alias(), AddressSpace::Configuration, 0x10, &payload) {
        transport.add_receive_frame(f);
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let (data, timing) = session
        .read_memory(0xFD, 0x10, 4, 500)
        .await
        .expect("read ok");
    assert_eq!(data, payload, "reply bytes returned verbatim");
    assert!(timing.frame_count >= 1, "at least one reply frame counted");

    // Exactly one ACK for the single reply datagram; no cleanup on success.
    assert_eq!(count_acks(&transport_probe, our_alias(), node_alias), 1);
    assert_eq!(count_terminate_due_to_error(&transport_probe, our_alias(), node_alias), 0);

    actor.shutdown().await;
}

// ── (b) write_memory completes on DatagramReceivedOk ──────────────────────

#[tokio::test]
async fn write_memory_completes_on_datagram_received_ok() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xB1);

    let mut transport = MockTransport::new();
    // Peer ACKs the write with no reply-pending flag → success.
    transport.add_receive_frame(build_datagram_received_ok(node_alias, our_alias(), 0x00));
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    session
        .write_memory(0xFD, 0x20, vec![0x01, 0x02], 500)
        .await
        .expect("write ok");

    assert_eq!(count_terminate_due_to_error(&transport_probe, our_alias(), node_alias), 0);

    actor.shutdown().await;
}

/// Pins that a memory-write OIR reply preserves the TRUE error code (bytes
/// 2..4) in `PeerError::Rejected.code` — not the rejected MTI (bytes 4..6).
/// Constructed inline (not via `build_oir`), citing OpenLCB Java's
/// `OptionalIntRejectedMessage.toPayload` byte order.
#[tokio::test]
async fn write_memory_oir_preserves_error_code_in_rejected() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xB2);

    let error_code: u16 = 0x1040; // Permanent Error, Not Implemented (S-9.7.3 §3.5.5)
    let rejected_mti: u16 = 0x0A28; // DatagramReceivedOk (S-9.7.3.2 §4.2)

    let header = MTI::OptionalInteractionRejected.to_header(node_alias).unwrap();
    let data = vec![
        ((our_alias() >> 8) & 0x0F) as u8,
        (our_alias() & 0xFF) as u8,
        ((error_code >> 8) & 0xFF) as u8,
        (error_code & 0xFF) as u8,
        ((rejected_mti >> 8) & 0xFF) as u8,
        (rejected_mti & 0xFF) as u8,
    ];
    let frame = GridConnectFrame { header, data };

    let mut transport = MockTransport::new();
    transport.add_receive_frame(frame.to_string());
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.write_memory(0xFD, 0x20, vec![0x01, 0x02], 500),
    ).await.expect("write call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, rejected_mti as u32, "rejected MTI must come from bytes 4..6, not the hardcoded OIR MTI");
            assert_eq!(code, error_code, "error code must come from bytes 2..4");
        }
        other => panic!("expected Rejected, got {:?}", other),
    }

    // JMRI never emits TerminateDueToError after OIR (ADR-0019); Bowties
    // mirrors that.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        0,
        "OIR must not emit TerminateDueToError (JMRI-aligned)",
    );

    actor.shutdown().await;
}

// ── Non-resend DR: permanent rejection, no TDE (ADR-0019 Option B) ───────
//
// Policy (Option B): JMRI never emits TerminateDueToError after any
// peer-initiated terminal rejection, including a non-resend-OK
// DatagramRejected received directly (no retries attempted).
// complete_memory_write(Err(Rejected)) still fires, but no cleanup frame is
// written to the wire.

#[tokio::test]
async fn write_memory_datagram_rejected_permanent_returns_rejected_and_no_tde() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xB3);

    // Rejection payload: bit 13 (0x2000) clear → permanent rejection, no resend.
    let error_code: u16 = 0x1042;
    let dr_frame = build_datagram_rejected(node_alias, our_alias(), error_code);

    let mut transport = MockTransport::new();
    transport.add_receive_frame(dr_frame);
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.write_memory(0xFD, 0x20, vec![0x01, 0x02], 500),
    ).await.expect("write call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, MTI::DatagramRejected.value(), "rejected MTI must be DatagramRejected");
            assert_eq!(code, error_code, "error code must come from the DR payload");
        }
        other => panic!("expected Rejected, got {:?}", other),
    }

    // Per ADR-0019 Option B, JMRI never emits TerminateDueToError after any
    // peer-initiated terminal rejection; Bowties mirrors that for DR too.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let count = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 0, "non-resend DR must not emit TerminateDueToError (JMRI-aligned), got {}", count);

    actor.shutdown().await;
}

// ── Resend-OK DR retry-cap exhaustion: no TDE (ADR-0019 Option B) ────────
//
// Policy (Option B): after WRITE_MEMORY_MAX_RETRIES consecutive resend-OK
// DRs, the session stops retrying and completes the write — this is
// classified as "peer told us it's done N times, we chose to stop", not a
// wire fault. JMRI's MemoryConfigurationService caps at MAX_TRIES = 3 and
// also emits no TerminateDueToError; Bowties mirrors that here.

#[tokio::test]
async fn write_memory_retry_exhaustion_returns_rejected_and_no_tde() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xB4);

    // Resend-OK DR (bit 13 / 0x2000 set): the session retries up to
    // WRITE_MEMORY_MAX_RETRIES times; the (N+1)th DR exhausts the cap and
    // terminates the exchange.
    let error_code: u16 = 0x2020;
    let dr_frame = build_datagram_rejected(node_alias, our_alias(), error_code);

    let mut transport = MockTransport::new();
    for _ in 0..(lcc_rs::constants::WRITE_MEMORY_MAX_RETRIES + 1) {
        transport.add_receive_frame(dr_frame.clone());
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.write_memory(0xFD, 0x20, vec![0x01, 0x02], 500),
    ).await.expect("write call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, MTI::DatagramRejected.value(), "rejected MTI must be DatagramRejected");
            assert_eq!(code, error_code, "error code must come from the last (cap-exhausting) DR payload");
        }
        other => panic!("expected Rejected after retry exhaustion, got {:?}", other),
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    let count = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 0, "retry-cap exhaustion must not emit TerminateDueToError (JMRI-aligned), got {}", count);

    actor.shutdown().await;
}

// ── ReadReply::Failed (non-0x1082): peer-reported read failure, no TDE ───
//
// Policy (Option B): a memory-config read reply with the FAILED command bit
// set (e.g. 0x59 for embedded config space) is a peer-initiated terminal on
// par with OIR and DR: the peer sent a syntactically valid reply that
// reports its own read failure. Per ADR-0019, JMRI never emits
// TerminateDueToError for this class either; Bowties mirrors that.

#[tokio::test]
async fn read_memory_read_reply_failed_returns_rejected_and_no_tde() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xD4);

    let error_code: u16 = 0x1032;
    let mut transport = MockTransport::new();
    for f in build_read_reply_failed_frames(node_alias, our_alias(), AddressSpace::Configuration, 0x00, error_code) {
        transport.add_receive_frame(f);
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.read_memory(0xFD, 0x00, 8, 500),
    ).await.expect("read call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, MTI::DatagramRejected.value(), "rejected MTI must be DatagramRejected");
            assert_eq!(code, error_code, "error code must come from the read-reply payload");
        }
        other => panic!("expected Rejected, got {:?}", other),
    }

    // Per ADR-0019 Option B, JMRI never emits TerminateDueToError after a
    // peer-reported read failure inside a valid memory-config reply;
    // Bowties mirrors that for ReadReply::Failed too.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let count = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 0, "ReadReply::Failed must not emit TerminateDueToError (JMRI-aligned), got {}", count);

    actor.shutdown().await;
}

// ── (c) stalled read → deadline → one TerminateDueToError → Timeout ───────

#[tokio::test]
async fn read_memory_timeout_emits_one_terminate_due_to_error() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xC1);

    // Empty receive queue → read stalls, per-op deadline fires.
    let transport = MockTransport::new();
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.read_memory(0xFD, 0x00, 8, 100),
    ).await.expect("read call returned within timeout");

    match result {
        Err(PeerError::Timeout { .. }) => {}
        other => panic!("expected Timeout, got {:?}", other),
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        1,
        "timeout must emit exactly one TerminateDueToError",
    );

    actor.shutdown().await;
}

// ── (d) OIR mid-read → Rejected with wrapped MTI ──────────────────────────

#[tokio::test]
async fn read_memory_oir_returns_rejected_and_no_tde() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xD1);

    let mut transport = MockTransport::new();
    transport.add_receive_frame(build_oir(node_alias, our_alias(), 0x1C48, 0x1000));
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.read_memory(0xFD, 0x00, 8, 500),
    ).await.expect("read call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, 0x1C48, "wrapped MTI from OIR payload");
            assert_eq!(code, 0x1000, "error code from OIR payload");
        }
        other => panic!("expected Rejected, got {:?}", other),
    }

    // JMRI never emits TerminateDueToError after OIR (ADR-0019); Bowties
    // mirrors that.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        0,
        "OIR must not emit TerminateDueToError (JMRI-aligned)",
    );

    actor.shutdown().await;
}

/// Pins the OIR payload byte order against the OpenLCB Java reference
/// implementation directly, independent of `build_oir`. Per
/// `OptionalIntRejectedMessage.toPayload` (OpenLCB_Java), the payload after
/// the destination alias is: error code (2 bytes), then rejected MTI
/// (2 bytes). Constructed inline (not via `build_oir`) so this test does not
/// share a fixture with the bug it's pinning against.
#[tokio::test]
async fn read_memory_oir_decodes_payload_per_openlcb_java() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xD2);

    let error_code: u16 = 0x1040; // Permanent Error, Not Implemented (S-9.7.3 §3.5.5)
    let rejected_mti: u16 = 0x0A28; // DatagramReceivedOk (S-9.7.3.2 §4.2)

    let header = MTI::OptionalInteractionRejected.to_header(node_alias).unwrap();
    let data = vec![
        ((our_alias() >> 8) & 0x0F) as u8,
        (our_alias() & 0xFF) as u8,
        ((error_code >> 8) & 0xFF) as u8,
        (error_code & 0xFF) as u8,
        ((rejected_mti >> 8) & 0xFF) as u8,
        (rejected_mti & 0xFF) as u8,
    ];
    let frame = GridConnectFrame { header, data };

    let mut transport = MockTransport::new();
    transport.add_receive_frame(frame.to_string());

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.read_memory(0xFD, 0x00, 8, 500),
    ).await.expect("read call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, rejected_mti as u32, "rejected MTI must come from bytes 4..6");
            assert_eq!(code, error_code, "error code must come from bytes 2..4");
        }
        other => panic!("expected Rejected, got {:?}", other),
    }

    actor.shutdown().await;
}

// ── Non-resend DR: permanent rejection, no TDE (ADR-0019 Option B) ───────
//
// Policy (Option B): JMRI never emits TerminateDueToError after any
// peer-initiated terminal rejection, including a non-resend-OK
// DatagramRejected. Bowties extends the OIR-only JMRI-alignment fix to
// cover this DR-terminal class too: complete_memory_read(Err(Rejected))
// still fires, but no cleanup frame is written to the wire.

#[tokio::test]
async fn read_memory_datagram_rejected_permanent_returns_rejected_and_no_tde() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xD3);

    // Rejection payload: bit 13 (0x2000) clear → permanent rejection, no resend.
    let error_code: u16 = 0x1042;
    let dr_frame = build_datagram_rejected(node_alias, our_alias(), error_code);

    let mut transport = MockTransport::new();
    transport.add_receive_frame(dr_frame);
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.read_memory(0xFD, 0x00, 8, 500),
    ).await.expect("read call returned within timeout");

    match result {
        Err(PeerError::Rejected { mti, code }) => {
            assert_eq!(mti, MTI::DatagramRejected.value(), "rejected MTI must be DatagramRejected");
            assert_eq!(code, error_code, "error code must come from the DR payload");
        }
        other => panic!("expected Rejected, got {:?}", other),
    }

    // Per ADR-0019 Option B, JMRI never emits TerminateDueToError after any
    // peer-initiated terminal rejection; Bowties mirrors that for DR too.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let count = count_terminate_due_to_error(&transport_probe, our_alias(), node_alias);
    assert_eq!(count, 0, "non-resend DR must not emit TerminateDueToError (JMRI-aligned), got {}", count);

    actor.shutdown().await;
}

// ── (e) Two concurrent reads serialise FIFO (no interleave) ───────────────

#[tokio::test]
async fn two_concurrent_reads_serialise_no_interleave() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xE1);

    // Two distinct replies, one per read address.
    let payload_a: Vec<u8> = vec![0x11, 0x22, 0x33, 0x44];
    let payload_b: Vec<u8> = vec![0x55, 0x66, 0x77, 0x88];
    let mut transport = MockTransport::new();
    for f in build_read_reply_frames(node_alias, our_alias(), AddressSpace::Configuration, 0x00, &payload_a) {
        transport.add_receive_frame(f);
    }
    for f in build_read_reply_frames(node_alias, our_alias(), AddressSpace::Configuration, 0x40, &payload_b) {
        transport.add_receive_frame(f);
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let s1 = session.clone();
    let s2 = session.clone();
    let (r1, r2) = tokio::join!(
        async move { s1.read_memory(0xFD, 0x00, 4, 500).await },
        async move { s2.read_memory(0xFD, 0x40, 4, 500).await },
    );
    let d1 = r1.expect("first read ok").0;
    let d2 = r2.expect("second read ok").0;
    let both = [d1, d2];
    assert!(both.contains(&payload_a), "one read returns payload A");
    assert!(both.contains(&payload_b), "one read returns payload B");

    // The outbound request/ACK sequence must be two contiguous [R, A] pairs
    // ("RARA"), never interleaved ("RRAA") — proving strict serialisation.
    let seq = request_ack_sequence(&transport_probe, our_alias(), node_alias);
    assert_eq!(seq, "RARA", "reads must serialise: request then ACK, twice");

    actor.shutdown().await;
}

// ── (f) Concurrent download_cdi + read_memory: one ACK per reply, no cross ─

#[tokio::test]
async fn concurrent_cdi_and_read_no_interleave_one_ack_each() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0xF1);

    // CDI reply (space 0x53) — 3 bytes null-terminated → clean short-read.
    let cdi_payload: Vec<u8> = vec![b'C', b'D', 0x00];
    // Config reply (space 0x51) — 4 bytes at address 0x00.
    let cfg_payload: Vec<u8> = vec![0xAB, 0xCD, 0xEF, 0x01];

    let mut transport = MockTransport::new();
    // CDI reply queued FIRST so the CDI exchange (started first) consumes it;
    // the config read is queued behind it and only becomes active after CDI
    // completes.
    for f in build_read_reply_frames(node_alias, our_alias(), AddressSpace::Cdi, 0x00, &cdi_payload) {
        transport.add_receive_frame(f);
    }
    for f in build_read_reply_frames(node_alias, our_alias(), AddressSpace::Configuration, 0x00, &cfg_payload) {
        transport.add_receive_frame(f);
    }
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let cfg = MemoryReadConfig { timeout_ms: 1000, max_retries: 3, post_ack_delay_ms: 0 };
    let s_cdi = session.clone();
    let cdi_task = tokio::spawn(async move { s_cdi.download_cdi(cfg).await });
    // Ensure the CDI exchange is active first, then dispatch the config read
    // concurrently (queued behind CDI).
    tokio::time::sleep(Duration::from_millis(10)).await;
    let s_read = session.clone();
    let read_task = tokio::spawn(async move { s_read.read_memory(0xFD, 0x00, 4, 1000).await });

    let cdi_res = tokio::time::timeout(Duration::from_secs(2), cdi_task)
        .await.expect("cdi within time").expect("cdi join");
    let read_res = tokio::time::timeout(Duration::from_secs(2), read_task)
        .await.expect("read within time").expect("read join");

    let cdi = cdi_res.expect("cdi ok");
    let (cfg_data, _timing) = read_res.expect("read ok");

    // No cross-contamination: CDI bytes are "CD", config bytes are the config
    // payload — neither exchange consumed the other's reply.
    assert_eq!(cdi.bytes, b"CD", "CDI assembled only its own reply");
    assert_eq!(cfg_data, cfg_payload, "config read got only its own reply");

    // Exactly one ACK per reply datagram: two reply datagrams → two ACKs.
    // The 2026-07-18 regression produced ~2x ACKs from interleaved assembly.
    assert_eq!(
        count_acks(&transport_probe, our_alias(), node_alias),
        2,
        "exactly one ACK per reply datagram (no double-ACK)",
    );
    assert_eq!(count_terminate_due_to_error(&transport_probe, our_alias(), node_alias), 0);

    actor.shutdown().await;
}

// ── (g) Wedged mid-read → TransportUnhealthy, NO cleanup ──────────────────

#[tokio::test]
async fn read_memory_wedged_returns_transport_unhealthy_without_cleanup() {
    let node_alias: u16 = 0x3AE;
    let node_id = peer_node_id(0x91);

    let transport = MockTransport::new();
    let transport_probe = transport.clone();

    let (mut actor, handle) = make_actor(transport);
    let session = PeerSession::spawn(node_id, node_alias, our_alias(), handle);

    let s2 = session.clone();
    let wedge = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = s2.command(PeerCommand::TransportWedged { reason: "wire-stall".into() }).await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        session.read_memory(0xFD, 0x00, 8, 1000),
    ).await.expect("read call returned within timeout");
    let _ = wedge.await;

    match result {
        Err(PeerError::TransportUnhealthy { .. }) => {}
        other => panic!("expected TransportUnhealthy, got {:?}", other),
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        count_terminate_due_to_error(&transport_probe, our_alias(), node_alias),
        0,
        "Wedged must NOT emit TerminateDueToError",
    );

    actor.shutdown().await;
}
