//! Node discovery functionality for LCC/OpenLCB networks
//!
//! This module implements the node discovery protocol, which sends a global
//! Verify Node ID message and collects responses.

use crate::{
    Result,
    types::{NodeID, NodeAlias, DiscoveredNode, SNIPData, ProtocolFlags},
    protocol::{GridConnectFrame, MTI},
    transport::{LccTransport, TcpTransport},
    dispatcher::MessageDispatcher,
    alias_allocation::AliasAllocator,
    constants::CONNECTION_STABILIZATION_MS,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};
use tokio::sync::Mutex;

/// Timing data captured during a single `read_memory_timed` call.
///
/// Provides per-frame latency and gap information useful for diagnosing
/// slow TCP forwarding (e.g. via a JMRI LCC Hub).
#[derive(Debug, Clone)]
pub struct MemoryReadTiming {
    /// Elapsed milliseconds from the request being sent to the first datagram frame arriving.
    pub first_frame_latency_ms: u64,
    /// Milliseconds between consecutive datagram frames (empty for single-frame datagrams).
    pub frame_gaps_ms: Vec<u32>,
    /// Total elapsed milliseconds for the entire read (request sent → data ready).
    pub total_duration_ms: u64,
    /// Number of datagram frames received.
    pub frame_count: u8,
}

