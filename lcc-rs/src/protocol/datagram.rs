//! Multi-frame datagram reassembly for LCC protocol
//!
//! LCC datagrams can span multiple CAN frames when the payload exceeds 8 bytes.
//! This module handles reassembly of DatagramFirst → DatagramMiddle* → DatagramFinal sequences.

use crate::protocol::mti::MTI;
use crate::protocol::frame::GridConnectFrame;
use crate::{Error, Result};
use std::collections::HashMap;

/// State of datagram reassembly for a specific source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatagramState {
    /// No datagram in progress
    Idle,
    /// Receiving multi-frame datagram
    Receiving,
    /// Datagram complete and ready for processing
    Complete,
    /// Error occurred during reassembly
    Error,
}

/// Datagram assembler for multi-frame datagrams
/// 
/// Maintains state for reassembling datagrams from multiple nodes concurrently.
/// Each source node can have one datagram in progress at a time.
#[derive(Debug)]
pub struct DatagramAssembler {
    /// Active datagrams indexed by source alias
    active_datagrams: HashMap<u16, DatagramBuffer>,
}

/// Buffer for a single datagram being assembled
#[derive(Debug, Clone)]
struct DatagramBuffer {
    /// Current state of this datagram
    state: DatagramState,
    /// Accumulated payload bytes (extracted from frames)
    payload: Vec<u8>,
    /// Destination alias for this datagram (for future multi-source handling)
    #[allow(dead_code)]
    dest_alias: u16,
}

impl DatagramAssembler {
    /// Create a new datagram assembler
    pub fn new() -> Self {
        Self {
            active_datagrams: HashMap::new(),
        }
    }

    /// Handle an incoming datagram frame
    /// 
    /// Returns Some(payload) if datagram is complete, None if more frames needed
    pub fn handle_frame(&mut self, frame: &GridConnectFrame) -> Result<Option<Vec<u8>>> {
        let (mti, source_alias, dest_alias) = MTI::from_datagram_header(frame.header)?;

        match mti {
            MTI::DatagramOnly => {
                // Single-frame datagram - return payload immediately
                let payload = Self::get_payload(&frame.data)?;
                Ok(Some(payload))
            }
            MTI::DatagramFirst => {
                // Start of multi-frame datagram
                let payload = Self::get_payload(&frame.data)?;
                
                let buffer = DatagramBuffer {
                    state: DatagramState::Receiving,
                    payload,
                    dest_alias,
                };
                
                self.active_datagrams.insert(source_alias, buffer);
                Ok(None) // Need more frames
            }
            MTI::DatagramMiddle => {
                // Middle frame of multi-frame datagram
                if let Some(buffer) = self.active_datagrams.get_mut(&source_alias) {
                    if buffer.state != DatagramState::Receiving {
                        return Err(Error::Protocol(format!(
                            "Unexpected DatagramMiddle from alias {:03X} in state {:?}",
                            source_alias, buffer.state
                        )));
                    }
                    
                    let payload = Self::get_payload(&frame.data)?;
                    buffer.payload.extend_from_slice(&payload);
                    Ok(None) // Need more frames
                } else {
                    Err(Error::Protocol(format!(
                        "DatagramMiddle from unknown source {:03X}",
                        source_alias
                    )))
                }
            }
            MTI::DatagramFinal => {
                // Final frame of multi-frame datagram
                if let Some(buffer) = self.active_datagrams.remove(&source_alias) {
                    if buffer.state != DatagramState::Receiving {
                        return Err(Error::Protocol(format!(
                            "Unexpected DatagramFinal from alias {:03X} in state {:?}",
                            source_alias, buffer.state
                        )));
                    }
                    
                    let payload = Self::get_payload(&frame.data)?;
                    let mut complete_payload = buffer.payload;
                    complete_payload.extend_from_slice(&payload);
                    
                    Ok(Some(complete_payload))
                } else {
                    Err(Error::Protocol(format!(
                        "DatagramFinal from unknown source {:03X}",
                        source_alias
                    )))
                }
            }
            _ => {
                Err(Error::Protocol(format!(
                    "Expected datagram MTI, got {:?}",
                    mti
                )))
            }
        }
    }

