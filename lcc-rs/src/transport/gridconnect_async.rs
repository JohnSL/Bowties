//! Async GridConnect serial transport using tokio-serial (IOCP/epoll/kqueue).
//!
//! Drop-in replacement for `GridConnectSerialTransport` that uses native OS
//! async I/O instead of blocking threads + polled `ReadFile`. On Windows this
//! means IOCP-based completion (sub-millisecond wake on data arrival); on Linux
//! `epoll`; on macOS `kqueue`.
//!
//! ## Why this exists
//!
//! The blocking-thread transport (`gridconnect_serial.rs`) polls with a 10ms
//! `ReadFile` timeout, which Windows rounds to ~16ms due to system timer
//! granularity. This adds 0–16ms latency per frame received and is the primary
//! reason Bowties config-reads are ~2× slower than LCCPro (which uses
//! jSerialComm's event-driven `WaitCommEvent` model).
//!
//! This transport eliminates that latency by using `tokio-serial`'s
//! `SerialStream`, which registers the serial port HANDLE with tokio's IOCP
//! driver. Reads complete the instant the USB-serial driver has data — no
//! polling, no timer dependency.
//!
//! ## Executor isolation
//!
//! Unlike the blocking-thread transport, serial I/O here runs on tokio worker
//! threads. A stuck serial port WILL occupy a worker thread. For production use
//! in the Bowties app, consider spawning this transport's read/write on a
//! dedicated single-threaded tokio runtime to preserve the isolation guarantee.
//! For `cdi-probe` (single-purpose CLI), this is not a concern.

use crate::transport::{LccTransport, TransportReader, TransportWriter};
use crate::{Error, Result, protocol::GridConnectFrame};
use serialport::SerialPort;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

/// Frame encoding variant for the GridConnect wire format.
///
/// Most adapters use standard GridConnect where the 8 hex digits represent the
/// raw 29-bit CAN ID. MERG CAN-RS/CANUSB4 hardware re-encodes the header bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameEncoding {
    /// Standard GridConnect: 29-bit CAN ID encoded directly as 8 hex digits.
    #[default]
    Standard,
    /// MERG CAN-RS / CANUSB4 encoding: the 29-bit extended CAN ID is sent as
    /// `<11-bit SID><0><1><0><18-bit EID>` in the 8 hex digit header field.
    ///
    /// Encode (host → adapter): `munged = ((id << 3) & 0xFFE0_0000) | 0x0008_0000 | (id & 0x3_FFFF)`
    /// Decode (adapter → host): `id = ((munged >> 3) & 0x1FFC_0000) | (munged & 0x3_FFFF)`
    MergCanRs,
}

impl FrameEncoding {
    /// Encode a 29-bit CAN header for transmission to the adapter.
    #[inline]
    pub fn encode_header(self, header: u32) -> u32 {
        match self {
            Self::Standard => header,
            Self::MergCanRs => ((header << 3) & 0xFFE0_0000) | 0x0008_0000 | (header & 0x0003_FFFF),
        }
    }

    /// Decode a CAN header received from the adapter back to a raw 29-bit ID.
    #[inline]
    pub fn decode_header(self, wire_header: u32) -> u32 {
        match self {
            Self::Standard => wire_header,
            Self::MergCanRs => ((wire_header >> 3) & 0x1FFC_0000) | (wire_header & 0x0003_FFFF),
        }
    }
}

/// Async GridConnect serial transport.
///
/// Uses `tokio-serial` (`SerialStream`) for IOCP/epoll/kqueue-based async I/O.
pub struct GridConnectAsyncTransport {
    port: SerialStream,
    encoding: FrameEncoding,
    read_buf: Vec<u8>,
}

// SerialStream's underlying handle is thread-safe; access serialized via &mut self.
unsafe impl Sync for GridConnectAsyncTransport {}

impl GridConnectAsyncTransport {
    /// Open the serial port with the given settings.
    ///
    /// Handles DTR/RTS assertion and flow control setup matching the
    /// blocking transport's sequence (and JMRI's configureLeads pattern).
    pub async fn open(
        path: &str,
        baud_rate: u32,
        flow_control: serialport::FlowControl,
        encoding: FrameEncoding,
    ) -> Result<Self> {
        // tokio-serial opens with the builder pattern. We open with no flow
        // control initially so we can assert DTR/RTS first (matching JMRI's
        // configureLeads → setFlowControl sequence).
        let mut port = tokio_serial::new(path, baud_rate)
            .data_bits(tokio_serial::DataBits::Eight)
            .stop_bits(tokio_serial::StopBits::One)
            .parity(tokio_serial::Parity::None)
            .flow_control(tokio_serial::FlowControl::None)
            .timeout(Duration::from_millis(10))
            .open_native_async()
            .map_err(|e| {
                Error::Transport(format!("Failed to open serial port {}: {}", path, e))
            })?;

        // Assert DTR and RTS before enabling flow control.
        if let Err(e) = port.write_data_terminal_ready(true) {
            eprintln!("Warning: Could not assert DTR on {}: {}", path, e);
        }
        if let Err(e) = port.write_request_to_send(true) {
            eprintln!("Warning: Could not assert RTS on {}: {}", path, e);
        }

        // Now enable flow control if requested.
        if flow_control != serialport::FlowControl::None {
            port.set_flow_control(flow_control).map_err(|e| {
                Error::Transport(format!("Failed to set flow control on {}: {}", path, e))
            })?;
        }

        Ok(Self {
            port,
            encoding,
            read_buf: Vec::with_capacity(128),
        })
    }

