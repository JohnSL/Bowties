//! Message Type Identifier (MTI) handling for LCC/OpenLCB protocol

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Message Type Identifier (MTI) enum
/// 
/// MTIs are encoded in the 29-bit CAN header and identify the message type.
/// Reference: OpenLCB Message Network Standard
/// 
/// Note: These are "on-wire" MTI values (bits 12-28 of the CAN header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MTI {
    /// Initialization Complete
    InitializationComplete,
    
    /// Verify Node ID Number - Global (0x19490)
    VerifyNodeGlobal,
    
    /// Verify Node ID Number - Addressed
    VerifyNodeAddressed,
    
    /// Verified Node ID Number (0x19170)
    VerifiedNode,
    
    /// Optional Interaction Rejected
    OptionalInteractionRejected,
    
    /// Terminate Due to Error
    TerminateDueToError,
    
    /// Protocol Support Inquiry
    ProtocolSupportInquiry,
    
    /// Protocol Support Reply
    ProtocolSupportReply,
    
    /// Identify Consumers
    IdentifyConsumers,
    
    /// Consumer Identified (Valid) (0x194C4)
    ConsumerIdentifiedValid,
    
    /// Consumer Identified (Invalid) (0x194C5)
    ConsumerIdentifiedInvalid,
    
    /// Consumer Identified (Unknown) (0x194C7)
    ConsumerIdentifiedUnknown,
    
    /// Identify Producers
    IdentifyProducers,
    
    /// Producer Identified (Valid) (0x19544)
    ProducerIdentifiedValid,
    
    /// Producer Identified (Invalid) (0x19545)
    ProducerIdentifiedInvalid,
    
    /// Producer Identified (Unknown) (0x19547)
    ProducerIdentifiedUnknown,
    
    /// Identify Events - Global (0x19970)
    IdentifyEventsGlobal,
    
    /// Identify Events - Addressed
    IdentifyEventsAddressed,
    
    /// Consumer Range Identified (0x194A4)
    ConsumerRangeIdentified,
    
    /// Producer Range Identified (0x19524)
    ProducerRangeIdentified,
    
    /// Check ID (CID) - Part of alias allocation (0x17020)
    CheckID,
    
    /// Reserve ID (RID) - Part of alias allocation (0x10700)
    ReserveID,

    /// Alias Map Definition (AMD) - Announce alias→NodeID mapping (0x10701)
    AliasMapDefinition,

    /// Alias Map Reset (AMR) (0x10703)
    AliasMapReset,
    
    /// Datagram (0x1C480) - Legacy/General
    Datagram,
    
    /// Datagram Only - Single frame datagram (0x1A000)
    DatagramOnly,
    
    /// Datagram First - First frame of multi-frame datagram (0x1B000)
    DatagramFirst,
    
    /// Datagram Middle - Middle frame of multi-frame datagram (0x1C000)
    DatagramMiddle,
    
    /// Datagram Final - Final frame of multi-frame datagram (0x1D000)
    DatagramFinal,
    
    /// Datagram Received OK
    DatagramReceivedOk,
    
    /// Datagram Rejected
    DatagramRejected,
    
    /// Simple Node Identification Protocol (SNIP) Request (0x19DE8)
    SNIPRequest,
    
    /// Simple Node Identification Protocol (SNIP) Response (0x19A08)
    SNIPResponse,
    
    /// Unknown/Raw MTI value
    Unknown(u32),
}