    /// Extract payload bytes from a datagram frame
    /// 
    /// For datagram frames, the destination is encoded ONLY in the header,
    /// not in the data bytes. So we return all data bytes as-is.
    fn get_payload(data: &[u8]) -> Result<Vec<u8>> {
        // Datagram data is pure payload - no destination encoding
        Ok(data.to_vec())
    }

    /// Send acknowledgment for a received datagram
    /// 
    /// Returns the GridConnectFrame for DatagramReceivedOk to be sent
    pub fn send_acknowledgment(
        source_alias: u16,
        dest_alias: u16,
    ) -> Result<GridConnectFrame> {
        // Delegate to the free constructor with flags=0x00 (no reply pending,
        // no timeout extension). JMRI always sends the flags byte; omitting
        // it is technically valid per S-9.7.3.2 but some nodes may expect it.
        Ok(build_datagram_received_ok_frame(source_alias, dest_alias, 0x00))
    }

    /// Clear any stale or errored datagrams for a specific source
    pub fn clear_source(&mut self, source_alias: u16) {
        self.active_datagrams.remove(&source_alias);
    }

    /// Get current state for a source
    pub fn get_state(&self, source_alias: u16) -> DatagramState {
        self.active_datagrams
            .get(&source_alias)
            .map(|b| b.state)
            .unwrap_or(DatagramState::Idle)
    }
}

impl Default for DatagramAssembler {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a `DatagramReceivedOk` addressed-message frame (S-9.7.3.2 §4.2).
///
/// Payload layout after the standard MTI header:
///
/// ```text
/// [dest_alias(2 BE, top nibble zeroed per addressed-message convention), flags(1)]
/// ```
///
/// `flags` = `0x00` in the common ACK case (no reply pending, no timeout
/// extension). JMRI always emits the flags byte; omitting it is technically
/// valid per S-9.7.3.2 but some nodes may expect it. The existing
/// production emitter [`DatagramAssembler::send_acknowledgment`] delegates
/// here with `flags = 0x00`.
///
/// # Panics
///
/// Panics if `source_alias` does not encode a valid 12-bit alias — the
/// underlying `MTI::to_header` returns `Err` on invalid aliases, and this
/// helper unwraps because well-formed 12-bit aliases are a caller
/// precondition (matching the OIR / memory-config builders in this crate).
pub fn build_datagram_received_ok_frame(
    source_alias: u16,
    dest_alias: u16,
    flags: u8,
) -> GridConnectFrame {
    let header = MTI::DatagramReceivedOk
        .to_header(source_alias)
        .expect("valid 12-bit source alias");
    let data = vec![
        ((dest_alias >> 8) & 0x0F) as u8,
        (dest_alias & 0xFF) as u8,
        flags,
    ];
    GridConnectFrame { header, data }
}

/// Build a `DatagramRejected` addressed-message frame (S-9.7.3 §3.3.3).
///
/// Payload layout after the standard MTI header:
///
/// ```text
/// [dest_alias(2 BE, top nibble zeroed per addressed-message convention),
///  error_code(2 BE)]
/// ```
///
/// The `0x1000` bit of `error_code` is the "resend OK" flag (temporary
/// error): peers with the flag set are expected to be retried by the
/// requester; peers with the flag clear are permanent rejections. The
/// encoder does not interpret the flag — callers pass the exact 16-bit
/// code from S-9.7.3 §3.5.5.
///
/// This builder exists per protocol-crate completeness
/// (`lcc-rs.instructions.md`) — Bowties has no production emitter of
/// `DatagramRejected` today (peer initiates DR toward us, not the other
/// way), but the builder is the single citation seam for the DR payload
/// byte order and removes verbatim duplication across integration test
/// fixtures.
///
/// # Panics
///
/// Panics if `source_alias` does not encode a valid 12-bit alias (same
/// caller precondition as [`build_datagram_received_ok_frame`]).
pub fn build_datagram_rejected_frame(
    source_alias: u16,
    dest_alias: u16,
    error_code: u16,
) -> GridConnectFrame {
    let header = MTI::DatagramRejected
        .to_header(source_alias)
        .expect("valid 12-bit source alias");
    let data = vec![
        ((dest_alias >> 8) & 0x0F) as u8,
        (dest_alias & 0xFF) as u8,
        (error_code >> 8) as u8,
        (error_code & 0xFF) as u8,
    ];
    GridConnectFrame { header, data }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_datagram_frame(mti: MTI, source: u16, dest: u16, data: Vec<u8>) -> GridConnectFrame {
        let header = mti.to_header_with_dest(source, dest).unwrap();
        GridConnectFrame { header, data }
    }

    #[test]
    fn test_single_frame_datagram() {
        let mut assembler = DatagramAssembler::new();
        
        // Create a DatagramOnly frame - data contains only the payload
        let frame = create_datagram_frame(
            MTI::DatagramOnly,
            0x123,
            0x456,
            vec![0x41, 0x42, 0x43, 0x44, 0x45, 0x46], // "ABCDEF" payload
        );
        
        let result = assembler.handle_frame(&frame).unwrap();
        assert_eq!(result, Some(vec![0x41, 0x42, 0x43, 0x44, 0x45, 0x46]));
    }

    #[test]
    fn test_multi_frame_datagram() {
        let mut assembler = DatagramAssembler::new();
        
        // First frame - 8 bytes of payload
        let frame1 = create_datagram_frame(
            MTI::DatagramFirst,
            0x123,
            0x456,
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        );
        assert_eq!(assembler.handle_frame(&frame1).unwrap(), None);
        
        // Middle frame - 8 bytes of payload
        let frame2 = create_datagram_frame(
            MTI::DatagramMiddle,
            0x123,
            0x456,
            vec![0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10],
        );
        assert_eq!(assembler.handle_frame(&frame2).unwrap(), None);
        
        // Final frame - 4 bytes of payload  
        let frame3 = create_datagram_frame(
            MTI::DatagramFinal,
            0x123,
            0x456,
            vec![0x11, 0x12, 0x13, 0x14],
        );
        let result = assembler.handle_frame(&frame3).unwrap();
        assert_eq!(
            result,
            Some(vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 
                      0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
                      0x11, 0x12, 0x13, 0x14])
        );
    }

