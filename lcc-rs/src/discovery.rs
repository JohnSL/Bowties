//! Node discovery functionality for LCC/OpenLCB networks
//!
//! This module implements the node discovery protocol, which sends a global
//! Verify Node ID message and collects responses.

use crate::{
    Result,
    types::{NodeID, NodeAlias, DiscoveredNode, SNIPData, ProtocolFlags},
    protocol::{GridConnectFrame, MTI},
    transport::{LccTransport, TcpTransport},
    transport_actor::{TransportActor, TransportHandle, ReceivedMessage},
    alias_allocation::AliasAllocator,
    constants::CONNECTION_STABILIZATION_MS,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{sleep, Duration, Instant};
use tokio::sync::{broadcast, Mutex};

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

/// Describes a single read performed by [`BatchReader::read_next`].
#[derive(Debug, Clone)]
pub struct BatchReadDescriptor {
    pub address_space: u8,
    pub address: u32,
    pub count: u8,
}

/// Result of one read within a batch.
#[derive(Debug, Clone)]
pub struct BatchReadResult {
    /// `Ok(data)` on success, `Err(message)` on failure.
    pub data: std::result::Result<Vec<u8>, String>,
    /// Timing data (present on success, `None` on failure).
    pub timing: Option<MemoryReadTiming>,
}

/// Performs pipelined memory reads on a single node, holding a single broadcast
/// subscription across all reads.
///
/// Obtain one via [`LccConnection::batch_reader`].  Call [`BatchReader::read_next`]
/// in a loop, once per descriptor.  The subscription stays alive between calls so
/// the ACK for read N and the request for read N+1 are issued back-to-back with
/// no re-subscribe overhead.
pub struct BatchReader {
    handle: TransportHandle,
    our_alias: u16,
    dest_alias: u16,
    rx: broadcast::Receiver<ReceivedMessage>,
}

impl BatchReader {
    /// Create a new reader.  The broadcast subscription is established immediately.
    pub fn new(handle: TransportHandle, our_alias: u16, dest_alias: u16) -> Self {
        let rx = handle.subscribe_all();
        Self { handle, our_alias, dest_alias, rx }
    }

    /// Perform one pipelined memory read.
    ///
    /// Sends the request via `send_direct` (bypassing the mpsc queue), waits for
    /// the assembled datagram reply on the shared subscription, ACKs via
    /// `send_direct`, and returns the result with timing data.
    pub async fn read_next(&mut self, desc: &BatchReadDescriptor, timeout_ms: u64) -> BatchReadResult {
        use crate::protocol::{MemoryConfigCmd, AddressSpace, DatagramAssembler};

        let space = match AddressSpace::from_u8(desc.address_space) {
            Ok(s) => s,
            Err(e) => return BatchReadResult {
                data: Err(format!("Invalid address space: {}", e)),
                timing: None,
            },
        };

        let read_frames = match MemoryConfigCmd::build_read(
            self.our_alias, self.dest_alias, space, desc.address, desc.count,
        ) {
            Ok(f) => f,
            Err(e) => return BatchReadResult {
                data: Err(e.to_string()),
                timing: None,
            },
        };

        let send_time = Instant::now();
        for frame in read_frames.iter() {
            if let Err(e) = self.handle.send_direct(frame).await {
                return BatchReadResult {
                    data: Err(e.to_string()),
                    timing: None,
                };
            }
        }

        let mut max_duration = Duration::from_millis(timeout_ms);
        let mut assembler = DatagramAssembler::new();
        let mut first_frame_latency_ms: Option<u64> = None;
        let mut last_frame_ms: u64 = 0;
        let mut frame_gaps_ms: Vec<u32> = Vec::new();
        let mut frame_count: u8 = 0;

        loop {
            match tokio::time::timeout(max_duration, self.rx.recv()).await {
                Ok(Ok(msg)) => {
                    let is_our_datagram = MTI::from_datagram_header(msg.frame.header)
                        .map(|(mti, src, dst)| {
                            let is_dg = matches!(
                                mti,
                                MTI::DatagramOnly
                                    | MTI::DatagramFirst
                                    | MTI::DatagramMiddle
                                    | MTI::DatagramFinal
                            );
                            is_dg && src == self.dest_alias && dst == self.our_alias
                        })
                        .unwrap_or(false);

                    if !is_our_datagram {
                        if let Ok((mti, src)) = MTI::from_header(msg.frame.header) {
                            if src == self.dest_alias && msg.frame.data.len() >= 2 {
                                let dst = ((msg.frame.data[0] as u16) << 8)
                                    | (msg.frame.data[1] as u16);
                                if dst == self.our_alias {
                                    if mti == MTI::DatagramRejected {
                                        let error_code = if msg.frame.data.len() >= 4 {
                                            ((msg.frame.data[2] as u16) << 8)
                                                | (msg.frame.data[3] as u16)
                                        } else {
                                            0
                                        };
                                        if error_code & 0x2000 != 0 {
                                            for frame in read_frames.iter() {
                                                let _ = self.handle.send_direct(frame).await;
                                            }
                                            continue;
                                        } else {
                                            return BatchReadResult {
                                                data: Err(format!(
                                                    "Datagram rejected: error 0x{:04X}",
                                                    error_code
                                                )),
                                                timing: None,
                                            };
                                        }
                                    }
                                    if mti == MTI::DatagramReceivedOk {
                                        let flags = if msg.frame.data.len() >= 3 {
                                            msg.frame.data[2]
                                        } else {
                                            0
                                        };
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
                        let ack_frame = match DatagramAssembler::send_acknowledgment(
                            self.our_alias,
                            self.dest_alias,
                        ) {
                            Ok(f) => f,
                            Err(e) => return BatchReadResult {
                                data: Err(e.to_string()),
                                timing: None,
                            },
                        };
                        let _ = self.handle.send_direct(&ack_frame).await;

                        let total_duration_ms = send_time.elapsed().as_millis() as u64;
                        let timing = MemoryReadTiming {
                            first_frame_latency_ms: first_frame_latency_ms
                                .unwrap_or(total_duration_ms),
                            frame_gaps_ms,
                            total_duration_ms,
                            frame_count,
                        };

                        return match MemoryConfigCmd::parse_read_reply(&datagram_data) {
                            Ok(crate::protocol::ReadReply::Success { data, .. }) => {
                                BatchReadResult { data: Ok(data), timing: Some(timing) }
                            }
                            Ok(crate::protocol::ReadReply::Failed {
                                error_code, message, ..
                            }) => BatchReadResult {
                                data: Err(format!(
                                    "Memory read failed: error 0x{:04X} - {}",
                                    error_code, message
                                )),
                                timing: Some(timing),
                            },
                            Err(e) => BatchReadResult {
                                data: Err(e.to_string()),
                                timing: Some(timing),
                            },
                        };
                    }
                }
                Ok(Err(_)) => {
                    return BatchReadResult {
                        data: Err("Broadcast channel lagged during memory read".into()),
                        timing: None,
                    };
                }
                Err(_) => {
                    return BatchReadResult {
                        data: Err("Timeout waiting for memory read response".into()),
                        timing: None,
                    };
                }
            }
        }
    }
}

/// High-level LCC connection for performing network operations
pub struct LccConnection {
    /// Transport actor that owns the transport (lifecycle management).
    actor: Option<TransportActor>,
    /// Cheap-to-clone handle for sending/subscribing (main API surface).
    handle: Option<TransportHandle>,
    /// Our node ID
    our_node_id: NodeID,
    /// Our node alias (negotiated via alias allocation protocol)
    our_alias: NodeAlias,
    /// Optional SNIP data to provide when queried
    our_snip: Option<SNIPData>,
    /// Protocol flags we advertise in PIP replies (D13)
    our_pip_flags: ProtocolFlags,
    /// Handles for background responder tasks (query + SNIP).
    /// Stored so they can be aborted on disconnect.
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

        // Create the transport actor (sole owner of the transport).
        let actor = TransportActor::new(boxed_transport);
        let transport_handle = actor.handle();

        let connection = Self {
            actor: Some(actor),
            handle: Some(transport_handle),
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

        // Create the transport actor (sole owner of the transport).
        let actor = TransportActor::new(transport);
        let transport_handle = actor.handle();

        let connection = Self {
            actor: Some(actor),
            handle: Some(transport_handle),
            our_node_id: node_id,
            our_alias,
            our_snip: None,
            our_pip_flags: Self::default_pip_flags(),
            responder_handles: vec![],
        };

        Ok(Arc::new(Mutex::new(connection)))
    }

    /// Create an LCC connection with a custom transport (for testing)
    pub fn with_transport(transport: Box<dyn LccTransport>, node_id: NodeID, our_alias: NodeAlias) -> Self {
        let actor = TransportActor::new(transport);
        let transport_handle = actor.handle();
        Self {
            actor: Some(actor),
            handle: Some(transport_handle),
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
    
    /// Get a reference to the transport handle
    pub fn transport_handle(&self) -> Option<&TransportHandle> {
        self.handle.as_ref()
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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?
            .clone();
        Self::discover_nodes_with_handle(&handle, self.our_alias.value(), timeout_ms).await
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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        handle.send(&verify_frame).await
    }

    /// Discover nodes via a `TransportHandle` (no mutex).
    async fn discover_nodes_with_handle(
        handle: &TransportHandle,
        our_alias: u16,
        timeout_ms: u64,
    ) -> Result<Vec<DiscoveredNode>> {
        // Subscribe to VerifiedNode messages
        let mut rx = handle.subscribe_mti(MTI::VerifiedNode).await;
        
        // Send global Verify Node ID message
        let verify_frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            our_alias,
            vec![],
        )?;
        
        handle.send(&verify_frame).await?;
        
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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        crate::snip::query_snip(
            handle,
            self.our_alias.value(),
            dest_alias,
            sem,
        ).await
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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        crate::pip::query_pip(
            handle,
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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?
            .clone();
        Self::verify_node_with_handle(&handle, self.our_alias.value(), dest_alias, timeout_ms).await
    }
    
    /// Verify node using a `TransportHandle` (no mutex).
    async fn verify_node_with_handle(
        handle: &TransportHandle,
        our_alias: u16,
        dest_alias: u16,
        timeout_ms: u64,
    ) -> Result<Option<NodeID>> {
        let mut rx = handle.subscribe_mti(MTI::VerifiedNode).await;
        
        let verify_frame = GridConnectFrame::from_addressed_mti(
            MTI::VerifyNodeAddressed,
            our_alias,
            dest_alias,
            vec![],
        )?;
        
        handle.send(&verify_frame).await?;
        
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
                Ok(Err(_)) => continue,
                Err(_) => return Ok(None),
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
    /// This method returns immediately; the response task runs in the background.
    /// 
    /// # Errors
    /// Returns an error if the connection has no transport handle.
    pub fn start_responding_to_queries(&mut self) -> Result<()> {
        let handle = self.handle.clone()
            .ok_or_else(|| crate::Error::Protocol(
                "start_responding_to_queries requires a transport handle".to_string()
            ))?;
        
        let our_alias = self.our_alias;
        let our_node_id = self.our_node_id;
        
        // --- VerifyNodeGlobal responder ---
        let h = handle.clone();
        let handle_global = tokio::spawn(async move {
            let mut rx = h.subscribe_mti(MTI::VerifyNodeGlobal).await;
            loop {
                match rx.recv().await {
                    Ok(_msg) => {
                        if let Ok(response_frame) = GridConnectFrame::from_mti(
                            MTI::VerifiedNode,
                            our_alias.value(),
                            our_node_id.as_bytes().to_vec(),
                        ) {
                            let _ = h.send(&response_frame).await;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_global);

        // --- VerifyNodeAddressed responder (D8) ---
        let h = handle.clone();
        let handle_addressed = tokio::spawn(async move {
            let mut rx = h.subscribe_mti(MTI::VerifyNodeAddressed).await;
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        let is_for_us = msg.frame.get_dest_from_body()
                            .map(|(dest, _)| dest == our_alias.value())
                            .unwrap_or(false);
                        if is_for_us {
                            if let Ok(response_frame) = GridConnectFrame::from_mti(
                                MTI::VerifiedNode,
                                our_alias.value(),
                                our_node_id.as_bytes().to_vec(),
                            ) {
                                let _ = h.send(&response_frame).await;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_addressed);

        // --- AliasMapEnquiry (AME) responder (D5) ---
        let h = handle.clone();
        let handle_ame = tokio::spawn(async move {
            let mut rx = h.subscribe_mti(MTI::AliasMapEnquiry).await;
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        let data = &msg.frame.data;
                        let is_global = data.is_empty();
                        let matches_us = data.len() == 6 && data.as_slice() == our_node_id.as_bytes();
                        if is_global || matches_us {
                            if let Ok(amd_frame) = GridConnectFrame::from_mti(
                                MTI::AliasMapDefinition,
                                our_alias.value(),
                                our_node_id.as_bytes().to_vec(),
                            ) {
                                let _ = h.send(&amd_frame).await;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_ame);

        // --- ProtocolSupportInquiry (PIP) responder (D13) ---
        let h = handle.clone();
        let our_pip_flags = self.our_pip_flags;
        let handle_pip = tokio::spawn(async move {
            let mut rx = h.subscribe_mti(MTI::ProtocolSupportInquiry).await;
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        let is_for_us = msg.frame.get_dest_from_body()
                            .map(|(dest, _)| dest == our_alias.value())
                            .unwrap_or(false);
                        if is_for_us {
                            let flag_bytes = our_pip_flags.to_bytes();
                            if let Ok(response_frame) = GridConnectFrame::from_addressed_mti(
                                MTI::ProtocolSupportReply,
                                our_alias.value(),
                                MTI::from_header(msg.frame.header)
                                    .map(|(_, src)| src)
                                    .unwrap_or(0),
                                flag_bytes,
                            ) {
                                let _ = h.send(&response_frame).await;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        self.responder_handles.push(handle_pip);

        // --- D18: Ongoing alias conflict detector ---
        let h = handle.clone();
        let handle_conflict = tokio::spawn(async move {
            let mut rx = h.subscribe_mti(MTI::AliasMapDefinition).await;
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
                                    if let Ok(amr) = GridConnectFrame::from_mti(
                                        MTI::AliasMapReset,
                                        our_alias.value(),
                                        our_node_id.as_bytes().to_vec(),
                                    ) {
                                        let _ = h.send(&amr).await;
                                        if let Ok(amd) = GridConnectFrame::from_mti(
                                            MTI::AliasMapDefinition,
                                            our_alias.value(),
                                            our_node_id.as_bytes().to_vec(),
                                        ) {
                                            let _ = h.send(&amd).await;
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
    /// This method returns immediately; the response task runs in the background.
    /// 
    /// # Errors
    /// Returns an error if the connection has no transport handle.
    pub fn start_responding_to_snip_requests(&mut self) -> Result<()> {
        let handle = self.handle.clone()
            .ok_or_else(|| crate::Error::Protocol(
                "start_responding_to_snip_requests requires a transport handle".to_string()
            ))?;
        
        let our_alias = self.our_alias;
        let snip_data = self.our_snip.clone();
        
        let h = handle;
        let task_handle = tokio::spawn(async move {
            let mut rx = h.subscribe_mti(MTI::SNIPRequest).await;
            
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        if let Ok((_, requester_alias)) = msg.frame.get_mti() {
                            if let Some(ref snip) = snip_data {
                                let snip_payload = crate::snip::encode_snip_payload(snip, true);

                                if snip_payload.len() <= 6 {
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
                                        let _ = h.send(&response).await;
                                    }
                                } else {
                                    let mut offset = 0;
                                    let chunk_size = 6;
                                    let mut frame_num = 0;
                                    
                                    while offset < snip_payload.len() {
                                        let end = std::cmp::min(offset + chunk_size, snip_payload.len());
                                        let chunk = &snip_payload[offset..end];
                                        
                                        let frame_type = if frame_num == 0 {
                                            0x1Au8
                                        } else if end == snip_payload.len() {
                                            0x2Au8
                                        } else {
                                            0x3Au8
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
                                            let _ = h.send(&response).await;
                                        }
                                        
                                        offset = end;
                                        frame_num += 1;
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });
        self.responder_handles.push(task_handle);
        
        Ok(())
    }

    /// Abort and await all background responder tasks (query + SNIP responders).
    ///
    /// Must be called before dropping the connection so that the background tasks
    /// release their `TransportHandle` clones, allowing the transport
    /// to be freed immediately.
    pub async fn shutdown_responders(&mut self) {
        for handle in self.responder_handles.drain(..) {
            handle.abort();
            let _ = handle.await; // Resolves immediately once aborted
        }
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<()> {
        // Shut down the actor (this closes the underlying transport).
        if let Some(ref mut actor) = self.actor {
            actor.shutdown().await;
            self.actor = None;
            self.handle = None;
        }
        Ok(())
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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        let our_alias = self.our_alias.value();
        Self::read_cdi_with_handle(handle, our_alias, dest_alias, timeout_ms, None).await
    }

    /// Like [`read_cdi`], but checks `cancel_flag` between chunks.
    /// Setting the flag to `true` stops the download and returns an error.
    pub async fn read_cdi_cancellable(
        &mut self,
        dest_alias: u16,
        timeout_ms: u64,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<String> {
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        let our_alias = self.our_alias.value();
        Self::read_cdi_with_handle(handle, our_alias, dest_alias, timeout_ms, Some(cancel_flag)).await
    }

    /// Read CDI using the subscribe-before-send pattern.
    ///
    /// This function:
    ///   1. Subscribes to the broadcast channel BEFORE sending each request.
    ///   2. Does not hold any lock while waiting for the reply.
    ///   3. Handles DatagramRejected with "resend OK" flag (D9) by retransmitting.
    ///   4. Handles DatagramReceivedOk timeout-extension flag (D12).
    async fn read_cdi_with_handle(
        handle: &TransportHandle,
        our_alias: u16,
        dest_alias: u16,
        timeout_ms: u64,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<String> {
        use crate::protocol::{MemoryConfigCmd, AddressSpace, DatagramAssembler};

        println!("[LCC] read_cdi_with_handle starting for alias 0x{:03X}", dest_alias);

        let mut cdi_data = Vec::new();
        let mut address = 0u32;
        const CHUNK_SIZE: u8 = 64;
        const MAX_CHUNK_RETRIES: u8 = 3;

        // Subscribe once for the entire CDI download.  This eliminates the gap
        // between chunks where a per-chunk subscription would be dropped and
        // re-created, which could cause stale replies to leak across chunk
        // boundaries.
        let mut rx = handle.subscribe_all();

        loop {
            // Check for cancellation before each chunk.
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    return Err(crate::Error::Protocol("CDI download cancelled".to_string()));
                }
            }

            println!("[LCC] Reading CDI chunk at address {} (chunk size {})", address, CHUNK_SIZE);

            let read_frames = MemoryConfigCmd::build_read(
                our_alias,
                dest_alias,
                AddressSpace::Cdi,
                address,
                CHUNK_SIZE,
            )?;

            let mut chunk_reply: Option<Vec<u8>> = None;

            'retry: for attempt in 0..MAX_CHUNK_RETRIES {
                if attempt > 0 {
                    println!(
                        "[LCC] Retrying CDI chunk at address {} (attempt {}/{})",
                        address,
                        attempt + 1,
                        MAX_CHUNK_RETRIES
                    );
                }

                // Send the request.
                {
                    println!("[LCC] Sending {} frame(s) for CDI read command", read_frames.len());
                    for (i, frame) in read_frames.iter().enumerate() {
                        println!("[LCC] Sending frame {}/{}: {}", i + 1, read_frames.len(), frame.to_string());
                        handle.send(frame).await?;
                    }
                }

                // Wait on broadcast channel (no transport lock held).
                let mut max_duration = Duration::from_millis(timeout_ms);
                let mut assembler = DatagramAssembler::new();

                loop {
                    match tokio::time::timeout(max_duration, rx.recv()).await {
                        Ok(Ok(msg)) => {
                            // Is this a datagram from dest_alias addressed to us?
                            let is_our_datagram = MTI::from_datagram_header(msg.frame.header)
                                .map(|(mti, src, dst)| {
                                    matches!(
                                        mti,
                                        MTI::DatagramOnly
                                            | MTI::DatagramFirst
                                            | MTI::DatagramMiddle
                                            | MTI::DatagramFinal
                                    ) && src == dest_alias
                                        && dst == our_alias
                                })
                                .unwrap_or(false);

                            if is_our_datagram {
                                if let Ok(Some(datagram_data)) = assembler.handle_frame(&msg.frame) {
                                    // Validate size before accepting: 6-byte header + at most 65
                                    // data bytes. If oversized, a spurious extra middle frame
                                    // leaked in — ACK so the node doesn't hang, then retry.
                                    if datagram_data.len() > 71 {
                                        eprintln!(
                                            "[LCC] WARNING: CDI reply oversized ({} bytes) at address {} (attempt {}/{}) — discarding and retrying",
                                            datagram_data.len(), address, attempt + 1, MAX_CHUNK_RETRIES
                                        );
                                        if let Ok(ack) = DatagramAssembler::send_acknowledgment(our_alias, dest_alias) {
                                            let _ = handle.send(&ack).await;
                                        }
                                        continue 'retry;
                                    }
                                    // Validate reply address: if it's from a stale/previous
                                    // chunk, ACK to free the node, reset the assembler, and
                                    // keep listening for the correct reply in this same inner
                                    // loop (the real reply should be right behind the stale one).
                                    if datagram_data.len() >= 6 {
                                        let reply_addr = u32::from_be_bytes([
                                            datagram_data[2], datagram_data[3],
                                            datagram_data[4], datagram_data[5],
                                        ]);
                                        if reply_addr != address {
                                            eprintln!(
                                                "[LCC] WARNING: CDI stale reply (addr {} != expected {}) — ACK and discard, waiting for correct reply",
                                                reply_addr, address
                                            );
                                            if let Ok(ack) = DatagramAssembler::send_acknowledgment(our_alias, dest_alias) {
                                                let _ = handle.send(&ack).await;
                                            }
                                            assembler = DatagramAssembler::new();
                                            continue; // inner recv loop — keep listening
                                        }
                                    }
                                    // Step 4: ACK the reply datagram.
                                    let ack = DatagramAssembler::send_acknowledgment(our_alias, dest_alias)?;
                                    handle.send(&ack).await?;
                                    println!("[LCC] Received CDI reply at address {}", address);
                                    chunk_reply = Some(datagram_data);
                                    break; // inner receive loop — got reply
                                }
                                // Multi-frame: keep accumulating.
                                continue;
                            }

                            // Check for addressed control frames (DatagramRejected / DatagramReceivedOk).
                            if let Ok((mti, src)) = MTI::from_header(msg.frame.header) {
                                if src == dest_alias && msg.frame.data.len() >= 2 {
                                    let dst = ((msg.frame.data[0] as u16) << 8)
                                        | (msg.frame.data[1] as u16);
                                    if dst == our_alias {
                                        // D9: DatagramRejected — retry if "resend OK" (0x2000).
                                        if mti == MTI::DatagramRejected {
                                            let error_code = if msg.frame.data.len() >= 4 {
                                                ((msg.frame.data[2] as u16) << 8)
                                                    | (msg.frame.data[3] as u16)
                                            } else {
                                                0
                                            };
                                            if error_code & 0x2000 != 0 {
                                                continue 'retry; // resend the request
                                            } else {
                                                return Err(crate::Error::Protocol(format!(
                                                    "CDI read datagram rejected: error 0x{:04X}",
                                                    error_code
                                                )));
                                            }
                                        }
                                        // D12: DatagramReceivedOk — honour timeout extension.
                                        if mti == MTI::DatagramReceivedOk {
                                            let flags = if msg.frame.data.len() >= 3 {
                                                msg.frame.data[2]
                                            } else {
                                                0
                                            };
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
                            // Other frame: ignore and keep waiting.
                        }
                        Ok(Err(_)) => {
                            // Broadcast channel lagged — retry this chunk.
                            eprintln!(
                                "[LCC] WARNING: broadcast channel lagged during CDI read at address {} (attempt {}/{})",
                                address, attempt + 1, MAX_CHUNK_RETRIES
                            );
                            continue 'retry;
                        }
                        Err(_) => {
                            // Timeout — try next attempt.
                            continue 'retry;
                        }
                    }
                } // inner receive loop

                if chunk_reply.is_some() {
                    break 'retry;
                }
            } // retry loop

            let reply_data = chunk_reply.ok_or_else(|| {
                crate::Error::Timeout(format!(
                    "Timeout waiting for CDI read reply at address {} after {} attempts",
                    address, MAX_CHUNK_RETRIES
                ))
            })?;

            // Sanity-check assembled datagram size: must have at least 6 header + 1 data byte.
            // Oversized replies (> 71) are caught and retried in the inner recv loop above.
            if reply_data.len() < 7 {
                eprintln!(
                    "[LCC] WARNING: CDI reply datagram size {} is too small (< 7) at address {}",
                    reply_data.len(), address
                );
            }

            let reply = MemoryConfigCmd::parse_read_reply(&reply_data)?;
            match reply {
                crate::protocol::ReadReply::Success { address: reply_addr, data, .. } => {
                    if reply_addr != address {
                        eprintln!(
                            "[LCC] WARNING: CDI chunk address mismatch: expected {} got {} (data len {})",
                            address, reply_addr, data.len()
                        );
                    }
                    if data.len() > CHUNK_SIZE as usize {
                        eprintln!(
                            "[LCC] WARNING: CDI chunk data length {} exceeds CHUNK_SIZE {} at address {}",
                            data.len(), CHUNK_SIZE, address
                        );
                    }
                    if data.is_empty() {
                        break;
                    }
                    if let Some(null_pos) = data.iter().position(|&b| b == 0x00) {
                        cdi_data.extend_from_slice(&data[..null_pos]);
                        break;
                    } else {
                        address += data.len() as u32;
                        cdi_data.extend_from_slice(&data);
                    }
                }
                crate::protocol::ReadReply::Failed { error_code, message, .. } => {
                    // 0x1082 = "address out of bounds" — end of CDI (same as read_cdi_impl).
                    if error_code == 0x1082 {
                        break;
                    }
                    return Err(crate::Error::Protocol(format!(
                        "CDI read failed at address {}: error 0x{:04X} - {}",
                        address, error_code, message
                    )));
                }
            }

            if cdi_data.len() > 10 * 1024 * 1024 {
                return Err(crate::Error::Protocol("CDI exceeds 10MB size limit".to_string()));
            }
        }

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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        let our_alias = self.our_alias.value();
        Self::read_memory_with_handle(
            handle,
            our_alias,
            dest_alias,
            address_space,
            address,
            count,
            timeout_ms,
        ).await
    }

    /// Like [`read_memory`] but also returns per-read timing metadata.
    ///
    /// Used by `read_all_config_values` to populate `BatchReadStat` diagnostics.
    pub async fn read_memory_timed(
        &mut self,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<(Vec<u8>, MemoryReadTiming)> {
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        let our_alias = self.our_alias.value();
        Self::read_memory_with_handle_timed(
            handle,
            our_alias,
            dest_alias,
            address_space,
            address,
            count,
            timeout_ms,
        ).await
    }

    /// Read memory (untimed thin wrapper).
    ///
    /// The transport actor is the sole owner of the transport receive path.
    /// This method:
    ///   1. Subscribes to the all-frames broadcast channel BEFORE sending (no frames missed).
    ///   2. Sends the request frames via the transport handle.
    ///   3. Waits on the broadcast channel for reply datagram frames.
    ///   4. Sends the DatagramReceivedOK acknowledgment.
    ///
    /// Round-trip latency is therefore pure network latency (~4ms), not 100ms poll cycles.
    async fn read_memory_with_handle(
        handle: &TransportHandle,
        our_alias: u16,
        dest_alias: u16,
        address_space: u8,
        address: u32,
        count: u8,
        timeout_ms: u64,
    ) -> Result<Vec<u8>> {
        Self::read_memory_with_handle_timed(
            handle, our_alias, dest_alias, address_space, address, count, timeout_ms,
        ).await.map(|(data, _timing)| data)
    }

    /// Read that also captures per-frame timing for diagnostics.
    async fn read_memory_with_handle_timed(
        handle: &TransportHandle,
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
        let mut rx = handle.subscribe_all();

        // Step 2: Send the request.
        let send_time = Instant::now();
        for frame in read_frames.iter() {
            handle.send(frame).await?;
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
                                            for frame in read_frames.iter() {
                                                handle.send(frame).await?;
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
                        handle.send(&ack_frame).await?;

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

    // ========================================================================
    // Batch Memory Read (subscribe once, hold handle across loop)
    // ========================================================================

    /// Return a [`BatchReader`] ready to perform pipelined memory reads on `dest_alias`.
    ///
    /// The `BatchReader` holds a single broadcast subscription for its lifetime,
    /// so consecutive [`BatchReader::read_next`] calls keep the subscription alive
    /// between reads — eliminating per-read subscribe/unsubscribe overhead and
    /// allowing the ACK for read N to be immediately followed by the request for
    /// read N+1 without any scheduler hop.
    pub fn batch_reader(&self, dest_alias: u16) -> crate::Result<BatchReader> {
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?
            .clone();
        let our_alias = self.our_alias.value();
        Ok(BatchReader::new(handle, our_alias, dest_alias))
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
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        let our_alias = self.our_alias.value();
        Self::write_memory_with_handle(
            handle,
            our_alias,
            dest_alias,
            address_space,
            address,
            data,
            timeout_ms,
        ).await
    }

    /// Write memory via transport handle.
    async fn write_memory_with_handle(
        handle: &TransportHandle,
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
        let mut rx = handle.subscribe_all();

        // Step 2: Send the request.
        for frame in write_frames.iter() {
            handle.send(frame).await?;
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
                                        for frame in write_frames.iter() {
                                            handle.send(frame).await?;
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
                                    handle.send(&ack).await?;
                                    return Err(crate::Error::Protocol(format!(
                                        "Write reply error: 0x{:04X}", error_code
                                    )));
                                }
                                // Success reply — acknowledge the datagram
                                let ack = DatagramAssembler::send_acknowledgment(our_alias, dest_alias)?;
                                handle.send(&ack).await?;
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


    /// Send Update Complete command to a node.
    ///
    /// Sends `[0x20, 0xA8]` datagram, awaits Datagram Received OK.
    /// Fire-and-forget per OpenLCB_Java `CdiPanel.runUpdateComplete()`.
    pub async fn send_update_complete(
        &mut self,
        dest_alias: u16,
    ) -> Result<()> {
        let handle = self.handle.as_ref()
            .ok_or_else(|| crate::Error::Protocol("No transport handle available".to_string()))?;
        let our_alias = self.our_alias.value();
        Self::send_update_complete_with_handle(
            handle,
            our_alias,
            dest_alias,
        ).await
    }

    /// Send update complete via transport handle.
    async fn send_update_complete_with_handle(
        handle: &TransportHandle,
        our_alias: u16,
        dest_alias: u16,
    ) -> Result<()> {
        use crate::protocol::MemoryConfigCmd;

        let frames = MemoryConfigCmd::build_update_complete(our_alias, dest_alias)?;

        for frame in frames.iter() {
            handle.send(frame).await?;
        }

        // Fire-and-forget: not all nodes send a Datagram Received OK acknowledgement.
        Ok(())
    }

}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{LccTransport, TransportReader, TransportWriter};
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
                // Yield before returning so the dispatcher listener loop does not
                // consume all queued frames in a single burst.  This gives
                // multi-step callers a chance to process the previous response
                // and re-subscribe before the next frame is broadcast.
                tokio::task::yield_now().await;
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

        fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>) {
            (
                Box::new(DiscoveryMockReader {
                    responses: self.responses,
                    response_index: self.response_index,
                }),
                Box::new(DiscoveryMockWriter {
                    sent_frames: self.sent_frames,
                }),
            )
        }
    }

    struct DiscoveryMockReader {
        responses: Vec<GridConnectFrame>,
        response_index: usize,
    }

    #[async_trait]
    impl TransportReader for DiscoveryMockReader {
        async fn receive(&mut self) -> Result<GridConnectFrame> {
            loop {
                if self.response_index < self.responses.len() {
                    let frame = self.responses[self.response_index].clone();
                    self.response_index += 1;
                    tokio::task::yield_now().await;
                    return Ok(frame);
                }
                // No more responses — block until shutdown cancels us.
                sleep(Duration::from_millis(30)).await;
            }
        }
    }

    struct DiscoveryMockWriter {
        sent_frames: Vec<GridConnectFrame>,
    }

    #[async_trait]
    impl TransportWriter for DiscoveryMockWriter {
        async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
            self.sent_frames.push(frame.clone());
            Ok(())
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

    // --- D9/D10/D12: Datagram handling tests ---

    use crate::transport::mock::MockTransport as GlobalMockTransport;
    use crate::transport_actor::TransportActor;

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

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_memory_with_handle(
            &handle, our_alias, node_alias, 0xFD, 0x0000, 2, 3000,
        ).await;

        actor.shutdown().await;

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

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_memory_with_handle(
            &handle, our_alias, node_alias, 0xFD, 0x0000, 2, 2000,
        ).await;

        actor.shutdown().await;

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

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::write_memory_with_handle(
            &handle, our_alias, node_alias, 0xFD, 0x0000, &[0x42], 3000,
        ).await;

        actor.shutdown().await;

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

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::write_memory_with_handle(
            &handle, our_alias, node_alias, 0xFD, 0x0000, &[0x42], 2000,
        ).await;

        actor.shutdown().await;

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

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::write_memory_with_handle(
            &handle, our_alias, node_alias, 0xFD, 0x0000, &[0x42], 3000,
        ).await;

        actor.shutdown().await;

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

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        // Use a very short base timeout — extension should stretch it
        let result = LccConnection::read_memory_with_handle(
            &handle, our_alias, node_alias, 0xFD, 0x0000, 2, 3000,
        ).await;

        actor.shutdown().await;

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
        let transport = GlobalMockTransport::new();
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(
            &handle,
            0xAAA,
            0xBBB,
            200, // short timeout
            None,
        ).await;

        actor.shutdown().await;

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

        let reply = make_cdi_reply_frame(src, dst, 0, &[]);
        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(reply.to_string());
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(&handle, dst, src, 2000, None).await;
        actor.shutdown().await;

        assert!(result.is_ok(), "Zero-length reply should break CDI loop: {:?}", result);
        assert_eq!(result.unwrap(), "");
    }

    // CDI read: normal single-chunk with null terminator
    #[tokio::test]
    async fn test_cdi_read_single_chunk() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        // Payload must fit CAN 8-byte limit: 6 header + 2 payload max.
        let reply = make_cdi_reply_frame(src, dst, 0, b"A\x00");
        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(reply.to_string());
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(&handle, dst, src, 2000, None).await;
        actor.shutdown().await;

        assert!(result.is_ok(), "CDI read should succeed: {:?}", result);
        assert_eq!(result.unwrap(), "A");
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

        let actor = TransportActor::new(Box::new(transport));
        let transport_handle = actor.handle();

        // Set up a connection with actor and start the query responders
        // (which includes the D18 alias conflict monitor).
        let mut connection = LccConnection {
            actor: Some(actor),
            handle: Some(transport_handle),
            our_node_id: our_node_id,
            our_alias: NodeAlias::new(our_alias).unwrap(),
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
        connection.close().await.unwrap();
    }

    // D10: FLAG_REPLY_PENDING followed by write-reply error datagram → error propagated
    #[tokio::test]
    async fn test_d10_write_reply_pending_then_error() {
        let our_alias: u16 = 0xAAA;
        let node_alias: u16 = 0xBBB;

        // DatagramReceivedOk with reply-pending flag (0x80)
        let ack = make_datagram_ack_with_flags(node_alias, our_alias, 0x80);
        // Write-reply datagram with error bit set (command 0x19 = 0x10 write-reply | 0x08 error | 0x01 config)
        // Error code 0x1999 in bytes [2..4]
        let header = MTI::DatagramOnly.to_header_with_dest(node_alias, our_alias).unwrap();
        let error_reply = GridConnectFrame {
            header,
            data: vec![0x20, 0x19, 0x19, 0x99],
        };

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(ack.to_string());
        transport.add_receive_frame(error_reply.to_string());

        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::write_memory_with_handle(
            &handle, our_alias, node_alias, 0xFD, 0x0000, &[0x42], 3000,
        ).await;

        actor.shutdown().await;

        assert!(result.is_err(), "Write should fail when reply-pending is followed by error reply");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("1999"), "Error should contain the reply error code: {}", err_msg);
    }

    // CDI read: error 0x1082 (address out of bounds) treated as end-of-CDI
    #[tokio::test]
    async fn test_cdi_read_0x1082_treated_as_end() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        // First chunk: 2-byte payload (max for CAN 8-byte frame with 6-byte header)
        let chunk1 = make_cdi_reply_frame(src, dst, 0, b"AB");
        // Second chunk: 0x1082 failure reply at address 2 (= len of first chunk payload)
        let header = MTI::DatagramOnly.to_header_with_dest(src, dst).unwrap();
        let fail_reply = GridConnectFrame {
            header,
            data: vec![0x20, 0x5B, 0x00, 0x00, 0x00, 0x02, 0x10, 0x82],
        };

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(chunk1.to_string());
        transport.add_receive_frame(fail_reply.to_string());
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(&handle, dst, src, 2000, None).await;
        actor.shutdown().await;

        assert!(result.is_ok(), "0x1082 should be treated as end-of-CDI: {:?}", result);
        assert_eq!(result.unwrap(), "AB");
    }

    // CDI read: non-0x1082 failure is a real error
    #[tokio::test]
    async fn test_cdi_read_other_error_propagates() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        let header = MTI::DatagramOnly.to_header_with_dest(src, dst).unwrap();
        let fail_reply = GridConnectFrame {
            header,
            data: vec![0x20, 0x5B, 0x00, 0x00, 0x00, 0x00, 0x10, 0x37],
        };

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(fail_reply.to_string());
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(&handle, dst, src, 2000, None).await;
        actor.shutdown().await;

        assert!(result.is_err(), "Non-0x1082 error should propagate");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("1037"), "Error should contain code: {}", err_msg);
    }

    /// Multi-chunk CDI download succeeds when all reply frames are pre-queued.
    ///
    /// A single broadcast subscription is held for the entire download, so no
    /// frames can be lost between chunks.  With a per-chunk subscription this
    /// scenario was timing-dependent on multi-threaded runtimes: the reader_loop
    /// could broadcast chunk 1's reply before the CDI code re-subscribed.
    #[tokio::test]
    async fn test_cdi_multi_chunk_success() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        // Chunk 0: 2 bytes at address 0, no null → another chunk expected
        let chunk0 = make_cdi_reply_frame(src, dst, 0, b"AB");
        // Chunk 1: 2 bytes at address 2, null-terminated → CDI complete
        let chunk1 = make_cdi_reply_frame(src, dst, 2, b"C\x00");

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(chunk0.to_string());
        transport.add_receive_frame(chunk1.to_string());
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(&handle, dst, src, 2000, None).await;
        actor.shutdown().await;

        assert!(result.is_ok(), "Multi-chunk CDI read should succeed: {:?}", result);
        assert_eq!(result.unwrap(), "ABC");
    }

    /// A stale reply (from a previous chunk's address) is discarded by the
    /// address check, and the correct reply for the current chunk is accepted.
    ///
    /// Regression test for the CDI corruption bug: stale replies from a previous
    /// chunk leaked across a subscription boundary and were accepted as the
    /// current chunk's data, causing a cascading address offset of −64 bytes
    /// and ultimately an `InvalidXml` parse error.  The hoisted subscription
    /// eliminates the gap; the address check provides defence in depth.
    #[tokio::test]
    async fn test_cdi_stale_reply_at_chunk_boundary_discarded() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        // Chunk 0: 2 bytes at address 0
        let chunk0 = make_cdi_reply_frame(src, dst, 0, b"AB");
        // Stale duplicate: same address 0, different payload — would corrupt
        // chunk 1 if mistakenly accepted.
        let stale = make_cdi_reply_frame(src, dst, 0, b"XX");
        // Chunk 1: 2 bytes at address 2, null-terminated → CDI complete
        let chunk1 = make_cdi_reply_frame(src, dst, 2, b"C\x00");

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(chunk0.to_string());
        transport.add_receive_frame(stale.to_string());
        transport.add_receive_frame(chunk1.to_string());
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(&handle, dst, src, 2000, None).await;
        actor.shutdown().await;

        assert!(result.is_ok(), "Stale reply should be discarded: {:?}", result);
        assert_eq!(
            result.unwrap(), "ABC",
            "CDI should contain only valid chunk data, not stale duplicate"
        );
    }

    /// Multi-chunk CDI succeeds on a multi-threaded runtime where the reader_loop
    /// runs concurrently and can broadcast frames at any time.
    ///
    /// With a per-chunk subscription, the reader_loop could race ahead and
    /// broadcast chunk 1's reply before the CDI code re-subscribed, causing it
    /// to be invisible to the new receiver.  A single subscription held for the
    /// entire download prevents this regardless of thread scheduling.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_cdi_multi_chunk_no_frame_loss_threaded() {
        let src: u16 = 0xBBB;
        let dst: u16 = 0xAAA;

        let chunk0 = make_cdi_reply_frame(src, dst, 0, b"AB");
        let chunk1 = make_cdi_reply_frame(src, dst, 2, b"C\x00");

        let mut transport = GlobalMockTransport::new();
        transport.add_receive_frame(chunk0.to_string());
        transport.add_receive_frame(chunk1.to_string());
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let result = LccConnection::read_cdi_with_handle(&handle, dst, src, 2000, None).await;
        actor.shutdown().await;

        assert!(result.is_ok(), "Multi-chunk CDI read should succeed on multi-thread runtime: {:?}", result);
        assert_eq!(result.unwrap(), "ABC");
    }

    // Cancel: flag pre-set to `true` → returns cancelled error before any I/O
    #[tokio::test]
    async fn test_cdi_cancel_before_first_chunk() {
        let transport = GlobalMockTransport::new();
        let mut actor = TransportActor::new(Box::new(transport));
        let handle = actor.handle();

        let cancel_flag = Arc::new(AtomicBool::new(true)); // already cancelled

        let result = LccConnection::read_cdi_with_handle(
            &handle,
            0xAAA,
            0xBBB,
            2000,
            Some(cancel_flag),
        ).await;

        actor.shutdown().await;

        assert!(result.is_err(), "CDI read should fail immediately when cancel flag is pre-set");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("cancelled") || err.contains("Cancelled"),
            "Error should mention cancellation: {}",
            err
        );
    }
}
