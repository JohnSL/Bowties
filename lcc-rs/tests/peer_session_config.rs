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
use lcc_rs::protocol::GridConnectFrame;
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

/// Build a MemoryConfigRead reply datagram from the peer with the given
/// reply command byte (embedded format: 0x51 config, 0x53 CDI). Splits into
/// multiple datagram frames as needed by the protocol frame builder.
fn build_read_reply_frames(
    from_alias: u16,
    to_alias: u16,
    reply_cmd: u8,
    address: u32,
    payload: &[u8],
) -> Vec<String> {
    let addr_bytes = address.to_be_bytes();
    let mut data = vec![
        0x20u8, reply_cmd,
        addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
    ];
    data.extend_from_slice(payload);
    let frames = GridConnectFrame::create_datagram_frames(from_alias, to_alias, data)
        .expect("build datagram frames");
    frames.iter().map(|f| f.to_string()).collect()
}

/// Build a DatagramReceivedOk frame from the peer addressed to us.
fn build_datagram_received_ok(from_alias: u16, to_alias: u16, flags: u8) -> String {
    let header = MTI::DatagramReceivedOk.to_header(from_alias).unwrap();
    let data = vec![
        ((to_alias >> 8) & 0x0F) as u8,
        (to_alias & 0xFF) as u8,
        flags,
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
    for f in build_read_reply_frames(node_alias, our_alias(), 0x51, 0x10, &payload) {
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
async fn read_memory_oir_returns_rejected() {
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

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(count_terminate_due_to_error(&transport_probe, our_alias(), node_alias), 1);

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
    for f in build_read_reply_frames(node_alias, our_alias(), 0x51, 0x00, &payload_a) {
        transport.add_receive_frame(f);
    }
    for f in build_read_reply_frames(node_alias, our_alias(), 0x51, 0x40, &payload_b) {
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
    for f in build_read_reply_frames(node_alias, our_alias(), 0x53, 0x00, &cdi_payload) {
        transport.add_receive_frame(f);
    }
    for f in build_read_reply_frames(node_alias, our_alias(), 0x51, 0x00, &cfg_payload) {
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
