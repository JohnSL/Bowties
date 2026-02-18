//! Node discovery functionality for LCC/OpenLCB networks
//!
//! This module implements the node discovery protocol, which sends a global
//! Verify Node ID message and collects responses.

use crate::{
    Result,
    types::{NodeID, NodeAlias, DiscoveredNode},
    protocol::{GridConnectFrame, MTI},
    transport::{LccTransport, TcpTransport},
};
use std::collections::HashMap;
use tokio::time::{sleep, Duration, Instant};

/// High-level LCC connection for performing network operations
pub struct LccConnection {
    transport: Box<dyn LccTransport>,
    our_alias: NodeAlias,
}

impl LccConnection {
    /// Connect to an LCC network via TCP
    /// 
    /// # Arguments
    /// * `host` - Hostname or IP address
    /// * `port` - Port number (typically 12021)
    /// 
    /// # Example
    /// ```no_run
    /// use lcc_rs::LccConnection;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut connection = LccConnection::connect("localhost", 12021).await?;
    ///     let nodes = connection.discover_nodes(250).await?;
    ///     println!("Found {} nodes", nodes.len());
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let transport = TcpTransport::connect(host, port).await?;
        
        // Use a fixed alias for our node (could be random in the future)
        let our_alias = NodeAlias::new(0xAAA).unwrap();
        
