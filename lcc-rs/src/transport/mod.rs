//! Transport layer for LCC/OpenLCB communication

pub mod tcp;

pub use tcp::{LccTransport, TcpTransport};
