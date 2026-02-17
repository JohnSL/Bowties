//! Simple Node Identification Protocol (SNIP) implementation
//!
//! SNIP provides manufacturer, model, version, and user-assigned identification
//! for LCC nodes via the datagram protocol.

use crate::protocol::datagram::DatagramAssembler;
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

    loop {
        // Wait for next frame with silence detection timeout
        let receive_result = transport.receive(SILENCE_TIMEOUT.as_millis() as u64).await;
        
        match receive_result {
            Ok(Some(frame)) => {
                // Parse header to extract MTI and source alias
                let (mti, source) = match MTI::from_header(frame.header) {
                    Ok(result) => result,
                    Err(_) => {
                        eprintln!("SNIP: Failed to parse header {:08X}", frame.header);
                        continue;
                    }
                };

                // Only process frames from our target node
                if source != dest_alias {
                    continue;
                }

                // Check for SNIP response MTI (0x19A08)
                if mti != MTI::SNIPResponse {
                    continue;
                }

                eprintln!("SNIP: Received SNIPResponse from {:03X}, data len: {}", source, frame.data.len());

                // SNIP responses have datagram frame type in data[0]
                // 0x1A = first, 0x3A = middle, 0x2A = final
                if frame.data.len() < 2 {
                    eprintln!("SNIP: Frame data too short: {} bytes", frame.data.len());
                    return Err(Error::Protocol(format!(
                        "SNIP frame data too short: {} bytes",
                        frame.data.len()
                    )));
                }

                let frame_type = frame.data[0];
                eprintln!("SNIP: Frame type byte: 0x{:02X}", frame_type);

                // Extract payload (skip bytes 0-1, take bytes 2+)
                let payload_chunk = &frame.data[2..];
                eprintln!("SNIP: Payload chunk: {} bytes", payload_chunk.len());

                match frame_type {
                    0x1A => {
                        // First frame - start new datagram
                        eprintln!("SNIP: First frame, starting new datagram");
                        snip_payload.clear();
                        snip_payload.extend_from_slice(payload_chunk);
                        receiving_datagram = true;
                    }
                    0x3A => {
                        // Middle frame - append to existing datagram
                        if !receiving_datagram {
                            eprintln!("SNIP: Middle frame without first frame");
                            return Err(Error::Protocol(
                                "SNIP middle frame received without first frame".to_string()
                            ));
                        }
                        eprintln!("SNIP: Middle frame, appending to datagram");
                        snip_payload.extend_from_slice(payload_chunk);
                    }
                    0x2A => {
                        // Final frame - complete the datagram
                        if !receiving_datagram {
                            eprintln!("SNIP: Final frame without first frame");
                            return Err(Error::Protocol(
                                "SNIP final frame received without first frame".to_string()
                            ));
                        }
                        eprintln!("SNIP: Final frame, completing datagram");
                        snip_payload.extend_from_slice(payload_chunk);
                        
                        eprintln!("SNIP: Complete payload: {} bytes", snip_payload.len());
                        
                        // Datagram complete - parse SNIP data
                        match parse_snip_payload(&snip_payload) {
                            Ok(snip_data) => {
                                // Send acknowledgment
                                let ack = DatagramAssembler::send_acknowledgment(source_alias, dest_alias)?;
                                transport.send(&ack).await?;

                                eprintln!("SNIP: Successfully parsed SNIP data");
                                return Ok((Some(snip_data), SNIPStatus::Complete));
                            }
                            Err(e) => {
                                eprintln!("SNIP: Failed to parse SNIP payload: {:?}", e);
                                return Err(e);
                            }
                        }
                    }
                    0x0A => {
                        // Single-frame datagram (DatagramOnly equivalent)
                        eprintln!("SNIP: Single-frame datagram");
                        snip_payload.clear();
                        snip_payload.extend_from_slice(payload_chunk);
                        
                        eprintln!("SNIP: Complete payload: {} bytes", snip_payload.len());
                        
                        // Datagram complete - parse SNIP data
                        match parse_snip_payload(&snip_payload) {
                            Ok(snip_data) => {
                                // Send acknowledgment
                                let ack = DatagramAssembler::send_acknowledgment(source_alias, dest_alias)?;
                                transport.send(&ack).await?;

                                eprintln!("SNIP: Successfully parsed SNIP data");
                                return Ok((Some(snip_data), SNIPStatus::Complete));
                            }
                            Err(e) => {
                                eprintln!("SNIP: Failed to parse SNIP payload: {:?}", e);
                                return Err(e);
                            }
                        }
                    }
                    _ => {
                        eprintln!("SNIP: Unknown frame type: 0x{:02X}", frame_type);
                        return Err(Error::Protocol(format!(
                            "Unknown SNIP frame type: 0x{:02X}",
                            frame_type
                        )));
                    }
                }
            }
            Ok(None) => {
                // Timeout with no frame - silence detected, query timed out
                eprintln!("SNIP: Timeout waiting for response");
                return Ok((None, SNIPStatus::Timeout));
            }
            Err(e) => {
                // Transport error
                eprintln!("SNIP: Transport error: {:?}", e);
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
    let manufacturer = parse_section(&payload, &mut offset)?;
    let model = parse_section(&payload, &mut offset)?;
    let hardware_version = parse_section(&payload, &mut offset)?;
    let software_version = parse_section(&payload, &mut offset)?;

    // Parse Section 2 (User ACDI) - may be missing for some nodes
    let (user_name, user_description) = if offset < payload.len() {
        let section2_version = payload[offset];
        offset += 1;

        if section2_version != 0x02 {
            // Section 2 exists but has wrong version - skip it
            (String::new(), String::new())
        } else {
            // Extract 2 strings from Section 2
            let name = parse_section(&payload, &mut offset)?;
            let description = parse_section(&payload, &mut offset)?;
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
}
