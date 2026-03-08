//! Integration tests for LCC protocol sequences
//! 
//! These tests validate complete protocol flows using real frame sequences
//! from the Python reference implementation.

use lcc_rs::protocol::{GridConnectFrame, MTI};
use lcc_rs::types::NodeID;

/// Test a complete node discovery sequence
/// Python reference: node_discovery.py discoverAllNodes()
#[test]
fn test_discovery_sequence() {
    // Step 1: Send global VerifyNodeID
    let verify_request = GridConnectFrame::from_mti(
        MTI::VerifyNodeGlobal,
        0xAAA,
        vec![],
    )
    .unwrap();
    
    assert_eq!(verify_request.to_string(), ":X19490AAAN;");
    
    // Step 2: Simulate receiving VerifiedNode responses from multiple nodes
    let responses = vec![
        ":X19170DDDN010203040506;", // Node 1
        ":X19170BBBNAABBCCDDEEFF;", // Node 2
    ];
    
    let mut discovered_nodes = Vec::new();
    
    for response in responses {
        let frame = GridConnectFrame::parse(response).unwrap();
        let (mti, alias) = frame.get_mti().unwrap();
        
        assert_eq!(mti, MTI::VerifiedNode);
        
        // Extract NodeID from frame data
        let node_id = NodeID::from_slice(&frame.data).unwrap();
        discovered_nodes.push((node_id, alias));
    }
    
    // Verify we found 2 nodes
    assert_eq!(discovered_nodes.len(), 2);
    
    // Verify first node
    assert_eq!(discovered_nodes[0].0, NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]));
    assert_eq!(discovered_nodes[0].1, 0xDDD);
    
    // Verify second node
    assert_eq!(discovered_nodes[1].0, NodeID::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]));
    assert_eq!(discovered_nodes[1].1, 0xBBB);
}

/// Test event query flow
/// Python reference: identifyProducers.py
#[test]
fn test_identify_producers_flow() {
    // Step 1: Send IdentifyProducers with EventID
    let event_id = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let query = GridConnectFrame::from_mti(
        MTI::IdentifyProducers,
        0xAAA,
        event_id.clone(),
    )
    .unwrap();
    
    assert_eq!(query.to_string(), ":X19914AAAN0102030405060708;");
    
    // Step 2: Receive ProducerIdentifiedValid response
    let response = ":X19544DDDN0102030405060708;";
    let frame = GridConnectFrame::parse(response).unwrap();
    let (mti, alias) = frame.get_mti().unwrap();
    
    assert_eq!(mti, MTI::ProducerIdentifiedValid);
    assert_eq!(alias, 0xDDD);
    assert_eq!(frame.data, event_id);
}

/// Test identify consumers flow
#[test]
fn test_identify_consumers_flow() {
    let event_id = vec![0x05, 0x02, 0x01, 0x02, 0x02, 0x00, 0x00, 0x00];
    
    // Send IdentifyConsumers
    let query = GridConnectFrame::from_mti(
        MTI::IdentifyConsumers,
        0xAAA,
        event_id.clone(),
    )
    .unwrap();
    
    assert_eq!(query.to_string(), ":X198F4AAAN0502010202000000;");
    
    // Receive multiple responses
    let responses = vec![
        (":X194C4DDDN0502010202000000;", MTI::ConsumerIdentifiedValid),
        (":X194C5BBBN0502010202000000;", MTI::ConsumerIdentifiedInvalid),
        (":X194C7CCCN0502010202000000;", MTI::ConsumerIdentifiedUnknown),
    ];
    
    for (response_str, expected_mti) in responses {
        let frame = GridConnectFrame::parse(response_str).unwrap();
        let (mti, _) = frame.get_mti().unwrap();
        assert_eq!(mti, expected_mti);
        assert_eq!(frame.data, event_id);
    }
}

/// Test addressed message flow
/// Python reference: verifyNodeAddressed.py
#[test]
fn test_verify_node_addressed_flow() {
    let node_id = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
    
    // Send VerifyNodeAddressed to specific node
    let query = GridConnectFrame::from_addressed_mti(
        MTI::VerifyNodeAddressed,
        0xAAA,  // source
        0xDDD,  // destination
        node_id.clone(),
    )
    .unwrap();
    
    // Expected format: dest alias in first 2 bytes, then NodeID
    assert_eq!(query.to_string(), ":X19488AAAN0DDD010203040506;");
    
    // Response: VerifiedNode (not addressed)
    let response = ":X19170DDDN010203040506;";
    let frame = GridConnectFrame::parse(response).unwrap();
    let (mti, alias) = frame.get_mti().unwrap();
    
    assert_eq!(mti, MTI::VerifiedNode);
    assert_eq!(alias, 0xDDD);
    assert_eq!(NodeID::from_slice(&frame.data).unwrap(), NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]));
}

