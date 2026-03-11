//! Core LCC/OpenLCB types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A 48-bit (6-byte) unique Node ID in the LCC network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeID(pub [u8; 6]);

impl NodeID {
    /// Create a new NodeID from a 6-byte array
    pub fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// Create a NodeID from a slice (must be exactly 6 bytes)
    pub fn from_slice(slice: &[u8]) -> Result<Self, String> {
        if slice.len() != 6 {
            return Err(format!("NodeID must be 6 bytes, got {}", slice.len()));
        }
        let mut bytes = [0u8; 6];
        bytes.copy_from_slice(slice);
        Ok(Self(bytes))
    }

    /// Convert NodeID to a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Convert NodeID to a hex string (e.g., "01.02.03.04.05.06")
    pub fn to_hex_string(&self) -> String {
        format!(
            "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }

    /// Parse a NodeID from a hex string (e.g., "01.02.03.04.05.06" or "010203040506")
    pub fn from_hex_string(s: &str) -> Result<Self, String> {
        let s = s.replace(['.', ' ', '-'], "");
        if s.len() != 12 {
            return Err(format!("Invalid NodeID hex string length: {}", s.len()));
        }
        
        let mut bytes = [0u8; 6];
        for i in 0..6 {
            bytes[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|e| format!("Invalid hex: {}", e))?;
        }
        Ok(Self(bytes))
    }

    /// Hash the Node ID to derive a 12-bit alias using the OpenLCB algorithm
    /// 
    /// The hash function mixes the 6-byte Node ID into a single 32-bit value,
    /// then masks to 12 bits for the alias.
    /// 
    /// OpenLCB spec algorithm:
    /// ```text
    /// reg = id[0] ^ (id[1]<<5) ^ (id[2]<<10) ^ (id[3]<<15) ^ (id[4]<<20) ^ (id[5]<<24) 
    ///     ^ ((id[3]^id[4]^id[5])<<8)
    /// alias = reg & 0x0FFF  // 12-bit mask
    /// ```
    pub fn hash_to_alias(&self) -> Result<NodeAlias, String> {
        let id = &self.0;
        
        // Compute hash: mix bytes with bit shifts
        let mut reg: u32 = id[0] as u32;
        reg ^= (id[1] as u32) << 5;
        reg ^= (id[2] as u32) << 10;
        reg ^= (id[3] as u32) << 15;
        reg ^= (id[4] as u32) << 20;
        reg ^= (id[5] as u32) << 24;
        
        // Add extra mixing term: (id[3] ^ id[4] ^ id[5]) << 8
        let extra = ((id[3] ^ id[4] ^ id[5]) as u32) << 8;
        reg ^= extra;
        
        // Extract 12-bit alias
        let alias_value = (reg & 0x0FFF) as u16;
        
        NodeAlias::new(alias_value)
    }
}

impl fmt::Display for NodeID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex_string())
    }
}

/// A 64-bit (8-byte) Event ID in the LCC network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventID(pub [u8; 8]);

impl EventID {
    /// Create a new EventID from an 8-byte array
    pub fn new(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Create an EventID from a slice (must be exactly 8 bytes)
    pub fn from_slice(slice: &[u8]) -> Result<Self, String> {
        if slice.len() != 8 {
            return Err(format!("EventID must be 8 bytes, got {}", slice.len()));
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(slice);
        Ok(Self(bytes))
    }

    /// Convert EventID to a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Convert EventID to a hex string (e.g., "01.02.03.04.05.06.07.08")
    pub fn to_hex_string(&self) -> String {
        format!(
            "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3],
            self.0[4], self.0[5], self.0[6], self.0[7]
        )
    }
}

impl fmt::Display for EventID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex_string())
    }
}

/// A 12-bit node alias used in CAN frames
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeAlias(pub u16);

impl NodeAlias {
    /// Create a new NodeAlias (must be 12-bit, i.e., <= 0xFFF)
    pub fn new(alias: u16) -> Result<Self, String> {
        if alias > 0xFFF {
            return Err(format!("Alias must be 12-bit (<=0xFFF), got 0x{:X}", alias));
        }
        Ok(Self(alias))
    }

    /// Get the raw alias value
    pub fn value(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for NodeAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:03X}", self.0)
    }
}

/// Information from Simple Node Identification Protocol (SNIP)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SNIPData {
    pub manufacturer: String,
    pub model: String,
    pub hardware_version: String,
    pub software_version: String,
    pub user_name: String,
    pub user_description: String,
}

