//! Protocol Identification Protocol (PIP) implementation
//!
//! PIP lets us query which optional LCC protocols a node supports before
//! attempting operations that require them (e.g. CDI/Memory Configuration).

use crate::protocol::frame::GridConnectFrame;
use crate::protocol::mti::MTI;
use crate::transport_actor::TransportHandle;
use crate::types::{PIPStatus, ProtocolFlags};
use crate::{Error, Result};
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
use std::sync::Arc;

/// Timeout for the complete PIP round-trip (request + reply).
/// PIP is a single addressed message exchange, so 2 s is generous.
const PIP_TIMEOUT: Duration = Duration::from_secs(2);

/// How long to wait for a frame before declaring silence.
const SILENCE_TIMEOUT: Duration = Duration::from_millis(200);

/// Query PIP data from a specific node using a TransportHandle (channel-based).
///
/// # Arguments
/// * `handle` - Transport handle for sending and subscribing
/// * `source_alias` - Our alias (source of the request)
/// * `dest_alias` - Target node's alias
/// * `semaphore` - Semaphore for concurrency limiting
///
/// # Returns
/// * `Ok((Some(ProtocolFlags), PIPStatus::Complete))` on success
/// * `Ok((None, PIPStatus::Timeout))` when the node does not reply
/// * `Err(_)` on transport errors
pub async fn query_pip(
    handle: &TransportHandle,
    source_alias: u16,
    dest_alias: u16,
    semaphore: Arc<Semaphore>,
) -> Result<(Option<ProtocolFlags>, PIPStatus)> {
    let _permit = semaphore.acquire().await.map_err(|e| {
        Error::Protocol(format!("Failed to acquire PIP semaphore: {}", e))
    })?;

    match timeout(PIP_TIMEOUT, query_pip_handle_internal(handle, source_alias, dest_alias)).await {
        Ok(result) => result,
        Err(_) => Ok((None, PIPStatus::Timeout)),
    }
}

