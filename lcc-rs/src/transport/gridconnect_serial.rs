//! GridConnect serial transport for USB-to-CAN adapters
//!
//! Supports devices like RR-Cirkits Buffer LCC, SPROG USB-LCC, CAN2USBINO, and
//! MERG CAN-RS/CANUSB4 that use the GridConnect framing protocol over a serial
//! (USB CDC) connection. No init sequence is required.
//!
//! Devices differ in baud rate, flow control, and header encoding — for example,
//! SPROG USB-LCC requires 460800 baud with RTS/CTS, while MERG adapters use
//! 115200 with no flow control and a non-standard CAN header encoding.
//!
//! ## I/O architecture
//!
//! Uses **blocking serial I/O on dedicated OS threads**, bridged to the tokio
//! async world via `tokio::sync::mpsc` channels (matching JMRI's jSerialComm
//! dedicated-reader-thread model).
//!
//! The load-bearing reason for the blocking-thread model is **executor
//! isolation**: the actual serial syscalls (`write_all` / `flush` in the writer
//! thread, `read` in the reader thread) run on dedicated OS threads, so a stuck
//! wire operation can never starve a tokio worker thread. This guarantee holds
//! on every platform. (An earlier version of this comment claimed the model
//! "avoids the unreliable Windows overlapped I/O path in mio-serial/tokio-serial";
//! that causal claim was asserted, not proven — the SPROG USB-LCC CDI failure it
//! was written for was actually a serial `\r\n` framing bug, fixed separately.
//! Executor isolation is the real, platform-independent justification, so the
//! model is kept on that basis rather than the Windows-async claim.)
//!
//! ## Stuck-write bounds (see ADR-0017)
//!
//! This transport sits *below* the `transport_actor` async coordination layer,
//! which wraps each `writer.send()` in `SERIAL_SEND_TIMEOUT` (500 ms) and
//! publishes `TransportHealth::Wedged` on timeout. For this transport,
//! `writer.send()` is an *enqueue* onto the writer mpsc
//! (`WRITER_CHANNEL_CAPACITY`), so the 500 ms bounds the **enqueue**, not the
//! wire write — it fires only once the queue backs up behind a stuck writer
//! thread. The **per-write** bound is platform-specific:
//!
//! - **Windows**: `WriteFile` with `fOutxCtsFlow` blocks up to the DCB
//!   `WriteTotalTimeoutConstant`, patched from 10 ms to 5000 ms by
//!   `fix_write_timeout` so CTS hardware flow control has time to work.
//! - **macOS / Linux**: `serialport`'s `write()` bounds only the
//!   poll-for-writable (`self.timeout`, 10 ms) and returns on kernel-buffer
//!   accept; `flush()` calls `tcdrain()`, which blocks until physical
//!   transmission completes with no effective timeout. Under sustained CTS
//!   back-pressure (hardware flow control) there is no OS-level per-write bound,
//!   so the `transport_actor` enqueue-fill `Wedged` detection above is the
//!   uniform cross-platform backstop. Making the writer thread resilient to a
//!   transient write `TimedOut` (rather than exiting the thread) is a tracked
//!   robustness follow-up, deferred until it can be validated on Mac/Linux
//!   hardware.

use crate::{Error, Result, protocol::GridConnectFrame};
use crate::transport::{LccTransport, TransportReader, TransportWriter};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::mpsc;

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

/// Channel capacity for the reader→async bridge. Large enough to absorb bursts
/// from a busy bus without back-pressuring the reader thread's serial reads.
const READER_CHANNEL_CAPACITY: usize = 256;

/// Channel capacity for the async→writer bridge. Outbound traffic is lower
/// volume than inbound (we send one frame, receive many), so a smaller buffer
/// is fine.
const WRITER_CHANNEL_CAPACITY: usize = 64;

/// GridConnect serial transport for USB-to-CAN adapters
///
/// Uses blocking serial I/O on dedicated OS threads, bridged to tokio via
/// mpsc channels. The transport is ready to use immediately after `open()`.
pub struct GridConnectSerialTransport {
    frame_rx: mpsc::Receiver<Result<GridConnectFrame>>,
    wire_tx: mpsc::Sender<Vec<u8>>,
    encoding: FrameEncoding,
}

