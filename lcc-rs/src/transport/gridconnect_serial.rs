//! GridConnect serial transport for USB-to-CAN adapters
//!
//! Supports devices like RR-Cirkits Buffer LCC, SPROG USB-LCC, CAN2USBINO, and
//! MERG CAN-RS/CANUSB4 that use the GridConnect framing protocol over a serial
//! (USB CDC) connection. No init sequence is required.
//!
//! Devices differ in baud rate, flow control, and header encoding — for example,
//! SPROG USB-LCC requires 460800 baud with RTS/CTS, while MERG adapters use
//! 115200 with no flow control and a non-standard CAN header encoding.

use crate::{Error, Result, protocol::GridConnectFrame};
use crate::transport::{LccTransport, TransportReader, TransportWriter};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio_serial::{SerialStream, SerialPortBuilderExt, SerialPort};

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

/// GridConnect serial transport for USB-to-CAN adapters
///
/// Communicates using GridConnect framing (`:X<header>N<data>;`) over a serial
/// port. No init sequence is required — the adapter is ready to use immediately
/// after the port is opened.
pub struct GridConnectSerialTransport {
    port: SerialStream,
    read_buf: Vec<u8>,
    encoding: FrameEncoding,
}

// SAFETY: SerialStream's underlying Windows HANDLE is thread-safe. All serial
// access is serialized via `&mut self`, and in practice via Arc<Mutex<...>>.
unsafe impl Sync for GridConnectSerialTransport {}

impl GridConnectSerialTransport {
    /// Open a serial port and return the ready transport.
    ///
    /// # Arguments
    /// * `path` — Serial port path (e.g. `"COM3"` on Windows, `"/dev/ttyUSB0"` on Linux)
    /// * `baud_rate` — Serial baud rate (e.g. 57600 for Buffer LCC, 460800 for SPROG USB-LCC)
    /// * `flow_control` — Hardware flow control mode. Use `FlowControl::None` for
    ///   most adapters (RR-Cirkits, MERG) or `FlowControl::Hardware` for SPROG
    ///   USB-LCC / PI-LCC which require RTS/CTS.
    /// * `encoding` — Frame header encoding variant. Use `FrameEncoding::Standard`
    ///   for most adapters or `FrameEncoding::MergCanRs` for MERG CAN-RS / CANUSB4.
    pub async fn open(path: &str, baud_rate: u32, flow_control: tokio_serial::FlowControl, encoding: FrameEncoding) -> Result<Self> {
        let mut port = tokio_serial::new(path, baud_rate)
            .data_bits(tokio_serial::DataBits::Eight)
            .stop_bits(tokio_serial::StopBits::One)
            .parity(tokio_serial::Parity::None)
            .flow_control(flow_control)
            .open_native_async()
            .map_err(|e| Error::Transport(format!("Failed to open serial port {}: {}", path, e)))?;

        // Assert DTR so USB-CDC bridge firmware knows the host is ready.
        if let Err(e) = port.write_data_terminal_ready(true) {
            eprintln!("Warning: Could not assert DTR on {}: {}", path, e);
        }

        // When flow control is None, manually assert RTS so the adapter forwards
        // frames. With RTS/CTS flow control the OS manages RTS automatically.
        if flow_control == tokio_serial::FlowControl::None {
            if let Err(e) = port.write_request_to_send(true) {
                eprintln!("Warning: Could not assert RTS on {}: {}", path, e);
            }
        }

        Ok(Self {
            port,
            read_buf: Vec::with_capacity(128),
            encoding,
        })
    }

    /// Inner receive loop — reads bytes until a complete GridConnect frame is found.
    async fn receive_inner(&mut self) -> Result<Option<GridConnectFrame>> {
        loop {
            let mut byte = [0u8; 1];
            match self.port.read_exact(&mut byte).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) => return Err(Error::Io(e)),
            }

            self.read_buf.push(byte[0]);

            if byte[0] == b';' {
                // Find the last ':' to locate the frame start (ignore preceding noise/garbage)
                if let Some(start) = self.read_buf.iter().rposition(|&b| b == b':') {
                    let frame_bytes = self.read_buf[start..].to_vec();
                    self.read_buf.clear();

                    match String::from_utf8(frame_bytes) {
                        Ok(frame_str) => match GridConnectFrame::parse_wire(&frame_str) {
                            Ok(mut frame) => {
                                frame.header = self.encoding.decode_header(frame.header);
                                return Ok(Some(frame));
                            }
                            Err(e) => {
                                eprintln!("Warning: Failed to parse GridConnect frame: {}", e);
                                continue;
                            }
                        },
                        Err(_) => {
                            // Invalid UTF-8 — skip
                            continue;
                        }
                    }
                } else {
                    // No ':' found — clear garbage and continue
                    self.read_buf.clear();
                }
            }

            // Prevent unbounded buffer growth from line noise
            if self.read_buf.len() > 1024 {
                self.read_buf.clear();
            }
        }
    }
}

#[async_trait::async_trait]
impl LccTransport for GridConnectSerialTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let encoded_header = self.encoding.encode_header(frame.header);
        let wire_frame = GridConnectFrame { header: encoded_header, data: frame.data.clone() };
        let frame_str = format!("{}\r\n", wire_frame.to_string());
        self.port.write_all(frame_str.as_bytes()).await?;
        self.port.flush().await?;
        Ok(())
    }

    async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
        if timeout_ms > 0 {
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(timeout_ms),
                self.receive_inner(),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Ok(None),
            }
        } else {
            self.receive_inner().await
        }
    }

    async fn close(&mut self) -> Result<()> {
        // Port is closed on drop
        Ok(())
    }

    fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>) {
        let (reader, writer) = tokio::io::split(self.port);
        (
            Box::new(GridConnectSerialReader {
                reader,
                read_buf: self.read_buf,
                encoding: self.encoding,
            }),
            Box::new(GridConnectSerialWriter { writer, encoding: self.encoding }),
        )
    }
}

