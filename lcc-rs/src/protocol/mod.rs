//! Protocol-level structures and utilities for LCC/OpenLCB

pub mod frame;
pub mod mti;

pub use frame::GridConnectFrame;
pub use mti::MTI;