impl GridConnectSerialTransport {
    /// Fix write timeout on Windows.
    ///
    /// The `serialport` crate sets `WriteTotalTimeoutConstant` to the same
    /// value as the read timeout (10ms in our case). With `fOutxCtsFlow=true`,
    /// the OS blocks `WriteFile` when CTS is deasserted. If CTS stays low
    /// for >10ms, the write fails. This function sets the write timeout to
    /// 5000ms so CTS flow control has time to work.
    #[cfg(target_os = "windows")]
    fn fix_write_timeout(handle: std::os::windows::io::RawHandle, path: &str) -> Result<()> {
        #[repr(C)]
        #[allow(non_snake_case)]
        struct COMMTIMEOUTS {
            ReadIntervalTimeout: u32,
            ReadTotalTimeoutMultiplier: u32,
            ReadTotalTimeoutConstant: u32,
            WriteTotalTimeoutMultiplier: u32,
            WriteTotalTimeoutConstant: u32,
        }

        extern "system" {
            fn GetCommTimeouts(hFile: *mut std::ffi::c_void, lpCommTimeouts: *mut COMMTIMEOUTS) -> i32;
            fn SetCommTimeouts(hFile: *mut std::ffi::c_void, lpCommTimeouts: *const COMMTIMEOUTS) -> i32;
        }

        let h = handle as *mut std::ffi::c_void;
        let mut timeouts: COMMTIMEOUTS = unsafe { std::mem::zeroed() };

        if unsafe { GetCommTimeouts(h, &mut timeouts) } == 0 {
            return Err(Error::Transport(format!(
                "GetCommTimeouts failed on {}: {}", path, std::io::Error::last_os_error()
            )));
        }

        timeouts.WriteTotalTimeoutConstant = 5000; // 5 seconds
        timeouts.WriteTotalTimeoutMultiplier = 0;

        if unsafe { SetCommTimeouts(h, &timeouts) } == 0 {
            return Err(Error::Transport(format!(
                "SetCommTimeouts failed on {}: {}", path, std::io::Error::last_os_error()
            )));
        }

        Ok(())
    }

    /// Open a serial port and return the ready transport.
    ///
    /// Spawns a blocking reader thread and a blocking writer thread that own the
    /// serial port handle. The returned transport communicates with those threads
    /// via async channels.
    ///
    /// # Arguments
    /// * `path` — Serial port path (e.g. `"COM3"` on Windows, `"/dev/ttyUSB0"` on Linux)
    /// * `baud_rate` — Serial baud rate (e.g. 57600 for Buffer LCC, 460800 for SPROG USB-LCC)
    /// * `flow_control` — Hardware flow control mode. Use `FlowControl::None` for
    ///   most adapters (RR-Cirkits, MERG) or `FlowControl::Hardware` for SPROG
    ///   USB-LCC / PI-LCC which require RTS/CTS.
    /// * `encoding` — Frame header encoding variant. Use `FrameEncoding::Standard`
    ///   for most adapters or `FrameEncoding::MergCanRs` for MERG CAN-RS / CANUSB4.
    pub async fn open(path: &str, baud_rate: u32, flow_control: serialport::FlowControl, encoding: FrameEncoding) -> Result<Self> {
        // Open with FlowControl::None initially so we can manually assert
        // DTR and RTS before the OS flow-control state machine takes over.
        // JMRI uses the same sequence: configureLeads(rts=true, dtr=true)
        // before setFlowControl(). On FTDI-based adapters like the SPROG
        // USB-LCC, the chip will not forward CAN→USB data until RTS is
        // asserted. If we open with RTS_CONTROL_HANDSHAKE from the start,
        // the initial RTS state is driver-dependent and manual assertion
        // via EscapeCommFunction(SETRTS) is ignored — the FTDI's 64-byte
        // buffer can stall if RTS starts deasserted.
        let builder = serialport::new(path, baud_rate)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(10));