/// Test complete alias allocation sequence
/// Python reference: testStartup.py (expected frames as documented there)
#[test]
fn test_alias_allocation_sequence() {
    let alias = 0xAAA;
    let node_id = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

    // Step 1: Send 4 CID frames (CID7–CID4) each encoding a NodeID segment.
    // Segments of NodeID 01.02.03.04.05.06 (48-bit = 0x010203040506):
    //   bits 47:36 = 0x010  → CID7 header: (0x17 << 24) | (0x010 << 12) | 0xAAA = 0x17010AAA
    //   bits 35:24 = 0x203  → CID6 header: 0x16203AAA
    //   bits 23:12 = 0x040  → CID5 header: 0x15040AAA
    //   bits 11:0  = 0x506  → CID4 header: 0x14506AAA
    let node_val: u64 = 0x010203040506;
    let segs: [u32; 4] = [
        ((node_val >> 36) & 0xFFF) as u32,
        ((node_val >> 24) & 0xFFF) as u32,
        ((node_val >> 12) & 0xFFF) as u32,
        (node_val & 0xFFF) as u32,
    ];
    for (cid_type, &seg) in [0x17u32, 0x16, 0x15, 0x14].iter().zip(segs.iter()) {
        let header = (cid_type << 24) | (seg << 12) | (alias as u32);
        let frame = GridConnectFrame::new(header, vec![]).unwrap();
        // Frames should be valid 29-bit CAN frames
        assert!(frame.header <= 0x1FFFFFFF);
    }

    // Step 2: Send ReserveID (RID) if no conflict
    let rid = GridConnectFrame::from_mti(MTI::ReserveID, alias, vec![]).unwrap();
    assert_eq!(rid.to_string(), ":X10700AAAN;");

    // Step 3: Send InitializationComplete with NodeID  (MTI 0x19100 per S-9.7.2.1)
    let init = GridConnectFrame::from_mti(
        MTI::InitializationComplete,
        alias,
        node_id.clone(),
    )
    .unwrap();
    assert_eq!(init.to_string(), ":X19100AAAN010203040506;");

    // Step 4: Respond to VerifyNodeGlobal
    let verify_global = GridConnectFrame::parse(":X19490BBBN;").unwrap();
    let (mti, _) = verify_global.get_mti().unwrap();
    assert_eq!(mti, MTI::VerifyNodeGlobal);

    // Send VerifiedNode response
    let verified = GridConnectFrame::from_mti(
        MTI::VerifiedNode,
        alias,
        node_id,
    )
    .unwrap();
    assert_eq!(verified.to_string(), ":X19170AAAN010203040506;");
}

/// Test alias conflict detection and recovery
/// Python reference: testAliasConflict.py
#[test]
fn test_alias_conflict_recovery() {
    let conflicted_alias = 0xAAA;
    let new_alias = 0xBBB;
    
    // Scenario: Receive a frame with our own alias (conflict!)
    let conflicting_frame = ":X19170AAANFFEEDDCCBBAA;";
    let frame = GridConnectFrame::parse(conflicting_frame).unwrap();
    assert_eq!(frame.source_alias(), conflicted_alias);
    
    // Step 1: Send Alias Map Reset (AMR)
    let amr = GridConnectFrame::from_mti(MTI::AliasMapReset, conflicted_alias, vec![]).unwrap();
    assert_eq!(amr.to_string(), ":X10703AAAN;");
    
    // Step 2: Re-acquire with new alias
    let cid = GridConnectFrame::from_mti(MTI::CheckID, new_alias, vec![]).unwrap();
    assert_eq!(cid.to_string(), ":X17020BBBN;");
    
    let rid = GridConnectFrame::from_mti(MTI::ReserveID, new_alias, vec![]).unwrap();
    assert_eq!(rid.to_string(), ":X10700BBBN;");
    
    let init = GridConnectFrame::from_mti(
        MTI::InitializationComplete,
        new_alias,
        vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
    )
    .unwrap();
    assert_eq!(init.to_string(), ":X19100BBBN010203040506;");
}

