//! Transport layer for LCC/OpenLCB communication

pub mod tcp;
pub mod gridconnect_serial;
pub mod slcan_serial;

#[cfg(test)]
pub mod mock;

pub use tcp::{LccTransport, TcpTransport};
pub use gridconnect_serial::GridConnectSerialTransport;
pub use slcan_serial::SlcanSerialTransport;