impl MTI {
    /// Get the raw MTI value (top 17 bits of the header, shifted right by 12)
    pub fn value(&self) -> u32 {
        match self {
            MTI::InitializationComplete => 0x19100,
            MTI::VerifyNodeGlobal => 0x19490,
            MTI::VerifyNodeAddressed => 0x19488,
            MTI::VerifiedNode => 0x19170,
            MTI::OptionalInteractionRejected => 0x19068,
            MTI::TerminateDueToError => 0x190A8,
            MTI::ProtocolSupportInquiry => 0x19828,
            MTI::ProtocolSupportReply => 0x19668,
            MTI::IdentifyConsumers => 0x198F4,
            MTI::ConsumerIdentifiedValid => 0x194C4,
            MTI::ConsumerIdentifiedInvalid => 0x194C5,
            MTI::ConsumerIdentifiedUnknown => 0x194C7,
            MTI::IdentifyProducers => 0x19914,
            MTI::ProducerIdentifiedValid => 0x19544,
            MTI::ProducerIdentifiedInvalid => 0x19545,
            MTI::ProducerIdentifiedUnknown => 0x19547,
            MTI::IdentifyEventsGlobal => 0x19970,
            MTI::IdentifyEventsAddressed => 0x19968,
            MTI::ConsumerRangeIdentified => 0x194A4,
            MTI::ProducerRangeIdentified => 0x19524,
            MTI::CheckID => 0x17020,
            MTI::ReserveID => 0x10700,
            MTI::AliasMapDefinition => 0x10701,
            MTI::AliasMapReset => 0x10703,
            MTI::Datagram => 0x1C480,
            MTI::DatagramOnly => 0x1A000,
            MTI::DatagramFirst => 0x1B000,
            MTI::DatagramMiddle => 0x1C000,
            MTI::DatagramFinal => 0x1D000,
            MTI::DatagramReceivedOk => 0x19A28,
            MTI::DatagramRejected => 0x19A48,
            MTI::SNIPRequest => 0x19DE8,
            MTI::SNIPResponse => 0x19A08,
            MTI::Unknown(v) => *v,
        }
    }
    
    /// Create MTI from raw value
    pub fn from_value(value: u32) -> Self {
        match value {
            0x19100 => MTI::InitializationComplete,
            0x19490 => MTI::VerifyNodeGlobal,
            0x19488 => MTI::VerifyNodeAddressed,
            0x19170 => MTI::VerifiedNode,
            0x19068 => MTI::OptionalInteractionRejected,
            0x190A8 => MTI::TerminateDueToError,
            0x19828 => MTI::ProtocolSupportInquiry,
            0x19668 => MTI::ProtocolSupportReply,
            0x198F4 => MTI::IdentifyConsumers,
            0x194C4 => MTI::ConsumerIdentifiedValid,
            0x194C5 => MTI::ConsumerIdentifiedInvalid,
            0x194C7 => MTI::ConsumerIdentifiedUnknown,
            0x19914 => MTI::IdentifyProducers,
            0x19544 => MTI::ProducerIdentifiedValid,
            0x19545 => MTI::ProducerIdentifiedInvalid,
            0x19547 => MTI::ProducerIdentifiedUnknown,
            0x19970 => MTI::IdentifyEventsGlobal,
            0x19968 => MTI::IdentifyEventsAddressed,
            0x194A4 => MTI::ConsumerRangeIdentified,
            0x19524 => MTI::ProducerRangeIdentified,
            0x17020 => MTI::CheckID,
            0x10700 => MTI::ReserveID,
            0x10701 => MTI::AliasMapDefinition,
            0x10703 => MTI::AliasMapReset,
            0x1C480 => MTI::Datagram,
            0x1A000 => MTI::DatagramOnly,
            0x1B000 => MTI::DatagramFirst,
            0x1C000 => MTI::DatagramMiddle,
            0x1D000 => MTI::DatagramFinal,
            0x19A28 => MTI::DatagramReceivedOk,
            0x19A48 => MTI::DatagramRejected,
            0x19DE8 => MTI::SNIPRequest,
            0x19A08 => MTI::SNIPResponse,
            _ => MTI::Unknown(value),
        }
    }
    
    /// Extract MTI and source alias from a 29-bit CAN header
    /// 
    /// The header format:
    /// - Bits 28-12: MTI (17 bits)
    /// - Bits 11-0: Source alias (12 bits)
    pub fn from_header(header: u32) -> Result<(MTI, u16)> {
        // Extract the MTI from bits 12-28 (17 bits)
        let mti_value = (header >> 12) & 0x1FFFF;
        
        // Extract source alias (12 bits)
        let source_alias = (header & 0xFFF) as u16;
        
        Ok((MTI::from_value(mti_value), source_alias))
    }
    
    /// Encode MTI and source alias into a 29-bit CAN header
    pub fn to_header(&self, source_alias: u16) -> Result<u32> {
        if source_alias > 0xFFF {
            return Err(Error::InvalidMTI(format!(
                "Source alias must be 12-bit (<=0xFFF), got 0x{:X}",
                source_alias
            )));
        }
        
        let mti_value = self.value();
        // Shift MTI to bits 12-28, combine with source alias in bits 0-11
        let header = (mti_value << 12) | (source_alias as u32);
        
        Ok(header)
    }
    
