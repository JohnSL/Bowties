//! LCC Traffic Monitor - Message decoding and formatting

use lcc_rs::protocol::frame::GridConnectFrame;
use lcc_rs::protocol::mti::MTI;
use serde::{Serialize, Deserialize};

/// Decoded LCC message for display in traffic monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedMessage {
    /// Timestamp of when the message was received/sent
    pub timestamp: String,
    /// Direction: "S" for sent (our messages), "R" for received
    pub direction: String,
    /// Stable protocol key matching the Rust enum variant name (e.g., "DatagramOnly", "SNIPResponse").
    /// Used by the frontend as a discriminator — never changes format.
    pub mti_name: String,
    /// Human-readable display label (e.g., "Datagram Only", "SNIP Response").
    /// Use this for display only, never for protocol logic.
    pub mti_label: String,
    /// Source alias (12-bit)
    pub source_alias: u16,
    /// Destination alias (12-bit), None for global messages
    pub dest_alias: Option<u16>,
    /// Node ID if this is a VerifiedNode message
    pub node_id: Option<String>,
    /// User-friendly summary for non-technical display
    pub decoded_payload: String,
    /// Protocol-level details for advanced troubleshooting
    pub technical_details: String,
    /// Raw GridConnect frame for debugging
    pub raw_frame: String,
}

impl DecodedMessage {
    /// Decode a GridConnect frame into a displayable message
    ///
    /// # Arguments
    /// * `frame` - The raw GridConnect frame
    /// * `our_alias` - Our node's alias (to determine direction)
    ///
    /// # Returns
    /// Decoded message with all fields populated
    pub fn decode(frame: &GridConnectFrame, our_alias: u16) -> Self {
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f").to_string();
        let raw_frame = frame.to_string();

        // Extract MTI and aliases (with datagram-header fallback)
        let (mti, source_alias, parsed_dest_alias) = match Self::parse_mti_and_alias(frame.header) {
            Ok(result) => result,
            Err(_) => {
                return Self {
                    timestamp,
                    direction: "?".to_string(),
                    mti_name: "Unknown".to_string(),
                    mti_label: "Unknown".to_string(),
                    source_alias: 0,
                    dest_alias: None,
                    node_id: None,
                    decoded_payload: "Unrecognized network message".to_string(),
                    technical_details: format!("Failed to parse header: {:08X}", frame.header),
                    raw_frame,
                };
            }
        };

        // Determine direction
        let direction = if source_alias == our_alias {
            "S".to_string()
        } else {
            "R".to_string()
        };

        // Stable key (protocol discriminator) and human-readable label (display only)
        let mti_name = format!("{:?}", mti);
        let mti_label = Self::mti_display_name(&mti);

        // Extract destination alias if addressed message
        let dest_alias = parsed_dest_alias.or_else(|| Self::extract_dest_alias(&frame, &mti));

        // Decode payload based on MTI
        let (decoded_payload, technical_details, node_id) = Self::decode_payload(&frame, &mti);

        Self {
            timestamp,
            direction,
            mti_name,
            mti_label,
            source_alias,
            dest_alias,
            node_id,
            decoded_payload,
            technical_details,
            raw_frame,
        }
    }

    fn parse_mti_and_alias(header: u32) -> Result<(MTI, u16, Option<u16>), ()> {
        if let Ok((mti, source_alias)) = MTI::from_header(header) {
            if let MTI::Unknown(_) = mti {
                if let Ok((datagram_mti, datagram_source, datagram_dest)) = MTI::from_datagram_header(header) {
                    if matches!(
                        datagram_mti,
                        MTI::DatagramOnly
                            | MTI::DatagramFirst
                            | MTI::DatagramMiddle
                            | MTI::DatagramFinal
                            | MTI::DatagramReceivedOk
                            | MTI::DatagramRejected
                    ) {
                        return Ok((datagram_mti, datagram_source, Some(datagram_dest)));
                    }
                }
            }
            return Ok((mti, source_alias, None));
        }

        if let Ok((datagram_mti, datagram_source, datagram_dest)) = MTI::from_datagram_header(header) {
            if matches!(
                datagram_mti,
                MTI::DatagramOnly
                    | MTI::DatagramFirst
                    | MTI::DatagramMiddle
                    | MTI::DatagramFinal
                    | MTI::DatagramReceivedOk
                    | MTI::DatagramRejected
            ) {
                return Ok((datagram_mti, datagram_source, Some(datagram_dest)));
            }
        }

        Err(())
    }