/// Test datagram exchange (single frame)
/// Python reference: testDatagram.py
#[test]
fn test_datagram_only_exchange() {
    let source = 0xAAA;
    let dest = 0xDDD;
    let data = vec![0x01, 0x02, 0x03, 0x04];
    
    // Send DatagramOnly (≤8 bytes, single frame)
    let header = MTI::DatagramOnly.to_header_with_dest(source, dest).unwrap();
    let datagram = GridConnectFrame::new(header, data.clone()).unwrap();
    
    assert_eq!(datagram.to_string(), ":X1ADDDAAAN01020304;");
    
    // Verify destination extraction
    let (mti, src, dst) = MTI::from_datagram_header(header).unwrap();
    assert_eq!(mti, MTI::DatagramOnly);
    assert_eq!(src, source);
    assert_eq!(dst, dest);
    
    // Response: DatagramReceivedOK
    let _ack = GridConnectFrame::from_mti(MTI::DatagramReceivedOk, dest, vec![]).unwrap();
    // Note: Response would be addressed, but we're simplifying for this test
}

/// Test multi-frame datagram sequence
#[test]
fn test_multi_frame_datagram() {
    let source = 0xAAA;
    let dest = 0xDDD;
    
    // First frame (8 bytes)
    let header = MTI::DatagramFirst.to_header_with_dest(source, dest).unwrap();
    let first = GridConnectFrame::new(header, vec![0xFF; 8]).unwrap();
    assert_eq!(first.to_string(), ":X1BDDDAAANFFFFFFFFFFFFFFFF;");
    
    // Middle frame (8 bytes)
    let header = MTI::DatagramMiddle.to_header_with_dest(source, dest).unwrap();
    let middle = GridConnectFrame::new(header, vec![0xAA; 8]).unwrap();
    assert_eq!(middle.to_string(), ":X1CDDDAAANAAAAAAAAAAAAAAAA;");
    
    // Final frame (partial)
    let header = MTI::DatagramFinal.to_header_with_dest(source, dest).unwrap();
    let final_frame = GridConnectFrame::new(header, vec![0x01, 0x02]).unwrap();
    assert_eq!(final_frame.to_string(), ":X1DDDDAAAN0102;");
    
    // Total data: 8 + 8 + 2 = 18 bytes
}

/// Test identify events (global query)
#[test]
fn test_identify_events_global() {
    // Send global IdentifyEvents query
    let query = GridConnectFrame::from_mti(
        MTI::IdentifyEventsGlobal,
        0xAAA,
        vec![],
    )
    .unwrap();
    
    assert_eq!(query.to_string(), ":X19970AAAN;");
    
    // Nodes respond with all their produced/consumed events
    // Each response is a ProducerIdentified or ConsumerIdentified with EventID
}

/// Test range identified messages
#[test]
fn test_range_identified() {
    let event_id = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    
    // ProducerRangeIdentified
    let producer_range = GridConnectFrame::from_mti(
        MTI::ProducerRangeIdentified,
        0xAAA,
        event_id.clone(),
    )
    .unwrap();
    assert_eq!(producer_range.to_string(), ":X19524AAAN0102030405060708;");
    
    // ConsumerRangeIdentified
    let consumer_range = GridConnectFrame::from_mti(
        MTI::ConsumerRangeIdentified,
        0xBBB,
        event_id,
    )
    .unwrap();
    assert_eq!(consumer_range.to_string(), ":X194A4BBBN0102030405060708;");
}

/// Test parsing real frame sequences from Python logs
#[test]
fn test_python_log_sequences() {
    // Real frame sequences from Python test execution
    let frames = vec![
        ":X19490AAAN;",                          // VerifyNodeGlobal
        ":X19170DDDN010203040506;",              // VerifiedNode response
        ":X198F4AAAN0102030405060708;",          // IdentifyConsumers
        ":X194C4DDDN0102030405060708;",          // ConsumerIdentifiedValid
        ":X19914AAAN0102030405060708;",          // IdentifyProducers
        ":X19544DDDN0102030405060708;",          // ProducerIdentifiedValid
        ":X17020AAAN;",                          // CheckID
        ":X10700AAAN;",                          // ReserveID
        ":X19100AAAN010203040506;",              // InitializationComplete (MTI 0x19100 per S-9.7.2.1)
        ":X19488AAAN0DDD010203040506;",          // VerifyNodeAddressed
    ];
    
    // Verify all frames parse correctly
    for frame_str in frames {
        let frame = GridConnectFrame::parse(frame_str).unwrap();
        
        // Verify round-trip
        assert_eq!(frame.to_string(), frame_str);
        
        // Verify MTI extraction works
        let (mti, _) = frame.get_mti().unwrap();
        
        // Verify it's a known MTI (not Unknown)
        match mti {
            MTI::Unknown(_) => panic!("Unexpected unknown MTI for frame: {}", frame_str),
            _ => {} // Expected
        }
    }
}