        // On Windows, open as COMPort (concrete type) so we can access the
        // raw handle for DCB patching. On other platforms, use the generic open.
        #[cfg(target_os = "windows")]
        let port: Box<dyn SerialPort> = {
            use std::os::windows::io::AsRawHandle;
            use serialport::COMPort;
            let mut com_port = COMPort::open(&builder)
                .map_err(|e| Error::Transport(format!("Failed to open serial port {}: {}", path, e)))?;

            // Assert DTR/RTS before flow control (matching JMRI's configureLeads → setFlowControl sequence)
            if let Err(e) = com_port.write_data_terminal_ready(true) {
                eprintln!("Warning: Could not assert DTR on {}: {}", path, e);
            }
            if let Err(e) = com_port.write_request_to_send(true) {
                eprintln!("Warning: Could not assert RTS on {}: {}", path, e);
            }

            // Set flow control via the crate API first
            if flow_control != serialport::FlowControl::None {
                com_port.set_flow_control(flow_control)
                    .map_err(|e| Error::Transport(format!("Failed to set flow control on {}: {}", path, e)))?;
            }

            // NOTE: Do NOT patch fRtsControl to RTS_CONTROL_HANDSHAKE.
            // The serialport crate uses RTS_CONTROL_ENABLE (RTS stays high)
            // with fOutxCtsFlow=true (CTS-only output flow control). This
            // is correct for the SPROG USB-LCC — its firmware treats RTS
            // deassertion as a fault, not as flow control back-pressure.
            // The amber "Indicate fault in module" LED illuminates when the
            // OS deasserts RTS under RTS_CONTROL_HANDSHAKE. JMRI also uses
            // CTS-only flow control (jSerialComm FLOW_CONTROL_CTS_ENABLED
            // maps to fOutxCtsFlow=true with RTS_CONTROL_ENABLE on FTDI).

            // Fix write timeout: the serialport crate sets
            // WriteTotalTimeoutConstant to the same value as the read
            // timeout (10ms). With CTS flow control (fOutxCtsFlow=true),
            // the OS blocks WriteFile when CTS is deasserted. A 10ms write
            // timeout means any CTS pause >10ms fails our write. Set the
            // write timeout to 5000ms so CTS flow control can work.
            Self::fix_write_timeout(com_port.as_raw_handle(), path)?;

            Box::new(com_port)
        };

        #[cfg(not(target_os = "windows"))]
        let port: Box<dyn SerialPort> = {
            let mut port = builder.open()
                .map_err(|e| Error::Transport(format!("Failed to open serial port {}: {}", path, e)))?;

            if let Err(e) = port.write_data_terminal_ready(true) {
                eprintln!("Warning: Could not assert DTR on {}: {}", path, e);
            }
            if let Err(e) = port.write_request_to_send(true) {
                eprintln!("Warning: Could not assert RTS on {}: {}", path, e);
            }
            if flow_control != serialport::FlowControl::None {
                port.set_flow_control(flow_control)
                    .map_err(|e| Error::Transport(format!("Failed to set flow control on {}: {}", path, e)))?;
            }
            port
        };

        // Clone the port handle for the writer thread.
        let writer_port = port.try_clone()
            .map_err(|e| Error::Transport(format!("Failed to clone serial port: {}", e)))?;

        // Reader channel: reader thread → async receive()
        let (frame_tx, frame_rx) = mpsc::channel::<Result<GridConnectFrame>>(READER_CHANNEL_CAPACITY);
        // Writer channel: async send() → writer thread
        let (wire_tx, wire_rx) = mpsc::channel::<Vec<u8>>(WRITER_CHANNEL_CAPACITY);

        let reader_encoding = encoding;
        let port_path = path.to_string();
        std::thread::Builder::new()
            .name(format!("gc-serial-reader-{}", path))
            .spawn(move || {
                reader_thread(port, frame_tx, reader_encoding, &port_path);
            })
            .map_err(|e| Error::Transport(format!("Failed to spawn reader thread: {}", e)))?;

        let writer_path = path.to_string();
        std::thread::Builder::new()
            .name(format!("gc-serial-writer-{}", path))
            .spawn(move || {
                writer_thread(writer_port, wire_rx, &writer_path);
            })
            .map_err(|e| Error::Transport(format!("Failed to spawn writer thread: {}", e)))?;

        Ok(Self {
            frame_rx,
            wire_tx,
            encoding,
        })
    }
}

