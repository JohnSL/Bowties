//! Simple Node Identification Protocol (SNIP) implementation
//!
//! SNIP provides manufacturer, model, version, and user-assigned identification
//! for LCC nodes via the datagram protocol.

use crate::protocol::frame::GridConnectFrame;
use crate::protocol::mti::MTI;
use crate::transport::LccTransport;
use crate::types::{SNIPData, SNIPStatus};
use crate::{Error, Result};
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
use std::sync::Arc;

/// Timeout for SNIP request (5 seconds)
const SNIP_TIMEOUT: Duration = Duration::from_secs(5);

/// Silence detection timeout (100ms with no frames = end of response)
const SILENCE_TIMEOUT: Duration = Duration::from_millis(100);

/// Query SNIP data from a specific node
///
/// # Arguments
/// * `transport` - LCC transport connection (mutable reference)
/// * `source_alias` - Our alias (source of the request)
/// * `dest_alias` - Target node's alias
/// * `semaphore` - Semaphore for concurrency limiting (capacity 5)
///
/// # Returns
/// * `Ok((SNIPData, SNIPStatus))` - Retrieved SNIP data and status
/// * `Err(_)` - Network or protocol error
pub async fn query_snip(
    transport: &mut dyn LccTransport,
    source_alias: u16,
    dest_alias: u16,
    semaphore: Arc<Semaphore>,
) -> Result<(Option<SNIPData>, SNIPStatus)> {
    // Acquire semaphore permit to limit concurrency
    let _permit = semaphore.acquire().await.map_err(|e| {
        Error::Protocol(format!("Failed to acquire SNIP semaphore: {}", e))
    })?;

    // Send SNIP request with timeout
    match timeout(SNIP_TIMEOUT, query_snip_internal(transport, source_alias, dest_alias)).await {
        Ok(result) => result,
        Err(_) => {
            // Timeout occurred
            Ok((None, SNIPStatus::Timeout))
        }
    }
}

