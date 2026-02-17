//! GridConnect frame parsing and encoding
//!
//! GridConnect format: `:X[8-hex-header]N[0-16-hex-data];`
//! Example: `:X19170123N0102030405060708;`
//!
//! This is the standard ASCII representation of CAN frames used in OpenLCB/LCC.

use crate::{Error, Result, protocol::mti::MTI};

/// A GridConnect-formatted CAN frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridConnectFrame {
    /// 29-bit CAN header
    pub header: u32,
    /// Data payload (0-8 bytes for standard CAN)
    pub data: Vec<u8>,
}

impl GridConnectFrame {
    /// Create a new GridConnect frame
    pub fn new(header: u32, data: Vec<u8>) -> Result<Self> {
        if header > 0x1FFFFFFF {
            return Err(Error::InvalidFrame(format!(
                "Header must be 29-bit (<=0x1FFFFFFF), got 0x{:X}",
                header
            )));
        }
        if data.len() > 8 {
            return Err(Error::InvalidFrame(format!(
                "Data must be <=8 bytes, got {}",
                data.len()
            )));
        }
        Ok(Self { header, data })
    }
    
    /// Create a frame from MTI and source alias
    pub fn from_mti(mti: MTI, source_alias: u16, data: Vec<u8>) -> Result<Self> {
        let header = mti.to_header(source_alias)?;
        Self::new(header, data)
    }
    
    /// Parse a GridConnect frame from a string
    /// 
    /// Format: `:X[8-hex-header]N[0-16-hex-data];`
    /// 
    /// # Examples
    /// ```
    /// use lcc_rs::protocol::GridConnectFrame;
    /// 
    /// let frame = GridConnectFrame::parse(":X19170123N0102030405060708;").unwrap();
    /// assert_eq!(frame.header, 0x19170123);
    /// assert_eq!(frame.data, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    /// ```
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        
        // Check start and end markers (case-insensitive for 'X')
        if s.len() < 2 || !s[..2].eq_ignore_ascii_case(":X") {
            return Err(Error::InvalidFrame(format!(
                "Frame must start with ':X' or ':x', got '{}'",
                s.chars().take(2).collect::<String>()
            )));
        }
        if !s.ends_with(';') {
            return Err(Error::InvalidFrame(format!(
                "Frame must end with ';', got '{}'",
                s.chars().last().unwrap_or(' ')
            )));
        }
        
        // Find the 'N' separator (case-insensitive)
        let n_pos = s.to_uppercase().find('N').ok_or_else(|| {
            Error::InvalidFrame("Frame must contain 'N' separator".to_string())
        })?;
        
        // Extract header (between :X and N)
        let header_str = &s[2..n_pos];
        if header_str.len() != 8 {
            return Err(Error::InvalidFrame(format!(
                "Header must be 8 hex digits, got {} digits",
                header_str.len()
            )));
        }
        
        let header = u32::from_str_radix(header_str, 16).map_err(|e| {
            Error::InvalidFrame(format!("Invalid header hex: {}", e))
        })?;
        
        // Extract data (between N and ;)
        let data_str = &s[n_pos + 1..s.len() - 1];
        
        // Data must be even number of hex digits
        if data_str.len() % 2 != 0 {
            return Err(Error::InvalidFrame(format!(
                "Data must have even number of hex digits, got {}",
                data_str.len()
            )));
        }
        
        // Parse data bytes
        let mut data = Vec::new();
        for i in (0..data_str.len()).step_by(2) {
            let byte = u8::from_str_radix(&data_str[i..i + 2], 16).map_err(|e| {
                Error::InvalidFrame(format!("Invalid data hex: {}", e))
            })?;
            data.push(byte);
        }
        
