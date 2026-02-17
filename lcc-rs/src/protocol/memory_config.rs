//! Memory Configuration Protocol implementation
//!
//! Implements the OpenLCB Memory Configuration Protocol for reading and writing
//! configuration data and CDI from nodes.

use crate::protocol::frame::GridConnectFrame;
use crate::{Error, Result};

/// Memory address space identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpace {
    /// Configuration space (0xFD)
    Configuration,
    /// All memory space (0xFE)
    AllMemory,
    /// CDI space (0xFF) - Configuration Description Information
    Cdi,
}

impl AddressSpace {
    /// Get the space byte value
    pub fn value(&self) -> u8 {
        match self {
            AddressSpace::Configuration => 0xFD,
            AddressSpace::AllMemory => 0xFE,
            AddressSpace::Cdi => 0xFF,
        }
    }

    /// Get the command flag for this space (without space byte)
    pub fn command_flag(&self) -> u8 {
        match self {
            AddressSpace::Configuration => 0x41,
            AddressSpace::AllMemory => 0x42,
            AddressSpace::Cdi => 0x43,
        }
    }
}

/// Memory configuration command builder
pub struct MemoryConfigCmd;

impl MemoryConfigCmd {
    /// Build a read command datagram
    ///
    /// # Arguments
    /// * `source_alias` - Our node alias
    /// * `dest_alias` - Target node alias
    /// * `space` - Address space to read from
    /// * `address` - Starting address (32-bit)
    /// * `count` - Number of bytes to read (1-64)
    ///
    /// # Returns
    /// Vector of GridConnect frames to send (may be single or multi-frame)
    pub fn build_read(
        source_alias: u16,
        dest_alias: u16,
        space: AddressSpace,
        address: u32,
        count: u8,
    ) -> Result<Vec<GridConnectFrame>> {
        if count == 0 || count > 64 {
            return Err(Error::Protocol(format!(
                "Invalid read count: {} (must be 1-64)",
                count
            )));
        }

        let mut data = Vec::new();
        data.push(0x20); // Memory Configuration command
        data.push(space.command_flag()); // Read command with space

        // Address (big-endian, 32-bit)
        data.extend_from_slice(&address.to_be_bytes());

        // Count
        data.push(count);

        // Create datagram frame(s) - may be multi-frame for 7-byte payload
        GridConnectFrame::create_datagram_frames(source_alias, dest_alias, data)
    }

    /// Parse a read reply datagram
    ///
    /// # Arguments
    /// * `data` - Datagram payload
    ///
    /// # Returns
    /// Read reply with success or failure information
    pub fn parse_read_reply(data: &[u8]) -> Result<ReadReply> {
        if data.len() < 7 {
            return Err(Error::Protocol(format!(
                "Read reply too short: {} bytes",
                data.len()
            )));
        }

        if data[0] != 0x20 {
            return Err(Error::Protocol(format!(
                "Invalid memory config command byte: 0x{:02X}",
                data[0]
            )));
        }

        let command = data[1];
        
        // Check if this is a read reply command (0x5x)
        if (command & 0xF0) != 0x50 {
            return Err(Error::Protocol(format!(
                "Not a read reply command: 0x{:02X}",
                command
            )));
        }
        
        // Bit 3 (0x08) indicates failure
        let is_fail = (command & 0x08) != 0;

        // Extract space from command byte
        let space = match command & 0x03 {
            0x01 => AddressSpace::Configuration,
            0x02 => AddressSpace::AllMemory,
            0x03 => AddressSpace::Cdi,
            _ => return Err(Error::Protocol(format!("Unknown address space: 0x{:02X}", command))),
        };

        // Address (big-endian, 32-bit)
        let address = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);

        if is_fail {
            // Failure - extract error code
            if data.len() < 8 {
                return Err(Error::Protocol("Missing error code in failed reply".into()));
            }

            let error_code = u16::from_be_bytes([data[6], data[7]]);
            let message = if data.len() > 8 {
                String::from_utf8_lossy(&data[8..])
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                String::new()
            };

            Ok(ReadReply::Failed {
                address,
                space,
                error_code,
                message,
            })
        } else {
            // Success - remaining bytes are payload (starts at byte 6)
            let payload = data[6..].to_vec();

            Ok(ReadReply::Success {
                address,
                space,
                data: payload,
            })
        }
    }
}

/// Read reply result
#[derive(Debug, Clone)]
pub enum ReadReply {
    /// Successful read
    Success {
        address: u32,
        space: AddressSpace,
        data: Vec<u8>,
    },
    /// Failed read
    Failed {
        address: u32,
        space: AddressSpace,
        error_code: u16,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_space_values() {
        assert_eq!(AddressSpace::Configuration.value(), 0xFD);
        assert_eq!(AddressSpace::AllMemory.value(), 0xFE);
        assert_eq!(AddressSpace::Cdi.value(), 0xFF);
    }

    #[test]
    fn test_build_read_cdi() {
        let frames = MemoryConfigCmd::build_read(0xAAA, 0xBBB, AddressSpace::Cdi, 0, 64).unwrap();
        
        // CDI read command is 7 bytes: 0x20 0x43 0x00 0x00 0x00 0x00 0x40
        // Should generate 1 frame since 7 bytes fits in single 8-byte frame
        assert_eq!(frames.len(), 1);
        
        // Frame should be DatagramOnly with payload directly in data
        let frame1 = &frames[0];
        assert_eq!(frame1.data.len(), 7);
        assert_eq!(frame1.data[0], 0x20); // Memory config command
        assert_eq!(frame1.data[1], 0x43); // Read CDI space
        assert_eq!(&frame1.data[2..6], &[0, 0, 0, 0]); // Address 0  
        assert_eq!(frame1.data[6], 64); // Count
    }

    #[test]
    fn test_parse_read_reply_success() {
        let data = vec![
            0x20, 0x53, // Command: Read reply OK, CDI space
            0x00, 0x00, 0x00, 0x00, // Address 0
            b'<', b'?', b'x', b'm', b'l', // Payload
        ];

        let reply = MemoryConfigCmd::parse_read_reply(&data).unwrap();
        
        match reply {
            ReadReply::Success { address, space, data } => {
                assert_eq!(address, 0);
                assert_eq!(space, AddressSpace::Cdi);
                assert_eq!(data, b"<?xml");
            }
            _ => panic!("Expected success reply"),
        }
    }

    #[test]
    fn test_parse_read_reply_failed() {
        let data = vec![
            0x20, 0x5B, // Command: Read reply FAIL, CDI space
            0x00, 0x00, 0x00, 0x00, // Address 0
            0x10, 0x82, // Error code
            b'N', b'o', b't', b' ', b'f', b'o', b'u', b'n', b'd', 0x00,
        ];

        let reply = MemoryConfigCmd::parse_read_reply(&data).unwrap();
        
        match reply {
            ReadReply::Failed { address, space, error_code, message } => {
                assert_eq!(address, 0);
                assert_eq!(space, AddressSpace::Cdi);
                assert_eq!(error_code, 0x1082);
                assert_eq!(message, "Not found");
            }
            _ => panic!("Expected failed reply"),
        }
    }
}