/// Channel-based PIP query implementation using TransportHandle.
async fn query_pip_handle_internal(
    handle: &TransportHandle,
    source_alias: u16,
    dest_alias: u16,
) -> Result<(Option<ProtocolFlags>, PIPStatus)> {
    // Subscribe BEFORE sending so we cannot miss the reply.
    let mut rx = handle.subscribe_all();

    let request = GridConnectFrame::from_addressed_mti(
        MTI::ProtocolSupportInquiry,
        source_alias,
        dest_alias,
        vec![],
    )?;
    handle.send(&request).await?;

    loop {
        match timeout(SILENCE_TIMEOUT, rx.recv()).await {
            Ok(Ok(msg)) => {
                let frame = &msg.frame;
                let (mti, source) = match MTI::from_header(frame.header) {
                    Ok(result) => result,
                    Err(_) => continue,
                };

                if source != dest_alias {
                    continue;
                }

                // Filter by destination alias to prevent interleaving when
                // another node (e.g. JMRI) queries the same target.
                if frame.data.len() >= 2 {
                    let dest_in_frame = ((frame.data[0] as u16 & 0x0F) << 8) | frame.data[1] as u16;
                    if dest_in_frame != source_alias {
                        continue;
                    }
                }

                // D20: OptionalInteractionRejected — node does not support PIP.
                if mti == MTI::OptionalInteractionRejected {
                    eprintln!("[PIP] {:03X}: OptionalInteractionRejected — node does not support PIP", dest_alias);
                    return Ok((None, PIPStatus::Timeout));
                }

                if mti != MTI::ProtocolSupportReply {
                    continue;
                }

                if frame.data.len() < 2 {
                    eprintln!("PIP: Reply frame too short ({} bytes)", frame.data.len());
                    return Err(Error::Protocol(format!(
                        "PIP reply frame too short: {} bytes",
                        frame.data.len()
                    )));
                }

                let flags_bytes = &frame.data[2..];
                let flags = ProtocolFlags::from_bytes(flags_bytes);

                let mut supported: Vec<&str> = Vec::new();
                if flags.simple_protocol       { supported.push("SimpleProtocol") }
                if flags.datagram              { supported.push("Datagram") }
                if flags.stream                { supported.push("Stream") }
                if flags.memory_configuration  { supported.push("MemoryConfig") }
                if flags.event_exchange        { supported.push("EventExchange") }
                if flags.identification        { supported.push("Identification") }
                if flags.acdi                  { supported.push("ACDI") }
                if flags.snip                  { supported.push("SNIP") }
                if flags.cdi                   { supported.push("CDI") }
                if flags.traction_control      { supported.push("Traction") }
                if flags.simple_train_node     { supported.push("SimpleTrainNode") }
                if flags.firmware_upgrade      { supported.push("FirmwareUpgrade") }
                eprintln!(
                    "[PIP] {:03X}: {}",
                    dest_alias,
                    if supported.is_empty() { "(none)".to_string() } else { supported.join(", ") },
                );

                return Ok((Some(flags), PIPStatus::Complete));
            }
            Ok(Err(_)) => {
                return Ok((None, PIPStatus::Timeout));
            }
            Err(_) => {
                return Ok((None, PIPStatus::Timeout));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::frame::GridConnectFrame;
    use crate::protocol::mti::MTI;

    /// Build a mock ProtocolSupportReply frame addressed to `our_alias` from
    /// `node_alias` with the given protocol flag bytes.
    fn make_pip_reply(our_alias: u16, node_alias: u16, flag_bytes: &[u8]) -> GridConnectFrame {
        // data = [dest_alias_hi, dest_alias_lo, flags...]
        let mut data = vec![(our_alias >> 8) as u8 & 0x0F, (our_alias & 0xFF) as u8];
        data.extend_from_slice(flag_bytes);
        GridConnectFrame::from_addressed_mti(MTI::ProtocolSupportReply, node_alias, our_alias, flag_bytes.to_vec())
            .expect("frame construction failed")
    }

    #[test]
    fn test_protocol_flags_from_bytes_jmri() {
        // JMRI typically reports: datagram + event_exchange + identification + SNIP
        // but NOT CDI or memory_configuration.
        // Byte 0: 0b0100_0110 = datagram(6) + event_exchange(2) + identification(1) = 0x46
        // Byte 1: 0b0001_0000 = snip(4) = 0x10
        let flags = ProtocolFlags::from_bytes(&[0x46, 0x10, 0x00, 0x00, 0x00, 0x00]);
        assert!(!flags.simple_protocol);
        assert!(flags.datagram);
        assert!(!flags.memory_configuration);
        assert!(flags.event_exchange);
        assert!(flags.identification);
        assert!(!flags.cdi);
        assert!(flags.snip);
    }

    #[test]
    fn test_protocol_flags_from_bytes_full_node() {
        // A fully-featured node: all of byte 0 + cdi + memory_configuration + snip
        // Byte 0: 0xFF (all 8 bits)
        // Byte 1: 0b0001_1000 = snip(4) + cdi(3) = 0x18
        let flags = ProtocolFlags::from_bytes(&[0xFF, 0x18]);
        assert!(flags.simple_protocol);
        assert!(flags.datagram);
        assert!(flags.memory_configuration);
        assert!(flags.cdi);
        assert!(flags.snip);
    }

    #[test]
    fn test_protocol_flags_from_bytes_short() {
        // Fewer than 6 bytes — trailing bytes treated as zero.
        let flags = ProtocolFlags::from_bytes(&[0x80]);
        assert!(flags.simple_protocol);
        assert!(!flags.datagram);
        assert!(!flags.cdi);
        assert!(!flags.snip);
    }

    #[test]
    fn test_protocol_flags_from_bytes_empty() {
        let flags = ProtocolFlags::from_bytes(&[]);
        assert!(!flags.simple_protocol);
        assert!(!flags.cdi);
    }

    // --- D20: OIR fast-fail tests ---

    #[tokio::test]
    async fn test_pip_oir_fast_fail() {
        use crate::transport::mock::MockTransport;
        use crate::transport_actor::TransportActor;

        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        let oir_frame = GridConnectFrame::from_addressed_mti(
            MTI::OptionalInteractionRejected,
            node_alias,
            our_alias,
            vec![0x10, 0x43],
        ).unwrap();

        let mut transport = MockTransport::new();
        transport.add_receive_frame(oir_frame.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let semaphore = Arc::new(Semaphore::new(1));
        let result = query_pip(&handle, our_alias, node_alias, semaphore).await;
        actor.shutdown().await;

        assert!(result.is_ok());
        let (flags, status) = result.unwrap();
        assert!(flags.is_none(), "OIR should return None flags");
        assert_eq!(status, PIPStatus::Timeout, "OIR should report Timeout status");
    }

    #[tokio::test]
    async fn test_pip_normal_reply_still_works() {
        use crate::transport::mock::MockTransport;
        use crate::transport_actor::TransportActor;

        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        let reply = make_pip_reply(our_alias, node_alias, &[0x50, 0x18, 0x00, 0x00, 0x00, 0x00]);
        let mut transport = MockTransport::new();
        transport.add_receive_frame(reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let semaphore = Arc::new(Semaphore::new(1));
        let result = query_pip(&handle, our_alias, node_alias, semaphore).await;
        actor.shutdown().await;

        assert!(result.is_ok());
        let (flags, status) = result.unwrap();
        assert!(flags.is_some());
        assert_eq!(status, PIPStatus::Complete);
        let f = flags.unwrap();
        assert!(f.datagram);
        assert!(f.snip);
        assert!(f.cdi);
    }

    /// Replies addressed to a different destination are ignored.
    /// Simulates Bowties and JMRI both querying the same node for PIP.
    #[tokio::test]
    async fn test_pip_ignores_other_destinations() {
        use crate::transport::mock::MockTransport;
        use crate::transport_actor::TransportActor;

        let our_alias: u16 = 0x825;
        let jmri_alias: u16 = 0x6AD;
        let node_alias: u16 = 0x036;

        // JMRI's PIP reply (different flags: just datagram + event_exchange)
        let jmri_reply = make_pip_reply(jmri_alias, node_alias, &[0x44, 0x00, 0x00, 0x00, 0x00, 0x00]);
        // Our PIP reply (datagram + SNIP + CDI)
        let our_reply = make_pip_reply(our_alias, node_alias, &[0x50, 0x18, 0x00, 0x00, 0x00, 0x00]);

        let mut transport = MockTransport::new();
        // JMRI's reply arrives first — should be filtered out
        transport.add_receive_frame(jmri_reply.to_string());
        transport.add_receive_frame(our_reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let semaphore = Arc::new(Semaphore::new(1));
        let result = query_pip(&handle, our_alias, node_alias, semaphore).await;
        actor.shutdown().await;

        let (flags, status) = result.unwrap();
        assert_eq!(status, PIPStatus::Complete);
        let f = flags.unwrap();
        // Must see OUR flags (datagram + SNIP + CDI), not JMRI's
        assert!(f.datagram);
        assert!(f.snip);
        assert!(f.cdi);
    }
}
