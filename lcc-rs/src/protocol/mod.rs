//! Protocol-level structures and utilities for LCC/OpenLCB

pub mod frame;
pub mod mti;
pub mod datagram;
pub mod memory_config;
pub mod oir;

pub use frame::GridConnectFrame;
pub use mti::MTI;
pub use datagram::{
    build_datagram_received_ok_frame, build_datagram_rejected_frame,
    DatagramAssembler, DatagramState,
};
pub use memory_config::{MemoryConfigCmd, AddressSpace, ReadReply};
pub use oir::{build_oir_payload, parse_oir_payload};