        Ok(Self {
            transport: Box::new(transport),
            our_alias,
        })
    }
    
    /// Create an LCC connection with a custom transport (for testing)
    pub fn with_transport(transport: Box<dyn LccTransport>, our_alias: NodeAlias) -> Self {
        Self {
            transport,
            our_alias,
        }
    }
    
    /// Discover all nodes on the network
    /// 
    /// Sends a global Verify Node ID message and collects Verified Node responses.
    /// 
    /// # Arguments
    /// * `timeout_ms` - Maximum time to wait for responses (recommended: 250ms)
    /// 
    /// # Returns
    /// A vector of discovered nodes with their Node IDs and aliases
    /// 
    /// # Implementation Notes
    /// - Sends Verify Node ID Global (MTI 0x0490)
    /// - Collects Verified Node (MTI 0x0170) responses
    /// - Uses silence detection: stops when no frames arrive for 25ms
    /// - Maximum timeout prevents hanging if network is busy
    pub async fn discover_nodes(&mut self, timeout_ms: u64) -> Result<Vec<DiscoveredNode>> {
        // Send global Verify Node ID message
        let verify_frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            self.our_alias.value(),
            vec![],
        )?;
        
        self.transport.send(&verify_frame).await?;
        
        // Collect responses
        let mut nodes = HashMap::new();
        let start_time = Instant::now();
        let max_duration = Duration::from_millis(timeout_ms);
        let silence_threshold = Duration::from_millis(25); // 25ms silence = done
        let mut last_receive_time = Instant::now();
        
        loop {
            // Check if we've exceeded max timeout
            if start_time.elapsed() >= max_duration {
                break;
            }
            
            // Check if we've had 25ms of silence
            if last_receive_time.elapsed() >= silence_threshold {
                break;
            }
            
            // Try to receive a frame with a short timeout
            let remaining_time = max_duration.saturating_sub(start_time.elapsed());
            let poll_timeout = std::cmp::min(remaining_time, Duration::from_millis(10));
            
            match self.transport.receive(poll_timeout.as_millis() as u64).await? {
                Some(frame) => {
                    last_receive_time = Instant::now();
                    
                    // Check if this is a Verified Node response
                    if let Ok((mti, alias)) = frame.get_mti() {
                        if mti == MTI::VerifiedNode && frame.data.len() == 6 {
                            // Extract Node ID from data
                            let node_id = NodeID::from_slice(&frame.data)?;
                            let node_alias = NodeAlias::new(alias)?;
                            
                            nodes.insert(
                                node_id,
                                DiscoveredNode {
                                    node_id,
                                    alias: node_alias,
                                    snip_data: None,
                                    snip_status: crate::types::SNIPStatus::Unknown,
                                    connection_status: crate::types::ConnectionStatus::Connected,
                                    last_verified: None,
                                    last_seen: chrono::Utc::now(),
                                    cdi: None,
                                },
                            );
                        }
                    }
                }
                None => {
                    // No frame received in this poll period
                    // Small sleep to avoid busy-waiting
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }
        
        Ok(nodes.into_values().collect())
    }
    
    /// Query SNIP data for a specific node
    /// 
    /// # Arguments
    /// * `dest_alias` - Target node's alias
    /// * `semaphore` - Optional semaphore for concurrency limiting
    /// 
    /// # Returns
    /// * `Ok((SNIPData, SNIPStatus))` - Retrieved SNIP data and status
    pub async fn query_snip(
        &mut self,
        dest_alias: u16,
        semaphore: Option<std::sync::Arc<tokio::sync::Semaphore>>,
    ) -> Result<(Option<crate::types::SNIPData>, crate::types::SNIPStatus)> {
        let sem = semaphore.unwrap_or_else(|| std::sync::Arc::new(tokio::sync::Semaphore::new(5)));
        crate::snip::query_snip(
            self.transport.as_mut(),
            self.our_alias.value(),
            dest_alias,
            sem,
        ).await
    }
    
    /// Verify a specific node's presence on the network
    /// 
    /// Sends an addressed Verify Node ID message to a specific node and waits for its response.
    /// 
    /// # Arguments
    /// * `dest_alias` - Target node's alias
    /// * `timeout_ms` - Maximum time to wait for response (recommended: 500ms)
    /// 
    /// # Returns
    /// * `Ok(Some(NodeID))` - Node responded with its Node ID
    /// * `Ok(None)` - Node did not respond within timeout
    pub async fn verify_node(&mut self, dest_alias: u16, timeout_ms: u64) -> Result<Option<NodeID>> {
        // Send addressed Verify Node ID message
        let verify_frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            self.our_alias.value(),
            dest_alias,
            vec![],
        )?;
        
        self.transport.send(&verify_frame).await?;
        
        // Wait for response
        let start_time = Instant::now();
        let max_duration = Duration::from_millis(timeout_ms);
        
        loop {
            // Check if we've exceeded timeout
            if start_time.elapsed() >= max_duration {
                return Ok(None); // Node did not respond
            }
            
            // Try to receive a frame
            let remaining_time = max_duration.saturating_sub(start_time.elapsed());
            
            match self.transport.receive(remaining_time.as_millis() as u64).await? {
                Some(frame) => {
                    // Check if this is a Verified Node response from our target
                    if let Ok((mti, alias)) = frame.get_mti() {
                        if mti == MTI::VerifiedNode && alias == dest_alias && frame.data.len() == 6 {
                            let node_id = NodeID::from_slice(&frame.data)?;
                            return Ok(Some(node_id));
                        }
                    }
                    // Continue waiting for the right response
                }
                None => {
                    // No frame received, continue waiting
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }
    }
    
    /// Close the connection
    pub async fn close(mut self) -> Result<()> {
        self.transport.close().await
    }

    /// Read CDI (Configuration Description Information) from a node
    ///
    /// Reads the CDI XML document from address space 0xFF using the Memory Configuration Protocol.
    /// CDI is read in 64-byte chunks until a null terminator (0x00) is found.
    ///
    /// # Arguments
    /// * `dest_alias` - Target node's alias
    /// * `timeout_ms` - Maximum time to wait for each response (recommended: 1000ms)
    ///
    /// # Returns
    /// * `Ok(String)` - Complete CDI XML document
    /// * `Err(_)` - Protocol error or timeout
    pub async fn read_cdi(&mut self, dest_alias: u16, timeout_ms: u64) -> Result<String> {
        use crate::protocol::{MemoryConfigCmd, AddressSpace, DatagramAssembler};

        println!("[LCC] read_cdi starting for alias 0x{:03X}", dest_alias);
        
        let mut assembler = DatagramAssembler::new();
        let mut cdi_data = Vec::new();
        let mut address = 0u32;
        const CHUNK_SIZE: u8 = 64;

        loop {
            println!("[LCC] Reading CDI chunk at address {} (chunk size {})", address, CHUNK_SIZE);
            
            // Send read command for next chunk (may be multi-frame)
            let read_frames = MemoryConfigCmd::build_read(
                self.our_alias.value(),
                dest_alias,
                AddressSpace::Cdi,
                address,
                CHUNK_SIZE,
            )?;

            // Send all frames in sequence
            println!("[LCC] Sending {} frame(s) for read command", read_frames.len());
            for (i, frame) in read_frames.iter().enumerate() {
                println!("[LCC] Sending frame {}/{}: {}", i + 1, read_frames.len(), frame.to_string());
                self.transport.send(frame).await?;
            }

            // Wait for response (may be multi-frame datagram)
            let start_time = Instant::now();
            let max_duration = Duration::from_millis(timeout_ms);
            let mut reply_payload: Option<Vec<u8>> = None;

            while reply_payload.is_none() {
                if start_time.elapsed() >= max_duration {
                    return Err(crate::Error::Timeout(format!(
                        "Timeout waiting for CDI read reply at address {}",
                        address
                    )));
                }

                let remaining = max_duration.saturating_sub(start_time.elapsed());
                if let Some(frame) = self.transport.receive(remaining.as_millis() as u64).await? {
                    // Check if this is a datagram frame from our target
                    // Note: Datagram frames have different header encoding than standard messages
                    // and must be parsed with from_datagram_header() to extract MTI correctly
                    if let Ok((mti, source, dest)) = crate::protocol::MTI::from_datagram_header(frame.header) {
                        if source == dest_alias && dest == self.our_alias.value() && 
                           matches!(mti, crate::protocol::MTI::DatagramOnly | crate::protocol::MTI::DatagramFirst | crate::protocol::MTI::DatagramMiddle | crate::protocol::MTI::DatagramFinal) {
                            // Handle datagram assembly
                            if let Some(complete_payload) = assembler.handle_frame(&frame)? {
                                // Send acknowledgment immediately
                                let ack_frame = DatagramAssembler::send_acknowledgment(
                                    self.our_alias.value(),
                                    dest_alias,
                                )?;
                                println!("[LCC] Sending DatagramReceivedOK: {}", ack_frame.to_string());
                                self.transport.send(&ack_frame).await?;
                                
                                reply_payload = Some(complete_payload);
                            }
                        }
                    }
                } else {
                    sleep(Duration::from_millis(10)).await;
                }
            }

            // Parse read reply
            let reply_data = reply_payload.unwrap();
            let reply = MemoryConfigCmd::parse_read_reply(&reply_data)?;

            match reply {
                crate::protocol::ReadReply::Success { data, .. } => {
                    // Check for null terminator
                    if let Some(null_pos) = data.iter().position(|&b| b == 0x00) {
                        // Found null terminator - append up to it and we're done
                        cdi_data.extend_from_slice(&data[..null_pos]);
                        break;
                    } else {
                        // No null terminator yet - append all data and continue
                        address += data.len() as u32;
                        cdi_data.extend_from_slice(&data);
                    }
                }
                crate::protocol::ReadReply::Failed { error_code, message, .. } => {
                    return Err(crate::Error::Protocol(format!(
                        "CDI read failed at address {}: error 0x{:04X} - {}",
                        address, error_code, message
                    )));
                }
            }

            // Safety limit: max 10MB CDI
            if cdi_data.len() > 10 * 1024 * 1024 {
                return Err(crate::Error::Protocol(
                    "CDI exceeds 10MB size limit".to_string()
                ));
            }
        }

        // Convert to UTF-8 string
        String::from_utf8(cdi_data).map_err(|e| {
            crate::Error::Protocol(format!("CDI is not valid UTF-8: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::LccTransport;
    use async_trait::async_trait;
    
    // Mock transport for testing
    struct MockTransport {
        responses: Vec<GridConnectFrame>,
        sent_frames: Vec<GridConnectFrame>,
        response_index: usize,
    }
    
    impl MockTransport {
        fn new(responses: Vec<GridConnectFrame>) -> Self {
            Self {
                responses,
                sent_frames: Vec::new(),
                response_index: 0,
            }
        }
    }
    
    #[async_trait]
    impl LccTransport for MockTransport {
        async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
            self.sent_frames.push(frame.clone());
            Ok(())
        }
        
        async fn receive(&mut self, _timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
            if self.response_index < self.responses.len() {
                let frame = self.responses[self.response_index].clone();
                self.response_index += 1;
                Ok(Some(frame))
            } else {
                // Simulate silence - return None after all responses
                sleep(Duration::from_millis(30)).await;
                Ok(None)
            }
        }
        
        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_discover_no_nodes() {
        let mock = MockTransport::new(vec![]);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeAlias::new(0xAAA).unwrap(),
        );
        
        let nodes = connection.discover_nodes(100).await.unwrap();
        assert_eq!(nodes.len(), 0);
    }
    
    #[tokio::test]
    async fn test_discover_single_node() {
        // Create a Verified Node response frame
        let response = GridConnectFrame::from_mti(
            MTI::VerifiedNode,
            0x123,
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ).unwrap();
        
        let mock = MockTransport::new(vec![response]);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeAlias::new(0xAAA).unwrap(),
        );
        
        let nodes = connection.discover_nodes(100).await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]));
        assert_eq!(nodes[0].alias.value(), 0x123);
    }
    
    #[tokio::test]
    async fn test_discover_multiple_nodes() {
        let responses = vec![
            GridConnectFrame::from_mti(
                MTI::VerifiedNode,
                0x111,
                vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
            ).unwrap(),
            GridConnectFrame::from_mti(
                MTI::VerifiedNode,
                0x222,
                vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66],
            ).unwrap(),
            GridConnectFrame::from_mti(
                MTI::VerifiedNode,
                0x333,
                vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF],
            ).unwrap(),
        ];
        
        let mock = MockTransport::new(responses);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeAlias::new(0xAAA).unwrap(),
        );
        
        let nodes = connection.discover_nodes(200).await.unwrap();
        assert_eq!(nodes.len(), 3);
        
        // Verify we have all three nodes (order doesn't matter)
        let node_ids: Vec<_> = nodes.iter().map(|n| n.node_id).collect();
        assert!(node_ids.contains(&NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06])));
        assert!(node_ids.contains(&NodeID::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66])));
        assert!(node_ids.contains(&NodeID::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])));
    }
    
    #[tokio::test]
    async fn test_discover_ignores_wrong_mti() {
        let responses = vec![
            // This should be ignored (wrong MTI)
            GridConnectFrame::from_mti(
                MTI::VerifyNodeGlobal,
                0x111,
                vec![],
            ).unwrap(),
            // This should be collected
            GridConnectFrame::from_mti(
                MTI::VerifiedNode,
                0x222,
                vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
            ).unwrap(),
        ];
        
        let mock = MockTransport::new(responses);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeAlias::new(0xAAA).unwrap(),
        );
        
        let nodes = connection.discover_nodes(100).await.unwrap();
        assert_eq!(nodes.len(), 1);
    }
    
    #[tokio::test]
    async fn test_discover_ignores_wrong_data_length() {
        let responses = vec![
            // This should be ignored (wrong data length - not 6 bytes)
            GridConnectFrame::from_mti(
                MTI::VerifiedNode,
                0x111,
                vec![0x01, 0x02, 0x03],
            ).unwrap(),
            // This should be collected
            GridConnectFrame::from_mti(
                MTI::VerifiedNode,
                0x222,
                vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
            ).unwrap(),
        ];
        
        let mock = MockTransport::new(responses);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeAlias::new(0xAAA).unwrap(),
        );
        
        let nodes = connection.discover_nodes(100).await.unwrap();
        assert_eq!(nodes.len(), 1);
    }
}