    /// Extract destination alias from addressed messages
    fn extract_dest_alias(frame: &GridConnectFrame, mti: &MTI) -> Option<u16> {
        // Check if this is an addressed MTI by examining the MTI variant
        match mti {
            MTI::VerifyNodeAddressed 
            | MTI::IdentifyEventsAddressed 
            | MTI::SNIPRequest 
            | MTI::SNIPResponse => {
                // For standard addressed messages, dest is in bits 27-16
                let dest = ((frame.header >> 16) & 0x0FFF) as u16;
                Some(dest)
            }
            MTI::DatagramOnly 
            | MTI::DatagramFirst 
            | MTI::DatagramMiddle 
            | MTI::DatagramFinal 
            | MTI::DatagramReceivedOk 
            | MTI::DatagramRejected => {
                // For datagram messages, dest is in bits 23-12
                let dest = ((frame.header >> 12) & 0x0FFF) as u16;
                Some(dest)
            }
            _ => None,
        }
    }

    /// Decode message payload based on MTI type
    ///
    /// Returns (summary_payload, technical_payload, node_id)
    fn decode_payload(frame: &GridConnectFrame, mti: &MTI) -> (String, String, Option<String>) {
        match mti {
            MTI::VerifiedNode => {
                // VerifiedNode contains 6-byte node ID
                if frame.data.len() == 6 {
                    let node_id = format!(
                        "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
                        frame.data[0], frame.data[1], frame.data[2],
                        frame.data[3], frame.data[4], frame.data[5]
                    );
                    (
                        format!("Node announced itself: {}", node_id),
                        format!("VerifiedNode payload Node ID: {}", node_id),
                        Some(node_id),
                    )
                } else {
                    (
                        "Node announcement received with invalid payload".to_string(),
                        "Invalid VerifiedNode payload".to_string(),
                        None,
                    )
                }
            }
            MTI::SNIPRequest => {
                (
                    "Requested device identity information".to_string(),
                    "SNIP Request".to_string(),
                    None,
                )
            }
            MTI::SNIPResponse => {
                Self::decode_snip_response(frame)
            }
            MTI::DatagramReceivedOk | MTI::DatagramRejected => {
                Self::decode_datagram_ack(frame, mti)
            }
            MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal => {
                Self::decode_datagram_chunk(frame, mti)
            }
            MTI::InitializationComplete => {
                if frame.data.len() == 6 {
                    let node_id = format!(
                        "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
                        frame.data[0], frame.data[1], frame.data[2],
                        frame.data[3], frame.data[4], frame.data[5]
                    );
                    (
                        format!("Node finished startup: {}", node_id),
                        format!("Initialization Complete: {}", node_id),
                        Some(node_id),
                    )
                } else {
                    (
                        "Node finished startup".to_string(),
                        "Initialization Complete".to_string(),
                        None,
                    )
                }
            }
            MTI::VerifyNodeGlobal => {
                (
                    "Checking for nodes on the network".to_string(),
                    "VerifyNodeGlobal".to_string(),
                    None,
                )
            }
            MTI::VerifyNodeAddressed => {
                (
                    "Checking a specific node".to_string(),
                    "VerifyNodeAddressed".to_string(),
                    None,
                )
            }
            _ => {
                // Generic payload: show as hex bytes
                if frame.data.is_empty() {
                    (
                        format!("{}", Self::friendly_mti_summary(mti)),
                        "(no data)".to_string(),
                        None,
                    )
                } else {
                    let hex = frame.data.iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    (
                        format!("{} ({} bytes)", Self::friendly_mti_summary(mti), frame.data.len()),
                        format!("Data: {}", hex),
                        None,
                    )
                }
            }
        }
    }