        Self::new(header, data)
    }
    
    /// Encode the frame to GridConnect string format
    /// 
    /// # Examples
    /// ```
    /// use lcc_rs::protocol::GridConnectFrame;
    /// 
    /// let frame = GridConnectFrame::new(0x19170123, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]).unwrap();
    /// assert_eq!(frame.to_string(), ":X19170123N0102030405060708;");
    /// ```
    pub fn to_string(&self) -> String {
        let mut s = format!(":X{:08X}N", self.header);
        for byte in &self.data {
            s.push_str(&format!("{:02X}", byte));
        }
        s.push(';');
        s
    }
    
    /// Get the MTI and source alias from this frame
    pub fn get_mti(&self) -> Result<(MTI, u16)> {
        MTI::from_header(self.header)
    }
    
    /// Get the source alias from this frame
    pub fn source_alias(&self) -> u16 {
        (self.header & 0xFFF) as u16
    }
    
    /// Create an addressed message frame with proper body format
    /// 
    /// Addressed messages encode the destination alias in the first 2 bytes
    /// of the body (big-endian), followed by the payload data.
    /// 
    /// Python reference pattern from verifyNodeAddressed.py:
    /// body = [(dest>>8)&0xFF, dest&0xFF] + nodeID
    pub fn from_addressed_mti(
        mti: MTI,
        source_alias: u16,
        dest_alias: u16,
        payload: Vec<u8>,
    ) -> Result<Self> {
        if dest_alias > 0xFFF {
            return Err(Error::InvalidFrame(format!(
                "Destination alias must be 12-bit (<=0xFFF), got 0x{:X}",
                dest_alias
            )));
        }
        
        // Encode destination alias in first 2 bytes (big-endian)
        let mut data = vec![(dest_alias >> 8) as u8 & 0x0F, (dest_alias & 0xFF) as u8];
        data.extend(payload);
        
        if data.len() > 8 {
            return Err(Error::InvalidFrame(format!(
                "Addressed message body too large: {} bytes (max 8)",
                data.len()
            )));
        }
        
        let header = mti.to_header(source_alias)?;
        Self::new(header, data)
    }
    
    /// Extract destination alias from addressed message body
    /// 
    /// Returns (dest_alias, payload) where payload is the data after the 2-byte dest alias
    pub fn get_dest_from_body(&self) -> Result<(u16, &[u8])> {
        if self.data.len() < 2 {
            return Err(Error::InvalidFrame(format!(
                "Addressed message must have at least 2 bytes for dest alias, got {}",
                self.data.len()
            )));
        }
        
        // Decode destination alias from first 2 bytes (big-endian, 12-bit)
        let dest_alias = (((self.data[0] & 0x0F) as u16) << 8) | (self.data[1] as u16);
        let payload = &self.data[2..];
        
        Ok((dest_alias, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frame_with_data() {
        let input = ":X19170123N0102030405060708;";
        let frame = GridConnectFrame::parse(input).unwrap();
        
        assert_eq!(frame.header, 0x19170123);
        assert_eq!(frame.data, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_parse_valid_frame_no_data() {
        let input = ":X04900AAAN;";
        let frame = GridConnectFrame::parse(input).unwrap();
        
        assert_eq!(frame.header, 0x04900AAA);
        assert_eq!(frame.data, vec![]);
    }

    #[test]
    fn test_parse_with_whitespace() {
        let input = "  :X19170123N0102030405060708;  \n";
        let frame = GridConnectFrame::parse(input).unwrap();
        
        assert_eq!(frame.header, 0x19170123);
        assert_eq!(frame.data, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_parse_missing_start() {
        let input = "19170123N0102030405060708;";
        assert!(GridConnectFrame::parse(input).is_err());
    }

    #[test]
    fn test_parse_missing_end() {
        let input = ":X19170123N0102030405060708";
        assert!(GridConnectFrame::parse(input).is_err());
    }

    #[test]
    fn test_parse_missing_n_separator() {
        let input = ":X191701230102030405060708;";
        assert!(GridConnectFrame::parse(input).is_err());
    }

    #[test]
    fn test_parse_invalid_header_length() {
        let input = ":X1917012N01;";
        assert!(GridConnectFrame::parse(input).is_err());
    }

    #[test]
    fn test_parse_invalid_header_hex() {
        let input = ":XZZZZZZZZNFF;";
        assert!(GridConnectFrame::parse(input).is_err());
    }

    #[test]
    fn test_parse_odd_data_length() {
        let input = ":X19170123N010;";
        assert!(GridConnectFrame::parse(input).is_err());
    }

    #[test]
    fn test_parse_invalid_data_hex() {
        let input = ":X19170123NZZ;";
        assert!(GridConnectFrame::parse(input).is_err());
    }

    #[test]
    fn test_encode_frame() {
        let frame = GridConnectFrame::new(
            0x19170123,
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        ).unwrap();
        
        assert_eq!(frame.to_string(), ":X19170123N0102030405060708;");
    }

    #[test]
    fn test_encode_frame_no_data() {
        let frame = GridConnectFrame::new(0x04900AAA, vec![]).unwrap();
        assert_eq!(frame.to_string(), ":X04900AAAN;");
    }

    #[test]
    fn test_round_trip_with_data() {
        let original = ":X19170123N0102030405060708;";
        let frame = GridConnectFrame::parse(original).unwrap();
        let encoded = frame.to_string();
        assert_eq!(encoded, original);
    }

    #[test]
    fn test_round_trip_no_data() {
        let original = ":X04900AAAN;";
        let frame = GridConnectFrame::parse(original).unwrap();
        let encoded = frame.to_string();
        assert_eq!(encoded, original);
    }

    #[test]
    fn test_frame_too_large_header() {
        let result = GridConnectFrame::new(0x20000000, vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_frame_too_large_data() {
        let result = GridConnectFrame::new(0x19170123, vec![0; 9]);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_mti() {
        let frame = GridConnectFrame::parse(":X19490AAAN;").unwrap();
        let (mti, alias) = frame.get_mti().unwrap();
        assert_eq!(mti, MTI::VerifyNodeGlobal);
        assert_eq!(alias, 0xAAA);
    }

    #[test]
    fn test_source_alias() {
        let frame = GridConnectFrame::parse(":X19170123N0102030405060708;").unwrap();
        assert_eq!(frame.source_alias(), 0x123);
    }

    #[test]
    fn test_from_mti() {
        let frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            0xAAA,
            vec![],
        ).unwrap();
        
        assert_eq!(frame.to_string(), ":X19490AAAN;");
    }

    #[test]
    fn test_property_parse_encode_identity() {
        let test_frames = vec![
            ":X19490AAAN;",
            ":X19170123N0102030405060708;",
            ":X19170DDDNFFFFFFFFFFFF;",
            ":X19970555N;",
        ];
        
        for original in test_frames {
            let frame = GridConnectFrame::parse(original).unwrap();
            let encoded = frame.to_string();
            assert_eq!(encoded, original, "Round-trip failed for {}", original);
            
            // Parse again to ensure stability
            let frame2 = GridConnectFrame::parse(&encoded).unwrap();
            assert_eq!(frame, frame2, "Double parse failed for {}", original);
        }
    }
    
    // ADDRESSED MESSAGE BODY FORMAT TESTS
    
    #[test]
    fn test_addressed_message_body_format() {
        // Test Python pattern: body = [(dest>>8)&0xFF, dest&0xFF] + payload
        // VerifyNodeAddressed to 0x0DDD with NodeID [0x01,0x02,0x03,0x04,0x05,0x06]
        let frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            0xAAA,
            0x0DDD,
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ).unwrap();
        
        // Expected body: [0x0D, 0xDD, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]
        assert_eq!(frame.data, vec![0x0D, 0xDD, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        
        // Verify the encoded string
        assert_eq!(frame.to_string(), ":X19488AAAN0DDD010203040506;");
    }
    
    #[test]
    fn test_addressed_message_extract_dest() {
        // Parse a frame with addressed message body
        let frame = GridConnectFrame::parse(":X19488AAAN0DDD010203040506;").unwrap();
        
        let (dest, payload) = frame.get_dest_from_body().unwrap();
        assert_eq!(dest, 0x0DDD);
        assert_eq!(payload, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    }
    
    #[test]
    fn test_addressed_message_empty_payload() {
        // Addressed message with just destination, no additional payload
        let frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            0xAAA,
            0x0DDD,
            vec![],
        ).unwrap();
        
        assert_eq!(frame.data, vec![0x0D, 0xDD]);
        assert_eq!(frame.to_string(), ":X19488AAAN0DDD;");
        
        let (dest, payload) = frame.get_dest_from_body().unwrap();
        assert_eq!(dest, 0x0DDD);
        assert_eq!(payload, &[]);
    }
    
    #[test]
    fn test_addressed_message_max_dest_alias() {
        // Test with maximum valid 12-bit destination alias
        let frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            0xAAA,
            0xFFF,
            vec![0xFF],
        ).unwrap();
        
        assert_eq!(frame.data, vec![0x0F, 0xFF, 0xFF]);
        
        let (dest, payload) = frame.get_dest_from_body().unwrap();
        assert_eq!(dest, 0xFFF);
        assert_eq!(payload, &[0xFF]);
    }
    
    #[test]
    fn test_addressed_message_dest_too_large() {
        // Destination alias must be 12-bit
        let result = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            0xAAA,
            0x1000,  // Too large
            vec![],
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_addressed_message_body_too_large() {
        // Body can be at most 8 bytes (2 for dest + 6 for payload)
        let result = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            0xAAA,
            0x0DDD,
            vec![0; 7],  // 2 + 7 = 9 bytes total
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_addressed_message_extract_from_short_body() {
        // Body must have at least 2 bytes
        let frame = GridConnectFrame::new(0x19488AAA, vec![0x0D]).unwrap();
        let result = frame.get_dest_from_body();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_python_reference_addressed_patterns() {
        // Test exact patterns from Python verifyNodeAddressed.py
        
        // Test 1: VerifyNodeAddressed to 0xDDD with full NodeID
        let node_id = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            0xAAA,
            0x0DDD,
            node_id.clone(),
        ).unwrap();
        assert_eq!(frame.to_string(), ":X19488AAAN0DDD010203040506;");
        
        // Verify round-trip
        let parsed = GridConnectFrame::parse(&frame.to_string()).unwrap();
        let (dest, payload) = parsed.get_dest_from_body().unwrap();
        assert_eq!(dest, 0x0DDD);
        assert_eq!(payload, node_id.as_slice());
        
        // Test 2: IdentifyEventsAddressed with EventID
        let event_id = vec![0x05, 0x02, 0x01, 0x02, 0x02, 0x00];
        let frame = GridConnectFrame::from_addressed_mti(
            MTI::IdentifyEventsAddressed,
            0xAAA,
            0x0DDD,
            event_id,
        ).unwrap();
        assert_eq!(frame.to_string(), ":X19968AAAN0DDD050201020200;");
    }
    
    #[test]
    fn test_addressed_message_12bit_boundaries() {
        // Test boundary values for 12-bit destination alias encoding
        let test_cases = vec![
            (0x000, vec![0x00, 0x00]),
            (0x001, vec![0x00, 0x01]),
            (0x0FF, vec![0x00, 0xFF]),
            (0x100, vec![0x01, 0x00]),
            (0xAAA, vec![0x0A, 0xAA]),
            (0xDDD, vec![0x0D, 0xDD]),
            (0xFFF, vec![0x0F, 0xFF]),
        ];
        
        for (dest_alias, expected_bytes) in test_cases {
            let frame = GridConnectFrame::from_addressed_mti(
                MTI::VerifyNodeAddressed,
                0xAAA,
                dest_alias,
                vec![],
            ).unwrap();
            
            assert_eq!(
                &frame.data[0..2],
                expected_bytes.as_slice(),
                "Failed for dest_alias=0x{:X}",
                dest_alias
            );
            
            let (decoded_dest, _) = frame.get_dest_from_body().unwrap();
            assert_eq!(decoded_dest, dest_alias, "Round-trip failed for 0x{:X}", dest_alias);
        }
    }
    
    // COMPREHENSIVE FRAME ENCODING TEST SUITE - Python Reference Patterns
    
    mod python_reference_fixtures {
        use super::*;
        
        #[test]
        fn test_makeframestring_pattern_no_data() {
            // Python: makeframestring(0x19490AAA, None) → ":X19490AAAN;"
            let frame = GridConnectFrame::new(0x19490AAA, vec![]).unwrap();
            assert_eq!(frame.to_string(), ":X19490AAAN;");
        }
        
        #[test]
        fn test_makeframestring_pattern_with_data() {
            // Python: makeframestring(0x19170DDD, [0xFF,0xEE,0xDD,0xCC,0xBB,0xAA])
            // Expected: ":X19170DDDNFFEEDDCCBBAA;"
            let frame = GridConnectFrame::new(
                0x19170DDD,
                vec![0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA],
            ).unwrap();
            assert_eq!(frame.to_string(), ":X19170DDDNFFEEDDCCBBAA;");
        }
        
        #[test]
        fn test_makeframestring_with_event_id() {
            // Python: makeframestring(0x198F4AAA, testEventID)
            // testEventID = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
            let frame = GridConnectFrame::new(
                0x198F4AAA,
                vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            ).unwrap();
            assert_eq!(frame.to_string(), ":X198F4AAAN0102030405060708;");
        }
        
        #[test]
        fn test_byte_zero_padding() {
            // Test that bytes are zero-padded to exactly 2 hex digits
            // [0x01] should produce "01" not "1"
            let frame = GridConnectFrame::new(0x19170AAA, vec![0x01]).unwrap();
            assert_eq!(frame.to_string(), ":X19170AAAN01;");
            
            // Multiple low bytes
            let frame = GridConnectFrame::new(
                0x19170AAA,
                vec![0x00, 0x01, 0x0F, 0x10],
            ).unwrap();
            assert_eq!(frame.to_string(), ":X19170AAAN00010F10;");
        }
        
        #[test]
        fn test_uppercase_hex_output() {
            // Python always outputs uppercase hex
            // Use valid 29-bit header: 0x1ABCDEF1 (not 0xABCDEF12 which is too large)
            let frame = GridConnectFrame::new(
                0x1ABCDEF1,
                vec![0xAB, 0xCD, 0xEF],
            ).unwrap();
            let encoded = frame.to_string();
            assert_eq!(encoded, ":X1ABCDEF1NABCDEF;");
            assert!(!encoded.contains('a'), "Should be uppercase");
            assert!(!encoded.contains('b'), "Should be uppercase");
        }
        
        #[test]
        fn test_case_insensitive_parsing() {
            // Parser should accept lowercase, uppercase, or mixed
            let lowercase = ":x19490aaan;";
            let uppercase = ":X19490AAAN;";
            let mixed = ":X19490aaaN;";
            
            let frame_lower = GridConnectFrame::parse(lowercase).unwrap();
            let frame_upper = GridConnectFrame::parse(uppercase).unwrap();
            let frame_mixed = GridConnectFrame::parse(mixed).unwrap();
            
            assert_eq!(frame_lower.header, 0x19490AAA);
            assert_eq!(frame_upper.header, 0x19490AAA);
            assert_eq!(frame_mixed.header, 0x19490AAA);
            
            // All should encode to uppercase
            assert_eq!(frame_lower.to_string(), ":X19490AAAN;");
            assert_eq!(frame_upper.to_string(), ":X19490AAAN;");
            assert_eq!(frame_mixed.to_string(), ":X19490AAAN;");
        }
        
        #[test]
        fn test_common_message_patterns_from_python() {
            // Collection of common frame patterns from Python test files
            let test_cases = vec![
                // (header, data, expected_string)
                (0x19490AAA, vec![], ":X19490AAAN;"),                          // VerifyNodeGlobal
                (0x19170DDD, vec![0xFF,0xEE,0xDD,0xCC,0xBB,0xAA], ":X19170DDDNFFEEDDCCBBAA;"), // VerifiedNode
                (0x198F4AAA, vec![0x01,0x02,0x03,0x04,0x05,0x06,0x07,0x08], ":X198F4AAAN0102030405060708;"), // IdentifyConsumers
                (0x194C4DDD, vec![0xFF,0xEE,0xDD,0xCC,0xBB,0xAA,0x99], ":X194C4DDDNFFEEDDCCBBAA99;"), // ConsumerIdentifiedValid
                (0x17020AAA, vec![], ":X17020AAAN;"),                          // CID
                (0x10700AAA, vec![], ":X10700AAAN;"),                          // RID
                (0x10703AAA, vec![], ":X10703AAAN;"),                          // AMR
            ];
            
            for (header, data, expected) in test_cases {
                let frame = GridConnectFrame::new(header, data).unwrap();
                assert_eq!(frame.to_string(), expected, "Failed for header 0x{:X}", header);
                
                // Verify parse round-trip
                let parsed = GridConnectFrame::parse(expected).unwrap();
                assert_eq!(parsed.header, header);
                assert_eq!(parsed.to_string(), expected);
            }
        }
        
        #[test]
        fn test_datagram_frame_patterns() {
            // Test datagram frame encoding patterns
            // DatagramOnly with ≤8 bytes
            let frame = GridConnectFrame::new(
                0x1A000AAA,
                vec![0x01, 0x02, 0x03, 0x04],
            ).unwrap();
            assert_eq!(frame.to_string(), ":X1A000AAAN01020304;");
            
            // DatagramFirst with full 8 bytes
            let frame = GridConnectFrame::new(
                0x1B000AAA,
                vec![0xFF; 8],
            ).unwrap();
            assert_eq!(frame.to_string(), ":X1B000AAANFFFFFFFFFFFFFFFF;");
            
            // DatagramFinal with 2 bytes
            let frame = GridConnectFrame::new(
                0x1D000AAA,
                vec![0x01, 0x02],
            ).unwrap();
            assert_eq!(frame.to_string(), ":X1D000AAAN0102;");
        }
        
        #[test]
        fn test_max_data_length() {
            // CAN frames support exactly 8 bytes max
            let frame = GridConnectFrame::new(0x19170AAA, vec![0; 8]).unwrap();
            assert_eq!(frame.to_string(), ":X19170AAAN0000000000000000;");
            
            // 9 bytes should fail
            let result = GridConnectFrame::new(0x19170AAA, vec![0; 9]);
            assert!(result.is_err());
        }
        
        #[test]
        fn test_empty_vs_no_data() {
            // Empty data (no bytes after N)
            let frame = GridConnectFrame::new(0x19490AAA, vec![]).unwrap();
            assert_eq!(frame.to_string(), ":X19490AAAN;");
            
            // Parse should handle empty data section
            let parsed = GridConnectFrame::parse(":X19490AAAN;").unwrap();
            assert_eq!(parsed.data.len(), 0);
            
            // One zero byte is different from no bytes
            let frame = GridConnectFrame::new(0x19490AAA, vec![0x00]).unwrap();
            assert_eq!(frame.to_string(), ":X19490AAAN00;");
        }
        
        #[test]
        fn test_header_exactly_8_hex_digits() {
            // Header must be exactly 8 hex digits
            // Test padding low values
            let frame = GridConnectFrame::new(0x00000AAA, vec![]).unwrap();
            assert_eq!(frame.to_string(), ":X00000AAAN;");
            
            let frame = GridConnectFrame::new(0x00000001, vec![]).unwrap();
            assert_eq!(frame.to_string(), ":X00000001N;");
            
            // Maximum 29-bit header
            let frame = GridConnectFrame::new(0x1FFFFFFF, vec![]).unwrap();
            assert_eq!(frame.to_string(), ":X1FFFFFFFN;");
        }
    }
    
    // FRAME DECODING EDGE CASE TESTS
    
    mod edge_cases {
        use super::*;
        
        #[test]
        fn test_standard_vs_extended_frame_format() {
            // Extended frame (29-bit): `:X[8-hex]N;` - what we support
            let extended = GridConnectFrame::parse(":X00000123N;").unwrap();
            assert_eq!(extended.header, 0x00000123);
            
            // Standard frame (11-bit): `:S123N;` - should fail (not supported yet)
            // This would require different parsing logic
            let standard = GridConnectFrame::parse(":S123N;");
            assert!(standard.is_err(), "Standard CAN frames not supported");
        }
        
        #[test]
        fn test_header_boundary_values() {
            // Test 29-bit boundary (0x1FFFFFFF is max)
            let max_valid = GridConnectFrame::new(0x1FFFFFFF, vec![]).unwrap();
            assert_eq!(max_valid.to_string(), ":X1FFFFFFFN;");
            
            // One bit over should fail
            let over = GridConnectFrame::new(0x20000000, vec![]);
            assert!(over.is_err());
            
            // Way over should fail
            let way_over = GridConnectFrame::new(0xFFFFFFFF, vec![]);
            assert!(way_over.is_err());
            
            // Minimum value should work
            let min = GridConnectFrame::new(0x00000000, vec![]).unwrap();
            assert_eq!(min.to_string(), ":X00000000N;");
        }
        
        #[test]
        fn test_data_length_boundaries() {
            // 0 bytes - valid
            assert!(GridConnectFrame::new(0x19490AAA, vec![]).is_ok());
            
            // 1-7 bytes - valid
            for len in 1..=7 {
                let result = GridConnectFrame::new(0x19490AAA, vec![0xFF; len]);
                assert!(result.is_ok(), "Failed for {} bytes", len);
            }
            
            // 8 bytes - valid (maximum)
            assert!(GridConnectFrame::new(0x19490AAA, vec![0xFF; 8]).is_ok());
            
            // 9 bytes - invalid
            assert!(GridConnectFrame::new(0x19490AAA, vec![0xFF; 9]).is_err());
            
            // More than 9 - invalid
            assert!(GridConnectFrame::new(0x19490AAA, vec![0xFF; 16]).is_err());
        }
        
        #[test]
        fn test_malformed_delimiters() {
            // Missing ':'
            assert!(GridConnectFrame::parse("X19490AAAN;").is_err());
            
            // Missing 'X'
            assert!(GridConnectFrame::parse(":19490AAAN;").is_err());
            
            // Wrong start marker
            assert!(GridConnectFrame::parse("#X19490AAAN;").is_err());
            
            // Missing 'N'
            assert!(GridConnectFrame::parse(":X19490AAA;").is_err());
            
            // Double 'N'
            assert!(GridConnectFrame::parse(":X19490AAANN;").is_err());
            
            // Missing ';'
            assert!(GridConnectFrame::parse(":X19490AAAN").is_err());
            
            // Wrong end marker
            assert!(GridConnectFrame::parse(":X19490AAAN:").is_err());
            
            // Extra semicolon
            assert!(GridConnectFrame::parse(":X19490AAAN;;").is_err());
        }
        
        #[test]
        fn test_invalid_hex_characters() {
            // Invalid hex in header
            assert!(GridConnectFrame::parse(":X1949GAAAN;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAZN;").is_err());
            
            // Invalid hex in data
            assert!(GridConnectFrame::parse(":X19490AAANZZ;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAAN0G;").is_err());
            
            // Special characters
            assert!(GridConnectFrame::parse(":X19490AA@N;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAAN!!;").is_err());
        }
        
        #[test]
        fn test_odd_number_of_hex_digits_in_data() {
            // Data must have even number of hex digits (pairs of bytes)
            assert!(GridConnectFrame::parse(":X19490AAAN0;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAAN010;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAAN01020;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAAN0102030;").is_err());
            
            // Even numbers should work
            assert!(GridConnectFrame::parse(":X19490AAAN01;").is_ok());
            assert!(GridConnectFrame::parse(":X19490AAAN0102;").is_ok());
            assert!(GridConnectFrame::parse(":X19490AAAN010203;").is_ok());
        }
        
        #[test]
        fn test_header_length_variations() {
            // Header must be exactly 8 hex digits
            
            // Too short
            assert!(GridConnectFrame::parse(":X1N;").is_err());
            assert!(GridConnectFrame::parse(":X12N;").is_err());
            assert!(GridConnectFrame::parse(":X123N;").is_err());
            assert!(GridConnectFrame::parse(":X1234567N;").is_err());
            
            // Exactly 8 - valid
            assert!(GridConnectFrame::parse(":X12345678N;").is_ok());
            
            // Too long - will fail because N won't be found at position 10
            assert!(GridConnectFrame::parse(":X123456789N;").is_err());
        }
        
        #[test]
        fn test_empty_string_and_whitespace_only() {
            assert!(GridConnectFrame::parse("").is_err());
            assert!(GridConnectFrame::parse(" ").is_err());
            assert!(GridConnectFrame::parse("\n").is_err());
            assert!(GridConnectFrame::parse("\t").is_err());
            assert!(GridConnectFrame::parse("   \n\t  ").is_err());
        }
        
        #[test]
        fn test_whitespace_variations() {
            // Leading/trailing whitespace should be trimmed
            let cases = vec![
                "  :X19490AAAN;  ",
                "\n:X19490AAAN;\n",
                "\t:X19490AAAN;\t",
                " \t\n:X19490AAAN; \n\t ",
            ];
            
            for case in cases {
                let frame = GridConnectFrame::parse(case).unwrap();
                assert_eq!(frame.header, 0x19490AAA, "Failed for: {:?}", case);
            }
            
            // Internal whitespace should fail
            assert!(GridConnectFrame::parse(":X 19490AAAN;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAA N;").is_err());
            assert!(GridConnectFrame::parse(":X19490AAAN 01;").is_err());
        }
        
        #[test]
        fn test_data_overflow_during_parsing() {
            // More than 8 bytes of data
            let result = GridConnectFrame::parse(":X19490AAAN010203040506070809;");
            assert!(result.is_err());
            
            // Exactly 16 hex digits (8 bytes) should work
            let result = GridConnectFrame::parse(":X19490AAAN0102030405060708;");
            assert!(result.is_ok());
            
            // 18 hex digits (9 bytes) should fail
            let result = GridConnectFrame::parse(":X19490AAAN01020304050607080;");
            // This will fail on odd digits first
            assert!(result.is_err());
        }
        
        #[test]
        fn test_mixed_case_throughout() {
            // Test various mixed case combinations
            let cases = vec![
                ":x19490AaaN;",
                ":X19490aAaN;",
                ":x19490AAAN01aB;",
                ":X19490aaaN01AB;",
            ];
            
            for case in cases {
                let frame = GridConnectFrame::parse(case).unwrap();
                assert_eq!(frame.header, 0x19490AAA, "Failed for: {}", case);
                // Output should always be uppercase
                assert!(frame.to_string().starts_with(":X"));
            }
        }
        
        #[test]
        fn test_zero_byte_encoding() {
            // Test that zero bytes are properly encoded with leading zeros
            let cases = vec![
                (vec![0x00], "00"),
                (vec![0x00, 0x00], "0000"),
                (vec![0x00, 0x01], "0001"),
                (vec![0x01, 0x00], "0100"),
                (vec![0x00, 0x00, 0x00, 0x00], "00000000"),
            ];
            
            for (data, expected_hex) in cases {
                let frame = GridConnectFrame::new(0x19490AAA, data).unwrap();
                let encoded = frame.to_string();
                assert!(encoded.contains(expected_hex), 
                    "Expected '{}' in '{}'", expected_hex, encoded);
            }
        }
        
        #[test]
        fn test_special_mti_patterns() {
            // Test parsing frames with special MTI patterns
            
            // CID frame (CheckID)
            let frame = GridConnectFrame::parse(":X17020AAAN;").unwrap();
            let (mti, alias) = frame.get_mti().unwrap();
            assert_eq!(mti, MTI::CheckID);
            assert_eq!(alias, 0xAAA);
            
            // RID frame (ReserveID)
            let frame = GridConnectFrame::parse(":X10700AAAN;").unwrap();
            let (mti, alias) = frame.get_mti().unwrap();
            assert_eq!(mti, MTI::ReserveID);
            assert_eq!(alias, 0xAAA);
            
            // AMR frame (Alias Map Reset)
            let frame = GridConnectFrame::parse(":X10703AAAN;").unwrap();
            let (mti, alias) = frame.get_mti().unwrap();
            assert_eq!(mti, MTI::AliasMapReset);
            assert_eq!(alias, 0xAAA);
        }
    }
    
    // ALIAS ALLOCATION SEQUENCE TESTS
    
    mod alias_allocation {
        use super::*;
        
        #[test]
        fn test_cid_frame_sequence() {
            // CID (Check ID) frames are sent during alias allocation
            // Python reference from testStartup.py
            
            // CID frames use MTI 0x17020
            let cid_frame = GridConnectFrame::from_mti(
                MTI::CheckID,
                0xAAA,
                vec![],
            ).unwrap();
            
            assert_eq!(cid_frame.to_string(), ":X17020AAAN;");
            
            // Verify parsing round-trip
            let parsed = GridConnectFrame::parse(":X17020AAAN;").unwrap();
            let (mti, alias) = parsed.get_mti().unwrap();
            assert_eq!(mti, MTI::CheckID);
            assert_eq!(alias, 0xAAA);
        }
        
        #[test]
        fn test_rid_frame() {
            // RID (Reserve ID) frame sent after successful CID sequence
            let rid_frame = GridConnectFrame::from_mti(
                MTI::ReserveID,
                0xAAA,
                vec![],
            ).unwrap();
            
            assert_eq!(rid_frame.to_string(), ":X10700AAAN;");
            
            // Verify MTI extraction
            let (mti, alias) = rid_frame.get_mti().unwrap();
            assert_eq!(mti, MTI::ReserveID);
            assert_eq!(alias, 0xAAA);
        }
        
        #[test]
        fn test_amr_frame() {
            // AMR (Alias Map Reset) sent on alias conflict
            // Python reference from testAliasConflict.py
            let amr_frame = GridConnectFrame::from_mti(
                MTI::AliasMapReset,
                0xAAA,
                vec![],
            ).unwrap();
            
            assert_eq!(amr_frame.to_string(), ":X10703AAAN;");
            
            let (mti, alias) = amr_frame.get_mti().unwrap();
            assert_eq!(mti, MTI::AliasMapReset);
            assert_eq!(alias, 0xAAA);
        }
        
        #[test]
        fn test_initialization_complete() {
            // InitializationComplete with NodeID in body
            // Python reference: NodeID follows after successful RID
            let node_id = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
            let init_frame = GridConnectFrame::from_mti(
                MTI::InitializationComplete,
                0xAAA,
                node_id.clone(),
            ).unwrap();
            
            assert_eq!(init_frame.to_string(), ":X10010AAAN010203040506;");
            
            // Verify NodeID is in the data
            assert_eq!(init_frame.data, node_id);
        }
        
        #[test]
        fn test_alias_boundary_values() {
            // Alias must be 12-bit (0x001 to 0xFFF)
            // Note: 0x000 is allowed by the frame encoding but may be invalid at protocol level
            
            // Minimum non-zero alias
            let frame = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0x001, vec![]).unwrap();
            assert_eq!(frame.source_alias(), 0x001);
            
            // Maximum alias
            let frame = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0xFFF, vec![]).unwrap();
            assert_eq!(frame.source_alias(), 0xFFF);
            
            // Zero alias (technically allowed in frame encoding)
            let frame = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0x000, vec![]).unwrap();
            assert_eq!(frame.source_alias(), 0x000);
            
            // Over 12-bit should fail
            let result = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0x1000, vec![]);
            assert!(result.is_err());
        }
        
        #[test]
        fn test_complete_alias_allocation_sequence() {
            // Test a complete alias allocation sequence from Python testStartup.py
            let alias = 0xAAA;
            let node_id = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
            
            // Step 1: Send CID frame
            let cid = GridConnectFrame::from_mti(MTI::CheckID, alias, vec![]).unwrap();
            assert_eq!(cid.to_string(), ":X17020AAAN;");
            
            // Step 2: Send RID frame (if no conflict)
            let rid = GridConnectFrame::from_mti(MTI::ReserveID, alias, vec![]).unwrap();
            assert_eq!(rid.to_string(), ":X10700AAAN;");
            
            // Step 3: Send InitializationComplete with NodeID
            let init = GridConnectFrame::from_mti(
                MTI::InitializationComplete,
                alias,
                node_id.clone(),
            ).unwrap();
            assert_eq!(init.to_string(), ":X10010AAAN010203040506;");
            
            // Step 4: Optionally send VerifiedNode (in response to VerifyNodeGlobal)
            let verified = GridConnectFrame::from_mti(
                MTI::VerifiedNode,
                alias,
                node_id.clone(),
            ).unwrap();
            assert_eq!(verified.to_string(), ":X19170AAAN010203040506;");
        }
        
        #[test]
        fn test_alias_conflict_recovery() {
            // From testAliasConflict.py: When conflict detected, send AMR and re-acquire
            let old_alias = 0xAAA;
            let new_alias = 0xBBB;
            
            // Conflict detected: send AMR with old alias
            let amr = GridConnectFrame::from_mti(MTI::AliasMapReset, old_alias, vec![]).unwrap();
            assert_eq!(amr.to_string(), ":X10703AAAN;");
            
            // Re-acquire with new alias: CID sequence
            let cid = GridConnectFrame::from_mti(MTI::CheckID, new_alias, vec![]).unwrap();
            assert_eq!(cid.to_string(), ":X17020BBBN;");
            
            // RID with new alias
            let rid = GridConnectFrame::from_mti(MTI::ReserveID, new_alias, vec![]).unwrap();
            assert_eq!(rid.to_string(), ":X10700BBBN;");
        }
        
        #[test]
        fn test_multiple_aliases_in_sequence() {
            // Test that different aliases encode correctly in the same sequence
            let aliases = vec![0x001, 0x100, 0x555, 0xAAA, 0xDDD, 0xFFF];
            
            for alias in aliases {
                let cid = GridConnectFrame::from_mti(MTI::CheckID, alias, vec![]).unwrap();
                let expected = format!(":X17020{:03X}N;", alias);
                assert_eq!(cid.to_string(), expected, "Failed for alias 0x{:03X}", alias);
                
                // Verify round-trip
                let parsed = GridConnectFrame::parse(&expected).unwrap();
                assert_eq!(parsed.source_alias(), alias);
            }
        }
    }
    
    // PROPERTY-BASED TESTS WITH PROPTEST
    
    mod property_tests {
        use super::*;
        use proptest::prelude::*;
        
        proptest! {
            #[test]
            fn prop_round_trip_encode_decode(
                header in 0u32..=0x1FFFFFFF,
                data in prop::collection::vec(any::<u8>(), 0..=8)
            ) {
                let frame = GridConnectFrame::new(header, data.clone()).unwrap();
                let encoded = frame.to_string();
                let decoded = GridConnectFrame::parse(&encoded).unwrap();
                
                prop_assert_eq!(decoded.header, header);
                prop_assert_eq!(decoded.data, data);
            }
            
            #[test]
            fn prop_header_preserves_bits(
                header in 0u32..=0x1FFFFFFF
            ) {
                // Verify all 29 bits are preserved through encoding
                let frame = GridConnectFrame::new(header, vec![]).unwrap();
                let decoded = GridConnectFrame::parse(&frame.to_string()).unwrap();
                prop_assert_eq!(decoded.header, header);
            }
            
            #[test]
            fn prop_alias_extraction(
                alias in 0u16..=0xFFF
            ) {
                // Any valid 12-bit alias should be extractable from header
                let frame = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, alias, vec![]).unwrap();
                prop_assert_eq!(frame.source_alias(), alias);
            }
            
            #[test]
            fn prop_data_length_invariant(
                header in 0u32..=0x1FFFFFFF,
                data in prop::collection::vec(any::<u8>(), 0..=8)
            ) {
                // Valid frames always have 0-8 bytes of data
                let frame = GridConnectFrame::new(header, data.clone()).unwrap();
                prop_assert!(frame.data.len() <= 8);
                prop_assert_eq!(frame.data.len(), data.len());
            }
            
            #[test]
            fn prop_hex_case_insensitivity(
                header in 0u32..=0x1FFFFFFF,
                data in prop::collection::vec(any::<u8>(), 0..=8)
            ) {
                let frame = GridConnectFrame::new(header, data).unwrap();
                let encoded = frame.to_string();
                let lowercase = encoded.to_lowercase();
                
                // Both should parse to same frame
                let upper_parsed = GridConnectFrame::parse(&encoded).unwrap();
                let lower_parsed = GridConnectFrame::parse(&lowercase).unwrap();
                
                prop_assert_eq!(upper_parsed.header, lower_parsed.header);
                prop_assert_eq!(upper_parsed.data, lower_parsed.data);
            }
            
            #[test]
            fn prop_mti_roundtrip(
                alias in 0u16..=0xFFF
            ) {
                // All MTI types should round-trip through header encoding
                let mtis = vec![
                    MTI::VerifyNodeGlobal,
                    MTI::VerifiedNode,
                    MTI::IdentifyConsumers,
                    MTI::IdentifyProducers,
                    MTI::CheckID,
                    MTI::ReserveID,
                    MTI::InitializationComplete,
                ];
                
                for mti in mtis {
                    let header = mti.to_header(alias).unwrap();
                    let (decoded_mti, decoded_alias) = MTI::from_header(header).unwrap();
                    prop_assert_eq!(decoded_mti, mti);
                    prop_assert_eq!(decoded_alias, alias);
                }
            }
            
            #[test]
            fn prop_output_always_uppercase(
                header in 0u32..=0x1FFFFFFF,
                data in prop::collection::vec(any::<u8>(), 0..=8)
            ) {
                let frame = GridConnectFrame::new(header, data).unwrap();
                let encoded = frame.to_string();
                
                // Should never contain lowercase hex digits
                prop_assert!(!encoded.chars().any(|c| c.is_ascii_lowercase() && c.is_ascii_hexdigit()));
            }
            
            #[test]
            fn prop_addressed_message_dest_roundtrip(
                source in 0u16..=0xFFF,
                dest in 0u16..=0xFFF,
                payload in prop::collection::vec(any::<u8>(), 0..=6)
            ) {
                // Addressed messages with dest in body should round-trip
                let frame = GridConnectFrame::from_addressed_mti(
                    MTI::VerifyNodeAddressed,
                    source,
                    dest,
                    payload.clone(),
                ).unwrap();
                
                let (decoded_dest, decoded_payload) = frame.get_dest_from_body().unwrap();
                prop_assert_eq!(decoded_dest, dest);
                prop_assert_eq!(decoded_payload, payload.as_slice());
            }
        }
    }
    
    // SNIP Response Frame Tests - Real data from LccPro
    
    #[test]
    fn test_snip_response_frame_parsing() {
        // Real SNIP response from LccPro logs
        // First frame: 19a08c41 1A AA 04 4F 70 65 6E 4D
        let frame_str = ":X19a08c41N1AAA044F70656E4D;";
        let frame = GridConnectFrame::parse(frame_str).unwrap();
        
        // Verify header
        assert_eq!(frame.header, 0x19a08c41);
        
        // Verify data bytes (8 bytes total)
        assert_eq!(frame.data.len(), 8);
        assert_eq!(frame.data[0], 0x1A); // Datagram frame indicator
        assert_eq!(frame.data[1], 0xAA); // Part of destination/control
        assert_eq!(frame.data[2], 0x04); // SNIP version
        
        // Verify beginning of SNIP payload ("OpenM" -> 4F 70 65 6E 4D)
        assert_eq!(&frame.data[3..8], &[0x4F, 0x70, 0x65, 0x6E, 0x4D]);
        
        // Verify MTI extraction
        let (mti, source) = frame.get_mti().unwrap();
        assert_eq!(mti, MTI::SNIPResponse);
        assert_eq!(source, 0xC41);
    }
    
    #[test]
    fn test_datagram_frame_type_indicators() {
        // Test that datagram first byte patterns can be identified
        
        // DatagramOnly/First typically starts with 0x20 (SNIP uses different encoding)
        // But SNIP specifically uses 0x1A for first frame in multi-frame datagram
        
        let test_cases = vec![
            (0x20, "Typical first/only datagram frame"),
            (0x1A, "SNIP first frame (seen in LccPro)"),
            (0x3A, "Typical middle frame"),
            (0x2A, "Typical final frame"),
        ];
        
        for (byte_value, description) in test_cases {
            // Just verify we can parse frames with these patterns
            let data = vec![byte_value, 0xAA, 0x04, 0x00];
            let frame = GridConnectFrame::new(0x19A08C41, data.clone()).unwrap();
            assert_eq!(frame.data[0], byte_value, "Failed for: {}", description);
        }
    }
    
    #[test]
    fn test_snip_gridconnect_frame_roundtrip() {
        // Ensure SNIP frames can round-trip through parse/encode
        let original = ":X19a08c41N1AAA044F70656E4D;";
        let frame = GridConnectFrame::parse(original).unwrap();
        let encoded = frame.to_string();
        
        // Case might differ (we normalize to uppercase)
        assert_eq!(encoded.to_uppercase(), original.to_uppercase());
    }
}