/// Internal SNIP query implementation (without semaphore/timeout wrapper)
async fn query_snip_internal(
    transport: &mut dyn LccTransport,
    source_alias: u16,
    dest_alias: u16,
) -> Result<(Option<SNIPData>, SNIPStatus)> {
    // Send SNIP request as addressed message
    let request_frame = GridConnectFrame::from_addressed_mti(
        MTI::SNIPRequest,
        source_alias,
        dest_alias,
        vec![],  // SNIP request has no payload beyond destination
    )?;

    transport.send(&request_frame).await?;

    // Manually assemble SNIP response payload
    // SNIP responses have MTI 0x19A08 in header with datagram frame type in data[0]
    let mut snip_payload = Vec::new();
    let mut receiving_datagram = false;
    // Some nodes (e.g. TCS UWT-100) take >100ms before sending their first frame.
    // Use the full SNIP_TIMEOUT while waiting for the first frame, then switch to
    // the shorter SILENCE_TIMEOUT for subsequent inter-frame gaps.
    let mut received_first_frame = false;

    loop {
        // Use full timeout until first frame arrives, then silence detection
        let recv_timeout_ms = if received_first_frame {
            SILENCE_TIMEOUT.as_millis() as u64
        } else {
            SNIP_TIMEOUT.as_millis() as u64
        };
        let receive_result = transport.receive(recv_timeout_ms).await;
        
        match receive_result {
            Ok(Some(frame)) => {
                // Parse header to extract MTI and source alias
                let (mti, source) = match MTI::from_header(frame.header) {
                    Ok(result) => result,
                    Err(_) => continue,
                };

                // Only process frames from our target node
                if source != dest_alias {
                    continue;
                }

                // D20: OptionalInteractionRejected — node does not support SNIP.
                if mti == MTI::OptionalInteractionRejected {
                    eprintln!("[SNIP] {:03X}: OptionalInteractionRejected — node does not support SNIP", dest_alias);
                    return Ok((None, SNIPStatus::Timeout));
                }

                // Check for SNIP response MTI (0x19A08)
                if mti != MTI::SNIPResponse {
                    continue;
                }

                // We have a frame from our target — switch to silence-detection timing
                received_first_frame = true;

                // SNIP responses have datagram frame type in data[0]
                // 0x1A = first, 0x3A = middle, 0x2A = final
                if frame.data.len() < 2 {
                    return Err(Error::Protocol(format!(
                        "SNIP frame data too short: {} bytes",
                        frame.data.len()
                    )));
                }

                // data[0] high nibble = frame-type flag; low nibble = high nibble
                // of the destination alias.  data[1] = low byte of dest alias.
                // Mask to the upper nibble so the match is alias-independent.
                let frame_type = frame.data[0] & 0xF0;

                // Extract payload (skip bytes 0-1, take bytes 2+)
                let payload_chunk = &frame.data[2..];

                match frame_type {
                    0x10 => {
                        // First frame - start new datagram
                        snip_payload.clear();
                        snip_payload.extend_from_slice(payload_chunk);
                        receiving_datagram = true;
                    }
                    0x30 => {
                        // Middle frame - append to existing datagram
                        if !receiving_datagram {
                            return Err(Error::Protocol(
                                "SNIP middle frame received without first frame".to_string()
                            ));
                        }
                        snip_payload.extend_from_slice(payload_chunk);
                    }
                    0x20 => {
                        // Final frame - complete the datagram
                        if !receiving_datagram {
                            return Err(Error::Protocol(
                                "SNIP final frame received without first frame".to_string()
                            ));
                        }
                        snip_payload.extend_from_slice(payload_chunk);

                        // Datagram complete - parse SNIP data
                        match parse_snip_payload(&snip_payload) {
                            Ok(snip_data) => {
                                // Note: SNIP responses are addressed messages (not
                                // datagrams), so we must NOT send DatagramReceivedOk.
                                return Ok((Some(snip_data), SNIPStatus::Complete));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    0x00 => {
                        // Single-frame datagram (DatagramOnly equivalent)
                        snip_payload.clear();
                        snip_payload.extend_from_slice(payload_chunk);

                        // Datagram complete - parse SNIP data
                        match parse_snip_payload(&snip_payload) {
                            Ok(snip_data) => {
                                // Note: SNIP responses are addressed messages (not
                                // datagrams), so we must NOT send DatagramReceivedOk.
                                return Ok((Some(snip_data), SNIPStatus::Complete));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    _ => {
                        // Unknown flag nibble — skip rather than aborting
                        continue;
                    }
                }
            }
            Ok(None) => {
                // Timeout with no frame - silence detected, query timed out
                return Ok((None, SNIPStatus::Timeout));
            }
            Err(e) => {
                // Transport error
                return Err(e);
            }
        }
    }
}

/// Parse SNIP payload into SNIPData struct
///
/// SNIP payload structure:
/// - Section 1 (Manufacturer ACDI):
///   - Byte 0: Version (0x04 = 4 fields)
///   - String 1: Manufacturer name (null-terminated)
///   - String 2: Model name (null-terminated)
///   - String 3: Hardware version (null-terminated)
///   - String 4: Software version (null-terminated)
/// - Section 2 (User ACDI):
///   - Byte N: Version (0x02 = 2 fields)
///   - String 5: User name (null-terminated)
///   - String 6: User description (null-terminated)

/// Encode SNIP data into a payload for transmission
///
/// Encodes a SNIPData struct into the binary format expected by SNIP requesters:
/// - Section 1 (version 0x04): manufacturer, model, hw_version, sw_version
/// - Section 2 (version 0x02, optional): user_name, user_description
///
/// # Arguments
/// * `snip` - The SNIP data to encode
/// * `include_user_section` - If true, include Section 2 with user name/description
///
/// # Returns
/// The encoded payload as bytes
pub fn encode_snip_payload(snip: &SNIPData, include_user_section: bool) -> Vec<u8> {
    let mut payload = Vec::new();

    // Section 1: Manufacturer ACDI (version 0x04)
    payload.push(0x04);
    payload.extend_from_slice(snip.manufacturer.as_bytes());
    payload.push(0x00);

    payload.extend_from_slice(snip.model.as_bytes());
    payload.push(0x00);

    payload.extend_from_slice(snip.hardware_version.as_bytes());
    payload.push(0x00);

    payload.extend_from_slice(snip.software_version.as_bytes());
    payload.push(0x00);

    // Section 2: User ACDI (optional, version 0x02)
    if include_user_section {
        payload.push(0x02);
        payload.extend_from_slice(snip.user_name.as_bytes());
        payload.push(0x00);

        payload.extend_from_slice(snip.user_description.as_bytes());
        payload.push(0x00);
    }

    payload
}

pub fn parse_snip_payload(payload: &[u8]) -> Result<SNIPData> {
    let mut offset = 0;

    // Parse Section 1 (Manufacturer ACDI)
    if offset >= payload.len() {
        return Err(Error::Protocol("SNIP payload too short for Section 1 version byte".to_string()));
    }

    let section1_version = payload[offset];
    offset += 1;

    if section1_version != 0x04 {
        return Err(Error::Protocol(format!(
            "Unexpected Section 1 version: {}, expected 0x04",
            section1_version
        )));
    }

    // Extract 4 strings from Section 1
    let manufacturer = parse_section(payload, &mut offset)?;
    let model = parse_section(payload, &mut offset)?;
    let hardware_version = parse_section(payload, &mut offset)?;
    let software_version = parse_section(payload, &mut offset)?;

    // Parse Section 2 (User ACDI) - may be missing for some nodes
    let (user_name, user_description) = if offset < payload.len() {
        let section2_version = payload[offset];
        offset += 1;

        if section2_version != 0x02 {
            // Section 2 exists but has wrong version - skip it
            (String::new(), String::new())
        } else {
            // Extract 2 strings from Section 2
            let name = parse_section(payload, &mut offset)?;
            let description = parse_section(payload, &mut offset)?;
            (name, description)
        }
    } else {
        // Section 2 not present
        (String::new(), String::new())
    };

    let mut snip_data = SNIPData {
        manufacturer,
        model,
        hardware_version,
        software_version,
        user_name,
        user_description,
    };

    // Sanitize all string fields
    snip_data.sanitize();

    Ok(snip_data)
}

/// Parse a null-terminated string from payload
///
/// Updates offset to point past the null terminator
fn parse_section(data: &[u8], offset: &mut usize) -> Result<String> {
    // Find null terminator starting from current offset
    let start = *offset;
    let mut end = start;

    while end < data.len() && data[end] != 0x00 {
        end += 1;
    }

    if end >= data.len() {
        // No null terminator found - use remaining data
        let s = String::from_utf8_lossy(&data[start..]).to_string();
        *offset = data.len();
        return Ok(s);
    }

    // Extract string between start and null terminator
    let s = String::from_utf8_lossy(&data[start..end]).to_string();
    *offset = end + 1; // Move past null terminator

    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;

    // ── helpers ────────────────────────────────────────────────────────────

    /// Build a SNIP reply frame the way a real node sends it.
    ///
    /// SNIP responses are **addressed messages** using MTI 0x19A08.  The CAN
    /// header carries only the MTI and the *source* alias; the destination is
    /// encoded in data[0..2]:
    ///
    ///   data[0]: (flag_nibble << 4) | (dest_alias >> 8) & 0x0F
    ///   data[1]: dest_alias & 0xFF
    ///   data[2..]: payload chunk
    ///
    /// flag_nibble values:
    ///   0x1 = DatagramFirst   0x3 = DatagramMiddle
    ///   0x2 = DatagramFinal   0x0 = DatagramOnly (single-frame)
    ///
    /// The real-world example from the logs:
    ///   header 0x19A083AE  →  MTI 0x19A08, source alias 0x3AE
    ///   data[0] = 0x18     →  flag 0x1 (first), dest-alias high nibble 0x8 (→ 0x825)
    ///   data[1] = 0x25     →  dest-alias low byte
    fn snip_reply_frame(source_alias: u16, dest_alias: u16, flag_nibble: u8, chunk: &[u8]) -> String {
        // Addressed-message header: (MTI << 12) | source_alias
        // dest_alias is NOT encoded in the header — it lives in data[0..2].
        let header = (0x19A08u32 << 12) | source_alias as u32;
        let dest_hi = ((dest_alias >> 8) & 0x0F) as u8;
        let dest_lo = (dest_alias & 0xFF) as u8;
        let frame_type_byte = (flag_nibble << 4) | dest_hi;
        let mut data = vec![frame_type_byte, dest_lo];
        data.extend_from_slice(chunk);
        // CAN frames hold at most 8 data bytes
        assert!(data.len() <= 8, "chunk too large: {} data bytes (max 6 after 2 header bytes)", data.len());
        let data_hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
        format!(":X{:08X}N{};", header, data_hex)
    }

    /// Short SNIP payload used in several tests.
    fn minimal_snip_payload() -> Vec<u8> {
        let mut p = vec![0x04u8];
        p.extend_from_slice(b"ACME\x00Widget\x001.0\x002.3\x00");
        p.push(0x02);
        p.extend_from_slice(b"MyNode\x00Desc\x00");
        p
    }

    // ── Frame-type nibble decoding tests ───────────────────────────────────

    /// Regression test for the bug: source alias 0x3AE → dest-alias high nibble 0xA
    /// caused data[0] = 0x1A for "first frame", which is what the OLD code matched.
    /// Any other alias (e.g. 0x825 → data[0] = 0x18) was silently rejected.
    #[tokio::test]
    async fn test_query_snip_multiframe_alias_825() {
        // Simulate receiving a SNIP reply FROM alias 0x3AE addressed TO alias 0x825.
        // data[0] will be 0x18 / 0x38 / 0x28 — the old code failed here.
        let our_alias: u16 = 0x825;
        let node_alias: u16 = 0x3AE;
        let payload = minimal_snip_payload();

        // Split into 3 chunks of up to 6 bytes (leaving room for 2 header bytes)
        let chunks: Vec<&[u8]> = payload.chunks(6).collect();
        let n = chunks.len();

        let mut transport = MockTransport::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let flag = if n == 1 { 0x0 } else if i == 0 { 0x1 } else if i == n - 1 { 0x2 } else { 0x3 };
            transport.add_receive_frame(snip_reply_frame(node_alias, our_alias, flag, chunk));
        }
        // Queue a DatgramReceived-OK ack from the node (not needed for this path but harmless)

        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        let (snip, status) = query_snip(&mut transport, our_alias, node_alias, sem).await.unwrap();

        assert_eq!(status, SNIPStatus::Complete, "status must be Complete");
        let snip = snip.expect("must have SNIP data");
        assert_eq!(snip.manufacturer, "ACME");
        assert_eq!(snip.model, "Widget");
    }

    /// Same test but with alias 0x3AE as OUR alias (data[0] low nibble = 0x3).
    #[tokio::test]
    async fn test_query_snip_multiframe_alias_3ae() {
        let our_alias: u16 = 0x3AE;
        let node_alias: u16 = 0xC41;
        let payload = minimal_snip_payload();
        let chunks: Vec<&[u8]> = payload.chunks(6).collect();
        let n = chunks.len();

        let mut transport = MockTransport::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let flag = if n == 1 { 0x0 } else if i == 0 { 0x1 } else if i == n - 1 { 0x2 } else { 0x3 };
            transport.add_receive_frame(snip_reply_frame(node_alias, our_alias, flag, chunk));
        }

        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        let (snip, status) = query_snip(&mut transport, our_alias, node_alias, sem).await.unwrap();

        assert_eq!(status, SNIPStatus::Complete);
        let snip = snip.unwrap();
        assert_eq!(snip.manufacturer, "ACME");
    }

    /// Alias 0xFFF — low nibble of data[0] = 0xF.  Old code: 0x1F ≠ 0x1A → broken.
    #[tokio::test]
    async fn test_query_snip_multiframe_alias_fff() {
        let our_alias: u16 = 0xFFF;
        let node_alias: u16 = 0x001;
        let payload = minimal_snip_payload();
        let chunks: Vec<&[u8]> = payload.chunks(6).collect();
        let n = chunks.len();

        let mut transport = MockTransport::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let flag = if n == 1 { 0x0 } else if i == 0 { 0x1 } else if i == n - 1 { 0x2 } else { 0x3 };
            transport.add_receive_frame(snip_reply_frame(node_alias, our_alias, flag, chunk));
        }

        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        let (snip, status) = query_snip(&mut transport, our_alias, node_alias, sem).await.unwrap();

        assert_eq!(status, SNIPStatus::Complete);
        let snip = snip.unwrap();
        assert_eq!(snip.manufacturer, "ACME");
    }

    /// Single-frame SNIP reply (DatagramOnly, flag nibble 0x0).
    #[tokio::test]
    async fn test_query_snip_single_frame() {
        let our_alias: u16 = 0x825;
        let node_alias: u16 = 0x3AE;
        // 6 bytes max payload per CAN frame (8 total - 2 header bytes)
        let payload = vec![
            0x04u8,
            b'A', 0x00,  // manufacturer "A"
            0x00,        // model ""
            0x00,        // hw ""
            0x00,        // sw ""
        ];

        let mut transport = MockTransport::new();
        transport.add_receive_frame(snip_reply_frame(node_alias, our_alias, 0x0, &payload));

        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        let (snip, status) = query_snip(&mut transport, our_alias, node_alias, sem).await.unwrap();

        assert_eq!(status, SNIPStatus::Complete);
        let snip = snip.unwrap();
        assert_eq!(snip.manufacturer, "A");
        assert_eq!(snip.model, "");
    }

    /// Frames from a different alias are ignored; only frames from dest_alias matter.
    #[tokio::test]
    async fn test_query_snip_ignores_other_sources() {
        let our_alias: u16 = 0x825;
        let node_alias: u16 = 0x3AE;
        let other_alias: u16 = 0x111;
        let payload = minimal_snip_payload();
        let chunks: Vec<&[u8]> = payload.chunks(6).collect();
        let n = chunks.len();

        let mut transport = MockTransport::new();

        // Interleave frames from another node — should be ignored
        for (i, chunk) in chunks.iter().enumerate() {
            // Noise from a different alias
            let noise_flag = if i == 0 { 0x1 } else { 0x3 };
            transport.add_receive_frame(snip_reply_frame(other_alias, our_alias, noise_flag, chunk));
            // Real frame from the correct alias
            let flag = if n == 1 { 0x0 } else if i == 0 { 0x1 } else if i == n - 1 { 0x2 } else { 0x3 };
            transport.add_receive_frame(snip_reply_frame(node_alias, our_alias, flag, chunk));
        }

        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        let (snip, status) = query_snip(&mut transport, our_alias, node_alias, sem).await.unwrap();

        assert_eq!(status, SNIPStatus::Complete);
        let snip = snip.unwrap();
        assert_eq!(snip.manufacturer, "ACME");
    }

    /// No response → timeout → (None, Timeout).
    #[tokio::test]
    async fn test_query_snip_timeout() {
        let mut transport = MockTransport::new();
        // No frames queued at all — MockTransport returns None after a short sleep

        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        // Use a 1-second overall timeout via the public wrapper
        let (snip, status) = query_snip(&mut transport, 0x825, 0x3AE, sem).await.unwrap();

        assert_eq!(status, SNIPStatus::Timeout);
        assert!(snip.is_none());
    }

    // ── Payload parsing tests (existing, unchanged) ────────────────────────

    #[test]
    fn test_snip_request_frame_format() {
        // Verify that SNIP request is correctly encoded as an addressed message
        // MTI 0x19DE8, source 0xAAA, dest 0x0DDD
        let frame = GridConnectFrame::from_addressed_mti(
            MTI::SNIPRequest,
            0xAAA,
            0x0DDD,
            vec![],
        ).unwrap();
        
        // Expected: MTI 0x19DE8 in header, destination 0x0DDD in first 2 bytes of data
        assert_eq!(frame.data, vec![0x0D, 0xDD]);
        assert_eq!(frame.to_string(), ":X19DE8AAAN0DDD;");
        
        // Verify MTI is NOT corrupted (should be 0x19DE8, not 0x19DE9)
        let (parsed_mti, parsed_source) = MTI::from_header(frame.header).unwrap();
        assert_eq!(parsed_mti, MTI::SNIPRequest);
        assert_eq!(parsed_source, 0xAAA);
        
        // Verify destination can be extracted from body
        let (dest, payload) = frame.get_dest_from_body().unwrap();
        assert_eq!(dest, 0x0DDD);
        assert_eq!(payload, &[]);
    }

    #[test]
    fn test_parse_snip_complete() {
        // Create a minimal SNIP payload with both sections
        let payload = vec![
            0x04, // Section 1 version
            b'A', b'C', b'M', b'E', 0x00, // Manufacturer: "ACME"
            b'W', b'i', b'd', b'g', b'e', b't', 0x00, // Model: "Widget"
            b'1', b'.', b'0', 0x00, // HW version: "1.0"
            b'2', b'.', b'3', b'.', b'1', 0x00, // SW version: "2.3.1"
            0x02, // Section 2 version
            b'M', b'y', b' ', b'N', b'o', b'd', b'e', 0x00, // User name: "My Node"
            b'T', b'e', b's', b't', 0x00, // User desc: "Test"
        ];

        let result = parse_snip_payload(&payload).unwrap();
        assert_eq!(result.manufacturer, "ACME");
        assert_eq!(result.model, "Widget");
        assert_eq!(result.hardware_version, "1.0");
        assert_eq!(result.software_version, "2.3.1");
        assert_eq!(result.user_name, "My Node");
        assert_eq!(result.user_description, "Test");
    }

    #[test]
    fn test_parse_snip_section1_only() {
        // SNIP payload with only Section 1 (no user data)
        let payload = vec![
            0x04, // Section 1 version
            b'A', b'C', b'M', b'E', 0x00, // Manufacturer
            b'W', b'i', b'd', b'g', b'e', b't', 0x00, // Model
            b'1', b'.', b'0', 0x00, // HW version
            b'2', b'.', b'3', 0x00, // SW version
        ];

        let result = parse_snip_payload(&payload).unwrap();
        assert_eq!(result.manufacturer, "ACME");
        assert_eq!(result.model, "Widget");
        assert_eq!(result.user_name, "");
        assert_eq!(result.user_description, "");
    }

    #[test]
    fn test_parse_snip_empty_strings() {
        // SNIP with empty strings (consecutive null bytes)
        let payload = vec![
            0x04, // Section 1 version
            0x00, // Empty manufacturer
            0x00, // Empty model
            0x00, // Empty HW version
            b'1', b'.', b'0', 0x00, // SW version
        ];

        let result = parse_snip_payload(&payload).unwrap();
        assert_eq!(result.manufacturer, "");
        assert_eq!(result.model, "");
        assert_eq!(result.hardware_version, "");
        assert_eq!(result.software_version, "1.0");
    }

    #[test]
    fn test_parse_snip_invalid_version() {
        // Wrong version byte in Section 1
        let payload = vec![
            0x05, // Wrong version (should be 0x04)
            b'A', b'C', b'M', b'E', 0x00,
        ];

        assert!(parse_snip_payload(&payload).is_err());
    }

    #[test]
    fn test_parse_section() {
        let data = b"Hello\x00World\x00";
        let mut offset = 0;

        let s1 = parse_section(data, &mut offset).unwrap();
        assert_eq!(s1, "Hello");
        assert_eq!(offset, 6);

        let s2 = parse_section(data, &mut offset).unwrap();
        assert_eq!(s2, "World");
        assert_eq!(offset, 12);
    }

    #[test]
    fn test_parse_section_no_null_terminator() {
        let data = b"Hello";
        let mut offset = 0;

        // Should handle missing null terminator gracefully
        let s = parse_section(data, &mut offset).unwrap();
        assert_eq!(s, "Hello");
        assert_eq!(offset, 5);
    }

    /// SNIP string spanning a CAN frame boundary.
    ///
    /// The manufacturer name "LongMfgName" (11 bytes + null) must be split
    /// across two CAN frames because each frame only carries 6 payload bytes
    /// (8 total minus the 2-byte addressed-message header).  The first frame
    /// delivers bytes 0-5 and the second frame delivers bytes 6-11.
    /// Verifies that the payload assembler correctly concatenates the chunks
    /// before parse_snip_payload sees them.
    #[tokio::test]
    async fn test_query_snip_string_spanning_frame_boundary() {
        let our_alias: u16 = 0x825;
        let node_alias: u16 = 0x3AE;

        // Build a SNIP payload where the manufacturer name spans a frame boundary.
        //   Byte layout:  [0x04] "LongMfgName\0" "M\0" "1\0" "2\0" [0x02] "U\0" "D\0"
        //   Chunk 0 (6 bytes): 0x04 'L' 'o' 'n' 'g' 'M'
        //   Chunk 1 (6 bytes): 'f' 'g' 'N' 'a' 'm' 'e'
        //   Chunk 2 (6 bytes): '\0' 'M' '\0'  '1' '\0' '2'
        //   Chunk 3 (5 bytes): '\0' 0x02 'U' '\0' 'D'   -- note: < 6
        //   Chunk 4 (1 byte):  '\0'
        let mut payload = vec![0x04u8];
        payload.extend_from_slice(b"LongMfgName\x00M\x001\x002\x00");
        payload.push(0x02);
        payload.extend_from_slice(b"U\x00D\x00");

        let chunks: Vec<&[u8]> = payload.chunks(6).collect();
        let n = chunks.len();

        let mut transport = MockTransport::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let flag = if n == 1 {
                0x0
            } else if i == 0 {
                0x1
            } else if i == n - 1 {
                0x2
            } else {
                0x3
            };
            transport.add_receive_frame(snip_reply_frame(node_alias, our_alias, flag, chunk));
        }

        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
        let (snip, status) = query_snip(&mut transport, our_alias, node_alias, sem).await.unwrap();

        assert_eq!(status, SNIPStatus::Complete, "status must be Complete");
        let snip = snip.expect("must have SNIP data");
        assert_eq!(snip.manufacturer, "LongMfgName", "name spanning frame boundary must reassemble");
        assert_eq!(snip.model, "M");
        assert_eq!(snip.hardware_version, "1");
        assert_eq!(snip.software_version, "2");
        assert_eq!(snip.user_name, "U");
        assert_eq!(snip.user_description, "D");
    }
}