impl SNIPData {
    /// Sanitize and validate SNIP string fields
    /// Replaces invalid UTF-8 and control characters with '?'
    pub fn sanitize(&mut self) {
        fn clean_string(s: &mut String) {
            *s = s.chars()
                .map(|c| {
                    if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
                        '?'
                    } else {
                        c
                    }
                })
                .collect();
        }

        clean_string(&mut self.manufacturer);
        clean_string(&mut self.model);
        clean_string(&mut self.hardware_version);
        clean_string(&mut self.software_version);
        clean_string(&mut self.user_name);
        clean_string(&mut self.user_description);
    }
}

/// Status of SNIP data retrieval operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SNIPStatus {
    /// SNIP status unknown (not yet queried)
    Unknown,
    /// SNIP request in progress
    InProgress,
    /// SNIP data completely retrieved
    Complete,
    /// SNIP data partially retrieved (some fields missing)
    Partial,
    /// Node does not support SNIP protocol
    NotSupported,
    /// SNIP request timed out
    Timeout,
    /// Error occurred during SNIP retrieval
    Error,
}

/// Connection status of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Connection status unknown
    Unknown,
    /// Verifying node connection
    Verifying,
    /// Node is connected and responding
    Connected,
    /// Node is not responding
    NotResponding,
}

/// Configuration Description Information (CDI) data for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdiData {
    /// Raw CDI XML content
    pub xml_content: String,
    /// Timestamp when CDI was retrieved
    pub retrieved_at: DateTime<Utc>,
}

/// Status of Protocol Identification Protocol (PIP) data retrieval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PIPStatus {
    /// PIP status unknown (not yet queried)
    Unknown,
    /// PIP request in progress
    InProgress,
    /// PIP data successfully retrieved
    Complete,
    /// Node does not support Protocol Identification Protocol
    NotSupported,
    /// PIP request timed out
    Timeout,
    /// Error occurred during PIP retrieval
    Error,
}

/// Protocol flags reported by a node via Protocol Identification Protocol (PIP).
///
/// Parsed from the 6 wire bytes returned in a ProtocolSupportReply frame.
/// Each byte is MSB-first: bit 7 of byte 0 is `simple_protocol`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolFlags {
    // Byte 0
    pub simple_protocol: bool,
    pub datagram: bool,
    pub stream: bool,
    pub memory_configuration: bool,
    pub reservation: bool,
    pub event_exchange: bool,
    pub identification: bool,
    pub teach_learn: bool,
    // Byte 1
    pub remote_button: bool,
    pub acdi: bool,
    pub display: bool,
    pub snip: bool,
    pub cdi: bool,
    pub traction_control: bool,
    pub function_description_information: bool,
    pub dcc_command_station: bool,
    // Byte 2
    pub simple_train_node: bool,
    pub function_configuration: bool,
    pub firmware_upgrade: bool,
    pub firmware_upgrade_active: bool,
}

impl ProtocolFlags {
    /// Parse a `ProtocolFlags` value from the wire bytes in a ProtocolSupportReply.
    ///
    /// The spec specifies 6 bytes, MSB-first within each byte.  Fewer bytes are
    /// accepted (trailing bytes treated as zero) so the caller need not zero-pad
    /// before calling this function.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let b = |idx: usize| if idx < bytes.len() { bytes[idx] } else { 0u8 };
        let bit = |byte: u8, n: u8| (byte >> n) & 1 == 1;

        let b0 = b(0);
        let b1 = b(1);
        let b2 = b(2);

        Self {
            simple_protocol:                  bit(b0, 7),
            datagram:                         bit(b0, 6),
            stream:                           bit(b0, 5),
            memory_configuration:             bit(b0, 4),
            reservation:                      bit(b0, 3),
            event_exchange:                   bit(b0, 2),
            identification:                   bit(b0, 1),
            teach_learn:                      bit(b0, 0),
            remote_button:                    bit(b1, 7),
            acdi:                             bit(b1, 6),
            display:                          bit(b1, 5),
            snip:                             bit(b1, 4),
            cdi:                              bit(b1, 3),
            traction_control:                 bit(b1, 2),
            function_description_information: bit(b1, 1),
            dcc_command_station:              bit(b1, 0),
            simple_train_node:                bit(b2, 7),
            function_configuration:           bit(b2, 6),
            firmware_upgrade:                 bit(b2, 5),
            firmware_upgrade_active:          bit(b2, 4),
        }
    }
}