    /// Encode MTI with source and destination aliases for addressed datagrams
    /// 
    /// For datagram MTIs (DatagramOnly/First/Middle/Final), the destination alias
    /// is encoded in bits 12-23 of the header, following the Python pattern:
    /// header = MTI_base + source_alias + (dest_alias << 12)
    /// 
    /// The header format for datagrams:
    /// - Bits 28-24: MTI upper bits
    /// - Bits 23-12: Destination alias (12 bits)
    /// - Bits 11-0: Source alias (12 bits)
    pub fn to_header_with_dest(&self, source_alias: u16, dest_alias: u16) -> Result<u32> {
        if source_alias > 0xFFF {
            return Err(Error::InvalidMTI(format!(
                "Source alias must be 12-bit (<=0xFFF), got 0x{:X}",
                source_alias
            )));
        }
        if dest_alias > 0xFFF {
            return Err(Error::InvalidMTI(format!(
                "Destination alias must be 12-bit (<=0xFFF), got 0x{:X}",
                dest_alias
            )));
        }
        
        let mti_base = self.value() << 12;
        // Combine: MTI base + (dest << 12) + source
        let header = mti_base | ((dest_alias as u32) << 12) | (source_alias as u32);
        
        Ok(header)
    }
    
    /// Extract MTI, source alias, and destination alias from a datagram header
    /// 
    /// Returns (MTI, source_alias, dest_alias)
    pub fn from_datagram_header(header: u32) -> Result<(MTI, u16, u16)> {
        // Extract destination alias from bits 12-23
        let dest_alias = ((header >> 12) & 0xFFF) as u16;
        
        // Extract source alias from bits 0-11
        let source_alias = (header & 0xFFF) as u16;
        
        // Extract MTI from bits 24-28 (upper 5 bits)
        let mti_upper = (header >> 24) & 0x1F;
        let mti_value = mti_upper << 12;
        
        Ok((MTI::from_value(mti_value), source_alias, dest_alias))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mti_values() {
        assert_eq!(MTI::VerifyNodeGlobal.value(), 0x19490);
        assert_eq!(MTI::VerifiedNode.value(), 0x19170);
        assert_eq!(MTI::IdentifyEventsGlobal.value(), 0x19970);
        assert_eq!(MTI::ConsumerIdentifiedValid.value(), 0x194C4);
        assert_eq!(MTI::ProducerIdentifiedValid.value(), 0x19544);
    }

    #[test]
    fn test_mti_from_value() {
       assert_eq!(MTI::from_value(0x19490), MTI::VerifyNodeGlobal);
        assert_eq!(MTI::from_value(0x19170), MTI::VerifiedNode);
        assert_eq!(MTI::from_value(0xFFFF), MTI::Unknown(0xFFFF));
    }

    #[test]
    fn test_mti_to_header() {
        // Test VerifyNodeGlobal (0x19490) with alias 0xAAA
        let header = MTI::VerifyNodeGlobal.to_header(0xAAA).unwrap();
        // Expected: 0x19490 << 12 | 0xAAA = 0x19490000 | 0x0AAA = 0x19490AAA
        assert_eq!(header, 0x19490AAA);
        
        // Test VerifiedNode (0x19170) with alias 0x123
        let header = MTI::VerifiedNode.to_header(0x123).unwrap();
        assert_eq!(header, 0x19170123);
    }

    #[test]
    fn test_mti_from_header() {
        // Test VerifyNodeGlobal with alias 0xAAA
        let (mti, alias) = MTI::from_header(0x19490AAA).unwrap();
        assert_eq!(mti, MTI::VerifyNodeGlobal);
        assert_eq!(alias, 0xAAA);
        
        // Test VerifiedNode with alias 0x123
        let (mti, alias) = MTI::from_header(0x19170123).unwrap();
        assert_eq!(mti, MTI::VerifiedNode);
        assert_eq!(alias, 0x123);
    }

    #[test]
    fn test_mti_round_trip() {
        let original_mti = MTI::IdentifyEventsGlobal;
        let alias = 0xABC;
        
        let header = original_mti.to_header(alias).unwrap();
        let (decoded_mti, decoded_alias) = MTI::from_header(header).unwrap();
        
        assert_eq!(decoded_mti, original_mti);
        assert_eq!(decoded_alias, alias);
    }

    #[test]
    fn test_invalid_alias() {
        // Alias too large (>12 bits)
        let result = MTI::VerifyNodeGlobal.to_header(0x1000);
        assert!(result.is_err());
    }
    
    // NEW COMPREHENSIVE TESTS
    
    #[test]
    fn test_all_mti_values_match_python() {
        // Test all MTI values against Python reference implementation
        assert_eq!(MTI::InitializationComplete.value(), 0x19100);
        assert_eq!(MTI::VerifyNodeGlobal.value(), 0x19490);
        assert_eq!(MTI::VerifyNodeAddressed.value(), 0x19488);
        assert_eq!(MTI::VerifiedNode.value(), 0x19170);
        assert_eq!(MTI::IdentifyConsumers.value(), 0x198F4);
        assert_eq!(MTI::IdentifyProducers.value(), 0x19914);
        assert_eq!(MTI::ConsumerIdentifiedValid.value(), 0x194C4);
        assert_eq!(MTI::ConsumerIdentifiedInvalid.value(), 0x194C5);
        assert_eq!(MTI::ConsumerIdentifiedUnknown.value(), 0x194C7);
        assert_eq!(MTI::ProducerIdentifiedValid.value(), 0x19544);
        assert_eq!(MTI::ProducerIdentifiedInvalid.value(), 0x19545);
        assert_eq!(MTI::ProducerIdentifiedUnknown.value(), 0x19547);
        assert_eq!(MTI::IdentifyEventsGlobal.value(), 0x19970);
        assert_eq!(MTI::IdentifyEventsAddressed.value(), 0x19968);
        assert_eq!(MTI::ConsumerRangeIdentified.value(), 0x194A4);
        assert_eq!(MTI::ProducerRangeIdentified.value(), 0x19524);
        assert_eq!(MTI::CheckID.value(), 0x17020);
        assert_eq!(MTI::ReserveID.value(), 0x10700);
        assert_eq!(MTI::AliasMapReset.value(), 0x10703);
        assert_eq!(MTI::DatagramOnly.value(), 0x1A000);
        assert_eq!(MTI::DatagramFirst.value(), 0x1B000);
        assert_eq!(MTI::DatagramMiddle.value(), 0x1C000);
        assert_eq!(MTI::DatagramFinal.value(), 0x1D000);
        assert_eq!(MTI::DatagramReceivedOk.value(), 0x19A28);
        assert_eq!(MTI::DatagramRejected.value(), 0x19A48);
    }
    
    #[test]
    fn test_all_mti_from_value_roundtrip() {
        // Test that all MTI types can round-trip through value
        let mtis = vec![
            MTI::InitializationComplete,
            MTI::VerifyNodeGlobal,
            MTI::VerifyNodeAddressed,
            MTI::VerifiedNode,
            MTI::IdentifyConsumers,
            MTI::IdentifyProducers,
            MTI::ConsumerIdentifiedValid,
            MTI::ConsumerIdentifiedInvalid,
            MTI::ConsumerIdentifiedUnknown,
            MTI::ProducerIdentifiedValid,
            MTI::ProducerIdentifiedInvalid,
            MTI::ProducerIdentifiedUnknown,
            MTI::IdentifyEventsGlobal,
            MTI::IdentifyEventsAddressed,
            MTI::ConsumerRangeIdentified,
            MTI::ProducerRangeIdentified,
            MTI::CheckID,
            MTI::ReserveID,
            MTI::AliasMapDefinition,
            MTI::AliasMapReset,
            MTI::DatagramOnly,
            MTI::DatagramFirst,
            MTI::DatagramMiddle,
            MTI::DatagramFinal,
            MTI::DatagramReceivedOk,
            MTI::DatagramRejected,
        ];
        
        for mti in mtis {
            let value = mti.value();
            let decoded = MTI::from_value(value);
            assert_eq!(decoded, mti, "Failed for {:?}", mti);
        }
    }
    
    #[test]
    fn test_alias_allocation_mtis() {
        // Test alias allocation sequence MTIs
        assert_eq!(MTI::CheckID.value(), 0x17020);
        assert_eq!(MTI::ReserveID.value(), 0x10700);
        assert_eq!(MTI::AliasMapDefinition.value(), 0x10701);
        assert_eq!(MTI::AliasMapReset.value(), 0x10703);
        assert_eq!(MTI::InitializationComplete.value(), 0x19100);

        // Test round-trip
        assert_eq!(MTI::from_value(0x17020), MTI::CheckID);
        assert_eq!(MTI::from_value(0x10700), MTI::ReserveID);
        assert_eq!(MTI::from_value(0x10701), MTI::AliasMapDefinition);
        assert_eq!(MTI::from_value(0x10703), MTI::AliasMapReset);
        assert_eq!(MTI::from_value(0x19100), MTI::InitializationComplete);
    }
    
    #[test]
    fn test_range_identified_mtis() {
        // Test range message MTIs
        assert_eq!(MTI::ConsumerRangeIdentified.value(), 0x194A4);
        assert_eq!(MTI::ProducerRangeIdentified.value(), 0x19524);
        
        // Test encoding with alias
        let header = MTI::ConsumerRangeIdentified.to_header(0xAAA).unwrap();
        assert_eq!(header, 0x194A4AAA);
        
        let header = MTI::ProducerRangeIdentified.to_header(0xDDD).unwrap();
        assert_eq!(header, 0x19524DDD);
    }
    
    #[test]
    fn test_datagram_mti_with_dest() {
        // Test DatagramOnly: source=0xAAA, dest=0x000
        let header = MTI::DatagramOnly.to_header_with_dest(0xAAA, 0x000).unwrap();
        assert_eq!(header, 0x1A000AAA);
        
        // Test DatagramOnly: source=0xAAA, dest=0xDDD
        let header = MTI::DatagramOnly.to_header_with_dest(0xAAA, 0xDDD).unwrap();
        // Expected: 0x1A000 << 12 | 0xDDD << 12 | 0xAAA
        // 0x1A000000 | 0x00DDD000 | 0x00000AAA = 0x1ADDDAAA
        assert_eq!(header, 0x1ADDDAAA);
    }
    
    #[test]
    fn test_all_datagram_types_with_dest() {
        let source = 0xAAA;
        let dest = 0xDDD;
        
        // DatagramOnly
        let header = MTI::DatagramOnly.to_header_with_dest(source, dest).unwrap();
        assert_eq!(header, 0x1ADDDAAA);
        
        // DatagramFirst
        let header = MTI::DatagramFirst.to_header_with_dest(source, dest).unwrap();
        assert_eq!(header, 0x1BDDDAAA);
        
        // DatagramMiddle
        let header = MTI::DatagramMiddle.to_header_with_dest(source, dest).unwrap();
        assert_eq!(header, 0x1CDDDAAA);
        
        // DatagramFinal
        let header = MTI::DatagramFinal.to_header_with_dest(source, dest).unwrap();
        assert_eq!(header, 0x1DDDDAAA);
    }
    
    #[test]
    fn test_from_datagram_header() {
        // Test extracting datagram information from header
        let (mti, source, dest) = MTI::from_datagram_header(0x1ADDDAAA).unwrap();
        assert_eq!(mti, MTI::DatagramOnly);
        assert_eq!(source, 0xAAA);
        assert_eq!(dest, 0xDDD);
        
        // Test DatagramFirst
        let (mti, source, dest) = MTI::from_datagram_header(0x1BDDDAAA).unwrap();
        assert_eq!(mti, MTI::DatagramFirst);
        assert_eq!(source, 0xAAA);
        assert_eq!(dest, 0xDDD);
    }
    
    #[test]
    fn test_datagram_dest_validation() {
        // Test that destination alias must be 12-bit
        let result = MTI::DatagramOnly.to_header_with_dest(0xAAA, 0x1000);
        assert!(result.is_err());
        
        // Test that source alias must be 12-bit
        let result = MTI::DatagramOnly.to_header_with_dest(0x1000, 0xDDD);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_mti_header_bit_boundaries() {
        // Test that MTI encoding preserves all bits correctly
        // Test with maximum valid alias (0xFFF)
        let header = MTI::VerifyNodeGlobal.to_header(0xFFF).unwrap();
        let (mti, alias) = MTI::from_header(header).unwrap();
        assert_eq!(mti, MTI::VerifyNodeGlobal);
        assert_eq!(alias, 0xFFF);
        
        // Test with minimum alias (0x000)
        let header = MTI::VerifyNodeGlobal.to_header(0x000).unwrap();
        let (mti, alias) = MTI::from_header(header).unwrap();
        assert_eq!(mti, MTI::VerifyNodeGlobal);
        assert_eq!(alias, 0x000);
    }
    
    #[test]
    fn test_python_reference_headers() {
        // Test exact header values from Python implementation
        // VerifyNodeGlobal from 0xAAA: :X19490AAAN;
        assert_eq!(MTI::VerifyNodeGlobal.to_header(0xAAA).unwrap(), 0x19490AAA);
        
        // VerifiedNode from 0xDDD: :X19170DDDN;
        assert_eq!(MTI::VerifiedNode.to_header(0xDDD).unwrap(), 0x19170DDD);
        
        // IdentifyConsumers from 0xAAA: :X198F4AAAN;
        assert_eq!(MTI::IdentifyConsumers.to_header(0xAAA).unwrap(), 0x198F4AAA);
        
        // CheckID from 0xAAA: :X17020AAAN;
        assert_eq!(MTI::CheckID.to_header(0xAAA).unwrap(), 0x17020AAA);
        
        // ReserveID from 0xAAA: :X10700AAAN;
        assert_eq!(MTI::ReserveID.to_header(0xAAA).unwrap(), 0x10700AAA);
        
        // AliasMapReset from 0xAAA: :X10703AAAN;
        assert_eq!(MTI::AliasMapReset.to_header(0xAAA).unwrap(), 0x10703AAA);
    }
    
    #[test]
    fn test_snip_response_header_parsing() {
        // Real SNIP response from LccPro logs
        // Frame: :X19a08c41N1AAA044F70656E4D;
        // Header: 19a08c41 - MTI 0x19A08 (SNIPResponse)
        // Note: SNIPResponse is a standard addressed MTI, NOT a datagram MTI
        // It carries datagram payload but uses normal MTI encoding
        let header: u32 = 0x19a08c41;
        
        // Test standard MTI parsing
        let (mti, source) = MTI::from_header(header).unwrap();
        assert_eq!(mti, MTI::SNIPResponse);
        assert_eq!(source, 0xC41);
        
        // Verify MTI value matches spec
        assert_eq!(MTI::SNIPResponse.value(), 0x19A08);
    }
    
    #[test]
    fn test_snip_request_header() {
        // SNIP Request MTI value from spec
        assert_eq!(MTI::SNIPRequest.value(), 0x19DE8);
        
        // Create a SNIP request header using standard addressed MTI encoding
        // Note: SNIP uses addressed messages, not datagram-specific MTI encoding
        let header = MTI::SNIPRequest.to_header(0xAAA).unwrap();
        
        // Parse it back
        let (mti, source) = MTI::from_header(header).unwrap();
        assert_eq!(mti, MTI::SNIPRequest);
        assert_eq!(source, 0xAAA);
    }
    
    #[test]
    fn test_snip_vs_datagram_mti_encoding() {
        // This test clarifies the difference between SNIP MTIs and Datagram MTIs
        
        // SNIP MTIs are standard addressed MTIs (17-bit MTI in bits 12-28)
        let snip_response = MTI::SNIPResponse.to_header(0xC41).unwrap();
        assert_eq!(snip_response, 0x19A08C41);
        
        let snip_request = MTI::SNIPRequest.to_header(0xAAA).unwrap();
        assert_eq!(snip_request, 0x19DE8AAA);
        
        // Datagram MTIs use special encoding (5-bit MTI in bits 24-28, dest in bits 12-23)
        let datagram_only = MTI::DatagramOnly.to_header_with_dest(0xAAA, 0xC41).unwrap();
        assert_eq!(datagram_only, 0x1AC41AAA);
        
        let datagram_first = MTI::DatagramFirst.to_header_with_dest(0xAAA, 0xC41).unwrap();
        assert_eq!(datagram_first, 0x1BC41AAA);
    }
    
    #[test]
    fn test_datagram_ack_headers() {
        // Test Datagram Received OK
        // Note: Like SNIP MTIs, these are standard addressed MTIs, not datagram MTIs
        assert_eq!(MTI::DatagramReceivedOk.value(), 0x19A28);
        let ack_header = MTI::DatagramReceivedOk.to_header(0xAAA).unwrap();
        let (mti, source) = MTI::from_header(ack_header).unwrap();
        assert_eq!(mti, MTI::DatagramReceivedOk);
        assert_eq!(source, 0xAAA);
        
        // Test Datagram Rejected
        assert_eq!(MTI::DatagramRejected.value(), 0x19A48);
        let nak_header = MTI::DatagramRejected.to_header(0xAAA).unwrap();
        let (mti, source) = MTI::from_header(nak_header).unwrap();
        assert_eq!(mti, MTI::DatagramRejected);
        assert_eq!(source, 0xAAA);
    }
}