    /// Read bytes until ';' and decode a GridConnect frame.
    async fn receive_inner(&mut self) -> Result<GridConnectFrame> {
        let mut byte = [0u8; 1];
        loop {
            match self.port.read_exact(&mut byte).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) => return Err(Error::Io(e)),
            }

            let ch = byte[0];

            if ch == b';' {
                // Complete frame received — parse it.
                let frame_str = String::from_utf8_lossy(&self.read_buf).to_string();
                self.read_buf.clear();

                if let Some(frame) = Self::parse_gc_frame(&frame_str, self.encoding) {
                    return Ok(frame);
                }
                // Unparseable frame — skip and keep reading.
                continue;
            } else if ch == b':' {
                // Start of a new frame — clear any noise before it.
                self.read_buf.clear();
            } else if ch != b'\r' && ch != b'\n' {
                self.read_buf.push(ch);
                // Prevent unbounded growth from line noise.
                if self.read_buf.len() > 512 {
                    self.read_buf.clear();
                }
            }
        }
    }

    /// Parse a GridConnect frame body (between ':' and ';') into a frame,
    /// applying header decoding for the configured encoding.
    fn parse_gc_frame(body: &str, encoding: FrameEncoding) -> Option<GridConnectFrame> {
        // The reader accumulates bytes between ':' and ';' (exclusive of both).
        // Reconstruct the full wire string for the existing parser.
        let full = format!(":{};" , body);
        match GridConnectFrame::parse_wire(&full) {
            Ok(wire_frame) => {
                // Decode the header from wire encoding to canonical 29-bit CAN ID.
                let decoded_header = encoding.decode_header(wire_frame.header);
                GridConnectFrame::new(decoded_header, wire_frame.data).ok()
            }
            Err(_) => None,
        }
    }
}

#[async_trait::async_trait]
impl LccTransport for GridConnectAsyncTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let encoded_header = self.encoding.encode_header(frame.header);
        let wire_frame = GridConnectFrame {
            header: encoded_header,
            data: frame.data.clone(),
        };
        // GridConnect: ':X<8hex>N<data_hex>;' — no trailing CR/LF on serial.
        let frame_str = wire_frame.to_string();
        self.port.write_all(frame_str.as_bytes()).await?;
        self.port.flush().await?;
        Ok(())
    }

    async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
        if timeout_ms > 0 {
            match tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                self.receive_inner(),
            )
            .await
            {
                Ok(result) => result.map(Some),
                Err(_) => Ok(None),
            }
        } else {
            self.receive_inner().await.map(Some)
        }
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }

    fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>) {
        let encoding = self.encoding;
        let (reader, writer) = tokio::io::split(self.port);
        (
            Box::new(GridConnectAsyncReader {
                reader,
                read_buf: self.read_buf,
                encoding,
            }),
            Box::new(GridConnectAsyncWriter { writer, encoding }),
        )
    }
}

/// Read half of the async GridConnect transport.
pub struct GridConnectAsyncReader {
    reader: ReadHalf<SerialStream>,
    read_buf: Vec<u8>,
    encoding: FrameEncoding,
}

#[async_trait::async_trait]
impl TransportReader for GridConnectAsyncReader {
    async fn receive(&mut self) -> Result<GridConnectFrame> {
        let mut byte = [0u8; 1];
        loop {
            match self.reader.read_exact(&mut byte).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) => return Err(Error::Io(e)),
            }

            let ch = byte[0];

            if ch == b';' {
                let frame_str = String::from_utf8_lossy(&self.read_buf).to_string();
                self.read_buf.clear();

                if let Some(frame) =
                    GridConnectAsyncTransport::parse_gc_frame(&frame_str, self.encoding)
                {
                    return Ok(frame);
                }
                continue;
            } else if ch == b':' {
                self.read_buf.clear();
            } else if ch != b'\r' && ch != b'\n' {
                self.read_buf.push(ch);
                if self.read_buf.len() > 512 {
                    self.read_buf.clear();
                }
            }
        }
    }
}

/// Write half of the async GridConnect transport.
pub struct GridConnectAsyncWriter {
    writer: WriteHalf<SerialStream>,
    encoding: FrameEncoding,
}

