//! LCC-RS: A Rust implementation of the Layout Command Control (LCC/OpenLCB) protocol
//!
//! This crate provides a Rust library for working with the LCC/OpenLCB protocol,
//! commonly used in model railroading for distributed control systems.
//!
//! # Features
//!
//! - GridConnect frame parsing and encoding
//! - TCP transport layer
//! - Node discovery
//! - Message Type Identifiers (MTI) handling
//!
//! # Example
//!
//! ```no_run
//! use lcc_rs::{LccConnection, NodeID};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let node_id = NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]);
//!     let connection = LccConnection::connect_with_dispatcher("localhost", 12021, node_id).await?;
//!     let mut conn = connection.lock().await;
//!     let nodes = conn.discover_nodes(250).await?;
//!     
//!     for node in nodes {
//!         println!("Found node: {}", node.node_id);
//!     }
//!     
//!     Ok(())
//! }
//! ```

pub mod constants;
pub mod types;
pub mod protocol;
pub mod transport;
pub mod discovery;
pub mod snip;
pub mod pip;
pub mod cdi;
pub mod transport_actor;
pub mod alias_allocation;

// Re-export commonly used types
pub use types::{NodeID, EventID, NodeAlias, DiscoveredNode, SNIPData, SNIPStatus, ConnectionStatus, CdiData, ProtocolFlags, PIPStatus};
pub use protocol::{GridConnectFrame, MTI, DatagramAssembler, DatagramState, MemoryConfigCmd, AddressSpace, ReadReply};
pub use transport::LccTransport;
pub use transport::{GridConnectSerialTransport, SlcanSerialTransport};
pub use discovery::LccConnection;
pub use discovery::MemoryReadTiming;
pub use discovery::{BatchReadDescriptor, BatchReadResult, BatchReader};
pub use snip::{query_snip, parse_snip_payload, encode_snip_payload};
pub use cdi::{Cdi, Segment, DataElement, Group, IntElement, EventIdElement, StringElement, FloatElement, ActionElement, BlobElement, EventRole, classify_event_slot, walk_event_slots};
pub use transport_actor::{TransportActor, TransportHandle, ReceivedMessage};
pub use alias_allocation::AliasAllocator;

/// LCC-RS error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid frame format: {0}")]
    InvalidFrame(String),
    
    #[error("Invalid MTI: {0}")]
    InvalidMTI(String),
    
    #[error("Transport error: {0}")]
    Transport(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Alias allocation failed: {0}")]
    AliasAllocation(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Parse(s)
    }
}

/// Result type for LCC-RS operations
pub type Result<T> = std::result::Result<T, Error>;