/// A discovered node on the LCC network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredNode {
    pub node_id: NodeID,
    pub alias: NodeAlias,
    pub snip_data: Option<SNIPData>,
    pub snip_status: SNIPStatus,
    pub connection_status: ConnectionStatus,
    pub last_verified: Option<DateTime<Utc>>,
    pub last_seen: DateTime<Utc>,
    /// Configuration Description Information (CDI) XML data
    pub cdi: Option<CdiData>,
    /// Protocol flags from Protocol Identification Protocol (PIP)
    pub pip_flags: Option<ProtocolFlags>,
    /// Status of PIP data retrieval
    pub pip_status: PIPStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_creation() {
        let node_id = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        assert_eq!(node_id.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    }

    #[test]
    fn test_node_id_from_slice() {
        let slice = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let node_id = NodeID::from_slice(slice).unwrap();
        assert_eq!(node_id.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        // Test invalid length
        let invalid = &[0x01, 0x02, 0x03];
        assert!(NodeID::from_slice(invalid).is_err());
    }

    #[test]
    fn test_node_id_hex_string() {
        let node_id = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        assert_eq!(node_id.to_hex_string(), "01.02.03.04.05.06");
        assert_eq!(node_id.to_string(), "01.02.03.04.05.06");
    }

    #[test]
    fn test_node_id_from_hex_string() {
        // Test with dots
        let node_id = NodeID::from_hex_string("01.02.03.04.05.06").unwrap();
        assert_eq!(node_id.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        // Test without dots
        let node_id = NodeID::from_hex_string("010203040506").unwrap();
        assert_eq!(node_id.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        // Test with dashes
        let node_id = NodeID::from_hex_string("01-02-03-04-05-06").unwrap();
        assert_eq!(node_id.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        // Test invalid
        assert!(NodeID::from_hex_string("01.02.03").is_err());
        assert!(NodeID::from_hex_string("zz.02.03.04.05.06").is_err());
    }

    #[test]
    fn test_event_id() {
        let event_id = EventID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(event_id.to_hex_string(), "01.02.03.04.05.06.07.08");
    }

    #[test]
    fn test_node_alias() {
        let alias = NodeAlias::new(0xAAA).unwrap();
        assert_eq!(alias.value(), 0xAAA);
        assert_eq!(alias.to_string(), "AAA");

        // Test invalid (too large)
        assert!(NodeAlias::new(0x1000).is_err());
    }
    
    // COMPREHENSIVE NODEID ENCODING/DECODING TESTS
    
    #[test]
    fn test_node_id_to_bytes() {
        // Test converting NodeID to bytes for frame encoding
        let node_id = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        let bytes = node_id.as_bytes();
        assert_eq!(bytes, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    }
    
    #[test]
    fn test_node_id_endianness() {
        // NodeID should be big-endian (network byte order)
        // Most significant byte first
        let node_id = NodeID::new([0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA]);
        assert_eq!(node_id.0[0], 0xFF); // MSB first
        assert_eq!(node_id.0[5], 0xAA); // LSB last
    }
    
    #[test]
    fn test_python_reference_node_ids() {
        // Test NodeID values from Python defaults.py
        
        // thisNodeID = [1,2,3,4,5,6]
        let this_node = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        assert_eq!(this_node.to_hex_string(), "01.02.03.04.05.06");
        
        // testNodeID = [2,3,4,5,6,1]
        let test_node = NodeID::new([0x02, 0x03, 0x04, 0x05, 0x06, 0x01]);
        assert_eq!(test_node.to_hex_string(), "02.03.04.05.06.01");
    }
    
    #[test]
    fn test_node_id_from_frame_data() {
        // Test extracting NodeID from VerifiedNode frame data
        let frame_data = vec![0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];
        let node_id = NodeID::from_slice(&frame_data).unwrap();
        assert_eq!(node_id.0, [0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA]);
    }
    
    #[test]
    fn test_node_id_zero_padding_in_hex() {
        // Verify that bytes with leading zeros are properly formatted
        let node_id = NodeID::new([0x00, 0x01, 0x0F, 0x10, 0xFF, 0xAA]);
        let hex = node_id.to_hex_string();
        assert_eq!(hex, "00.01.0F.10.FF.AA");
        
        // Each byte should be exactly 2 hex digits
        let parts: Vec<&str> = hex.split('.').collect();
        assert_eq!(parts.len(), 6);
        for part in parts {
            assert_eq!(part.len(), 2, "Each byte should be 2 hex digits");
        }
    }
    
    #[test]
    fn test_node_id_roundtrip_through_hex() {
        let original = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        let hex = original.to_hex_string();
        let parsed = NodeID::from_hex_string(&hex).unwrap();
        assert_eq!(parsed, original);
    }
    
    #[test]
    fn test_node_id_various_hex_formats() {
        let expected = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        
        // Try different separator styles
        assert_eq!(NodeID::from_hex_string("01.02.03.04.05.06").unwrap(), expected);
        assert_eq!(NodeID::from_hex_string("01-02-03-04-05-06").unwrap(), expected);
        assert_eq!(NodeID::from_hex_string("01 02 03 04 05 06").unwrap(), expected);
        assert_eq!(NodeID::from_hex_string("010203040506").unwrap(), expected);
        
        // Mixed separators should work (all get stripped)
        assert_eq!(NodeID::from_hex_string("01.02-03 04.05-06").unwrap(), expected);
    }
    
    #[test]
    fn test_node_id_uppercase_and_lowercase_hex() {
        // Both should parse correctly
        let upper = NodeID::from_hex_string("AABBCCDDEEFF").unwrap();
        let lower = NodeID::from_hex_string("aabbccddeeff").unwrap();
        let mixed = NodeID::from_hex_string("AaBbCcDdEeFf").unwrap();
        
        assert_eq!(upper.0, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        assert_eq!(lower, upper);
        assert_eq!(mixed, upper);
        
        // Output should always be uppercase
        assert_eq!(upper.to_hex_string(), "AA.BB.CC.DD.EE.FF");
    }
    
    #[test]
    fn test_node_id_invalid_lengths() {
        // Too short
        assert!(NodeID::from_hex_string("0102030405").is_err());
        assert!(NodeID::from_hex_string("01.02.03.04.05").is_err());
        
        // Too long
        assert!(NodeID::from_hex_string("01020304050607").is_err());
        assert!(NodeID::from_hex_string("01.02.03.04.05.06.07").is_err());
        
        // Completely wrong
        assert!(NodeID::from_hex_string("").is_err());
        assert!(NodeID::from_hex_string("xyz").is_err());
    }
    
    #[test]
    fn test_node_id_all_zeros() {
        let zeros = NodeID::new([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(zeros.to_hex_string(), "00.00.00.00.00.00");
        
        let parsed = NodeID::from_hex_string("000000000000").unwrap();
        assert_eq!(parsed, zeros);
    }
    
    #[test]
    fn test_node_id_all_ones() {
        let ones = NodeID::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(ones.to_hex_string(), "FF.FF.FF.FF.FF.FF");
        
        let parsed = NodeID::from_hex_string("FFFFFFFFFFFF").unwrap();
        assert_eq!(parsed, ones);
    }
    
    #[test]
    fn test_event_id_encoding() {
        // Test EventID encoding (8 bytes vs NodeID's 6 bytes)
        let event_id = EventID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(event_id.as_bytes(), &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }
    
    #[test]
    fn test_python_reference_event_id() {
        // testEventID = [0x05, 0x02, 0x01, 0x02, 0x02, 0x00, 0x00, 0x00]
        let test_event = EventID::new([0x05, 0x02, 0x01, 0x02, 0x02, 0x00, 0x00, 0x00]);
        assert_eq!(test_event.to_hex_string(), "05.02.01.02.02.00.00.00");
    }
    
    #[test]
    fn test_event_id_from_slice() {
        let data = vec![0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA, 0x99, 0x88];
        let event_id = EventID::from_slice(&data).unwrap();
        assert_eq!(event_id.0, [0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA, 0x99, 0x88]);
        
        // Wrong length should fail
        assert!(EventID::from_slice(&[0x01, 0x02, 0x03]).is_err());
    }
    
    #[test]
    fn test_alias_boundary_values() {
        // Test 12-bit alias boundaries
        assert!(NodeAlias::new(0x000).is_ok()); // Minimum
        assert!(NodeAlias::new(0x001).is_ok());
        assert!(NodeAlias::new(0x7FF).is_ok()); // Middle
        assert!(NodeAlias::new(0xAAA).is_ok()); // Common test value
        assert!(NodeAlias::new(0xDDD).is_ok()); // Another test value
        assert!(NodeAlias::new(0xFFE).is_ok());
        assert!(NodeAlias::new(0xFFF).is_ok()); // Maximum
        
        // Over 12-bit should fail
        assert!(NodeAlias::new(0x1000).is_err());
        assert!(NodeAlias::new(0xFFFF).is_err());
    }
    
    #[test]
    fn test_alias_display_format() {
        // Aliases should display as 3 uppercase hex digits
        assert_eq!(NodeAlias::new(0x000).unwrap().to_string(), "000");
        assert_eq!(NodeAlias::new(0x001).unwrap().to_string(), "001");
        assert_eq!(NodeAlias::new(0x0AA).unwrap().to_string(), "0AA");
        assert_eq!(NodeAlias::new(0xAAA).unwrap().to_string(), "AAA");
        assert_eq!(NodeAlias::new(0xFFF).unwrap().to_string(), "FFF");
    }
}

