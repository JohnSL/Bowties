//! Integration tests for `PeerSessionRegistry` lifecycle.
//!
//! Behaviours verified:
//! 1. `registry.shutdown()` aborts the spawn-watcher task so its
//!    `Arc<RegistryInner>` clone (and the transitively-held broadcast
//!    sender) is released. Regression guard for the post-ADR-0016
//!    Windows serial reconnect failure (`COM7: Access is denied`) caused
//!    by a self-referential Arc closure: the watcher subscribed to the
//!    transport's broadcast channel and would only exit on
//!    `RecvError::Closed`, but the watcher itself held a `TransportHandle`
//!    (via `Arc<RegistryInner>`) whose `all_tx` sender kept the channel
//!    open. Dropping the registry only *detached* the `JoinHandle`, so
//!    the watcher lived forever and pinned the transport (and the OS
//!    serial handle).

use lcc_rs::peer_session::InboundEvent;
use lcc_rs::peer_session_registry::PeerSessionRegistry;
use lcc_rs::transport::mock::MockTransport;
use lcc_rs::{TransportActor, TransportHandle};
use std::time::Duration;
use tokio::sync::broadcast;

fn our_alias() -> u16 {
    0x825
}

fn make_actor(transport: MockTransport) -> (TransportActor, TransportHandle) {
    let actor = TransportActor::new(Box::new(transport));
    let handle = actor.handle();
    (actor, handle)
}

#[tokio::test]
async fn registry_shutdown_aborts_spawn_watcher_and_releases_transport() {
    let (mut actor, handle) = make_actor(MockTransport::new());

    // Probe receiver on the same broadcast channel the watcher subscribes to.
    // When every sender is dropped the probe's `recv()` returns `Closed`.
    let mut probe = handle.subscribe_all();

    let registry = PeerSessionRegistry::new(handle.clone(), our_alias());

    // The new API: shutdown must abort the spawn-watcher task so it
    // releases its `Arc<RegistryInner>` clone (and thus its transitively
    // held `all_tx` sender).
    registry.shutdown().await;

    // Drop every other outside sender. Only the actor's own senders and
    // our locally-held `handle` clone remain; after we drop them and shut
    // the actor down the broadcast channel must fully close — which can
    // only happen if the watcher's clone has already been released.
    drop(registry);
    drop(handle);
    actor.shutdown().await;
    drop(actor);

    let result = tokio::time::timeout(Duration::from_millis(100), probe.recv()).await;
    assert!(
        matches!(result, Ok(Err(broadcast::error::RecvError::Closed))),
        "expected broadcast channel to be Closed after registry.shutdown() + drops; \
         got {:?}. The spawn-watcher task is still holding a TransportHandle clone.",
        result
    );
}

// ── S7-T7: inbound-demux construction-window contract ─────────────────────

/// Build an addressed frame (MTI 0x19A08, SNIPResponse-shaped) from `source`
/// to `dest`, tagged with a distinguishing `marker` byte in data[2]. Its
/// routing key (source alias) is decoded from the header.
fn addressed_frame_from(source: u16, dest: u16, marker: u8) -> String {
    let header = (0x19A08u32 << 12) | source as u32;
    let dest_hi = ((dest >> 8) & 0x0F) as u8;
    let dest_lo = (dest & 0xFF) as u8;
    let data = vec![dest_hi, dest_lo, marker];
    let data_hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
    format!(":X{:08X}N{};", header, data_hex)
}

/// A frame whose source alias has no registered route (the alias
/// construction window, S7-T6) is deterministically **dropped** — the
/// documented-drop contract. A happens-before sync peer (`Y`, whose route is
/// already registered) proves the demux processed and discarded the
/// pre-registration frame for `X` before `X`'s route was inserted; the route
/// created afterward then receives only the post-registration frame.
#[tokio::test]
async fn construction_window_frame_before_route_is_dropped() {
    let x_alias: u16 = 0x3AE;
    let y_alias: u16 = 0x4C1;

    let transport = MockTransport::new();
    let mut feed = transport.clone();
    let (mut actor, handle) = make_actor(transport);

    let registry = PeerSessionRegistry::new(handle, our_alias());

    // Sync peer Y: route registered up front so its delivery is a
    // happens-before marker for the demux processing order.
    let mut rx_y = registry.insert_route_for_test(y_alias).await;

    // Pre-registration frame for X (no X route yet) followed by Y's marker.
    feed.add_receive_frame(addressed_frame_from(x_alias, our_alias(), 0x11));
    feed.add_receive_frame(addressed_frame_from(y_alias, our_alias(), 0x22));

    // Y's marker proves the demux has already processed — and dropped —
    // X's pre-registration frame.
    let y_evt = tokio::time::timeout(Duration::from_secs(2), rx_y.recv())
        .await
        .expect("Y marker delivered within deadline")
        .expect("Y route open");
    match y_evt {
        InboundEvent::Frame(msg) => {
            assert_eq!(msg.frame.data.get(2), Some(&0x22), "Y marker frame");
        }
        other => panic!("expected Y frame, got {:?}", other),
    }

    // Register X's route now, then inject a post-registration frame for X.
    let mut rx_x = registry.insert_route_for_test(x_alias).await;
    feed.add_receive_frame(addressed_frame_from(x_alias, our_alias(), 0x33));

    let x_evt = tokio::time::timeout(Duration::from_secs(2), rx_x.recv())
        .await
        .expect("X frame delivered within deadline")
        .expect("X route open");
    match x_evt {
        InboundEvent::Frame(msg) => assert_eq!(
            msg.frame.data.get(2),
            Some(&0x33),
            "only the post-registration frame is delivered; the pre-registration \
             frame (0x11) was dropped, not buffered",
        ),
        other => panic!("expected X frame, got {:?}", other),
    }

    // No earlier (0x11) frame and no spurious Lagged were buffered for X.
    assert!(
        rx_x.try_recv().is_err(),
        "X route must hold nothing beyond the single post-registration frame",
    );

    registry.shutdown().await;
    actor.shutdown().await;
}