    fn decode_datagram_chunk(frame: &GridConnectFrame, mti: &MTI) -> (String, String, Option<String>) {
        let phase = match mti {
            MTI::DatagramOnly => "Only",
            MTI::DatagramFirst => "First",
            MTI::DatagramMiddle => "Middle",
            MTI::DatagramFinal => "Final",
            _ => "Chunk",
        };

        let hex = frame.data.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let text = Self::ascii_preview(&frame.data, 24);

        (
            format!("Received datagram chunk ({}, {} bytes)", phase, frame.data.len()),
            format!("| {} | \"{}\"", hex, text),
            None,
        )
    }

    /// Decode SNIP response datagram
    fn decode_snip_response(frame: &GridConnectFrame) -> (String, String, Option<String>) {
        if frame.data.len() < 2 {
            return (
                "Received device identity response fragment".to_string(),
                "Invalid SNIP response".to_string(),
                None,
            );
        }

        // Try to decode SNIP data from payload (bytes 2+)
        let payload = &frame.data[2..];

        // SNIP format: version byte + null-terminated strings
        // manufacturer\0model\0hardware_version\0software_version\0user_name\0user_description\0
        if payload.len() > 1 {
            let version = payload[0];
            let mut details = format!("v{}", version);

            // Try to extract first string (manufacturer)
            if let Some(null_pos) = payload[1..].iter().position(|&b| b == 0) {
                if let Ok(manufacturer) = std::str::from_utf8(&payload[1..=null_pos]) {
                    if !manufacturer.is_empty() {
                        details.push_str(&format!(", Mfr: {}", manufacturer));
                    }
                }
            }

            details.push_str(&format!(" | {} | \"{}\"", Self::hex_bytes(&frame.data), Self::ascii_preview(payload, 24)));

            (
                "Received device identity response fragment".to_string(),
                details,
                None,
            )
        } else {
            (
                "Received device identity response fragment".to_string(),
                format!("bytes: {}", Self::hex_bytes(&frame.data)),
                None,
            )
        }
    }

    /// Decode datagram acknowledgment messages
    fn decode_datagram_ack(frame: &GridConnectFrame, mti: &MTI) -> (String, String, Option<String>) {
        let status = match mti {
            MTI::DatagramReceivedOk => "OK",
            MTI::DatagramRejected => "Rejected",
            _ => "Unknown",
        };

        if frame.data.is_empty() {
            (
                if status == "OK" {
                    "Node acknowledged datagram".to_string()
                } else {
                    "Node rejected datagram".to_string()
                },
                "(no flags)".to_string(),
                None,
            )
        } else {
            let flags = frame.data[0];
            let reason = if *mti == MTI::DatagramRejected && frame.data.len() > 2 {
                format!(" (error: 0x{:04X})", u16::from_be_bytes([frame.data[1], frame.data[2]]))
            } else {
                String::new()
            };
            (
                if status == "OK" {
                    "Node acknowledged datagram".to_string()
                } else {
                    "Node rejected datagram".to_string()
                },
                format!("flags: 0x{:02X}{}", flags, reason),
                None,
            )
        }
    }

    /// Returns a human-readable, space-separated display name for the MTI.
    fn mti_display_name(mti: &MTI) -> String {
        match mti {
            MTI::DatagramOnly       => "Datagram Only".to_string(),
            MTI::DatagramFirst      => "Datagram First".to_string(),
            MTI::DatagramMiddle     => "Datagram Middle".to_string(),
            MTI::DatagramFinal      => "Datagram Final".to_string(),
            MTI::DatagramReceivedOk => "Datagram Received Ok".to_string(),
            MTI::DatagramRejected   => "Datagram Rejected".to_string(),
            _                       => format!("{:?}", mti),
        }
    }

