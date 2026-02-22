//! Memory Configuration Protocol implementation
//!
//! Implements the OpenLCB Memory Configuration Protocol for reading and writing
//! configuration data and CDI from nodes.

use crate::protocol::frame::GridConnectFrame;
use crate::{Error, Result};

/// Memory address space identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpace {
    /// ACDI manufacturer-defined space (0xFC)
    AcdiManufacturer,
    /// ACDI user-defined space (0xFB) - holds User Name and User Description
    AcdiUser,
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
            AddressSpace::AcdiUser => 0xFB,
            AddressSpace::AcdiManufacturer => 0xFC,
            AddressSpace::Configuration => 0xFD,
            AddressSpace::AllMemory => 0xFE,
            AddressSpace::Cdi => 0xFF,
        }
    }

    /// Get the command flag for this space (without space byte).
    /// AcdiUser and AcdiManufacturer use the generic format (0x40 + space byte).
    pub fn command_flag(&self) -> u8 {
        match self {
            AddressSpace::AcdiUser => 0x40,
            AddressSpace::AcdiManufacturer => 0x40,
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
    /// Uses embedded format (`0x41`/`0x42`/`0x43`) for spaces `>= 0xFD`
    /// (Configuration/AllMemory/CDI), omitting the separate space byte → 7-byte payload.
    /// Uses generic format (`0x40` + space byte) for `AcdiUser`/`AcdiManufacturer`
    /// → 8-byte payload.  Mirrors `MemoryConfigurationService.fillRequest()` in OpenLCB_Java.
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
        data.push(0x20); // Memory Configuration command byte

        // Embedded format for spaces >= 0xFD (space encoded in low 2 bits of command);
        // generic format for lower spaces (separate space byte follows address).
        let cmd = space.command_flag();
        data.push(cmd);

        // Address (big-endian, 32-bit)
        data.extend_from_slice(&address.to_be_bytes());

        // Generic format only: include the separate address-space byte
        if cmd == 0x40 {
            data.push(space.value());
        }

        // Count
        data.push(count);

        GridConnectFrame::create_datagram_frames(source_alias, dest_alias, data)
    }

    /// Parse a read reply datagram.
    ///
    /// Canonical rule from `OpenLCB_Java MemoryConfigurationService.getPayloadOffset(data)`:
    /// - `reply[1] & 0x03 != 0` → **embedded** reply (`0x51`/`0x52`/`0x53`/`0x59`–`0x5B`):
    ///   space encoded in low bits of command, **no** separate space byte, payload at `[6..]`.
    /// - `reply[1] & 0x03 == 0` → **generic** reply (`0x50`/`0x58`):
    ///   space byte **always** present at `[6]`, payload at `[7..]`.
    ///
    /// # Arguments
    /// * `data` - Datagram payload
    ///
    /// # Returns
    /// Read reply with success or failure information
    pub fn parse_read_reply(data: &[u8]) -> Result<ReadReply> {
        if data.len() < 6 {
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

        // Must be a read reply command (0x5x)
        if (command & 0xF0) != 0x50 {
            return Err(Error::Protocol(format!(
                "Not a read reply command: 0x{:02X}",
                command
            )));
        }

        // Bit 3 (0x08) indicates failure
        let is_fail = (command & 0x08) != 0;

        // Bits 0-1 non-zero → embedded format; zero → generic format
        let is_embedded = (command & 0x03) != 0;

        // Address (big-endian, 32-bit)
        let address = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);

        if is_embedded {
            // Embedded: space = 0xFC + (bits 0-1); no space byte; payload/error at [6..]
            let space = match command & 0x03 {
                0x01 => AddressSpace::Configuration,
                0x02 => AddressSpace::AllMemory,
                0x03 => AddressSpace::Cdi,
                _ => unreachable!(),
            };

            if is_fail {
                if data.len() < 8 {
                    return Err(Error::Protocol("Missing error code in failed embedded reply".into()));
                }
                let error_code = u16::from_be_bytes([data[6], data[7]]);
                let message = if data.len() > 8 {
                    String::from_utf8_lossy(&data[8..])
                        .trim_end_matches('\0')
                        .to_string()
                } else {
                    String::new()
                };
                Ok(ReadReply::Failed { address, space, error_code, message })
            } else {
                Ok(ReadReply::Success { address, space, data: data[6..].to_vec() })
            }
        } else {
            // Generic: space byte ALWAYS at [6], payload/error at [7..]
            if data.len() < 7 {
                return Err(Error::Protocol("Generic format reply too short".into()));
            }
            let space = match data[6] {
                0xFB => AddressSpace::AcdiUser,
                0xFC => AddressSpace::AcdiManufacturer,
                0xFD => AddressSpace::Configuration,
                0xFE => AddressSpace::AllMemory,
                0xFF => AddressSpace::Cdi,
                b => return Err(Error::Protocol(format!(
                    "Unknown space byte in generic reply: 0x{:02X}", b
                ))),
            };

            if is_fail {
                if data.len() < 9 {
                    return Err(Error::Protocol("Missing error code in failed generic reply".into()));
                }
                let error_code = u16::from_be_bytes([data[7], data[8]]);
                let message = if data.len() > 9 {
                    String::from_utf8_lossy(&data[9..])
                        .trim_end_matches('\0')
                        .to_string()
                } else {
                    String::new()
                };
                Ok(ReadReply::Failed { address, space, error_code, message })
            } else {
                Ok(ReadReply::Success { address, space, data: data[7..].to_vec() })
            }
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

    // --- build_read tests ---

    #[test]
    fn test_build_read_cdi() {
        // CDI (0xFF) → embedded format: command 0x43, 7-byte payload, no space byte
        let frames = MemoryConfigCmd::build_read(0xAAA, 0xBBB, AddressSpace::Cdi, 0, 64).unwrap();
        assert_eq!(frames.len(), 1);
        let f = &frames[0];
        assert_eq!(f.data.len(), 7);
        assert_eq!(f.data[0], 0x20); // datagram type
        assert_eq!(f.data[1], 0x43); // embedded CDI read command
        assert_eq!(&f.data[2..6], &[0, 0, 0, 0]); // address 0
        assert_eq!(f.data[6], 64);   // count (no space byte between addr and count)
    }

    #[test]
    fn test_build_read_configuration() {
        // Configuration (0xFD) → embedded format: command 0x41, 7-byte payload, no space byte
        let frames = MemoryConfigCmd::build_read(0x111, 0x222, AddressSpace::Configuration, 0x100, 8).unwrap();
        assert_eq!(frames.len(), 1);
        let f = &frames[0];
        assert_eq!(f.data.len(), 7);
        assert_eq!(f.data[1], 0x41); // embedded Configuration read command
        assert_eq!(&f.data[2..6], &[0x00, 0x00, 0x01, 0x00]); // address 0x100
        assert_eq!(f.data[6], 8);    // count
    }

    #[test]
    fn test_build_read_acdi_user() {
        // AcdiUser (0xFB) → generic format: command 0x40, space byte 0xFB at [6], count at [7], 8-byte payload
        let frames = MemoryConfigCmd::build_read(0x333, 0x444, AddressSpace::AcdiUser, 0, 32).unwrap();
        assert_eq!(frames.len(), 1);
        let f = &frames[0];
        assert_eq!(f.data.len(), 8);
        assert_eq!(f.data[1], 0x40); // generic read command
        assert_eq!(&f.data[2..6], &[0, 0, 0, 0]); // address 0
        assert_eq!(f.data[6], 0xFB); // space byte: AcdiUser
        assert_eq!(f.data[7], 32);   // count
    }

    #[test]
    fn test_build_read_all_memory() {
        // AllMemory (0xFE) → embedded format: command 0x42, 7-byte payload
        let frames = MemoryConfigCmd::build_read(0x555, 0x666, AddressSpace::AllMemory, 0, 1).unwrap();
        assert_eq!(frames.len(), 1);
        let f = &frames[0];
        assert_eq!(f.data.len(), 7);
        assert_eq!(f.data[1], 0x42); // embedded AllMemory read command
    }

    // --- parse_read_reply tests ---

    #[test]
    fn test_parse_read_reply_success_embedded() {
        // Embedded CDI success reply: 0x53 (0x50 | 0x03) — no space byte, data at [6..]
        let data = vec![
            0x20, 0x53, // embedded CDI success
            0x00, 0x00, 0x00, 0x00, // address 0
            b'<', b'?', b'x', b'm', b'l', // payload at [6..]
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
    fn test_parse_read_reply_generic_with_space_byte() {
        // Generic success reply: 0x50 — space byte ALWAYS at [6], data at [7..]
        let data = vec![
            0x20, 0x50, // generic success
            0x00, 0x00, 0x00, 0x80, // address 0x80
            0xFF,       // space byte: CDI
            b'A', b'B', b'C', // payload at [7..]
        ];
        let reply = MemoryConfigCmd::parse_read_reply(&data).unwrap();
        match reply {
            ReadReply::Success { address, space, data } => {
                assert_eq!(address, 0x80);
                assert_eq!(space, AddressSpace::Cdi);
                assert_eq!(data, b"ABC");
            }
            _ => panic!("Expected success reply"),
        }
    }

    #[test]
    fn test_parse_read_reply_generic_acdi_user() {
        // Generic success for AcdiUser (0xFB) — space byte at [6]
        let data = vec![
            0x20, 0x50,
            0x00, 0x00, 0x00, 0x00,
            0xFB, // space byte: AcdiUser
            b'N', b'a', b'm', b'e',
        ];
        let reply = MemoryConfigCmd::parse_read_reply(&data).unwrap();
        match reply {
            ReadReply::Success { space, data, .. } => {
                assert_eq!(space, AddressSpace::AcdiUser);
                assert_eq!(data, b"Name");
            }
            _ => panic!("Expected success reply"),
        }
    }

    #[test]
    fn test_parse_read_reply_failed_embedded() {
        // Embedded CDI fail: 0x5B (0x58 | 0x03) — error code at [6-7], message at [8..]
        let data = vec![
            0x20, 0x5B, // embedded CDI failure
            0x00, 0x00, 0x00, 0x00, // address 0
            0x10, 0x82, // error code at [6-7]
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

    #[test]
    fn test_parse_read_reply_failed_generic() {
        // Generic fail: 0x58 — space byte at [6], error code at [7-8]
        let data = vec![
            0x20, 0x58, // generic failure
            0x00, 0x00, 0x00, 0x00, // address 0
            0xFB, // space byte: AcdiUser
            0x10, 0x82, // error code at [7-8]
            b'E', b'r', b'r', 0x00,
        ];
        let reply = MemoryConfigCmd::parse_read_reply(&data).unwrap();
        match reply {
            ReadReply::Failed { space, error_code, message, .. } => {
                assert_eq!(space, AddressSpace::AcdiUser);
                assert_eq!(error_code, 0x1082);
                assert_eq!(message, "Err");
            }
            _ => panic!("Expected failed reply"),
        }
    }
}
