//! Protocol-level structures and utilities for LCC/OpenLCB

pub mod frame;
pub mod mti;
pub mod datagram;
pub mod memory_config;

pub use frame::GridConnectFrame;
pub use mti::MTI;
pub use datagram::{DatagramAssembler, DatagramState};
pub use memory_config::{MemoryConfigCmd, AddressSpace, ReadReply};