    fn friendly_mti_summary(mti: &MTI) -> &'static str {
        match mti {
            MTI::IdentifyConsumers | MTI::IdentifyProducers => "Event query",
            MTI::IdentifyEventsGlobal => "Requesting all events",
            MTI::IdentifyEventsAddressed => "Requesting events from one node",
            MTI::ConsumerIdentifiedValid
            | MTI::ConsumerIdentifiedInvalid
            | MTI::ConsumerIdentifiedUnknown => "Consumer event status",
            MTI::ProducerIdentifiedValid
            | MTI::ProducerIdentifiedInvalid
            | MTI::ProducerIdentifiedUnknown => "Producer event status",
            MTI::ConsumerRangeIdentified | MTI::ProducerRangeIdentified => "Event range status",
            MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal => "Datagram data transfer",
            _ => "Protocol message",
        }
    }

    fn hex_bytes(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn ascii_preview(bytes: &[u8], max_len: usize) -> String {
        bytes.iter()
            .take(max_len)
            .map(|b| {
                if *b == 0 {
                    '\\'
                } else if b.is_ascii_graphic() || *b == b' ' {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect::<String>()
            .replace('\\', "\\0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_verified_node() {
        // Construct a VerifiedNode frame (MTI 0x19170 = VerifiedNode, source_alias=0xABC)
        let frame = GridConnectFrame {
            header: 0x19170ABC, // MTI=VerifiedNode, source_alias=0xABC
            data: vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        };

        let decoded = DecodedMessage::decode(&frame, 0x123);
        
        assert_eq!(decoded.direction, "R"); // Not our alias
        assert_eq!(decoded.source_alias, 0xABC);
        assert_eq!(decoded.node_id, Some("01.02.03.04.05.06".to_string()));
        assert!(decoded.decoded_payload.contains("01.02.03.04.05.06"));
    }

    #[test]
    fn test_decode_direction() {
        let frame = GridConnectFrame {
            header: 0x10700123, // source_alias=0x123
            data: vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        };

        let decoded = DecodedMessage::decode(&frame, 0x123);
        assert_eq!(decoded.direction, "S"); // Our alias = sent

        let decoded2 = DecodedMessage::decode(&frame, 0x456);
        assert_eq!(decoded2.direction, "R"); // Not our alias = received
    }

    // ── parse_mti_and_alias ────────────────────────────────────────────────────

    #[test]
    fn parse_mti_standard_mti_header() {
        // VerifiedNode (0x19170) with source alias 0xABC
        // header = (0x19170 << 12) | 0xABC = 0x19170ABC
        let result = DecodedMessage::parse_mti_and_alias(0x19170ABC);
        let (mti, src, dest) = result.expect("Should parse successfully");
        assert_eq!(mti, MTI::VerifiedNode);
        assert_eq!(src, 0xABC);
        assert!(dest.is_none(), "No dest in standard global frame");
    }

    #[test]
    fn parse_mti_datagram_header() {
        // DatagramOnly (0x1A000), source=0x100, dest=0x200
        // Datagram header format: (mti_upper << 24) | (dest << 12) | source
        // mti_upper for DatagramOnly = 0x1A
        // header = (0x1A << 24) | (0x200 << 12) | 0x100 = 0x1A200100
        let result = DecodedMessage::parse_mti_and_alias(0x1A200100);
        let (mti, src, dest) = result.expect("Should parse successfully");
        assert_eq!(mti, MTI::DatagramOnly);
        assert_eq!(src, 0x100);
        assert_eq!(dest, Some(0x200));
    }

    // ── extract_dest_alias ────────────────────────────────────────────────────

    #[test]
    fn extract_dest_alias_snip_request_uses_header_bits_27_16() {
        // For standard addressed MTIs, dest = (header >> 16) & 0x0FFF
        let frame = GridConnectFrame {
            header: 0x19DE8100, // SNIPRequest source=0x100
            data: vec![],
        };
        let dest = DecodedMessage::extract_dest_alias(&frame, &MTI::SNIPRequest);
        // (0x19DE8100 >> 16) & 0xFFF = 0x9DE
        assert_eq!(dest, Some(0x9DE));
    }

    #[test]
    fn extract_dest_alias_datagram_only_uses_header_bits_23_12() {
        // For datagram MTIs, dest = (header >> 12) & 0x0FFF
        // header = 0x1A200100 → (header >> 12) & 0xFFF = 0x200
        let frame = GridConnectFrame {
            header: 0x1A200100,
            data: vec![],
        };
        let dest = DecodedMessage::extract_dest_alias(&frame, &MTI::DatagramOnly);
        assert_eq!(dest, Some(0x200));
    }

    #[test]
    fn extract_dest_alias_global_returns_none() {
        let frame = GridConnectFrame {
            header: 0x19490123, // VerifyNodeGlobal source=0x123
            data: vec![],
        };
        let dest = DecodedMessage::extract_dest_alias(&frame, &MTI::VerifyNodeGlobal);
        assert!(dest.is_none(), "Global messages have no destination alias");
    }

    // ── decode_payload ────────────────────────────────────────────────────────

    #[test]
    fn decode_payload_initialization_complete_valid() {
        let frame = GridConnectFrame {
            header: 0x19100ABC,
            data: vec![0x05, 0x02, 0x01, 0x00, 0x00, 0x01],
        };
        let (summary, detail, node_id) = DecodedMessage::decode_payload(&frame, &MTI::InitializationComplete);
        assert!(summary.contains("05.02.01.00.00.01"));
        assert!(detail.contains("05.02.01.00.00.01"));
        assert_eq!(node_id, Some("05.02.01.00.00.01".to_string()));
    }

    #[test]
    fn decode_payload_initialization_complete_invalid_len() {
        let frame = GridConnectFrame {
            header: 0x19100ABC,
            data: vec![0x01, 0x02], // Too short
        };
        let (summary, _detail, node_id) = DecodedMessage::decode_payload(&frame, &MTI::InitializationComplete);
        assert!(summary.contains("startup"));
        assert!(node_id.is_none());
    }

    #[test]
    fn decode_payload_verify_node_global() {
        let frame = GridConnectFrame {
            header: 0x19490123,
            data: vec![],
        };
        let (summary, _detail, node_id) = DecodedMessage::decode_payload(&frame, &MTI::VerifyNodeGlobal);
        assert!(summary.contains("nodes"));
        assert!(node_id.is_none());
    }

    #[test]
    fn decode_payload_snip_request() {
        let frame = GridConnectFrame {
            header: 0x19DE8200,
            data: vec![0x02, 0x00], // dest alias in body
        };
        let (summary, detail, node_id) = DecodedMessage::decode_payload(&frame, &MTI::SNIPRequest);
        assert_eq!(summary, "Requested device identity information");
        assert_eq!(detail, "SNIP Request");
        assert!(node_id.is_none());
    }

    #[test]
    fn decode_payload_datagram_received_ok_no_flags() {
        let frame = GridConnectFrame {
            header: 0x19A28ABC,
            data: vec![],
        };
        let (summary, detail, _) = DecodedMessage::decode_payload(&frame, &MTI::DatagramReceivedOk);
        assert!(summary.contains("acknowledged"));
        assert_eq!(detail, "(no flags)");
    }

    #[test]
    fn decode_payload_datagram_rejected_with_error_code() {
        let frame = GridConnectFrame {
            header: 0x19A48ABC,
            data: vec![0x10, 0x10, 0x80], // flags + error code 0x1080
        };
        let (summary, detail, _) = DecodedMessage::decode_payload(&frame, &MTI::DatagramRejected);
        assert!(summary.contains("rejected"));
        assert!(detail.contains("0x1080"));
    }

    #[test]
    fn decode_payload_generic_no_data() {
        let frame = GridConnectFrame {
            header: 0x19914ABC,
            data: vec![],
        };
        let (summary, detail, _) = DecodedMessage::decode_payload(&frame, &MTI::IdentifyProducers);
        assert!(!summary.is_empty());
        assert_eq!(detail, "(no data)");
    }

    #[test]
    fn decode_payload_generic_with_data() {
        let frame = GridConnectFrame {
            header: 0x198F4ABC,
            data: vec![0xAB, 0xCD],
        };
        let (summary, detail, _) = DecodedMessage::decode_payload(&frame, &MTI::IdentifyConsumers);
        assert!(summary.contains("2 bytes"));
        assert!(detail.contains("AB CD"));
    }

    // ── decode_snip_response ──────────────────────────────────────────────────

    #[test]
    fn decode_snip_response_valid_with_manufacturer() {
        // Version byte + manufacturer string "Acme\0"
        let mut data = vec![0x00, 0x00]; // 2 header bytes
        data.push(4); // SNIP version
        data.extend_from_slice(b"Acme\0");
        let frame = GridConnectFrame {
            header: 0x19A08ABC,
            data,
        };
        let (summary, detail, node_id) = DecodedMessage::decode_snip_response(&frame);
        assert_eq!(summary, "Received device identity response fragment");
        assert!(detail.contains("Acme"), "Detail should contain manufacturer name");
        assert!(node_id.is_none());
    }

    #[test]
    fn decode_snip_response_too_short() {
        let frame = GridConnectFrame {
            header: 0x19A08ABC,
            data: vec![0x01], // Only 1 byte — less than 2 required
        };
        let (summary, _detail, _) = DecodedMessage::decode_snip_response(&frame);
        assert_eq!(summary, "Received device identity response fragment");
    }

    // ── decode_datagram_ack ───────────────────────────────────────────────────

    #[test]
    fn decode_datagram_ack_accepted_no_flags() {
        let frame = GridConnectFrame {
            header: 0x19A28ABC,
            data: vec![],
        };
        let (summary, detail, _) = DecodedMessage::decode_datagram_ack(&frame, &MTI::DatagramReceivedOk);
        assert!(summary.contains("acknowledged"));
        assert_eq!(detail, "(no flags)");
    }

    #[test]
    fn decode_datagram_ack_rejected_with_flags() {
        let frame = GridConnectFrame {
            header: 0x19A48ABC,
            data: vec![0x20], // flags byte only
        };
        let (summary, detail, _) = DecodedMessage::decode_datagram_ack(&frame, &MTI::DatagramRejected);
        assert!(summary.contains("rejected"));
        assert!(detail.contains("0x20"));
    }

    #[test]
    fn decode_datagram_ack_rejected_with_error_code() {
        let frame = GridConnectFrame {
            header: 0x19A48ABC,
            data: vec![0x10, 0x20, 0x30], // flags + 2-byte error code 0x2030
        };
        let (summary, detail, _) = DecodedMessage::decode_datagram_ack(&frame, &MTI::DatagramRejected);
        assert!(summary.contains("rejected"));
        assert!(detail.contains("0x2030"));
    }

    // ── ascii_preview ─────────────────────────────────────────────────────────

    #[test]
    fn ascii_preview_printable_passes_through() {
        let result = DecodedMessage::ascii_preview(b"Hello", 10);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn ascii_preview_null_byte_produces_backslash_zero() {
        // Null byte 0x00 → '\' which is then replaced with "\0" (two chars: \ and 0)
        let result = DecodedMessage::ascii_preview(&[0x00], 10);
        assert_eq!(result, "\\0");
    }

    #[test]
    fn ascii_preview_control_char_becomes_dot() {
        // 0x01 (SOH) is not graphic/space → '.'
        let result = DecodedMessage::ascii_preview(&[0x41, 0x01, 0x42], 10); // A, control, B
        assert_eq!(result, "A.B");
    }

    #[test]
    fn ascii_preview_truncates_at_max_len() {
        let result = DecodedMessage::ascii_preview(b"ABCDEFGHIJ", 5); // 10 bytes, max 5
        assert_eq!(result, "ABCDE");
    }

    // ── mti_display_name ──────────────────────────────────────────────────────

    #[test]
    fn mti_display_name_datagram_variants() {
        assert_eq!(DecodedMessage::mti_display_name(&MTI::DatagramOnly), "Datagram Only");
        assert_eq!(DecodedMessage::mti_display_name(&MTI::DatagramFirst), "Datagram First");
        assert_eq!(DecodedMessage::mti_display_name(&MTI::DatagramMiddle), "Datagram Middle");
        assert_eq!(DecodedMessage::mti_display_name(&MTI::DatagramFinal), "Datagram Final");
        assert_eq!(DecodedMessage::mti_display_name(&MTI::DatagramReceivedOk), "Datagram Received Ok");
        assert_eq!(DecodedMessage::mti_display_name(&MTI::DatagramRejected), "Datagram Rejected");
    }

    #[test]
    fn mti_display_name_fallback_uses_debug_format() {
        // Non-special MTIs fall back to Debug format ("VerifiedNode", etc.)
        let name = DecodedMessage::mti_display_name(&MTI::VerifiedNode);
        assert_eq!(name, "VerifiedNode");
        let name2 = DecodedMessage::mti_display_name(&MTI::SNIPRequest);
        assert_eq!(name2, "SNIPRequest");
    }
}