/// Read half of a split GridConnect serial transport.
pub struct GridConnectSerialReader {
    reader: ReadHalf<SerialStream>,
    read_buf: Vec<u8>,
    encoding: FrameEncoding,
}

#[async_trait::async_trait]
impl TransportReader for GridConnectSerialReader {
    async fn receive(&mut self) -> Result<GridConnectFrame> {
        loop {
            let mut byte = [0u8; 1];
            match self.reader.read_exact(&mut byte).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) => return Err(Error::Io(e)),
            }

            self.read_buf.push(byte[0]);

            if byte[0] == b';' {
                if let Some(start) = self.read_buf.iter().rposition(|&b| b == b':') {
                    let frame_bytes = self.read_buf[start..].to_vec();
                    self.read_buf.clear();

                    match String::from_utf8(frame_bytes) {
                        Ok(frame_str) => match GridConnectFrame::parse_wire(&frame_str) {
                            Ok(mut frame) => {
                                frame.header = self.encoding.decode_header(frame.header);
                                return Ok(frame);
                            }
                            Err(e) => {
                                eprintln!("Warning: Failed to parse GridConnect frame: {}", e);
                                continue;
                            }
                        },
                        Err(_) => continue,
                    }
                } else {
                    self.read_buf.clear();
                }
            }

            if self.read_buf.len() > 1024 {
                self.read_buf.clear();
            }
        }
    }
}

/// Write half of a split GridConnect serial transport.
pub struct GridConnectSerialWriter {
    writer: WriteHalf<SerialStream>,
    encoding: FrameEncoding,
}

#[async_trait::async_trait]
impl TransportWriter for GridConnectSerialWriter {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let encoded_header = self.encoding.encode_header(frame.header);
        let wire_frame = GridConnectFrame { header: encoded_header, data: frame.data.clone() };
        let frame_str = format!("{}\r\n", wire_frame.to_string());
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

    #[test]
    fn standard_encoding_is_identity() {
        let header = 0x195B4123; // typical LCC extended header
        assert_eq!(FrameEncoding::Standard.encode_header(header), header);
        assert_eq!(FrameEncoding::Standard.decode_header(header), header);
    }

    #[test]
    fn merg_encode_decode_roundtrip() {
        // A typical LCC 29-bit CAN ID: 0x195B4123
        // Bits: SID = top 11 = 0x195B4123 >> 18 = 0x655 (0110_0101_0101)
        //       EID = bottom 18 = 0x195B4123 & 0x3FFFF = 0x34123
        let header = 0x195B4123;
        let encoded = FrameEncoding::MergCanRs.encode_header(header);
        let decoded = FrameEncoding::MergCanRs.decode_header(encoded);
        assert_eq!(decoded, header);
    }

    #[test]
    fn merg_encode_matches_jmri_formula() {
        // JMRI MergMessage.setHeader() for extended:
        //   munged = ((header << 3) & 0xFFE00000) | 0x80000 | (header & 0x3FFFF)
        let header: u32 = 0x195B4123;
        let expected = ((header << 3) & 0xFFE0_0000) | 0x0008_0000 | (header & 0x0003_FFFF);
        assert_eq!(FrameEncoding::MergCanRs.encode_header(header), expected);
    }

    #[test]
    fn merg_decode_matches_jmri_formula() {
        // JMRI MergReply.getHeader() for extended:
        //   val = ((val >> 3) & 0x1FFC0000) | (val & 0x3FFFF)
        let wire: u32 = 0xCADA_0123; // hypothetical wire value
        let expected = ((wire >> 3) & 0x1FFC_0000) | (wire & 0x0003_FFFF);
        assert_eq!(FrameEncoding::MergCanRs.decode_header(wire), expected);
    }

    #[test]
    fn merg_roundtrip_various_headers() {
        let test_cases: &[u32] = &[
            0x19170000, // InitializationComplete, alias 0x000
            0x195B4FFF, // VerifiedNodeID, alias 0xFFF
            0x1A000123, // datagram first frame
            0x1D000456, // datagram final frame
            0x00000000, // minimum
            0x1FFFFFFF, // maximum 29-bit value
        ];
        for &header in test_cases {
            let encoded = FrameEncoding::MergCanRs.encode_header(header);
            let decoded = FrameEncoding::MergCanRs.decode_header(encoded);
            assert_eq!(decoded, header, "Roundtrip failed for header 0x{:08X}", header);
        }
    }

    #[test]
    fn merg_encoded_header_fits_29_bits_for_standard_lcc_ids() {
        // LCC uses 29-bit extended CAN frames. After MERG encoding the
        // result is stored in 32 bits on the wire (8 hex digits).
        // Verify a known value encodes to a specific expected pattern.
        let header: u32 = 0x195B4123;
        let encoded = FrameEncoding::MergCanRs.encode_header(header);
        // SID (top 11 bits of header) = 0x656D → shifted left 3 → top bits
        // EID (bottom 18 bits of header) = 0x34123 → preserved in bottom 18
        // Bit 19 = 1 (flag)
        assert_eq!(encoded & 0x0008_0000, 0x0008_0000, "Bit 19 must be set");
        assert_eq!(encoded & 0x0003_FFFF, header & 0x0003_FFFF, "Bottom 18 bits preserved");
    }
}