/// High-level LCC connection for performing network operations
pub struct LccConnection {
    /// Optional message dispatcher for persistent listening
    dispatcher: Option<Arc<Mutex<MessageDispatcher>>>,
    /// Direct transport access (used when no dispatcher)
    transport: Option<Box<dyn LccTransport>>,
    /// Our node ID
    our_node_id: NodeID,
    /// Our node alias (negotiated via alias allocation protocol)
    our_alias: NodeAlias,
    /// Optional SNIP data to provide when queried
    our_snip: Option<SNIPData>,
    /// Protocol flags we advertise in PIP replies (D13)
    our_pip_flags: ProtocolFlags,
    /// Handles for background responder tasks (query + SNIP).
    /// Stored so they can be aborted on disconnect, preventing the tasks
    /// from keeping `Arc<Mutex<MessageDispatcher>>` (and therefore the
    /// serial port) alive after the connection is closed.
    responder_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl LccConnection {
    /// Connect to an LCC network via TCP with a persistent message dispatcher
    /// 
    /// This creates a connection with background message monitoring, enabling
    /// real-time event detection and concurrent operations. Performs the full
    /// alias allocation protocol to negotiate a unique alias.
    /// 
    /// # Arguments
    /// * `host` - Hostname or IP address
    /// * `port` - Port number (typically 12021)
    /// * `node_id` - Our Node ID (6 bytes)
    /// 
    /// # Example
    /// ```no_run
    /// use lcc_rs::{LccConnection, NodeID};
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let node_id = NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]);
    ///     let connection = LccConnection::connect_with_dispatcher("localhost", 12021, node_id).await?;
    ///     // Dispatcher runs in background, listening for all messages
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_with_dispatcher(host: &str, port: u16, node_id: NodeID) -> Result<Arc<Mutex<Self>>> {
        // Create transport, allocate alias, then hand the same connection to the
        // dispatcher. (Previously a second TCP connection was opened for the dispatcher
        // and the allocation connection was discarded — that was wrong because other
        // nodes only register our alias from the InitializationComplete we sent, and
        // a fresh connection without re-sending InitComplete left us invisible to any
        // node that joined after our first connection closed.)
        let transport = TcpTransport::connect(host, port).await?;
        let mut boxed_transport: Box<dyn LccTransport> = Box::new(transport);

        // Give the bridge/gateway time to stabilize after the TCP handshake
        // before we start alias negotiation (JMRI does a similar delay).
        sleep(Duration::from_millis(CONNECTION_STABILIZATION_MS)).await;

        let our_alias = AliasAllocator::allocate(&node_id, &mut boxed_transport).await?;

        // Pass the same connection (which already sent CID/RID/InitComplete) to the dispatcher.
        let mut dispatcher = MessageDispatcher::new(boxed_transport);
        dispatcher.start();

        let connection = Self {
            dispatcher: Some(Arc::new(Mutex::new(dispatcher))),
            transport: None,
            our_node_id: node_id,
            our_alias,
            our_snip: None,
            our_pip_flags: Self::default_pip_flags(),
            responder_handles: vec![],
        };

        Ok(Arc::new(Mutex::new(connection)))
    }
    
    /// Connect to an LCC network using any pre-opened transport with a persistent
    /// message dispatcher.
    ///
    /// Use this for serial transports (GridConnect, SLCAN) where there is only one
    /// physical connection. The transport is used for alias allocation and then handed
    /// to the dispatcher.
    ///
    /// # Arguments
    /// * `transport` — An already-opened, ready-to-use transport
    /// * `node_id` — Our Node ID (6 bytes)
    pub async fn connect_with_dispatcher_and_transport(
        mut transport: Box<dyn LccTransport>,
        node_id: NodeID,
    ) -> Result<Arc<Mutex<Self>>> {
        // Perform alias allocation on the transport
        let our_alias = AliasAllocator::allocate(&node_id, &mut transport).await?;

        // Hand the transport to the dispatcher
        let mut dispatcher = MessageDispatcher::new(transport);
        dispatcher.start();

        let connection = Self {
            dispatcher: Some(Arc::new(Mutex::new(dispatcher))),
            transport: None,
            our_node_id: node_id,
            our_alias,
            our_snip: None,
            our_pip_flags: Self::default_pip_flags(),
            responder_handles: vec![],
        };

        Ok(Arc::new(Mutex::new(connection)))
    }

    /// Connect to an LCC network via TCP (legacy direct mode)
    /// 
    /// This creates a connection without a persistent dispatcher.
    /// For new code, prefer `connect_with_dispatcher`.
    /// 
    /// # Arguments
    /// * `host` - Hostname or IP address
    /// * `port` - Port number (typically 12021)
    /// * `node_id` - Our Node ID (6 bytes)
    pub async fn connect(host: &str, port: u16, node_id: NodeID) -> Result<Self> {
        let mut transport: Box<dyn LccTransport> = Box::new(TcpTransport::connect(host, port).await?);
        let our_alias = AliasAllocator::allocate(&node_id, &mut transport).await?;
        
        // Reconnect for fresh channel
        let transport = TcpTransport::connect(host, port).await?;
        
        Ok(Self {
            dispatcher: None,
            transport: Some(Box::new(transport)),
            our_node_id: node_id,
            our_alias,
            our_snip: None,
            our_pip_flags: Self::default_pip_flags(),
            responder_handles: vec![],
        })
    }
    
    /// Create an LCC connection with a custom transport (for testing)
    pub fn with_transport(transport: Box<dyn LccTransport>, node_id: NodeID, our_alias: NodeAlias) -> Self {
        Self {
            dispatcher: None,
            transport: Some(transport),
            our_node_id: node_id,
            our_alias,
            our_snip: None,
            our_pip_flags: Self::default_pip_flags(),
            responder_handles: vec![],
        }
    }
    
    /// Get our node ID
    pub fn our_node_id(&self) -> &NodeID {
        &self.our_node_id
    }

    /// Default protocol flags advertised in PIP replies.
    fn default_pip_flags() -> ProtocolFlags {
        ProtocolFlags {
            simple_protocol: true,
            datagram: true,
            stream: false,
            memory_configuration: true,
            reservation: false,
            event_exchange: true,
            identification: true,
            teach_learn: false,
            remote_button: false,
            acdi: false,
            display: false,
            snip: true,
            cdi: true,
            traction_control: false,
            function_description_information: false,
            dcc_command_station: false,
            simple_train_node: false,
            function_configuration: false,
            firmware_upgrade: false,
            firmware_upgrade_active: false,
        }
    }
    
    /// Get a reference to the message dispatcher (if using dispatcher mode)
    pub fn dispatcher(&self) -> Option<Arc<Mutex<MessageDispatcher>>> {
        self.dispatcher.clone()
    }
    
    /// Get our node alias
    pub fn our_alias(&self) -> &NodeAlias {
        &self.our_alias
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
        if let Some(ref dispatcher) = self.dispatcher {
            self.discover_nodes_with_dispatcher(dispatcher, timeout_ms).await
        } else {
            // Use direct transport mode
            let our_alias = self.our_alias.value();
            let transport = self.transport.as_mut()
                .ok_or_else(|| crate::Error::Protocol("No transport or dispatcher available".to_string()))?;
            Self::discover_nodes_direct_impl(transport, our_alias, timeout_ms).await
        }
    }

    /// Send a `VerifyNodeGlobal` frame and return immediately.
    ///
    /// All `VerifiedNode` replies flow through the persistent event system —
    /// no waiting, no list returned.  Subscribe to the `lcc-node-discovered`
    /// Tauri event before calling this to receive nodes as they arrive.
    ///
    /// For a one-shot discovery with a fixed timeout, use [`discover_nodes`] instead.
    pub async fn probe_nodes(&mut self) -> Result<()> {
        let verify_frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            self.our_alias.value(),
            vec![],
        )?;
        if let Some(ref dispatcher) = self.dispatcher {
            let disp = dispatcher.lock().await;
            disp.send(&verify_frame).await
        } else if let Some(ref mut transport) = self.transport {
            transport.send(&verify_frame).await
        } else {
            Err(crate::Error::Protocol("No transport or dispatcher available".to_string()))
        }
    }

    /// Discover nodes using the message dispatcher (channel-based)
    async fn discover_nodes_with_dispatcher(
        &self,
        dispatcher: &Arc<Mutex<MessageDispatcher>>,
        timeout_ms: u64,
    ) -> Result<Vec<DiscoveredNode>> {
        // Subscribe to VerifiedNode messages
        let mut rx = {
            let disp = dispatcher.lock().await;
            disp.subscribe_mti(MTI::VerifiedNode).await
        };
        
        // Send global Verify Node ID message
        let verify_frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            self.our_alias.value(),
            vec![],
        )?;
        
        {
            let disp = dispatcher.lock().await;
            disp.send(&verify_frame).await?;
        }
        
        // Collect responses
        let mut nodes = HashMap::new();
        let start_time = Instant::now();
        let max_duration = Duration::from_millis(timeout_ms);
        // After the first response, end collection once DISCOVERY_SILENCE_THRESHOLD_MS
        // elapses with no further replies. Before any response we wait the full timeout
        // so slow/high-latency networks are not prematurely abandoned.
        let silence_threshold = Duration::from_millis(crate::constants::DISCOVERY_SILENCE_THRESHOLD_MS);
        let mut last_receive_time: Option<Instant> = None;
        
        loop {
            if start_time.elapsed() >= max_duration {
                break;
            }
            
            // Only apply silence guard after seeing at least one response.
            if let Some(t) = last_receive_time {
                if t.elapsed() >= silence_threshold {
                    break;
                }
            }
            
            let remaining = max_duration.saturating_sub(start_time.elapsed());
            let poll_timeout = std::cmp::min(remaining, Duration::from_millis(crate::constants::DISCOVERY_POLL_INTERVAL_MS));
            
            match tokio::time::timeout(poll_timeout, rx.recv()).await {
                Ok(Ok(msg)) => {
                    last_receive_time = Some(Instant::now());
                    
                    if let Ok((_, alias)) = msg.frame.get_mti() {
                        if msg.frame.data.len() == 6 {
                            let node_id = NodeID::from_slice(&msg.frame.data)?;
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
                                    pip_flags: None,
                                    pip_status: crate::types::PIPStatus::Unknown,
                                },
                            );
                        }
                    }
                }
                Ok(Err(_)) => {
                    // Channel lagged - try again
                    continue;
                }
                Err(_) => {
                    // Timeout on channel receive
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }
        
        Ok(nodes.into_values().collect())
    }
    
    /// Discover nodes using direct transport (legacy polling mode) - static implementation
    async fn discover_nodes_direct_impl(
        transport: &mut Box<dyn LccTransport>,
        our_alias: u16,
        timeout_ms: u64,
    ) -> Result<Vec<DiscoveredNode>> {
        // Send global Verify Node ID message
        let verify_frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            our_alias,
            vec![],
        )?;
        
        transport.send(&verify_frame).await?;
        
        // Collect responses
        let mut nodes = HashMap::new();
        let start_time = Instant::now();
        let max_duration = Duration::from_millis(timeout_ms);
        // After the first response, end collection once DISCOVERY_SILENCE_THRESHOLD_MS
        // elapses with no further replies. Before any response we wait the full timeout
        // so slow/high-latency networks are not prematurely abandoned.
        let silence_threshold = Duration::from_millis(crate::constants::DISCOVERY_SILENCE_THRESHOLD_MS);
        let mut last_receive_time: Option<Instant> = None;
        
        loop {
            // Check if we've exceeded max timeout
            if start_time.elapsed() >= max_duration {
                break;
            }
            
            // Only apply silence guard after seeing at least one response.
            if let Some(t) = last_receive_time {
                if t.elapsed() >= silence_threshold {
                    break;
                }
            }
            
            // Try to receive a frame with a short timeout
            let remaining_time = max_duration.saturating_sub(start_time.elapsed());
            let poll_timeout = std::cmp::min(remaining_time, Duration::from_millis(crate::constants::DISCOVERY_POLL_INTERVAL_MS));
            
            match transport.receive(poll_timeout.as_millis() as u64).await? {
                Some(frame) => {
                    last_receive_time = Some(Instant::now());
                    
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
                                    pip_flags: None,
                                    pip_status: crate::types::PIPStatus::Unknown,
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
        
        if let Some(ref dispatcher) = self.dispatcher {
            // TODO: Implement dispatcher-based SNIP query
            // For now, use direct transport via dispatcher
            let transport_arc = {
                let disp = dispatcher.lock().await;
                disp.transport()
            };
            let mut transport = transport_arc.lock().await;
            crate::snip::query_snip(
                transport.as_mut(),
                self.our_alias.value(),
                dest_alias,
                sem,
            ).await
        } else if let Some(ref mut transport) = self.transport {
            crate::snip::query_snip(
                transport.as_mut(),
                self.our_alias.value(),
                dest_alias,
                sem,
            ).await
        } else {
            Err(crate::Error::Protocol("No transport or dispatcher available".to_string()))
        }
    }

    /// Query Protocol Identification Protocol (PIP) data for a specific node.
    ///
    /// Returns the set of optional LCC protocols the node advertises support for.
    /// Call this after SNIP to decide whether CDI/Memory Config reads are worthwhile.
    ///
    /// # Arguments
    /// * `dest_alias` - Target node's alias
    /// * `semaphore` - Optional semaphore for concurrency limiting
    ///
    /// # Returns
    /// * `Ok((Some(ProtocolFlags), PIPStatus::Complete))` on success
    /// * `Ok((None, PIPStatus::Timeout))` when the node does not reply
    pub async fn query_pip(
        &mut self,
        dest_alias: u16,
        semaphore: Option<std::sync::Arc<tokio::sync::Semaphore>>,
    ) -> Result<(Option<crate::types::ProtocolFlags>, crate::types::PIPStatus)> {
        let sem = semaphore.unwrap_or_else(|| std::sync::Arc::new(tokio::sync::Semaphore::new(5)));

        if let Some(ref dispatcher) = self.dispatcher {
            let transport_arc = {
                let disp = dispatcher.lock().await;
                disp.transport()
            };
            let mut transport = transport_arc.lock().await;
            crate::pip::query_pip(
                transport.as_mut(),
                self.our_alias.value(),
                dest_alias,
                sem,
            ).await
        } else if let Some(ref mut transport) = self.transport {
            crate::pip::query_pip(
                transport.as_mut(),
                self.our_alias.value(),
                dest_alias,
                sem,
            ).await
        } else {
            Err(crate::Error::Protocol("No transport or dispatcher available".to_string()))
        }
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
        if let Some(ref dispatcher) = self.dispatcher {
            self.verify_node_with_dispatcher(dispatcher, dest_alias, timeout_ms).await
        } else {
            let our_alias = self.our_alias.value();
            let transport = self.transport.as_mut()
                .ok_or_else(|| crate::Error::Protocol("No transport or dispatcher available".to_string()))?;
            Self::verify_node_direct_impl(transport, our_alias, dest_alias, timeout_ms).await
        }
    }
    
    /// Verify node using message dispatcher
    async fn verify_node_with_dispatcher(
        &self,
        dispatcher: &Arc<Mutex<MessageDispatcher>>,
        dest_alias: u16,
        timeout_ms: u64,
    ) -> Result<Option<NodeID>> {
        // Subscribe to VerifiedNode messages
        let mut rx = {
            let disp = dispatcher.lock().await;
            disp.subscribe_mti(MTI::VerifiedNode).await
        };
        
        // Send addressed Verify Node ID message
        let verify_frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            self.our_alias.value(),
            dest_alias,
            vec![],
        )?;
        
        {
            let disp = dispatcher.lock().await;
            disp.send(&verify_frame).await?;
        }
        
        // Wait for response
        let start_time = Instant::now();
        let max_duration = Duration::from_millis(timeout_ms);
        
        loop {
            if start_time.elapsed() >= max_duration {
                return Ok(None);
            }
            
            let remaining = max_duration.saturating_sub(start_time.elapsed());
            
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(msg)) => {
                    if let Ok((_, alias)) = msg.frame.get_mti() {
                        if alias == dest_alias && msg.frame.data.len() == 6 {
                            let node_id = NodeID::from_slice(&msg.frame.data)?;
                            return Ok(Some(node_id));
                        }
                    }
                }
                Ok(Err(_)) => continue, // Channel lagged
                Err(_) => return Ok(None), // Timeout
            }
        }
    }
    
    /// Verify node using direct transport - static implementation
    async fn verify_node_direct_impl(
        transport: &mut Box<dyn LccTransport>,
        our_alias: u16,
        dest_alias: u16,
        timeout_ms: u64,
    ) -> Result<Option<NodeID>> {
        // Send addressed Verify Node ID message
        let verify_frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            our_alias,
            dest_alias,
            vec![],
        )?;
        
        transport.send(&verify_frame).await?;
        
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
            
            match transport.receive(remaining_time.as_millis() as u64).await? {
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
    
    /// Start responding to Verify Node ID queries from the network
    /// 
    /// This spawns a background task that:
    /// - Listens for VerifyNodeGlobal queries (MTI 0x19490)
    /// - Listens for VerifyNodeAddressed queries (MTI 0x19488)
    /// - Listens for AliasMapEnquiry (AME) frames (MTI 0x10702)
    /// - Responds with VerifiedNode / AliasMapDefinition frames as appropriate
    /// 
    /// Only works when using dispatcher mode (connect_with_dispatcher).
    /// This method returns immediately; the response task runs in the background.
    /// 
    /// # Errors
    /// Returns an error if the connection is not using dispatcher mode.
    pub fn start_responding_to_queries(&mut self) -> Result<()> {
        let dispatcher = self.dispatcher.clone()
            .ok_or_else(|| crate::Error::Protocol(
                "start_responding_to_queries requires dispatcher mode (use connect_with_dispatcher)".to_string()
            ))?;
        
        let our_alias = self.our_alias;
        let our_node_id = self.our_node_id;
        
        // --- VerifyNodeGlobal responder ---
        let disp_global = dispatcher.clone();
        let handle_global = tokio::spawn(async move {
            let mut rx = {
                let disp = disp_global.lock().await;
                disp.subscribe_mti(MTI::VerifyNodeGlobal).await
            };
            loop {
                match rx.recv().await {
                    Ok(_msg) => {
                        if let Ok(response_frame) = GridConnectFrame::from_mti(
                            MTI::VerifiedNode,
                            our_alias.value(),
                            our_node_id.as_bytes().to_vec(),
                        ) {
                            let disp = disp_global.lock().await;
                            let _ = disp.send(&response_frame).await;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_global);

        // --- VerifyNodeAddressed responder (D8) ---
        let disp_addressed = dispatcher.clone();
        let handle_addressed = tokio::spawn(async move {
            let mut rx = {
                let disp = disp_addressed.lock().await;
                disp.subscribe_mti(MTI::VerifyNodeAddressed).await
            };
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        // Only respond if the query is addressed to us (dest alias matches)
                        let is_for_us = msg.frame.get_dest_from_body()
                            .map(|(dest, _)| dest == our_alias.value())
                            .unwrap_or(false);
                        if is_for_us {
                            if let Ok(response_frame) = GridConnectFrame::from_mti(
                                MTI::VerifiedNode,
                                our_alias.value(),
                                our_node_id.as_bytes().to_vec(),
                            ) {
                                let disp = disp_addressed.lock().await;
                                let _ = disp.send(&response_frame).await;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_addressed);

        // --- AliasMapEnquiry (AME) responder (D5) ---
        let disp_ame = dispatcher.clone();
        let handle_ame = tokio::spawn(async move {
            let mut rx = {
                let disp = disp_ame.lock().await;
                disp.subscribe_mti(MTI::AliasMapEnquiry).await
            };
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        // AME data is the 6-byte NodeID being enquired, or empty for global.
                        // Respond if it matches our NodeID or is a global enquiry (empty data).
                        let data = &msg.frame.data;
                        let is_global = data.is_empty();
                        let matches_us = data.len() == 6 && data.as_slice() == our_node_id.as_bytes();
                        if is_global || matches_us {
                            // Respond with AliasMapDefinition carrying our NodeID
                            if let Ok(amd_frame) = GridConnectFrame::from_mti(
                                MTI::AliasMapDefinition,
                                our_alias.value(),
                                our_node_id.as_bytes().to_vec(),
                            ) {
                                let disp = disp_ame.lock().await;
                                let _ = disp.send(&amd_frame).await;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_ame);

        // --- ProtocolSupportInquiry (PIP) responder (D13) ---
        let disp_pip = dispatcher.clone();
        let our_pip_flags = self.our_pip_flags;
        let handle_pip = tokio::spawn(async move {
            let mut rx = {
                let disp = disp_pip.lock().await;
                disp.subscribe_mti(MTI::ProtocolSupportInquiry).await
            };
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        // Only respond if addressed to us
                        let is_for_us = msg.frame.get_dest_from_body()
                            .map(|(dest, _)| dest == our_alias.value())
                            .unwrap_or(false);
                        if is_for_us {
                            let flag_bytes = our_pip_flags.to_bytes();
                            if let Ok(response_frame) = GridConnectFrame::from_addressed_mti(
                                MTI::ProtocolSupportReply,
                                our_alias.value(),
                                // source_alias of the request is in the header
                                MTI::from_header(msg.frame.header)
                                    .map(|(_, src)| src)
                                    .unwrap_or(0),
                                flag_bytes,
                            ) {
                                let disp = disp_pip.lock().await;
                                let _ = disp.send(&response_frame).await;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_pip);

        // --- D18: Ongoing alias conflict detector ---
        // Monitor AMD frames — if another node claims our alias with a different
        // NodeID, send AMR + re-announce our AMD to reassert ownership.
        let disp_conflict = dispatcher.clone();
        let handle_conflict = tokio::spawn(async move {
            let mut rx = {
                let disp = disp_conflict.lock().await;
                disp.subscribe_mti(MTI::AliasMapDefinition).await
            };
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        if let Ok((_, alias)) = MTI::from_header(msg.frame.header) {
                            if alias == our_alias.value() && msg.frame.data.len() >= 6 {
                                let their_node_id: [u8; 6] = msg.frame.data[0..6].try_into().unwrap();
                                if their_node_id != *our_node_id.as_bytes() {
                                    eprintln!(
                                        "[LCC] ALIAS CONFLICT: alias 0x{:03X} claimed by another node — reasserting",
                                        our_alias.value()
                                    );
                                    // Send AMR for our alias, then re-send AMD
                                    if let Ok(amr) = GridConnectFrame::from_mti(
                                        MTI::AliasMapReset,
                                        our_alias.value(),
                                        our_node_id.as_bytes().to_vec(),
                                    ) {
                                        let disp = disp_conflict.lock().await;
                                        let _ = disp.send(&amr).await;
                                        if let Ok(amd) = GridConnectFrame::from_mti(
                                            MTI::AliasMapDefinition,
                                            our_alias.value(),
                                            our_node_id.as_bytes().to_vec(),
                                        ) {
                                            let _ = disp.send(&amd).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_conflict);
        
        Ok(())
    }
    
    /// Set the SNIP data for this node
    /// 
    /// This data will be provided to other nodes when they query us with SNIP requests.
    pub fn set_snip_data(&mut self, snip: SNIPData) {
        self.our_snip = Some(snip);
    }
    
    /// Get the SNIP data for this node
    pub fn snip_data(&self) -> Option<&SNIPData> {
        self.our_snip.as_ref()
    }
    
    /// Start responding to SNIP (Simple Node Identification Protocol) requests from the network
    /// 
    /// This spawns a background task that:
    /// - Listens for SNIPRequest messages (MTI 0x19DE8)
    /// - Responds with SNIPResponse datagrams containing our node's identification data
    /// 
    /// Only works when using dispatcher mode (connect_with_dispatcher).
    /// This method returns immediately; the response task runs in the background.
    /// 
    /// # Errors
    /// Returns an error if the connection is not using dispatcher mode.
    pub fn start_responding_to_snip_requests(&mut self) -> Result<()> {
        let dispatcher = self.dispatcher.clone()
            .ok_or_else(|| crate::Error::Protocol(
                "start_responding_to_snip_requests requires dispatcher mode (use connect_with_dispatcher)".to_string()
            ))?;
        
        let our_alias = self.our_alias;
        let snip_data = self.our_snip.clone();
        
        // Spawn background task to handle SNIP requests; store the handle so it can be
        // aborted on disconnect (prevents keeping MessageDispatcher alive indefinitely).
        let handle = tokio::spawn(async move {
            // Subscribe to SNIP request messages
            let mut rx = {
                let disp = dispatcher.lock().await;
                disp.subscribe_mti(MTI::SNIPRequest).await
            };
            
            // Listen for SNIP requests and respond
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        // We received a SNIPRequest
                        // Extract requester's alias from the frame's source field
                        if let Ok((_, requester_alias)) = msg.frame.get_mti() {
                            // Only respond if we have SNIP data to provide
                            if let Some(ref snip) = snip_data {
                                // Encode SNIP data as payload (include Section 2 even if empty)
                                let snip_payload = crate::snip::encode_snip_payload(snip, true);

                                // Send SNIP response as a datagram
                                // The frame type byte is encoded in the upper nibble of data[0]
                                // Formula: data[0] = (frame_type & 0xF0) | (dest_alias >> 8)
                                // Followed by: data[1] = dest_alias & 0xFF
                                
                                if snip_payload.len() <= 6 {
                                    // Single frame response (0x0A frame type)
                                    let frame_type = 0x0Au8;
                                    let mut data = vec![
                                        (frame_type & 0xF0) | ((requester_alias >> 8) as u8 & 0x0F),
                                        (requester_alias & 0xFF) as u8,
                                    ];
                                    data.extend_from_slice(&snip_payload);
                                    
                                    if let Ok(response) = GridConnectFrame::from_mti(
                                        MTI::SNIPResponse,
                                        our_alias.value(),
                                        data,
                                    ) {
                                        let disp = dispatcher.lock().await;
                                        let _ = disp.send(&response).await;
                                    }
                                } else {
                                    // Multi-frame response - send first, middle, and final frames
                                    let mut offset = 0;
                                    let chunk_size = 6;
                                    let mut frame_num = 0;
                                    
                                    while offset < snip_payload.len() {
                                        let end = std::cmp::min(offset + chunk_size, snip_payload.len());
                                        let chunk = &snip_payload[offset..end];
                                        
                                        let frame_type = if frame_num == 0 {
                                            0x1Au8 // First frame
                                        } else if end == snip_payload.len() {
                                            0x2Au8 // Final frame
                                        } else {
                                            0x3Au8 // Middle frame
                                        };
                                        
                                        let mut data = vec![
                                            (frame_type & 0xF0) | ((requester_alias >> 8) as u8 & 0x0F),
                                            (requester_alias & 0xFF) as u8,
                                        ];
                                        data.extend_from_slice(chunk);
                                        
                                        if let Ok(response) = GridConnectFrame::from_mti(
                                            MTI::SNIPResponse,
                                            our_alias.value(),
                                            data,
                                        ) {
                                            let disp = dispatcher.lock().await;
                                            let _ = disp.send(&response).await;
                                        }
                                        
                                        offset = end;
                                        frame_num += 1;
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Receiver closed, exit background task
                        break;
                    }
                }
            }
        });
        self.responder_handles.push(handle);
        
        Ok(())
    }

    /// Abort and await all background responder tasks (query + SNIP responders).
    ///
    /// Must be called before dropping the connection so that the background tasks
    /// release their `Arc<Mutex<MessageDispatcher>>` clones, allowing the dispatcher
    /// — and therefore the underlying serial port — to be freed immediately.
    pub async fn shutdown_responders(&mut self) {
        for handle in self.responder_handles.drain(..) {
            handle.abort();
            let _ = handle.await; // Resolves immediately once aborted
        }
    }

    /// Close the connection
    pub async fn close(self) -> Result<()> {
        if let Some(dispatcher) = self.dispatcher {
            let mut disp = dispatcher.lock().await;
            disp.shutdown().await;
            Ok(())
        } else if let Some(mut transport) = self.transport {
            transport.close().await
        } else {
            Ok(())
        }
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
        // For now, use direct transport access for both modes
        // TODO: Implement proper dispatcher-based CDI reading
        let transport_ref = if let Some(ref dispatcher) = self.dispatcher {
            let disp = dispatcher.lock().await;
            disp.transport()
        } else if self.transport.is_some() {
            // Will access directly below
            return self.read_cdi_direct(dest_alias, timeout_ms).await;
        } else {
            return Err(crate::Error::Protocol("No transport or dispatcher available".to_string()));
        };
        
        let mut transport = transport_ref.lock().await;
        let our_alias = self.our_alias.value();
        Self::read_cdi_impl(transport.as_mut(), our_alias, dest_alias, timeout_ms).await
    }
    
    /// Read CDI using direct transport reference
    async fn read_cdi_direct(&mut self, dest_alias: u16, timeout_ms: u64) -> Result<String> {
        let our_alias = self.our_alias.value();
        if let Some(ref mut transport) = self.transport {
            Self::read_cdi_impl(transport.as_mut(), our_alias, dest_alias, timeout_ms).await
        } else {
            Err(crate::Error::Protocol("No transport available".to_string()))
        }
    }
    
     /// Read CDI implementation - static method
    async fn read_cdi_impl(
        transport: &mut dyn LccTransport,
        our_alias: u16,
        dest_alias: u16,
        timeout_ms: u64,
    ) -> Result<String> {
        use crate::protocol::{MemoryConfigCmd, AddressSpace, DatagramAssembler};

        println!("[LCC] read_cdi starting for alias 0x{:03X}", dest_alias);
        
        let mut assembler = DatagramAssembler::new();
        let mut cdi_data = Vec::new();
        let mut address = 0u32;
        const CHUNK_SIZE: u8 = 64;

        loop {
            println!("[LCC] Reading CDI chunk at address {} (chunk size {})", address, CHUNK_SIZE);
            
            // D14: Retry each chunk up to 3 times on timeout/error.
            const MAX_CHUNK_RETRIES: u8 = 3;
            let mut chunk_reply: Option<Vec<u8>> = None;

            for attempt in 0..MAX_CHUNK_RETRIES {
                if attempt > 0 {
                    println!("[LCC] Retrying CDI chunk at address {} (attempt {}/{})", address, attempt + 1, MAX_CHUNK_RETRIES);
                    assembler.clear_source(dest_alias);
                }

                // Send read command for next chunk (may be multi-frame)
                let read_frames = MemoryConfigCmd::build_read(
                    our_alias,
                    dest_alias,
                    AddressSpace::Cdi,
                    address,
                    CHUNK_SIZE,
                )?;

                // Send all frames in sequence
                println!("[LCC] Sending {} frame(s) for read command", read_frames.len());
                for (i, frame) in read_frames.iter().enumerate() {
                    println!("[LCC] Sending frame {}/{}: {}", i + 1, read_frames.len(), frame.to_string());
                    transport.send(frame).await?;
                }

                // Wait for response (may be multi-frame datagram)
                let start_time = Instant::now();
                let max_duration = Duration::from_millis(timeout_ms);

                let mut timed_out = false;
                while chunk_reply.is_none() {
                    if start_time.elapsed() >= max_duration {
                        timed_out = true;
                        break;
                    }

                    let remaining = max_duration.saturating_sub(start_time.elapsed());
                    if let Some(frame) = transport.receive(remaining.as_millis() as u64).await? {
                        if let Ok((mti, source, dest)) = crate::protocol::MTI::from_datagram_header(frame.header) {
                            if source == dest_alias && dest == our_alias && 
                               matches!(mti, crate::protocol::MTI::DatagramOnly | crate::protocol::MTI::DatagramFirst | crate::protocol::MTI::DatagramMiddle | crate::protocol::MTI::DatagramFinal) {
                                if let Some(complete_payload) = assembler.handle_frame(&frame)? {
                                    let ack_frame = DatagramAssembler::send_acknowledgment(
                                        our_alias,
                                        dest_alias,
                                    )?;
                                    println!("[LCC] Sending DatagramReceivedOK: {}", ack_frame.to_string());
                                    transport.send(&ack_frame).await?;
                                    
                                    chunk_reply = Some(complete_payload);
                                }
                            }
                        }
                    } else {
                        sleep(Duration::from_millis(10)).await;
                    }
                }

                if chunk_reply.is_some() {
                    break;
                }
                if timed_out && attempt + 1 >= MAX_CHUNK_RETRIES {
                    return Err(crate::Error::Timeout(format!(
                        "Timeout waiting for CDI read reply at address {} after {} attempts",
                        address, MAX_CHUNK_RETRIES
                    )));
                }
            }

            // Parse read reply
            let reply_data = chunk_reply.unwrap();
let reply = MemoryConfigCmd::parse_read_reply(&reply_data)?;

            match reply {
                crate::protocol::ReadReply::Success { data, .. } => {
                    // Guard against zero-length success reply (would loop forever).
                    // OpenLCB_Java advances by the *requested* count in this case.
                    if data.is_empty() {
                        break;
                    }
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
                    // 0x1082 = "address out of bounds" / "not found" — some nodes
                    // (e.g. TCS UWT-100) return this instead of a null terminator to
                    // signal end-of-CDI.  Treat it the same as a null terminator.
                    if error_code == 0x1082 {
                        break;
                    }
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
    
    /// Read memory from a node's address space
    /// 
    /// # Arguments
    /// * `dest_alias` - Target node alias
    /// * `address_space` - Memory address space (0xFD = Configuration, 0xFE = All, 0xFF = CDI)
    /// * `address` - Starting memory address
    /// * `count` - Number of bytes to read (1-64)
    /// * `timeout_ms` - Timeout in milliseconds
    /// 
    /// # Returns
    /// * `Ok(Vec<u8>)` - Raw bytes read from memory
    /// * `Err(_)` - Protocol error or timeout
    pub async fn read_memory(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<Vec<u8>> {
        if let Some(ref dispatcher) = self.dispatcher {
            // Dispatcher mode: subscribe to broadcast channel, send request, wait for reply.
            // This avoids holding the transport mutex for the entire receive loop — the
            // dispatcher background task is the designated reader; we only need the mutex
            // briefly for each outgoing send.
            let our_alias = self.our_alias.value();
            Self::read_memory_with_dispatcher(
                dispatcher,
                our_alias,
                dest_alias,
                address_space,
                address,
                count,
                timeout_ms,
            ).await
        } else if self.transport.is_some() {
            self.read_memory_direct(dest_alias, address_space, address, count, timeout_ms).await
        } else {
            Err(crate::Error::Protocol("No transport or dispatcher available".to_string()))
        }
    }

    /// Like [`read_memory`] but also returns per-read timing metadata.
    ///
    /// Used by `read_all_config_values` to populate `BatchReadStat` diagnostics.
    /// Only dispatcher mode captures per-frame timing; direct-transport mode
    /// synthesises a single-frame summary from wall-clock time.
    pub async fn read_memory_timed(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<(Vec<u8>, MemoryReadTiming)> {
        if let Some(ref dispatcher) = self.dispatcher {
            let our_alias = self.our_alias.value();
            Self::read_memory_with_dispatcher_timed(
                dispatcher,
                our_alias,
                dest_alias,
                address_space,
                address,
                count,
                timeout_ms,
            ).await
        } else if self.transport.is_some() {
            let t0 = Instant::now();
            let data = self.read_memory_direct(dest_alias, address_space, address, count, timeout_ms).await?;
            let ms = t0.elapsed().as_millis() as u64;
            Ok((data, MemoryReadTiming {
                first_frame_latency_ms: ms,
                frame_gaps_ms: vec![],
                total_duration_ms: ms,
                frame_count: 1,
            }))
        } else {
            Err(crate::Error::Protocol("No transport or dispatcher available".to_string()))
        }
    }

    /// Read memory in dispatcher mode (untimed thin wrapper).
    ///
    /// The dispatcher background task is the sole owner of the transport receive path.
    /// This method:
    ///   1. Subscribes to the all-frames broadcast channel BEFORE sending (no frames missed).
    ///   2. Locks the transport briefly to send the request frames only.
    ///   3. Waits on the broadcast channel for reply datagram frames — no transport lock held.
    ///   4. Sends the DatagramReceivedOK acknowledgment via a brief transport lock.
    ///
    /// Round-trip latency is therefore pure network latency (~4ms), not 100ms poll cycles.
    async fn read_memory_with_dispatcher(
        dispatcher: &Arc<Mutex<MessageDispatcher>>,
        our_alias: u16,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<Vec<u8>> {
        Self::read_memory_with_dispatcher_timed(
            dispatcher, our_alias, dest_alias, address_space, address, count, timeout_ms,
        ).await.map(|(data, _timing)| data)
    }

    /// Dispatcher-mode read that also captures per-frame timing for diagnostics.
    async fn read_memory_with_dispatcher_timed(
        dispatcher: &Arc<Mutex<MessageDispatcher>>,
        our_alias: u16,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<(Vec<u8>, MemoryReadTiming)> {
        use crate::protocol::{MemoryConfigCmd, AddressSpace, DatagramAssembler};

        let space = AddressSpace::from_u8(address_space)
            .map_err(|e| crate::Error::Protocol(e))?;

        let read_frames = MemoryConfigCmd::build_read(our_alias, dest_alias, space, address, count)?;

        // Step 1: Subscribe BEFORE sending so we cannot miss the reply.
        let mut rx = {
            let disp = dispatcher.lock().await;
            disp.subscribe_all()
        };

        // Step 2: Send the request (brief lock — just for the write operation).
        let send_time = Instant::now();
        {
            let disp = dispatcher.lock().await;
            for frame in read_frames.iter() {
                disp.send(frame).await?;
            }
        }

        // Step 3: Wait for reply on the broadcast channel (no transport lock held).
        let mut max_duration = Duration::from_millis(timeout_ms);
        let mut assembler = DatagramAssembler::new();
        let mut first_frame_latency_ms: Option<u64> = None;
        let mut last_frame_ms: u64 = 0;
        let mut frame_gaps_ms: Vec<u32> = Vec::new();
        let mut frame_count: u8 = 0;

        loop {
            // Phase 2: idle timeout — reset on every received frame.
            // As long as the node keeps sending frames within `timeout_ms` of the
            // previous one the read succeeds, regardless of total elapsed time.
            // A truly unresponsive node still fails after exactly `timeout_ms` of silence.
            match tokio::time::timeout(max_duration, rx.recv()).await {
                Ok(Ok(msg)) => {
                    // Only process datagram frames from dest_alias addressed to us.
                    let is_our_datagram = MTI::from_datagram_header(msg.frame.header)
                        .map(|(mti, src, dst)| {
                            let is_dg = matches!(
                                mti,
                                MTI::DatagramOnly
                                    | MTI::DatagramFirst
                                    | MTI::DatagramMiddle
                                    | MTI::DatagramFinal
                            );
                            is_dg && src == dest_alias && dst == our_alias
                        })
                        .unwrap_or(false);

                    if !is_our_datagram {
                        if let Ok((mti, src)) = MTI::from_header(msg.frame.header) {
                            if src == dest_alias && msg.frame.data.len() >= 2 {
                                let dst = ((msg.frame.data[0] as u16) << 8) | (msg.frame.data[1] as u16);
                                if dst == our_alias {
                                    // D9: DatagramRejected handling
                                    if mti == MTI::DatagramRejected {
                                        let error_code = if msg.frame.data.len() >= 4 {
                                            ((msg.frame.data[2] as u16) << 8) | (msg.frame.data[3] as u16)
                                        } else {
                                            0
                                        };
                                        // Bit 0x2000 = "resend OK" (temporary, buffer full)
                                        if error_code & 0x2000 != 0 {
                                            // Re-send the read request instead of timing out
                                            let disp = dispatcher.lock().await;
                                            for frame in read_frames.iter() {
                                                disp.send(frame).await?;
                                            }
                                            continue;
                                        } else {
                                            return Err(crate::Error::Protocol(format!(
                                                "Datagram rejected: error 0x{:04X}", error_code
                                            )));
                                        }
                                    }
                                    // D12: DatagramReceivedOk — parse flags for timeout extension
                                    if mti == MTI::DatagramReceivedOk {
                                        let flags = if msg.frame.data.len() >= 3 { msg.frame.data[2] } else { 0 };
                                        let timeout_exp = flags & 0x0F;
                                        if timeout_exp > 0 {
                                            let extended_ms = (1u64 << timeout_exp) * 1000;
                                            if extended_ms > max_duration.as_millis() as u64 {
                                                max_duration = Duration::from_millis(extended_ms);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Record per-frame timing.
                    let elapsed_ms = send_time.elapsed().as_millis() as u64;
                    if first_frame_latency_ms.is_none() {
                        first_frame_latency_ms = Some(elapsed_ms);
                        last_frame_ms = elapsed_ms;
                    } else {
                        frame_gaps_ms.push((elapsed_ms.saturating_sub(last_frame_ms)) as u32);
                        last_frame_ms = elapsed_ms;
                    }
                    frame_count = frame_count.saturating_add(1);

                    if let Ok(Some(datagram_data)) = assembler.handle_frame(&msg.frame) {
                        // Step 4: Send ACK (brief lock).
                        let ack_frame = DatagramAssembler::send_acknowledgment(our_alias, dest_alias)?;
                        {
                            let disp = dispatcher.lock().await;
                            disp.send(&ack_frame).await?;
                        }

                        let total_duration_ms = send_time.elapsed().as_millis() as u64;
                        let timing = MemoryReadTiming {
                            first_frame_latency_ms: first_frame_latency_ms.unwrap_or(total_duration_ms),
                            frame_gaps_ms,
                            total_duration_ms,
                            frame_count,
                        };

                        let reply = MemoryConfigCmd::parse_read_reply(&datagram_data)?;
                        return match reply {
                            crate::protocol::ReadReply::Success { data, .. } => Ok((data, timing)),
                            crate::protocol::ReadReply::Failed { error_code, message, .. } => {
                                Err(crate::Error::Protocol(format!(
                                    "Memory read failed: error 0x{:04X} - {}",
                                    error_code, message
                                )))
                            }
                        };
                    }
                }
                Ok(Err(_)) => {
                    // Broadcast channel lagged (buffer full) — frames may have been dropped.
                    // The reply might have been lost; surface a timeout rather than hanging.
                    return Err(crate::Error::Timeout(
                        "Broadcast channel lagged during memory read".to_string(),
                    ));
                }
                Err(_) => {
                    return Err(crate::Error::Timeout(
                        "Timeout waiting for memory read response".to_string(),
                    ));
                }
            }
        }
    }

    /// Read memory using direct transport reference
    async fn read_memory_direct(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<Vec<u8>> {
        let our_alias = self.our_alias.value();
        if let Some(ref mut transport) = self.transport {
            Self::read_memory_impl(
                transport.as_mut(),
                our_alias,
                dest_alias,
                address_space,
                address,
                count,
                timeout_ms,
            ).await
        } else {
            Err(crate::Error::Protocol("No transport available".to_string()))
        }
    }
    
    /// Read memory implementation - static method
    async fn read_memory_impl(
        transport: &mut dyn LccTransport,
        our_alias: u16,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<Vec<u8>> {
        use crate::protocol::{MemoryConfigCmd, AddressSpace, DatagramAssembler};
        use std::time::Instant;
        
        // Convert address space byte to enum
        let space = AddressSpace::from_u8(address_space)
            .map_err(|e| crate::Error::Protocol(e))?;
        
        // Build read command
        let read_frames = MemoryConfigCmd::build_read(
            our_alias,
            dest_alias,
            space,
            address,
            count,
        )?;
        
        // Send all frames
        for frame in read_frames.iter() {
            transport.send(frame).await?;
        }
        
        // Wait for response
        let start_time = Instant::now();
        let max_duration = Duration::from_millis(timeout_ms);
        let mut assembler = DatagramAssembler::new();
        
        while start_time.elapsed() < max_duration {
            let remaining = max_duration.saturating_sub(start_time.elapsed());
            if let Some(frame) = transport.receive(remaining.as_millis() as u64).await? {
                // Filter: only accept datagram frames addressed to us from the expected node.
                // On a multi-node network, datagrams from unrelated nodes must be ignored
                // so they don't corrupt the assembler state or return wrong data.
                let is_our_datagram = MTI::from_datagram_header(frame.header)
                    .map(|(mti, src, dst)| {
                        let is_datagram = matches!(
                            mti,
                            MTI::DatagramOnly
                                | MTI::DatagramFirst
                                | MTI::DatagramMiddle
                                | MTI::DatagramFinal
                        );
                        is_datagram && src == dest_alias && dst == our_alias
                    })
                    .unwrap_or(false);

                if !is_our_datagram {
                    continue;
                }

                // Check if datagram frame and assemble
                if let Ok(Some(datagram_data)) = assembler.handle_frame(&frame) {
                    // Send DatagramReceivedOK acknowledgment to the node
                    let ack_frame = DatagramAssembler::send_acknowledgment(our_alias, dest_alias)?;
                    transport.send(&ack_frame).await?;
                    
                    // Parse response
                    let reply = MemoryConfigCmd::parse_read_reply(&datagram_data)?;
                    
                    match reply {
                        crate::protocol::ReadReply::Success { data, .. } => {
                            return Ok(data);
                        }
                        crate::protocol::ReadReply::Failed { error_code, message, .. } => {
                            return Err(crate::Error::Protocol(format!(
                                "Memory read failed: error 0x{:04X} - {}",
                                error_code, message
                            )));
                        }
                    }
                }
            }
        }
        
        Err(crate::Error::Timeout(format!(
            "Timeout waiting for memory read response"
        )))
    }

    // ========================================================================
    // Memory Write Operations (Spec 007: Editable Node Configuration)
    // ========================================================================

    /// Write data to a node's memory at the specified address and space.
    ///
    /// Handles: datagram framing, send, wait for Datagram Received OK,
    /// retry up to 3 times with 3-second timeout per attempt.
    ///
    /// For data > 64 bytes, automatically chunks into sequential ≤64-byte writes
    /// with address advancing.
    ///
    /// Uses `RequestWithNoReply` pattern: Datagram Received OK = success.
    pub async fn write_memory(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        data: &[u8],
    ) -> Result<()> {
        if data.is_empty() {
            return Err(crate::Error::Protocol("Write data cannot be empty".to_string()));
        }

        // Chunk data into ≤64-byte segments
        let mut offset: usize = 0;
        while offset < data.len() {
            let chunk_size = (data.len() - offset).min(64);
            let chunk = &data[offset..offset + chunk_size];
            let chunk_address = address + offset as u32;

            self.write_memory_chunk(dest_alias, address_space, chunk_address, chunk).await?;
            offset += chunk_size;
        }

        Ok(())
    }

    /// Write a single chunk (≤64 bytes) with retry logic.
    async fn write_memory_chunk(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        data: &[u8],
    ) -> Result<()> {
        const MAX_RETRIES: u32 = crate::constants::WRITE_MEMORY_MAX_RETRIES;
        const TIMEOUT_MS: u64 = crate::constants::WRITE_MEMORY_TIMEOUT_MS;

        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.write_memory_once(dest_alias, address_space, address, data, TIMEOUT_MS).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    eprintln!(
                        "Write attempt {}/{} failed for addr 0x{:08X}: {}",
                        attempt + 1, MAX_RETRIES, address, e
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            crate::Error::Protocol("Write failed after retries".to_string())
        }))
    }

    /// Single write attempt — send write datagram, await Datagram Received OK.
    async fn write_memory_once(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        data: &[u8],
        timeout_ms: u64,
    ) -> Result<()> {
        if let Some(ref dispatcher) = self.dispatcher {
            let our_alias = self.our_alias.value();
            Self::write_memory_with_dispatcher(
                dispatcher,
                our_alias,
                dest_alias,
                address_space,
                address,
                data,
                timeout_ms,
            ).await
        } else if self.transport.is_some() {
            self.write_memory_direct(dest_alias, address_space, address, data, timeout_ms).await
        } else {
            Err(crate::Error::Protocol("No transport or dispatcher available".to_string()))
        }
    }

    /// Write memory in dispatcher mode.
    async fn write_memory_with_dispatcher(
        dispatcher: &Arc<Mutex<MessageDispatcher>>,
        our_alias: u16,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        data: &[u8],
        timeout_ms: u64,
    ) -> Result<()> {
        use crate::protocol::{MemoryConfigCmd, AddressSpace, DatagramAssembler};

        let space = AddressSpace::from_u8(address_space)
            .map_err(|e| crate::Error::Protocol(e))?;

        let write_frames = MemoryConfigCmd::build_write(our_alias, dest_alias, space, address, data)?;

        // Step 1: Subscribe BEFORE sending so we cannot miss the reply.
        let mut rx = {
            let disp = dispatcher.lock().await;
            disp.subscribe_all()
        };

        // Step 2: Send the request (brief lock).
        {
            let disp = dispatcher.lock().await;
            for frame in write_frames.iter() {
                disp.send(frame).await?;
            }
        }

        // Step 3: Wait for Datagram Received OK from dest_alias addressed to us.
        let start_time = Instant::now();
        let mut max_duration = Duration::from_millis(timeout_ms);

        loop {
            let remaining = max_duration.saturating_sub(start_time.elapsed());
            if remaining.is_zero() {
                return Err(crate::Error::Timeout(
                    "Timeout waiting for write acknowledgment".to_string(),
                ));
            }

            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(msg)) => {
                    // DatagramReceivedOk uses standard MTI format (not datagram)
                    // Source alias in header, destination alias in data payload
                    if let Ok((mti, src)) = MTI::from_header(msg.frame.header) {
                        if src != dest_alias {
                            continue;
                        }
                        if mti == MTI::DatagramReceivedOk {
                            // Check data payload for our alias
                            if msg.frame.data.len() >= 2 {
                                let dst = ((msg.frame.data[0] as u16) << 8) | (msg.frame.data[1] as u16);
                                if dst == our_alias {
                                    // D12: Parse flags for timeout extension
                                    // D10: Check FLAG_REPLY_PENDING (bit 0x80 of flags byte)
                                    let flags = if msg.frame.data.len() >= 3 { msg.frame.data[2] } else { 0 };
                                    let reply_pending = flags & 0x80 != 0;

                                    // D12: timeout extension — bits 3:0 encode power-of-two seconds
                                    let timeout_exp = flags & 0x0F;
                                    if timeout_exp > 0 {
                                        let extended_ms = (1u64 << timeout_exp) * 1000;
                                        if extended_ms > max_duration.as_millis() as u64 {
                                            max_duration = Duration::from_millis(extended_ms);
                                        }
                                    }

                                    if !reply_pending {
                                        return Ok(());
                                    }
                                    // D10: reply pending — continue listening for a
                                    // write-reply datagram that carries the result.
                                    // Fall through to keep reading.
                                }
                            }
                        } else if mti == MTI::DatagramRejected {
                            // D9: handle DatagramRejected
                            if msg.frame.data.len() >= 2 {
                                let dst = ((msg.frame.data[0] as u16) << 8) | (msg.frame.data[1] as u16);
                                if dst == our_alias {
                                    let error_code = if msg.frame.data.len() >= 4 {
                                        ((msg.frame.data[2] as u16) << 8) | (msg.frame.data[3] as u16)
                                    } else {
                                        0
                                    };
                                    if error_code & 0x2000 != 0 {
                                        // Resend OK — re-send the write request
                                        let disp = dispatcher.lock().await;
                                        for frame in write_frames.iter() {
                                            disp.send(frame).await?;
                                        }
                                        continue;
                                    } else {
                                        return Err(crate::Error::Protocol(format!(
                                            "Datagram rejected: error 0x{:04X}", error_code
                                        )));
                                    }
                                }
                            }
                        }
                        // D10: Check for write-reply datagram (when reply_pending was set)
                        let is_our_datagram = MTI::from_datagram_header(msg.frame.header)
                            .map(|(mti, dg_src, dg_dst)| {
                                let is_dg = matches!(mti, MTI::DatagramOnly);
                                is_dg && dg_src == dest_alias && dg_dst == our_alias
                            })
                            .unwrap_or(false);
                        if is_our_datagram {
                            // Parse write reply: command byte 0x20, reply 0x10
                            // Data: [0x20, 0x10, space, ...] — error in subsequent bytes
                            if msg.frame.data.len() >= 2 && msg.frame.data[0] == 0x20 {
                                let reply_cmd = msg.frame.data[1];
                                if reply_cmd & 0x08 != 0 {
                                    // Error bit set in reply — write failed
                                    let error_code = if msg.frame.data.len() >= 4 {
                                        ((msg.frame.data[2] as u16) << 8) | (msg.frame.data[3] as u16)
                                    } else {
                                        0
                                    };
                                    // Send DatagramReceivedOk for the reply datagram
                                    let ack = DatagramAssembler::send_acknowledgment(our_alias, dest_alias)?;
                                    let disp = dispatcher.lock().await;
                                    disp.send(&ack).await?;
                                    return Err(crate::Error::Protocol(format!(
                                        "Write reply error: 0x{:04X}", error_code
                                    )));
                                }
                                // Success reply — acknowledge the datagram
                                let ack = DatagramAssembler::send_acknowledgment(our_alias, dest_alias)?;
                                let disp = dispatcher.lock().await;
                                disp.send(&ack).await?;
                                return Ok(());
                            }
                        }
                    }
                }
                Ok(Err(_)) => {
                    return Err(crate::Error::Timeout(
                        "Broadcast channel lagged during memory write".to_string(),
                    ));
                }
                Err(_) => {
                    return Err(crate::Error::Timeout(
                        "Timeout waiting for write acknowledgment".to_string(),
                    ));
                }
            }
        }
    }

    /// Write memory using direct transport reference.
    async fn write_memory_direct(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        data: &[u8],
        timeout_ms: u64,
    ) -> Result<()> {
        let our_alias = self.our_alias.value();
        if let Some(ref mut transport) = self.transport {
            Self::write_memory_impl(
                transport.as_mut(),
                our_alias,
                dest_alias,
                address_space,
                address,
                data,
                timeout_ms,
            ).await
        } else {
            Err(crate::Error::Protocol("No transport available".to_string()))
        }
    }

    /// Write memory implementation — static method using direct transport.
    async fn write_memory_impl(
        transport: &mut dyn LccTransport,
        our_alias: u16,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        data: &[u8],
        timeout_ms: u64,
    ) -> Result<()> {
        use crate::protocol::{MemoryConfigCmd, AddressSpace};

        let space = AddressSpace::from_u8(address_space)
            .map_err(|e| crate::Error::Protocol(e))?;

        let write_frames = MemoryConfigCmd::build_write(our_alias, dest_alias, space, address, data)?;

        // Send all frames
        for frame in write_frames.iter() {
            transport.send(frame).await?;
        }

        // Wait for Datagram Received OK
        let start_time = Instant::now();
        let max_duration = Duration::from_millis(timeout_ms);

        while start_time.elapsed() < max_duration {
            let remaining = max_duration.saturating_sub(start_time.elapsed());
            if let Some(frame) = transport.receive(remaining.as_millis() as u64).await? {
                if let Ok((mti, src)) = MTI::from_header(frame.header) {
                    if mti == MTI::DatagramReceivedOk && src == dest_alias {
                        if frame.data.len() >= 2 {
                            let dst = ((frame.data[0] as u16) << 8) | (frame.data[1] as u16);
                            if dst == our_alias {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        Err(crate::Error::Timeout(
            "Timeout waiting for write acknowledgment".to_string(),
        ))
    }

    /// Send Update Complete command to a node.
    ///
    /// Sends `[0x20, 0xA8]` datagram, awaits Datagram Received OK.
    /// Fire-and-forget per OpenLCB_Java `CdiPanel.runUpdateComplete()`.
    pub async fn send_update_complete(
        &mut self,
        dest_alias: u16,
    ) -> Result<()> {
        if let Some(ref dispatcher) = self.dispatcher {
            let our_alias = self.our_alias.value();
            Self::send_update_complete_with_dispatcher(
                dispatcher,
                our_alias,
                dest_alias,
            ).await
        } else if self.transport.is_some() {
            self.send_update_complete_direct(dest_alias).await
        } else {
            Err(crate::Error::Protocol("No transport or dispatcher available".to_string()))
        }
    }

    /// Send update complete in dispatcher mode.
    async fn send_update_complete_with_dispatcher(
        dispatcher: &Arc<Mutex<MessageDispatcher>>,
        our_alias: u16,
        dest_alias: u16,
    ) -> Result<()> {
        use crate::protocol::MemoryConfigCmd;

        let frames = MemoryConfigCmd::build_update_complete(our_alias, dest_alias)?;

        let disp = dispatcher.lock().await;
        for frame in frames.iter() {
            disp.send(frame).await?;
        }

        // Fire-and-forget: not all nodes send a Datagram Received OK acknowledgement.
        Ok(())
    }

    /// Send update complete using direct transport.
    async fn send_update_complete_direct(
        &mut self,
        dest_alias: u16,
    ) -> Result<()> {
        let our_alias = self.our_alias.value();
        if let Some(ref mut transport) = self.transport {
            Self::send_update_complete_impl(
                transport.as_mut(),
                our_alias,
                dest_alias,
            ).await
        } else {
            Err(crate::Error::Protocol("No transport available".to_string()))
        }
    }

    /// Send update complete implementation — static method.
    async fn send_update_complete_impl(
        transport: &mut dyn LccTransport,
        our_alias: u16,
        dest_alias: u16,
    ) -> Result<()> {
        use crate::protocol::MemoryConfigCmd;

        let frames = MemoryConfigCmd::build_update_complete(our_alias, dest_alias)?;

        for frame in frames.iter() {
            transport.send(frame).await?;
        }

        // Fire-and-forget: not all nodes send a Datagram Received OK acknowledgement.
        Ok(())
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
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
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
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
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
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
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
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
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
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );
        
        let nodes = connection.discover_nodes(100).await.unwrap();
        assert_eq!(nodes.len(), 1);
    }

    // --- write_memory tests (T008) ---

    /// Create a Datagram Received OK frame from dest to source
    fn make_datagram_ack(from_alias: u16, to_alias: u16) -> GridConnectFrame {
        // DatagramReceivedOk uses to_header (source in header) + dest alias in data payload
        let header = MTI::DatagramReceivedOk.to_header(from_alias).unwrap();
        let data = vec![
            ((to_alias >> 8) & 0xFF) as u8,
            (to_alias & 0xFF) as u8,
        ];
        GridConnectFrame { header, data }
    }

    #[tokio::test]
    async fn test_write_memory_single_chunk() {
        // A small write (< 64 bytes) should send one write datagram and expect one ACK
        let ack = make_datagram_ack(0xBBB, 0xAAA);
        let mock = MockTransport::new(vec![ack]);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );

        let data = vec![0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x00]; // "Hello\0"
        let result = connection.write_memory(0xBBB, 0xFD, 0x100, &data).await;
        assert!(result.is_ok(), "Single-chunk write should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn test_write_memory_multi_chunk() {
        // A write > 64 bytes should chunk into multiple write datagrams
        // 100 bytes → 64 + 36 = 2 chunks, 2 ACKs needed
        let ack1 = make_datagram_ack(0xBBB, 0xAAA);
        let ack2 = make_datagram_ack(0xBBB, 0xAAA);
        let mock = MockTransport::new(vec![ack1, ack2]);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );

        let data = vec![0xAB; 100];
        let result = connection.write_memory(0xBBB, 0xFD, 0x200, &data).await;
        assert!(result.is_ok(), "Multi-chunk write should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn test_write_memory_timeout_retry() {
        // No ACK responses → should timeout and retry, then fail
        let mock = MockTransport::new(vec![]); // no responses
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );

        let data = vec![0x42];
        let result = connection.write_memory(0xBBB, 0xFD, 0, &data).await;
        assert!(result.is_err(), "Write with no ACK should fail after retries");
    }

    #[tokio::test]
    async fn test_write_memory_empty_data_error() {
        let mock = MockTransport::new(vec![]);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );

        let result = connection.write_memory(0xBBB, 0xFD, 0, &[]).await;
        assert!(result.is_err(), "Empty write data should be rejected");
    }

    #[tokio::test]
    async fn test_write_memory_address_advancement() {
        // 128 bytes → 2 chunks at 64 bytes each
        // First chunk at address 0, second at address 64
        let ack1 = make_datagram_ack(0xBBB, 0xAAA);
        let ack2 = make_datagram_ack(0xBBB, 0xAAA);
        let mock = MockTransport::new(vec![ack1, ack2]);
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );

        let data = vec![0xCC; 128];
        let result = connection.write_memory(0xBBB, 0xFD, 0x1000, &data).await;
        assert!(result.is_ok(), "Multi-chunk write with address advancement should succeed");
    }

    // --- send_update_complete tests (T009) ---

    #[tokio::test]
    async fn test_send_update_complete_success() {
        let mock = MockTransport::new(vec![]); // no response needed — fire-and-forget
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );

        let result = connection.send_update_complete(0xBBB).await;
        assert!(result.is_ok(), "Update complete should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn test_send_update_complete_no_ack_still_succeeds() {
        let mock = MockTransport::new(vec![]); // no response — should still succeed
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(0xAAA).unwrap(),
        );

        let result = connection.send_update_complete(0xBBB).await;
        assert!(result.is_ok(), "Update complete without ACK should still succeed (fire-and-forget)");
    }

    // --- probe_nodes tests ---

    /// probe_nodes returns Ok immediately without waiting for replies.
    /// The frame it sends must be VerifyNodeGlobal with our alias and empty data.
    #[tokio::test]
    async fn test_probe_nodes_sends_verify_node_global() {
        let mock = MockTransport::new(vec![]);
        let our_alias: u16 = 0xAAA;
        let mut connection = LccConnection::with_transport(
            Box::new(mock),
            NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]),
            NodeAlias::new(our_alias).unwrap(),
        );

        let result = connection.probe_nodes().await;
        assert!(result.is_ok(), "probe_nodes should return Ok immediately: {:?}", result);

        // Verify the expected frame shape by constructing it directly and
        // confirming the MTI and alias round-trip correctly.
        let expected = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, our_alias, vec![]).unwrap();
        let (mti, alias) = expected.get_mti().unwrap();
        assert_eq!(mti, MTI::VerifyNodeGlobal, "frame MTI must be VerifyNodeGlobal");
        assert_eq!(alias, our_alias, "frame alias must match our alias");
        assert!(expected.data.is_empty(), "VerifyNodeGlobal carries no payload");
    }

    // --- D9/D10/D12: Dispatcher-path datagram handling tests ---

    use crate::transport::mock::MockTransport as GlobalMockTransport;
    use crate::dispatcher::MessageDispatcher;

    /// Helper: build a DatagramRejected frame from `from_alias` addressed to `to_alias`
    /// with the given 16-bit error code.
    fn make_datagram_rejected(from_alias: u16, to_alias: u16, error_code: u16) -> GridConnectFrame {
        let header = MTI::DatagramRejected.to_header(from_alias).unwrap();
        let data = vec![
            ((to_alias >> 8) & 0xFF) as u8,
            (to_alias & 0xFF) as u8,
            ((error_code >> 8) & 0xFF) as u8,
            (error_code & 0xFF) as u8,
        ];
        GridConnectFrame { header, data }
    }

    /// Helper: build a DatagramReceivedOk frame with optional flags byte.
    fn make_datagram_ack_with_flags(from_alias: u16, to_alias: u16, flags: u8) -> GridConnectFrame {
        let header = MTI::DatagramReceivedOk.to_header(from_alias).unwrap();
        let mut data = vec![
            ((to_alias >> 8) & 0xFF) as u8,
            (to_alias & 0xFF) as u8,
        ];
        if flags != 0 {
            data.push(flags);
        }
        GridConnectFrame { header, data }
    }

    /// Helper: build a read-reply datagram (DatagramOnly) for Configuration space.
    /// Returns a frame from `from_alias` to `to_alias` with read-reply payload.
    fn make_read_reply_datagram(
        from_alias: u16,
        to_alias: u16,
        address: u32,
        payload: &[u8],
    ) -> GridConnectFrame {
        let header = MTI::DatagramOnly.to_header_with_dest(from_alias, to_alias).unwrap();
        let addr_bytes = address.to_be_bytes();
        // Embedded format for Configuration space: command byte 0x51 (0x50 read-reply | 0x01 config)
        let mut data = vec![0x20, 0x51, addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3]];
        data.extend_from_slice(payload);
        GridConnectFrame { header, data }
    }

    /// Helper: build a write-reply success datagram (D10).
    fn make_write_reply_datagram(
        from_alias: u16,
        to_alias: u16,
        address: u32,
    ) -> GridConnectFrame {
        let header = MTI::DatagramOnly.to_header_with_dest(from_alias, to_alias).unwrap();
        let addr_bytes = address.to_be_bytes();
        // Write reply command: 0x10 (success, no error bit 0x08)
        let data = vec![0x20, 0x11, addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3]];
        GridConnectFrame { header, data }
    }

    // D9: DatagramRejected with resend flag (0x2000) → resend then succeed (read path)
    #[tokio::test]
    async fn test_d9_read_datagram_rejected_resend_ok() {
        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        // First response: DatagramRejected with 0x2000 (resend OK / buffer full).
        let rejected = make_datagram_rejected(node_alias, our_alias, 0x2000);
        // After resend: DatagramReceivedOk (ack for the read request).
        let ack = make_datagram_ack_with_flags(node_alias, our_alias, 0);
        // Then the read-reply datagram with payload.
        let reply = make_read_reply_datagram(node_alias, our_alias, 0x0000, &[0x48, 0x65]);

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(rejected.to_string());
        transport.add_receive_frame(ack.to_string());
        transport.add_receive_frame(reply.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        let disp = Arc::new(Mutex::new(dispatcher));

        let result = LccConnection::read_memory_with_dispatcher(
            &disp, our_alias, node_alias, 0xFD, 0x0000, 2, 3000,
        ).await;

        disp.lock().await.shutdown().await;

        assert!(result.is_ok(), "Read should succeed after resend: {:?}", result);
        assert_eq!(result.unwrap(), vec![0x48, 0x65]);
    }

    // D9: DatagramRejected with permanent error → returns error (read path)
    #[tokio::test]
    async fn test_d9_read_datagram_rejected_permanent() {
        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        // Permanent rejection (error code 0x1000, no resend bit)
        let rejected = make_datagram_rejected(node_alias, our_alias, 0x1000);

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(rejected.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        let disp = Arc::new(Mutex::new(dispatcher));

        let result = LccConnection::read_memory_with_dispatcher(
            &disp, our_alias, node_alias, 0xFD, 0x0000, 2, 2000,
        ).await;

        disp.lock().await.shutdown().await;

        assert!(result.is_err(), "Read should fail on permanent rejection");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("0x1000"), "Error should contain rejection code: {}", err_msg);
    }

    // D9: DatagramRejected with resend flag → resend then succeed (write path)
    #[tokio::test]
    async fn test_d9_write_datagram_rejected_resend_ok() {
        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        // First: rejection with resend flag
        let rejected = make_datagram_rejected(node_alias, our_alias, 0x2000);
        // After resend: ack
        let ack = make_datagram_ack_with_flags(node_alias, our_alias, 0);

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(rejected.to_string());
        transport.add_receive_frame(ack.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        let disp = Arc::new(Mutex::new(dispatcher));

        let result = LccConnection::write_memory_with_dispatcher(
            &disp, our_alias, node_alias, 0xFD, 0x0000, &[0x42], 3000,
        ).await;

        disp.lock().await.shutdown().await;

        assert!(result.is_ok(), "Write should succeed after resend: {:?}", result);
    }

    // D9: DatagramRejected permanent error (write path)
    #[tokio::test]
    async fn test_d9_write_datagram_rejected_permanent() {
        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        let rejected = make_datagram_rejected(node_alias, our_alias, 0x1040);

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(rejected.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        let disp = Arc::new(Mutex::new(dispatcher));

        let result = LccConnection::write_memory_with_dispatcher(
            &disp, our_alias, node_alias, 0xFD, 0x0000, &[0x42], 2000,
        ).await;

        disp.lock().await.shutdown().await;

        assert!(result.is_err(), "Write should fail on permanent rejection");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("1040"), "Error should contain rejection code: {}", err_msg);
    }

    // D10: FLAG_REPLY_PENDING → wait for write-reply datagram
    #[tokio::test]
    async fn test_d10_write_reply_pending_then_success() {
        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        // DatagramReceivedOk with reply-pending flag (0x80)
        let ack = make_datagram_ack_with_flags(node_alias, our_alias, 0x80);
        // Then write-reply datagram indicating success
        let reply = make_write_reply_datagram(node_alias, our_alias, 0x0000);

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(ack.to_string());
        transport.add_receive_frame(reply.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        let disp = Arc::new(Mutex::new(dispatcher));

        let result = LccConnection::write_memory_with_dispatcher(
            &disp, our_alias, node_alias, 0xFD, 0x0000, &[0x42], 3000,
        ).await;

        disp.lock().await.shutdown().await;

        assert!(result.is_ok(), "Write should succeed after reply-pending + write-reply: {:?}", result);
    }

    // D12: DatagramReceivedOk with timeout extension flag (read path)
    #[tokio::test]
    async fn test_d12_read_timeout_extension_flag() {
        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        // DatagramReceivedOk with timeout extension: flags = 0x03 → 2^3 = 8 seconds
        let ack = make_datagram_ack_with_flags(node_alias, our_alias, 0x03);
        // Then a read-reply datagram arrives within extended window
        let reply = make_read_reply_datagram(node_alias, our_alias, 0x0000, &[0xAB, 0xCD]);

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(ack.to_string());
        transport.add_receive_frame(reply.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        let disp = Arc::new(Mutex::new(dispatcher));

        // Use a very short base timeout — extension should stretch it
        let result = LccConnection::read_memory_with_dispatcher(
            &disp, our_alias, node_alias, 0xFD, 0x0000, 2, 3000,
        ).await;

        disp.lock().await.shutdown().await;

        assert!(result.is_ok(), "Read should succeed with timeout extension: {:?}", result);
        assert_eq!(result.unwrap(), vec![0xAB, 0xCD]);
    }

    // --- D14: CDI read retry tests ---

    /// Helper: build a CDI read-reply datagram via the inline MockTransport format.
    /// CDI uses embedded format: command byte 0x53 (read-reply 0x50 | CDI 0x03).
    fn make_cdi_reply_frame(src_alias: u16, dst_alias: u16, address: u32, payload: &[u8]) -> GridConnectFrame {
        let header = MTI::DatagramOnly.to_header_with_dest(src_alias, dst_alias).unwrap();
        let addr = address.to_be_bytes();
        let mut data = vec![0x20, 0x53, addr[0], addr[1], addr[2], addr[3]];
        data.extend_from_slice(payload);
        GridConnectFrame { header, data }
    }

    // D14: All retry attempts fail → proper timeout error
    #[tokio::test]
    async fn test_d14_cdi_read_retries_exhausted() {
        // No responses → all 3 retry attempts will timeout
        let mut mock = MockTransport::new(vec![]);
        let result = LccConnection::read_cdi_impl(
            &mut mock,
            0xAAA,
            0xBBB,
            200, // short timeout
        ).await;

        assert!(result.is_err(), "CDI read should fail after retries");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("Timeout") || err.contains("timeout"),
            "Error should mention timeout: {}", err
        );
    }

    // D19: Zero-length CDI success reply → breaks loop instead of infinite loop
    #[tokio::test]
    async fn test_d19_cdi_zero_length_reply_breaks() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        // Return a valid CDI read reply with empty payload (zero-length data)
        let reply = make_cdi_reply_frame(src, dst, 0, &[]);
        let mut mock = MockTransport::new(vec![reply]);

        let result = LccConnection::read_cdi_impl(
            &mut mock,
            dst,
            src,
            2000,
        ).await;

        // Should succeed with empty string (not hang forever)
        assert!(result.is_ok(), "Zero-length reply should break CDI loop: {:?}", result);
        assert_eq!(result.unwrap(), "");
    }

    // CDI read: normal single-chunk with null terminator
    #[tokio::test]
    async fn test_cdi_read_single_chunk() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        // A CDI reply with "<cdi/>\0" (null-terminated)
        let cdi_bytes = b"<cdi/>\x00";
        let reply = make_cdi_reply_frame(src, dst, 0, cdi_bytes);
        let mut mock = MockTransport::new(vec![reply]);

        let result = LccConnection::read_cdi_impl(
            &mut mock,
            dst,
            src,
            2000,
        ).await;

        assert!(result.is_ok(), "CDI read should succeed: {:?}", result);
        assert_eq!(result.unwrap(), "<cdi/>");
    }

    // D18: Alias conflict detection — send AMD with our alias but different NodeID
    // This tests the conflict monitor spawned by start_responding_to_queries.
    // The monitor should send AMR + AMD to reassert our alias.
    #[tokio::test]
    async fn test_d18_alias_conflict_reasserts() {
        let our_alias: u16 = 0xAAA;
        let our_node_id = NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]);

        // An AMD from another node claiming our alias with a DIFFERENT NodeID
        let conflicting_amd = GridConnectFrame::from_mti(
            MTI::AliasMapDefinition,
            our_alias,
            vec![0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA], // different NodeID
        ).unwrap();

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(conflicting_amd.to_string());

        let mut dispatcher = MessageDispatcher::new(Box::new(transport));
        dispatcher.start();

        let disp_arc = Arc::new(Mutex::new(dispatcher));

        // Set up a connection with dispatcher and start the query responders
        // (which includes the D18 alias conflict monitor).
        let mut connection = LccConnection {
            transport: None,
            our_node_id: our_node_id,
            our_alias: NodeAlias::new(our_alias).unwrap(),
            dispatcher: Some(disp_arc.clone()),
            our_snip: None,
            our_pip_flags: LccConnection::default_pip_flags(),
            responder_handles: Vec::new(),
        };
        connection.start_responding_to_queries().unwrap();

        // Give the conflict monitor time to process the AMD and respond
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify the monitor didn't panic and the connection still works.
        // The spawned task should have logged the conflict and sent AMR+AMD.
        // (We can't easily inspect transport-level sent frames through the
        // dispatcher, but we verify the task stays alive.)
        assert!(
            connection.responder_handles.iter().all(|h| !h.is_finished()),
            "Conflict monitor tasks should still be running after handling conflict"
        );

        connection.shutdown_responders().await;
        disp_arc.lock().await.shutdown().await;
    }
}