#[async_trait::async_trait]
impl TransportWriter for GridConnectAsyncWriter {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let encoded_header = self.encoding.encode_header(frame.header);
        let wire_frame = GridConnectFrame {
            header: encoded_header,
            data: frame.data.clone(),
        };
        let frame_str = wire_frame.to_string();
        self.writer.write_all(frame_str.as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── parse_gc_frame unit tests ──────────────────────────────────────────

    #[test]
    fn parse_standard_frame_no_data() {
        let frame = GridConnectAsyncTransport::parse_gc_frame(
            "X19490AAAN", FrameEncoding::Standard,
        ).unwrap();
        assert_eq!(frame.header, 0x19490AAA);
        assert!(frame.data.is_empty());
    }

    #[test]
    fn parse_standard_frame_with_data() {
        let frame = GridConnectAsyncTransport::parse_gc_frame(
            "X195B4123N0102030405060708", FrameEncoding::Standard,
        ).unwrap();
        assert_eq!(frame.header, 0x195B4123);
        assert_eq!(frame.data, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn parse_merg_encoded_frame() {
        // MERG wire header for canonical 0x195B4123:
        let canonical = 0x195B4123u32;
        let wire_header = FrameEncoding::MergCanRs.encode_header(canonical);
        let body = format!("X{:08X}N01", wire_header);

        let frame = GridConnectAsyncTransport::parse_gc_frame(
            &body, FrameEncoding::MergCanRs,
        ).unwrap();
        assert_eq!(frame.header, canonical);
        assert_eq!(frame.data, vec![0x01]);
    }

    #[test]
    fn parse_invalid_body_returns_none() {
        // Too short
        assert!(GridConnectAsyncTransport::parse_gc_frame("X1234", FrameEncoding::Standard).is_none());
        // Not a valid frame
        assert!(GridConnectAsyncTransport::parse_gc_frame("garbage", FrameEncoding::Standard).is_none());
        // Empty
        assert!(GridConnectAsyncTransport::parse_gc_frame("", FrameEncoding::Standard).is_none());
    }

    #[test]
    fn parse_datagram_frame() {
        // Typical memory config read reply (first frame)
        let frame = GridConnectAsyncTransport::parse_gc_frame(
            "X1A3AEB3EN2050000080", FrameEncoding::Standard,
        ).unwrap();
        assert_eq!(frame.header, 0x1A3AEB3E);
        assert_eq!(frame.data, vec![0x20, 0x50, 0x00, 0x00, 0x80]);
    }

    // ─── FrameEncoding unit tests (moved from gridconnect_serial.rs) ────────

    #[test]
    fn standard_encoding_is_identity() {
        let header = 0x195B4123;
        assert_eq!(FrameEncoding::Standard.encode_header(header), header);
        assert_eq!(FrameEncoding::Standard.decode_header(header), header);
    }

    #[test]
    fn merg_encode_decode_roundtrip() {
        let header = 0x195B4123;
        let encoded = FrameEncoding::MergCanRs.encode_header(header);
        let decoded = FrameEncoding::MergCanRs.decode_header(encoded);
        assert_eq!(decoded, header);
    }

    #[test]
    fn merg_encode_matches_jmri_formula() {
        let header: u32 = 0x195B4123;
        let expected = ((header << 3) & 0xFFE0_0000) | 0x0008_0000 | (header & 0x0003_FFFF);
        assert_eq!(FrameEncoding::MergCanRs.encode_header(header), expected);
    }

    #[test]
    fn merg_decode_matches_jmri_formula() {
        let wire: u32 = 0xCADA_0123;
        let expected = ((wire >> 3) & 0x1FFC_0000) | (wire & 0x0003_FFFF);
        assert_eq!(FrameEncoding::MergCanRs.decode_header(wire), expected);
    }

    #[test]
    fn merg_roundtrip_various_headers() {
        let test_cases: &[u32] = &[
            0x19170000, 0x195B4FFF, 0x1A000123, 0x1D000456,
            0x00000000, 0x1FFFFFFF,
        ];
        for &header in test_cases {
            let encoded = FrameEncoding::MergCanRs.encode_header(header);
            let decoded = FrameEncoding::MergCanRs.decode_header(encoded);
            assert_eq!(decoded, header, "Roundtrip failed for header 0x{:08X}", header);
        }
    }

    #[test]
    fn merg_encoded_header_fits_29_bits_for_standard_lcc_ids() {
        let header: u32 = 0x195B4123;
        let encoded = FrameEncoding::MergCanRs.encode_header(header);
        assert_eq!(encoded & 0x0008_0000, 0x0008_0000, "Bit 19 must be set");
        assert_eq!(encoded & 0x0003_FFFF, header & 0x0003_FFFF, "Bottom 18 bits preserved");
    }
}
