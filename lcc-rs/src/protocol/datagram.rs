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
        // DatagramReceivedOk is a standard addressed MTI, not a datagram MTI
        // The destination is encoded in the data payload as 2 bytes (alias)
        let header = MTI::DatagramReceivedOk.to_header(source_alias)?;
        
        // Include destination alias in the payload (2 bytes, big-endian)
        let data = vec![
            ((dest_alias >> 8) & 0xFF) as u8,
            (dest_alias & 0xFF) as u8,
        ];
        
        let frame = GridConnectFrame {
            header,
            data,
        };
        
        Ok(frame)
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
        
        // Destination should be in the data payload (2 bytes)
        assert_eq!(ack_frame.data.len(), 2);
        let dest = ((ack_frame.data[0] as u16) << 8) | (ack_frame.data[1] as u16);
        assert_eq!(dest, 0x456);
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
}
