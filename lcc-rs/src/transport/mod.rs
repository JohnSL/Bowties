//! Transport layer for LCC/OpenLCB communication

pub mod tcp;
pub mod gridconnect_async;
pub mod slcan_serial;

// `mock` is publicly available so integration tests in `lcc-rs/tests/**`
// (and downstream crates in future test binaries) can construct
// `MockTransport` and `MockTransportWriter`. Production code does not depend
// on it; the module has no runtime cost when unused.
pub mod mock;

pub use tcp::{LccTransport, TcpTransport, TransportReader, TransportWriter};
pub use gridconnect_async::{GridConnectAsyncTransport, FrameEncoding};
pub use slcan_serial::SlcanSerialTransport;
