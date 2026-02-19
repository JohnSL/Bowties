//! Transport layer for LCC/OpenLCB communication

pub mod tcp;

#[cfg(test)]
pub mod mock;

pub use tcp::{LccTransport, TcpTransport};