    #[test]
    fn test_concurrent_datagrams() {
        let mut assembler = DatagramAssembler::new();
        
        // Start datagram from source 0x111
        let frame1a = create_datagram_frame(
            MTI::DatagramFirst,
            0x111,
            0x456,
            vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x01],
        );
        assert_eq!(assembler.handle_frame(&frame1a).unwrap(), None);
        
        // Start datagram from different source 0x222
        let frame2a = create_datagram_frame(
            MTI::DatagramFirst,
            0x222,
            0x456,
            vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
        );
        assert_eq!(assembler.handle_frame(&frame2a).unwrap(), None);
        
        // Complete first datagram
        let frame1b = create_datagram_frame(
            MTI::DatagramFinal,
            0x111,
            0x456,
            vec![0x02, 0x03],
        );
        let result1 = assembler.handle_frame(&frame1b).unwrap();
        assert_eq!(result1, Some(vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x01, 0x02, 0x03]));
        
        // Complete second datagram
        let frame2b = create_datagram_frame(
            MTI::DatagramFinal,
            0x222,
            0x456,
            vec![0x99, 0xAA],
        );
        let result2 = assembler.handle_frame(&frame2b).unwrap();
        assert_eq!(result2, Some(vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA]));
    }

    #[test]
    fn test_acknowledgment() {
        let ack_frame = DatagramAssembler::send_acknowledgment(0x123, 0x456).unwrap();
        
        // DatagramReceivedOk is a standard addressed MTI
        let (mti, source) = ack_frame.get_mti().unwrap();
        assert_eq!(mti, MTI::DatagramReceivedOk);
        assert_eq!(source, 0x123);
        
        // Destination should be in the data payload (2 bytes + flags byte)
        assert_eq!(ack_frame.data.len(), 3);
        let dest = ((ack_frame.data[0] as u16) << 8) | (ack_frame.data[1] as u16);
        assert_eq!(dest, 0x456);
        assert_eq!(ack_frame.data[2], 0x00); // flags: no reply pending, no timeout
    }
    
    #[test]
    fn test_snip_response_frame_analysis() {
        // From LccPro logs - actual SNIP response frames
        // First frame: 19a08c41 1A AA 04 4F 70 65 6E 4D - MTI 0x19A08 (SNIPResponse)
        
        let frame_str = ":X19a08c41N1AAA044F70656E4D;";
        let frame = GridConnectFrame::parse(frame_str).unwrap();
        
        println!("\n=== SNIP Response Frame Analysis ===");
        println!("Frame: {}", frame_str);
        println!("Header: 0x{:08X}", frame.header);
        println!("Data: {:02X?}", frame.data);
        
        // Standard MTI parsing
        let (mti, source) = MTI::from_header(frame.header).unwrap();
        println!("\nStandard MTI parsing:");
        println!("  MTI: {:?} (0x{:X})", mti, mti.value());
        println!("  Source: 0x{:03X}", source);
        
        // Datagram header parsing (WRONG for SNIP responses!)
        let (dg_mti, dg_source, dg_dest) = MTI::from_datagram_header(frame.header).unwrap();
        println!("\nDatagram header parsing (WRONG):");
        println!("  MTI: {:?} (0x{:X})", dg_mti, dg_mti.value());
        println!("  Source: 0x{:03X}, Dest: 0x{:03X}", dg_source, dg_dest);
        
        // Data analysis
        println!("\nData byte analysis:");
        println!("  Byte 0: 0x{:02X} - Datagram frame type", frame.data[0]);
        println!("  Byte 1: 0x{:02X} - Datagram overhead?", frame.data[1]);
        println!("  Bytes 2+: {:02X?} - SNIP payload", &frame.data[2..]);
        
        // This test demonstrates the bug: SNIP responses use MTI 0x19A08,
        // not datagram MTIs (0x1A000, 0x1B000, etc.)
        assert_eq!(mti, MTI::SNIPResponse);
        assert_ne!(dg_mti, MTI::SNIPResponse);  // Datagram parsing gets it wrong!
    }

    // --- build_datagram_received_ok_frame / build_datagram_rejected_frame tests ---

    #[test]
    fn build_datagram_received_ok_frame_encodes_addressed_ack() {
        let frame = build_datagram_received_ok_frame(0x123, 0x456, 0x00);
        let (mti, source) = MTI::from_header(frame.header).unwrap();
        assert_eq!(mti, MTI::DatagramReceivedOk);
        assert_eq!(source, 0x123);
        // Payload: [dest_hi & 0x0F, dest_lo, flags].
        assert_eq!(frame.data, vec![0x04, 0x56, 0x00]);
    }

    #[test]
    fn build_datagram_received_ok_frame_forwards_flags_byte() {
        // Reply-pending bit (0x80) still ends up in the flags byte verbatim.
        let frame = build_datagram_received_ok_frame(0x123, 0x456, 0x80);
        assert_eq!(frame.data[2], 0x80);
    }

    #[test]
    fn send_acknowledgment_matches_free_constructor() {
        // The instance-associated helper delegates to the free constructor
        // with flags=0x00. Both must produce identical frames.
        let via_method = DatagramAssembler::send_acknowledgment(0xAAA, 0xBBB).unwrap();
        let via_free = build_datagram_received_ok_frame(0xAAA, 0xBBB, 0x00);
        assert_eq!(via_method.header, via_free.header);
        assert_eq!(via_method.data, via_free.data);
    }

    #[test]
    fn build_datagram_rejected_frame_encodes_addressed_reject() {
        let frame = build_datagram_rejected_frame(0x123, 0x456, 0x1042);
        let (mti, source) = MTI::from_header(frame.header).unwrap();
        assert_eq!(mti, MTI::DatagramRejected);
        assert_eq!(source, 0x123);
        // Payload: [dest_hi & 0x0F, dest_lo, code_hi, code_lo].
        assert_eq!(frame.data, vec![0x04, 0x56, 0x10, 0x42]);
    }

    #[test]
    fn build_datagram_rejected_frame_preserves_resend_ok_bit() {
        // The 0x2000 bit (temporary error, resend OK per S-9.7.3 §3.5.5) is
        // caller-owned semantics; the encoder must not touch it.
        let frame = build_datagram_rejected_frame(0x111, 0x222, 0x2020);
        assert_eq!(frame.data[2..4], [0x20, 0x20]);
    }
}
