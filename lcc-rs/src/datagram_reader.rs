//! Unified datagram memory-read exchange.
//!
//! Provides [`DatagramExchange`] — a single async function that performs one
//! complete datagram read round-trip:
//!
//! 1. Send memory-config read request frame(s)
//! 2. Wait for DatagramReceivedOk / DatagramRejected (with resend-OK retry)
//! 3. Wait for the reply datagram (multi-frame assembly)
//! 4. ACK the reply
//! 5. Optionally sleep `post_ack_delay_ms` before returning
//!
//! All memory-config read paths (BatchReader, CDI download, single reads) share
//! this function, ensuring consistent timeout, retry, pacing, stale-reply
//! detection, and oversized-reply handling.

use crate::{
    constants::{DEFAULT_POST_ACK_DELAY_MS, MAX_DATAGRAM_RETRIES, READ_MEMORY_TIMEOUT_MS},
    discovery::MemoryReadTiming,
    protocol::{DatagramAssembler, MTI, MemoryConfigCmd, ReadReply},
    transport_actor::{ReceivedMessage, TransportHandle},
};
use tokio::sync::broadcast;
use tokio::time::{Duration, Instant};

/// Configuration for datagram read operations.
///
/// Constructed at the app layer (e.g. from `tuning.toml`) and threaded through
/// to `BatchReader`, CDI download, and single reads.
#[derive(Debug, Clone)]
pub struct MemoryReadConfig {
    /// Per-attempt timeout in milliseconds waiting for a read reply.
    pub timeout_ms: u64,
    /// Maximum retries when a node rejects with the "resend OK" flag (0x2000).
    pub max_retries: u32,
    /// Delay in milliseconds after ACK-ing a reply before returning.
    /// Gives CAN gateways time to finish forwarding the ACK frame.
    pub post_ack_delay_ms: u64,
}

impl Default for MemoryReadConfig {
    fn default() -> Self {
        Self {
            timeout_ms: READ_MEMORY_TIMEOUT_MS,
            max_retries: MAX_DATAGRAM_RETRIES,
            post_ack_delay_ms: DEFAULT_POST_ACK_DELAY_MS,
        }
    }
}

/// Result of a single datagram read exchange.
#[derive(Debug, Clone)]
pub struct ExchangeResult {
    /// The raw datagram payload (excluding the 6-byte memory-config header)
    /// on success, or an error message.
    pub data: std::result::Result<Vec<u8>, String>,
    /// Per-frame timing data (present even on some failure paths).
    pub timing: Option<MemoryReadTiming>,
    /// Number of resend retries that occurred during this exchange.
    pub retry_count: u32,
}

/// Describes what to read in a single exchange.
#[derive(Debug, Clone)]
pub struct ReadDescriptor {
    pub address_space: u8,
    pub address: u32,
    pub count: u8,
}