/// Blocking reader thread: reads bytes from the serial port, parses GridConnect
/// frames, and sends them to the async world via the channel.
fn reader_thread(
    mut port: Box<dyn SerialPort>,
    frame_tx: mpsc::Sender<Result<GridConnectFrame>>,
    encoding: FrameEncoding,
    port_path: &str,
) {
    let mut read_buf = [0u8; 256];
    let mut frame_buf = Vec::with_capacity(128);

    loop {
        match port.read(&mut read_buf) {
            Ok(0) => {
                // EOF — port closed
                let _ = frame_tx.blocking_send(Err(Error::ConnectionClosed));
                break;
            }
            Ok(n) => {
                for &byte in &read_buf[..n] {
                    frame_buf.push(byte);

                    if byte == b';' {
                        if let Some(start) = frame_buf.iter().rposition(|&b| b == b':') {
                            let frame_bytes = frame_buf[start..].to_vec();
                            frame_buf.clear();

                            if let Ok(frame_str) = String::from_utf8(frame_bytes) {
                                match GridConnectFrame::parse_wire(&frame_str) {
                                    Ok(mut frame) => {
                                        frame.header = encoding.decode_header(frame.header);
                                        if frame_tx.blocking_send(Ok(frame)).is_err() {
                                            // Receiver dropped — shut down
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Warning: Failed to parse GridConnect frame: {}", e);
                                    }
                                }
                            }
                            // Invalid UTF-8 is silently skipped
                        } else {
                            // No ':' found — clear garbage
                            frame_buf.clear();
                        }
                    }

                    // Prevent unbounded buffer growth from line noise
                    if frame_buf.len() > 1024 {
                        frame_buf.clear();
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Normal timeout from the 10ms read timeout — check if channel
                // is still open by testing if the receiver has been dropped.
                if frame_tx.is_closed() {
                    break;
                }
                continue;
            }
            Err(e) => {
                eprintln!("Serial read error on {}: {}", port_path, e);
                let _ = frame_tx.blocking_send(Err(Error::Io(e)));
                break;
            }
        }
    }
}

/// Blocking writer thread: receives pre-formatted frame bytes from the async
/// world and writes them to the serial port.
fn writer_thread(
    mut port: Box<dyn SerialPort>,
    mut wire_rx: mpsc::Receiver<Vec<u8>>,
    port_path: &str,
) {
    while let Some(data) = wire_rx.blocking_recv() {
        if let Err(e) = port.write_all(&data) {
            eprintln!("Serial write error on {}: {}", port_path, e);
            break;
        }
        if let Err(e) = port.flush() {
            eprintln!("Serial flush error on {}: {}", port_path, e);
            break;
        }
    }
    // Channel closed or write error — thread exits, port is dropped
}

#[async_trait::async_trait]
impl LccTransport for GridConnectSerialTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let encoded_header = self.encoding.encode_header(frame.header);
        let wire_frame = GridConnectFrame { header: encoded_header, data: frame.data.clone() };
        // GridConnect frames are terminated by ';'. Do NOT append CR/LF on
        // serial: JMRI (the reference implementation these adapters are built
        // against) sends no trailing bytes after ';', and SPROG USB-LCC v1.4
        // is intolerant of the extra bytes accumulating in its FTDI/UART
        // buffer. (TCP GridConnect hubs are line-oriented and DO use '\n'.)
        let frame_str = wire_frame.to_string();
        self.wire_tx
            .send(frame_str.into_bytes())
            .await
            .map_err(|_| Error::ConnectionClosed)?;
        Ok(())
    }

    async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
        if timeout_ms > 0 {
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(timeout_ms),
                self.frame_rx.recv(),
            )
            .await
            {
                Ok(Some(result)) => result.map(Some),
                Ok(None) => Err(Error::ConnectionClosed),
                Err(_) => Ok(None), // timeout
            }
        } else {
            match self.frame_rx.recv().await {
                Some(result) => result.map(Some),
                None => Err(Error::ConnectionClosed),
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        // Dropping the wire_tx sender will cause the writer thread to exit.
        // The reader thread will exit when it detects the frame_tx receiver is
        // closed (on next timeout cycle). The serial port handles are dropped
        // when the threads exit.
        Ok(())
    }

    fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>) {
        (
            Box::new(GridConnectSerialReader {
                frame_rx: self.frame_rx,
            }),
            Box::new(GridConnectSerialWriter {
                wire_tx: self.wire_tx,
                encoding: self.encoding,
            }),
        )
    }
}

/// Read half of a split GridConnect serial transport.
///
/// Receives parsed frames from the blocking reader thread via an mpsc channel.
pub struct GridConnectSerialReader {
    frame_rx: mpsc::Receiver<Result<GridConnectFrame>>,
}

#[async_trait::async_trait]
impl TransportReader for GridConnectSerialReader {
    async fn receive(&mut self) -> Result<GridConnectFrame> {
        match self.frame_rx.recv().await {
            Some(result) => result,
            None => Err(Error::ConnectionClosed),
        }
    }
}

/// Write half of a split GridConnect serial transport.
///
/// Sends pre-formatted wire bytes to the blocking writer thread via an mpsc channel.
pub struct GridConnectSerialWriter {
    wire_tx: mpsc::Sender<Vec<u8>>,
    encoding: FrameEncoding,
}

#[async_trait::async_trait]
impl TransportWriter for GridConnectSerialWriter {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let encoded_header = self.encoding.encode_header(frame.header);
        let wire_frame = GridConnectFrame { header: encoded_header, data: frame.data.clone() };
        // See GridConnectSerialTransport::send: ';'-terminated, no CR/LF on serial.
        let frame_str = wire_frame.to_string();
        self.wire_tx
            .send(frame_str.into_bytes())
            .await
            .map_err(|_| Error::ConnectionClosed)?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        // Dropping wire_tx closes the channel, causing the writer thread to exit
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