/// Perform one complete memory-config read datagram exchange.
///
/// This is the unified core of all read paths. It:
/// - Builds and sends the memory-config read request
/// - Handles DatagramReceivedOk (with timeout extension from flags)
/// - Handles DatagramRejected with resend-OK retry (capped at `config.max_retries`)
/// - Assembles multi-frame reply datagrams
/// - ACKs the reply
/// - Sleeps `config.post_ack_delay_ms` after the ACK (gateway pacing)
/// - Detects stale replies (wrong address) and discards them
/// - Detects oversized replies and retries
///
/// The caller provides a mutable `broadcast::Receiver` so that subscriptions
/// can be held open across multiple sequential exchanges (as in BatchReader or
/// CDI download loops).
pub async fn datagram_read_exchange(
    handle: &TransportHandle,
    rx: &mut broadcast::Receiver<ReceivedMessage>,
    our_alias: u16,
    dest_alias: u16,
    desc: &ReadDescriptor,
    config: &MemoryReadConfig,
    use_send_direct: bool,
) -> ExchangeResult {
    use crate::protocol::AddressSpace;

    let space = match AddressSpace::from_u8(desc.address_space) {
        Ok(s) => s,
        Err(e) => return ExchangeResult {
            data: Err(format!("Invalid address space: {}", e)),
            timing: None,
            retry_count: 0,
        },
    };

    let read_frames = match MemoryConfigCmd::build_read(
        our_alias, dest_alias, space, desc.address, desc.count,
    ) {
        Ok(f) => f,
        Err(e) => return ExchangeResult {
            data: Err(e.to_string()),
            timing: None,
            retry_count: 0,
        },
    };

    let mut retry_count: u32 = 0;

    'retry: loop {
        // Send the request frames.
        let send_time = Instant::now();
        for frame in read_frames.iter() {
            let send_result = if use_send_direct {
                handle.send_direct(frame).await
            } else {
                handle.send(frame).await
            };
            if let Err(e) = send_result {
                return ExchangeResult {
                    data: Err(e.to_string()),
                    timing: None,
                    retry_count,
                };
            }
        }

        // Wait for reply.
        let mut max_duration = Duration::from_millis(config.timeout_ms);
        let mut assembler = DatagramAssembler::new();
        let mut first_frame_latency_ms: Option<u64> = None;
        let mut last_frame_ms: u64 = 0;
        let mut frame_gaps_ms: Vec<u32> = Vec::new();
        let mut frame_count: u8 = 0;

        loop {
            match tokio::time::timeout(max_duration, rx.recv()).await {
                Ok(Ok(msg)) => {
                    // Check for datagram frames from dest_alias addressed to us.
                    let is_our_datagram = MTI::from_datagram_header(msg.frame.header)
                        .map(|(mti, src, dst)| {
                            let is_dg = matches!(
                                mti,
                                MTI::DatagramOnly
                                    | MTI::DatagramFirst
                                    | MTI::DatagramMiddle
                                    | MTI::DatagramFinal
                            );
                            is_dg && src == dest_alias && dst == our_alias
                        })
                        .unwrap_or(false);

                    if !is_our_datagram {
                        // Check for addressed control frames.
                        if let Ok((mti, src)) = MTI::from_header(msg.frame.header) {
                            if src == dest_alias && msg.frame.data.len() >= 2 {
                                let dst = ((msg.frame.data[0] as u16) << 8)
                                    | (msg.frame.data[1] as u16);
                                if dst == our_alias {
                                    // DatagramRejected — retry if resend-OK.
                                    if mti == MTI::DatagramRejected {
                                        let error_code = if msg.frame.data.len() >= 4 {
                                            ((msg.frame.data[2] as u16) << 8)
                                                | (msg.frame.data[3] as u16)
                                        } else {
                                            0
                                        };
                                        if error_code & 0x2000 != 0 {
                                            if retry_count >= config.max_retries {
                                                return ExchangeResult {
                                                    data: Err(format!(
                                                        "Datagram rejected (resend-OK) after {} retries: error 0x{:04X}",
                                                        retry_count, error_code
                                                    )),
                                                    timing: None,
                                                    retry_count,
                                                };
                                            }
                                            retry_count += 1;
                                            continue 'retry;
                                        } else {
                                            return ExchangeResult {
                                                data: Err(format!(
                                                    "Datagram rejected: error 0x{:04X}",
                                                    error_code
                                                )),
                                                timing: None,
                                                retry_count,
                                            };
                                        }
                                    }
                                    // DatagramReceivedOk — honour timeout extension.
                                    if mti == MTI::DatagramReceivedOk {
                                        let flags = if msg.frame.data.len() >= 3 {
                                            msg.frame.data[2]
                                        } else {
                                            0
                                        };
                                        let timeout_exp = flags & 0x0F;
                                        if timeout_exp > 0 {
                                            let extended_ms = (1u64 << timeout_exp) * 1000;
                                            if extended_ms > max_duration.as_millis() as u64 {
                                                max_duration = Duration::from_millis(extended_ms);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Record timing.
                    let elapsed_ms = send_time.elapsed().as_millis() as u64;
                    if first_frame_latency_ms.is_none() {
                        first_frame_latency_ms = Some(elapsed_ms);
                        last_frame_ms = elapsed_ms;
                    } else {
                        frame_gaps_ms.push((elapsed_ms.saturating_sub(last_frame_ms)) as u32);
                        last_frame_ms = elapsed_ms;
                    }
                    frame_count = frame_count.saturating_add(1);

                    if let Ok(Some(datagram_data)) = assembler.handle_frame(&msg.frame) {
                        // Validate size: 6-byte header + at most 65 data bytes.
                        if datagram_data.len() > 71 {
                            // Oversized — ACK to free the node, then retry.
                            if let Ok(ack) = DatagramAssembler::send_acknowledgment(our_alias, dest_alias) {
                                let _ = if use_send_direct {
                                    handle.send_direct(&ack).await
                                } else {
                                    handle.send(&ack).await
                                };
                            }
                            if retry_count >= config.max_retries {
                                return ExchangeResult {
                                    data: Err(format!(
                                        "Oversized datagram reply ({} bytes) after {} retries",
                                        datagram_data.len(), retry_count
                                    )),
                                    timing: None,
                                    retry_count,
                                };
                            }
                            retry_count += 1;
                            assembler = DatagramAssembler::new();
                            continue 'retry;
                        }

                        // Validate reply address (stale-reply detection).
                        if datagram_data.len() >= 6 {
                            let reply_addr = u32::from_be_bytes([
                                datagram_data[2], datagram_data[3],
                                datagram_data[4], datagram_data[5],
                            ]);
                            if reply_addr != desc.address {
                                // Stale reply from a previous exchange — ACK and discard,
                                // keep listening for the correct reply.
                                if let Ok(ack) = DatagramAssembler::send_acknowledgment(our_alias, dest_alias) {
                                    let _ = if use_send_direct {
                                        handle.send_direct(&ack).await
                                    } else {
                                        handle.send(&ack).await
                                    };
                                }
                                assembler = DatagramAssembler::new();
                                continue;
                            }
                        }

                        // ACK the reply.
                        let ack_frame = match DatagramAssembler::send_acknowledgment(
                            our_alias, dest_alias,
                        ) {
                            Ok(f) => f,
                            Err(e) => return ExchangeResult {
                                data: Err(e.to_string()),
                                timing: None,
                                retry_count,
                            },
                        };
                        let _ = if use_send_direct {
                            handle.send_direct(&ack_frame).await
                        } else {
                            handle.send(&ack_frame).await
                        };

                        // Post-ACK delay: give CAN gateways time to forward the ACK
                        // before the next request arrives.
                        if config.post_ack_delay_ms > 0 {
                            tokio::time::sleep(Duration::from_millis(config.post_ack_delay_ms)).await;
                        }

                        let total_duration_ms = send_time.elapsed().as_millis() as u64;
                        let timing = MemoryReadTiming {
                            first_frame_latency_ms: first_frame_latency_ms
                                .unwrap_or(total_duration_ms),
                            frame_gaps_ms,
                            total_duration_ms,
                            frame_count,
                        };

                        // Parse the memory-config reply.
                        return match MemoryConfigCmd::parse_read_reply(&datagram_data) {
                            Ok(ReadReply::Success { data, .. }) => ExchangeResult {
                                data: Ok(data),
                                timing: Some(timing),
                                retry_count,
                            },
                            Ok(ReadReply::Failed { error_code, message, .. }) => ExchangeResult {
                                data: Err(format!(
                                    "Memory read failed: error 0x{:04X} - {}",
                                    error_code, message
                                )),
                                timing: Some(timing),
                                retry_count,
                            },
                            Err(e) => ExchangeResult {
                                data: Err(e.to_string()),
                                timing: Some(timing),
                                retry_count,
                            },
                        };
                    }
                }
                Ok(Err(_)) => {
                    return ExchangeResult {
                        data: Err("Broadcast channel lagged during memory read".into()),
                        timing: None,
                        retry_count,
                    };
                }
                Err(_) => {
                    return ExchangeResult {
                        data: Err("Timeout waiting for memory read response".into()),
                        timing: None,
                        retry_count,
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::GridConnectFrame;
    use crate::transport::mock::MockTransport as GlobalMockTransport;
    use crate::transport_actor::TransportActor;

    /// Helper: build a read-reply datagram (DatagramOnly) for Configuration space.
    fn make_read_reply(from_alias: u16, to_alias: u16, address: u32, payload: &[u8]) -> GridConnectFrame {
        let header = MTI::DatagramOnly.to_header_with_dest(from_alias, to_alias).unwrap();
        let addr_bytes = address.to_be_bytes();
        // Embedded format for Configuration space: command byte 0x51 (0x50 read-reply | 0x01 config)
        let mut data = vec![0x20, 0x51, addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3]];
        data.extend_from_slice(payload);
        GridConnectFrame { header, data }
    }

    /// Helper: build a DatagramRejected frame.
    fn make_rejected(from_alias: u16, to_alias: u16, error_code: u16) -> GridConnectFrame {
        let header = MTI::DatagramRejected.to_header(from_alias).unwrap();
        let data = vec![
            ((to_alias >> 8) & 0xFF) as u8,
            (to_alias & 0xFF) as u8,
            ((error_code >> 8) & 0xFF) as u8,
            (error_code & 0xFF) as u8,
        ];
        GridConnectFrame { header, data }
    }

    /// Helper: build a DatagramReceivedOk frame with optional flags.
    fn make_ack_with_flags(from_alias: u16, to_alias: u16, flags: u8) -> GridConnectFrame {
        let header = MTI::DatagramReceivedOk.to_header(from_alias).unwrap();
        let mut data = vec![
            ((to_alias >> 8) & 0xFF) as u8,
            (to_alias & 0xFF) as u8,
        ];
        if flags != 0 {
            data.push(flags);
        }
        GridConnectFrame { header, data }
    }

    fn test_config() -> MemoryReadConfig {
        MemoryReadConfig {
            timeout_ms: 200, // short for tests
            max_retries: 3,
            post_ack_delay_ms: 0, // no delay in tests
        }
    }

    const OUR: u16 = 0xAAA;
    const NODE: u16 = 0xBBB;

    fn config_space_desc(address: u32, count: u8) -> ReadDescriptor {
        ReadDescriptor {
            address_space: 0xFD, // configuration space
            address,
            count,
        }
    }

    // ─── Successful read ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_successful_read_returns_data() {
        let reply = make_read_reply(NODE, OUR, 0x0000, &[0x48, 0x65]);
        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 2),
            &test_config(),
            false,
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_ok(), "Read should succeed: {:?}", result.data);
        assert_eq!(result.data.unwrap(), vec![0x48, 0x65]);
        assert_eq!(result.retry_count, 0);
        assert!(result.timing.is_some());
    }

    // ─── Timeout ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_timeout_returns_error() {
        // No frames queued — will timeout.
        let transport = GlobalMockTransport::new();
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 64),
            &test_config(),
            false,
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_err());
        assert!(result.data.unwrap_err().contains("Timeout"));
    }

    // ─── Retry cap on DatagramRejected with resend-OK ───────────────────────

    #[tokio::test]
    async fn test_resend_ok_capped_at_max_retries() {
        let mut transport = GlobalMockTransport::new();
        // Queue 4 rejections (more than max_retries=3) — should fail after 3 retries.
        for _ in 0..4 {
            transport.add_receive_frame(make_rejected(NODE, OUR, 0x2000).to_string());
        }

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 64),
            &test_config(),
            false,
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_err());
        let err = result.data.unwrap_err();
        assert!(err.contains("retries"), "Error should mention retries: {}", err);
        assert_eq!(result.retry_count, 3);
    }

    // ─── Resend-OK then success ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_resend_ok_then_success() {
        let mut transport = GlobalMockTransport::new();
        // First: rejection with resend-OK.
        transport.add_receive_frame(make_rejected(NODE, OUR, 0x2000).to_string());
        // After resend: successful reply.
        let reply = make_read_reply(NODE, OUR, 0x0000, &[0xAB]);
        transport.add_receive_frame(reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 1),
            &test_config(),
            false,
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_ok());
        assert_eq!(result.data.unwrap(), vec![0xAB]);
        assert_eq!(result.retry_count, 1);
    }

    // ─── Permanent rejection (not resend-OK) ────────────────────────────────

    #[tokio::test]
    async fn test_permanent_rejection_returns_immediately() {
        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(make_rejected(NODE, OUR, 0x1000).to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 64),
            &test_config(),
            false,
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_err());
        let err = result.data.unwrap_err();
        assert!(err.contains("0x1000"), "Error should contain error code: {}", err);
        assert_eq!(result.retry_count, 0);
    }

    // ─── Timeout extension via DatagramReceivedOk flags ─────────────────────

    #[tokio::test]
    async fn test_timeout_extension_via_ack_flags() {
        let mut transport = GlobalMockTransport::new();
        // DatagramReceivedOk with flags=0x02 → extend timeout to 2^2 * 1000 = 4000ms
        transport.add_receive_frame(make_ack_with_flags(NODE, OUR, 0x02).to_string());
        // Then the actual reply (arrives after what would have been the original timeout).
        let reply = make_read_reply(NODE, OUR, 0x0000, &[0x42]);
        transport.add_receive_frame(reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        // Use a very short base timeout — extension should stretch it.
        let config = MemoryReadConfig {
            timeout_ms: 50,
            max_retries: 3,
            post_ack_delay_ms: 0,
        };

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 1),
            &config,
            false,
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_ok(), "Should succeed with timeout extension: {:?}", result.data);
        assert_eq!(result.data.unwrap(), vec![0x42]);
    }

    // ─── Stale reply detection ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_stale_reply_discarded_and_correct_reply_accepted() {
        let mut transport = GlobalMockTransport::new();
        // First: a stale reply from a previous read (wrong address).
        let stale = make_read_reply(NODE, OUR, 0x0040, &[0xFF]);
        transport.add_receive_frame(stale.to_string());
        // Then: the correct reply.
        let correct = make_read_reply(NODE, OUR, 0x0000, &[0x42]);
        transport.add_receive_frame(correct.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 1),
            &test_config(),
            false,
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_ok(), "Should accept correct reply after discarding stale: {:?}", result.data);
        assert_eq!(result.data.unwrap(), vec![0x42]);
    }

    // ─── Post-ACK delay is applied ─────────────────────────────────────────

    #[tokio::test]
    async fn test_post_ack_delay_adds_latency() {
        let reply = make_read_reply(NODE, OUR, 0x0000, &[0x01]);
        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let config = MemoryReadConfig {
            timeout_ms: 200,
            max_retries: 3,
            post_ack_delay_ms: 50, // 50ms delay
        };

        let start = Instant::now();
        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 1),
            &config,
            false,
        ).await;
        let elapsed = start.elapsed();

        actor.shutdown().await;

        assert!(result.data.is_ok());
        // The exchange should have taken at least 50ms due to post-ACK delay.
        assert!(
            elapsed.as_millis() >= 45, // allow slight timing tolerance
            "Expected >= 45ms due to post_ack_delay, got {}ms",
            elapsed.as_millis()
        );
    }

    // ─── send_direct path works identically ─────────────────────────────────

    #[tokio::test]
    async fn test_send_direct_path_succeeds() {
        let reply = make_read_reply(NODE, OUR, 0x0000, &[0xDE, 0xAD]);
        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();
        let mut rx = handle.subscribe_all();

        let result = datagram_read_exchange(
            &handle, &mut rx, OUR, NODE,
            &config_space_desc(0x0000, 2),
            &test_config(),
            true, // use send_direct
        ).await;

        actor.shutdown().await;

        assert!(result.data.is_ok());
        assert_eq!(result.data.unwrap(), vec![0xDE, 0xAD]);
    }
}
